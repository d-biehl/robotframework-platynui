## 1. Driving tests (test-first, red on the compositor lane)

- [x] 1.1 Remove the Wayland `Skip If` from the three submenu tests in `tests/acceptance/qt/context_menu.robot` (and drop the skip explanation from their docs). On the compositor lane they now FAIL — the red bar for the popup-geometry half. The X11 lane must stay green throughout.
- [x] 1.2 Confirm `Acceptance.Egui.Inspector Picker` currently fails on the local compositor lane and capture WHY precisely (greyed-out toggle vs. armed-but-blind modifiers): run it once with inspector `RUST_LOG` trace and note whether the session leaks the host `DISPLAY` into the modifier reader. This is the red bar for the picker half AND the evidence for the CI-vs-local discrepancy.
  - **Evidence (2026-07-14):** FAIL reproduced on the nested compositor lane ("Inspector did not reveal/select the picked button"). Inspector trace: `live picker support probe picker_supported=true has_modifier_reader=true pointer_position_ok=true` — the session has no XWayland, so the X11 reader bound the **host** X server via leaked `DISPLAY=:0` (`startcompositor.sh` unsets DBUS/AT-SPI but not `DISPLAY`) → **armed-but-blind**. CI (headless, no `DISPLAY`) is the greyed-out variant and fails too; its lane looks green only because the compositor session does not propagate the robot exit code. Local and CI agree in outcome; the discrepancy was a reporting artifact.

## 2. Spike: verify the geometry arithmetic against reality

- [x] 2.1 Prototype the popup global-rect computation in the compositor (root `space.element_location` + `PopupManager::get_popup_toplevel_coords` + popup geometry offset — the `unconstrain_popup` arithmetic, xdg_shell.rs:387–499) and log it while the Qt test app shows its context-menu cascade; compare against a compositor screenshot. Locks the exact formula, the cascade behavior (per-level rects), and whether any decoration/scale offset applies to popups (assumed none).
  - **Locked formula:** `element_location(root) + root.geometry().loc + accumulated placement` (`PopupManager::popups_for_surface`, which already sums the chain incl. the popup's own placement); size = positioner-placed rect from `XdgPopupSurfaceData.current.geometry`. The parent's geometry offset matters: this compositor renders buffers at `element_location` (render.rs), so CSD parents (GTK, offset 25,25) shift popups — Qt/SSD masked it (offset 0,0). All three Qt cascade levels pixel-verified by cropping the reported rects out of a compositor screenshot (each crop frames exactly one menu level); popups themselves carry no decoration offset; coordinates are logical (scale-independent).
- [x] 2.2 Check GTK popovers under the compositor with the same logging: they are xdg_popups too, so `list_popups` will report them — confirm the provider-side consumption (only grafted popup-class nodes consult the query) keeps them from interfering.
  - Confirmed with gtk4-widget-factory: the hamburger popover is listed with an exact rect (crop-verified, incl. its arrow nub — part of GTK's declared geometry). Non-interference holds because the provider consults the query only for popup-class nodes grafted directly under the Application (Qt-style); GTK popovers attach in-tree and keep the parent-chain path. Byproduct fix while spiking: control-socket responses larger than the socket buffer (rich screenshots) were silently truncated on `WouldBlock` — `write_response` now retries with a 5s deadline.

## 3. Compositor: control-socket commands

- [x] 3.1 `list_popups`: enumeration helper over `popup_manager` + per-popup global rect, parent toplevel `window_id` and `pid`; new dispatch arm in `process_command` (control.rs:313) returning `{status:"ok", popups:[…]}` (empty list when none). Document it in the control.rs doc block.
- [x] 3.2 `get_modifiers`: read the seat keyboard's `ModifiersState` and return `{status:"ok", ctrl, alt, shift, logo}`. Document it likewise.
- [x] 3.3 `ipc_tests` for both commands (pattern of the existing screenshot IPC test): popups empty without popups; a client with an open popup yields a plausible rect; injected `key_event` modifiers are reflected by `get_modifiers` and cleared on release (spec scenario "Held modifiers are observable").
  - Note: the popup ipc test asserts the **exact** global rect via a raw wayland-client fixture (dev-dependency), not just plausibility. Two byproduct fixes: `list_popups` reads the positioner-placed rect from `XdgPopupSurfaceData` (NOT `PopupKind::geometry()`, which is the client's optional `set_window_geometry`), and `handle_client_data` now processes buffered lines arriving together with EOF (write-then-close clients were silently dropped — this is what fire-and-forget injection over a short-lived connection hits).

## 4. Platform: WindowManager popup query

- [x] 4.1 Extend the `WindowManager` trait (crates/core/src/platform/window_manager.rs) with a popup-geometry query returning the global rects of a process's popups; default implementation reports "no popups" so X11/Windows/mock stay untouched. Compile-check all impls.
- [x] 4.2 Implement it in `PlatynUiIpcBackend` (platynui_ipc.rs) via `list_popups` over `send_command`, filtered by pid.

## 5. Provider: popup bounds from the window manager

- [x] 5.1 In the AT-SPI provider's bounds resolution for grafted popup-class nodes, consult the window manager's popup query (match by pid + size, most-recent on ties) before the Screen-extents fallback; keep X11 on the existing path (backends with the default "no popups" answer never alter behavior). Matching logic as a pure function next to popups.rs with unit tests (single popup, cascade with distinct sizes, size tie, no match → fallback).
- [x] 5.2 `just build-native` so the Python bindings pick up the provider/platform change.

## 6. Inspector: compositor modifier reader

- [x] 6.1 Add a compositor branch to `apps/inspector/src/modifiers.rs` polling `get_modifiers` over the control socket, and make reader selection explicit: PlatynUI control socket present (answers `ping`) → compositor reader; else X11; else unsupported. This must also fix the leaked-host-`DISPLAY` misbinding (spec scenario).
- [x] 6.2 If task 1.2 confirmed `DISPLAY` leaking into the compositor session, isolate it in `scripts/startcompositor.sh` (mirroring `startxsession.sh`'s env hygiene) so no component binds to the host X server.
  - Leak confirmed (1.2). The isolation lives in the compositor's child spawn (child.rs), not the script: the nested-winit compositor process itself may legitimately need the host display, so `spawn_child` now strips `DISPLAY` from the *session child's* environment unless XWayland provides one — which is also what child.rs's doc always claimed. The script documents this decision at its isolation block.

## 7. Verification

- [x] 7.1 The three submenu tests pass UNSKIPPED on the compositor lane; the full Qt context-menu suite is green on both compositor and X11 lanes.
  - Compositor lane (headless, 2026-07-14): all three ran and passed; full lane 45 pass / 0 fail (5 pre-existing suite-level skips: coexisting-runtimes/config-display). X11 lane: 50/50, zero skips.
- [x] 7.2 `Acceptance.Egui.Inspector Picker` passes on the compositor lane (and still on the X11 lane); local and CI compositor lanes agree.
  - Passed on headless AND on the nested (winit) lane where the failure was originally observed; inspector trace shows `live picker modifier source: PlatynUI compositor control socket` (compositor session) and `… source: X11` (X11 session). The 1.2 investigation showed local and CI never actually disagreed in outcome — CI's green was the swallowed compositor-lane exit code (pre-existing, flagged separately).
- [x] 7.3 No regression: full `real` acceptance lanes green on X11 and the compositor; `just check` and `just test` (workspace clippy + nextest, incl. the new ipc/matching tests) pass.
  - `just check` clean; `just test` 2042/2042 (incl. the three new ipc_tests and four popup-matcher unit tests).

## Notes / out of scope

- Generic Wayland (no PlatynUI control socket): popup geometry and picking stay unsupported — unchanged.
- Tree-invalidation events on popup open/close (Inspector auto-refresh) remain the separate follow-up from `atspi-event-driven-tree`.
- `window_at_point` / `list_windows` keep toplevel-only semantics; popups are a separate query by design.
