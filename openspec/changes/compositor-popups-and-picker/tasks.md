## 1. Driving tests (test-first, red on the compositor lane)

- [ ] 1.1 Remove the Wayland `Skip If` from the three submenu tests in `tests/acceptance/qt/context_menu.robot` (and drop the skip explanation from their docs). On the compositor lane they now FAIL — the red bar for the popup-geometry half. The X11 lane must stay green throughout.
- [ ] 1.2 Confirm `Acceptance.Egui.Inspector Picker` currently fails on the local compositor lane and capture WHY precisely (greyed-out toggle vs. armed-but-blind modifiers): run it once with inspector `RUST_LOG` trace and note whether the session leaks the host `DISPLAY` into the modifier reader. This is the red bar for the picker half AND the evidence for the CI-vs-local discrepancy.

## 2. Spike: verify the geometry arithmetic against reality

- [ ] 2.1 Prototype the popup global-rect computation in the compositor (root `space.element_location` + `PopupManager::get_popup_toplevel_coords` + popup geometry offset — the `unconstrain_popup` arithmetic, xdg_shell.rs:387–499) and log it while the Qt test app shows its context-menu cascade; compare against a compositor screenshot. Locks the exact formula, the cascade behavior (per-level rects), and whether any decoration/scale offset applies to popups (assumed none).
- [ ] 2.2 Check GTK popovers under the compositor with the same logging: they are xdg_popups too, so `list_popups` will report them — confirm the provider-side consumption (only grafted popup-class nodes consult the query) keeps them from interfering.

## 3. Compositor: control-socket commands

- [ ] 3.1 `list_popups`: enumeration helper over `popup_manager` + per-popup global rect, parent toplevel `window_id` and `pid`; new dispatch arm in `process_command` (control.rs:313) returning `{status:"ok", popups:[…]}` (empty list when none). Document it in the control.rs doc block.
- [ ] 3.2 `get_modifiers`: read the seat keyboard's `ModifiersState` and return `{status:"ok", ctrl, alt, shift, logo}`. Document it likewise.
- [ ] 3.3 `ipc_tests` for both commands (pattern of the existing screenshot IPC test): popups empty without popups; a client with an open popup yields a plausible rect; injected `key_event` modifiers are reflected by `get_modifiers` and cleared on release (spec scenario "Held modifiers are observable").

## 4. Platform: WindowManager popup query

- [ ] 4.1 Extend the `WindowManager` trait (crates/core/src/platform/window_manager.rs) with a popup-geometry query returning the global rects of a process's popups; default implementation reports "no popups" so X11/Windows/mock stay untouched. Compile-check all impls.
- [ ] 4.2 Implement it in `PlatynUiIpcBackend` (platynui_ipc.rs) via `list_popups` over `send_command`, filtered by pid.

## 5. Provider: popup bounds from the window manager

- [ ] 5.1 In the AT-SPI provider's bounds resolution for grafted popup-class nodes, consult the window manager's popup query (match by pid + size, most-recent on ties) before the Screen-extents fallback; keep X11 on the existing path (backends with the default "no popups" answer never alter behavior). Matching logic as a pure function next to popups.rs with unit tests (single popup, cascade with distinct sizes, size tie, no match → fallback).
- [ ] 5.2 `just build-native` so the Python bindings pick up the provider/platform change.

## 6. Inspector: compositor modifier reader

- [ ] 6.1 Add a compositor branch to `apps/inspector/src/modifiers.rs` polling `get_modifiers` over the control socket, and make reader selection explicit: PlatynUI control socket present (answers `ping`) → compositor reader; else X11; else unsupported. This must also fix the leaked-host-`DISPLAY` misbinding (spec scenario).
- [ ] 6.2 If task 1.2 confirmed `DISPLAY` leaking into the compositor session, isolate it in `scripts/startcompositor.sh` (mirroring `startxsession.sh`'s env hygiene) so no component binds to the host X server.

## 7. Verification

- [ ] 7.1 The three submenu tests pass UNSKIPPED on the compositor lane; the full Qt context-menu suite is green on both compositor and X11 lanes.
- [ ] 7.2 `Acceptance.Egui.Inspector Picker` passes on the compositor lane (and still on the X11 lane); local and CI compositor lanes agree.
- [ ] 7.3 No regression: full `real` acceptance lanes green on X11 and the compositor; `just check` and `just test` (workspace clippy + nextest, incl. the new ipc/matching tests) pass.

## Notes / out of scope

- Generic Wayland (no PlatynUI control socket): popup geometry and picking stay unsupported — unchanged.
- Tree-invalidation events on popup open/close (Inspector auto-refresh) remain the separate follow-up from `atspi-event-driven-tree`.
- `window_at_point` / `list_windows` keep toplevel-only semantics; popups are a separate query by design.
