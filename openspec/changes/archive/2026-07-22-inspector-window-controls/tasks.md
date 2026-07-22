## 1. Chrome mode and viewport setup

- [x] 1.1 Add a chrome-mode decision as a pure, unit-tested function (Wayland session → headerbar/no decorations; anything else → native), following winit's own backend rule (`WAYLAND_DISPLAY` present); tests first: Wayland env → headerbar, X11-only env → native, empty env → native
- [x] 1.2 Wire the decision into `run()`: `ViewportBuilder::with_decorations(false)` in headerbar mode, unchanged otherwise; pass the mode into `InspectorApp` and down to the menu-bar view

## 2. Window buttons in the menu bar

- [x] 2.1 Add white-normalized Phosphor SVGs `x`, `corners-out`, `corners-in` to `apps/inspector/assets/icons/` (follow the README normalization rule, extend the license note)
- [x] 2.2 Add `ToolbarButtonSpec`s for Close (`author_id = "window-close"`) and Maximize/Restore (`author_id = "window-maximize"`, icon and label swap on maximized state — pure helper, unit-tested) and render them right-to-left in `show_menu_bar` in headerbar mode only, rightmost Close; actions send `ViewportCommand::Close` / `ViewportCommand::Maximized(!maximized)` with the state read from `ViewportInfo::maximized`

## 3. Move grip

- [x] 3.1 In headerbar mode, register the move grip as the leftover width between the menus and the window buttons (`allocate_response` after all clickable widgets — no overlap, so no z-order dependence; refined from the full-rect-interact sketch, whose rect is unknowable in a content-sized panel); `drag_started_by(Primary)` → `ViewportCommand::StartDrag`, `double_clicked()` → toggle `Maximized`; plain click stays a no-op; no label/AccessKit customization on the grip

## 4. Acceptance suite

- [x] 4.1 New `tests/acceptance/egui/inspector_window_controls.robot` (hermetic settings via `PLATYNUI_INSPECTOR_SETTINGS_PATH`, `@Id` locators): window buttons resolve uniquely; pointer-drag on empty menu-bar space (right of the menus, left of the buttons, coordinates from the menu bar's rectangle) changes the window's screen-rectangle position; `window-maximize` grows the rectangle and restores on second activation; `window-close` removes the window from the tree — last test, teardown tolerant of the app being gone
- [x] 4.2a Fix the compositor bug the suite exposed: `MoveSurfaceGrab`/`TouchMoveSurfaceGrab` truncated each per-event motion delta to whole pixels, dropping entire drags from sub-pixel-per-event sources (EIS virtual pointer, high-rate mice); the grabs now accumulate the fractional remainder (`apps/wayland-compositor/src/grabs.rs`)
- [x] 4.2 Run the full egui compositor lane; existing menu/toolbar suites green proves the grip steals no clicks; judge via `robotcode results` (58 passed / 0 failed / 29 skipped)

## 5. Verification and docs

- [x] 5.1 `just check` and `just test` (workspace gates)
- [x] 5.2 Manual pass under niri: drag from empty menu-bar space works (confirmed by the user on the real desktop); buttons/tooltips verified via the lane's a11y contracts
- [x] 5.3 Update `dev-docs/inspector.md`: decoration policy per platform (Wayland headerbar model, declare-don't-detect rationale), window buttons and their IDs, move grip, known trade-offs (KDE SSD replaced, no pointer resize edges on GNOME floating)
