# Design — compositor popup geometry + picker modifier state

## Context

All findings below were verified against the current code (2026-07-14, after `atspi-event-driven-tree` merged):

- **Control socket** ([apps/wayland-compositor/src/control.rs](../../../apps/wayland-compositor/src/control.rs)): newline-delimited JSON on `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY.control`; dispatch in `process_command` (control.rs:313). Geometry commands (`list_windows` control.rs:329, `get_window` :335, `window_at_point` :472) iterate only `state.space.elements()` — mapped **toplevels**. `get_pointer_position` (:467) already exposes `state.pointer_location` (state.rs:193). No command exposes popups or keyboard modifiers.
- **Popups in the compositor**: xdg_popups are tracked in `state.popup_manager: PopupManager` (state.rs:181), *separate* from the `Space`; registered in `XdgShellHandler::new_popup` (handlers/xdg_shell.rs:77). Their **global position is not stored but computable**: `root toplevel location (space.element_location) + PopupManager::get_popup_toplevel_coords(popup) + popup geometry offset` — exactly the arithmetic `unconstrain_popup` already does (xdg_shell.rs:387–499). No enumeration helper with geometry exists yet; smithay's `PopupManager::popups_for_surface(root)` is the building block.
- **Modifiers in the compositor**: smithay's seat keyboard tracks XKB modifier state (visible in the input closure, input.rs:105); nothing exposes it over IPC. Injection (`key_event`, `pointer_*`, control.rs:569–711) and EIS already exist — this change only needs the *read* side.
- **Wayland WM backend** ([crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs](../../../crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs)): `resolve_window` matches AT-SPI windows to compositor `window_id`s by PID → title → size (`match_best_window`, :266–326); `bounds` (:90) returns `content_* + csd_shadow_offset`. The model is toplevel-only: popups have no `window_id`, usually no title, and share the parent's PID — they cannot fit `resolve_window` and need a separate query.
- **AT-SPI provider popup bounds**: since `atspi-event-driven-tree`, popup-class nodes (`PopupMenu`/`Menu`/`ToolTip`) are deliberately **not** window surfaces (node.rs:209) — their bounds resolve via parent-chain/Screen-extents fallback (node.rs:1310–1355). On Wayland, Qt's Screen extents are client-local, and a popup grafted directly under the `Application` has no geometric parent → wrong global bounds. Menu **items** below the popup are fine once the popup root rect is right (parent-relative accumulation).
- **Inspector picker gating** ([apps/inspector/src/lib.rs:481](../../../apps/inspector/src/lib.rs)): `picker_supported = modifier_reader.is_some() && pointer_position works`. The modifier reader (modifiers.rs) is X11/Windows-only; on Linux it unconditionally tries `x11rb::connect` — under the compositor session this either fails (picker greyed out) or, with a leaked host `DISPLAY`, binds to the **host** X server and never sees the modifiers PlatynUI injects into the compositor seat. Pointer position already flows via `get_pointer_position` (control_socket.rs:250).

## Goals / Non-Goals

**Goals:**
- Physically correct global bounds for grafted popups (and thus their items) under the PlatynUI compositor → pointer clicks into open menus land where they look.
- The three Wayland-skipped submenu tests in `tests/acceptance/qt/context_menu.robot` run and pass on the compositor lane.
- The Inspector live picker is enabled and functional under the compositor; `Acceptance.Egui.Inspector Picker` passes on the compositor lane.
- Local and CI compositor lanes agree (root-cause the current discrepancy; suspected host-`DISPLAY` leak).

**Non-Goals:**
- Generic Wayland (no PlatynUI control socket): stays unsupported for popup geometry and picking — unchanged.
- X11 / Windows: no behavior change (X11 popup extents already correct; UIA exposes popups natively).
- Emitting tree-invalidation events on popup open/close (still the separate follow-up from `atspi-event-driven-tree`).
- Exposing popups in `window_at_point` / `list_windows` (toplevel semantics stay; popups get their own query).

## Decisions

1. **Two narrow control-socket commands instead of widening existing ones.**
   - `list_popups` → `{status:"ok", popups:[{parent_window_id, pid, x, y, width, height}]}` with **global** logical coordinates, computed on demand (root location + `get_popup_toplevel_coords` + geometry offset — the `unconstrain_popup` arithmetic factored into a helper). Optionally `serial`/index for stable ordering; no naming, popups have none.
   - `get_modifiers` → `{status:"ok", ctrl, alt, shift, logo}` from the seat keyboard's `ModifiersState`.
   - Rationale: `list_windows`/`window_at_point` have toplevel semantics that `resolve_window`/`match_best_window` and the hit-test rely on; mixing popups in would ripple through every consumer. New commands are additive and independently testable via the existing `ipc_tests` pattern.
2. **WindowManager trait gains one method with a safe default.** `fn popups(&self, pid: u32) -> Result<Vec<Rect>, PlatformError>` (name/shape final at implementation) with a default returning an empty list. Only the PlatynUI-IPC backend implements it. X11/Windows/mock stay untouched — the provider treats "no popups reported" as "use the existing extents path".
   - Alternative considered — resolve popups through `resolve_window`: rejected; popups have no title/window_id and share the parent PID, `match_best_window` cannot disambiguate them, and window identity (`RESOLVED_WINDOWS`) is the wrong lifetime model for surfaces that live milliseconds.
3. **Provider-side matching by PID + size, at bounds-resolution time.** When a popup-class node resolves extents and the toolkit path yields no usable global rect, ask the WM for the process's popup rects and match by size (AT-SPI reports width/height correctly even on Wayland; positions are what is missing). Exactly one popup per size is the overwhelmingly common case (cascades: each level has a distinct size; ties broken by most-recent/last). Matching lives next to the existing popup code (popups.rs) and is unit-testable as a pure function.
   - Alternative considered — order-based matching (registry insertion order vs compositor stacking): more fragile than size matching and needs stacking info the protocol does not carry; revisit only if size collisions show up in practice.
4. **Bounds resolution order for popup-class nodes:** try WM popup geometry first *when the backend reports any popups for the PID*, else the current fallback (Screen extents — correct on X11). This keeps X11 zero-cost (its WM backend uses the default empty implementation, so the query never fires there... verified at implementation time; if the trait default is "unavailable", the provider skips the call unless the platform advertises support).
5. **Inspector modifier reader: compositor branch + explicit selection order.** New reader variant polling `get_modifiers` (one-shot connect per poll, like `send_command` in control_ipc.rs:10 — the picker polls at UI tick rate, which the socket handles fine; switch to a persistent connection only if profiling says so). Selection: if `$WAYLAND_DISPLAY` names a PlatynUI control socket that answers `ping` → compositor reader; else if X11 connect succeeds → X11 reader; else unsupported. This ordering also fixes the host-`DISPLAY` misbinding, because the compositor session prefers its own seat state over a leaked X connection.
6. **Session hygiene as part of the fix:** `startcompositor.sh` should isolate `DISPLAY` from the host (as `startxsession.sh` already unsets host state) so no component accidentally binds to the host X server. Investigated and fixed together with the reader ordering — this is the leading suspect for why the local compositor lane and CI disagree on the picker suite.

## Risks / Trade-offs

- **Size-based popup matching can mismatch** when two popups of identical size are open in one process. Consequence: a click lands in the wrong (same-sized) popup — rare, and the acceptance tests (distinct cascade sizes) will not mask it. Mitigation documented in code; order/stacking matching is the escalation path.
- **CSD/decoration offsets for popups** are assumed zero (popups draw no decorations under the compositor); the `csd_shadow_offset` handling in `bounds` (platynui_ipc.rs:486) applies to toplevels only. Verified during implementation against the real Qt app.
- **GTK popovers** are *not* separate xdg_popups grafted under the Application (they attach in-tree, per the `atspi-event-driven-tree` spike) — `list_popups` still reports them (they are xdg_popups), but the provider only consults popup geometry for grafted popup-class nodes, so no interference is expected. Checked by the spike task.
- **Polling `get_modifiers` per UI tick** adds one tiny IPC roundtrip per tick while the picker is armed — same cadence the X11 reader already uses against the X server; negligible.
- The trait addition touches `platynui_core` — every `WindowManager` impl (X11, Windows, mock, compositor) must compile; the default implementation keeps that mechanical.

## Migration Plan

- **Additive.** New IPC commands, a defaulted trait method, and a new bounds source consulted only for popup-class nodes under a backend that reports popups. No data migration.
- **Native rebuild required** (`just build-native`) for provider/platform changes; the compositor and inspector binaries rebuild via cargo as usual (the acceptance session scripts build them).
- **Rollback:** revert the provider's popup-geometry consultation (one call site) to fall back to today's behavior; the IPC commands are inert without consumers. The existing `providers.atspi.surface_popups` flag is unaffected and still disables popup surfacing wholesale.
