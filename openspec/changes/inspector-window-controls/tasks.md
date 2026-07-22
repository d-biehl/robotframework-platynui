## 1. Chrome mode and viewport setup

- [ ] 1.1 Add a chrome-mode decision as a pure, unit-tested function (Wayland session → headerbar/no decorations; anything else → native), following winit's own backend rule (`WAYLAND_DISPLAY` present); tests first: Wayland env → headerbar, X11-only env → native, empty env → native
- [ ] 1.2 Wire the decision into `run()`: `ViewportBuilder::with_decorations(false)` in headerbar mode, unchanged otherwise; pass the mode into `InspectorApp` and down to the menu-bar view

## 2. Window buttons in the menu bar

- [ ] 2.1 Add white-normalized Phosphor SVGs `x`, `corners-out`, `corners-in` to `apps/inspector/assets/icons/` (follow the README normalization rule, extend the license note)
- [ ] 2.2 Add `ToolbarButtonSpec`s for Close (`author_id = "window-close"`) and Maximize/Restore (`author_id = "window-maximize"`, icon and label swap on maximized state — pure helper, unit-tested) and render them right-to-left in `show_menu_bar` in headerbar mode only, rightmost Close; actions send `ViewportCommand::Close` / `ViewportCommand::Maximized(!maximized)` with the state read from `ViewportInfo::maximized`

## 3. Move grip

- [ ] 3.1 In headerbar mode, register a full-width `Sense::click_and_drag()` interact over the menu-bar panel rect **before** the `MenuBar` and window buttons render; `drag_started_by(Primary)` → `ViewportCommand::StartDrag`, `double_clicked()` → toggle `Maximized`; plain click stays a no-op; no label/AccessKit customization on the interact

## 4. Acceptance suite

- [ ] 4.1 New `tests/acceptance/egui/inspector_window_controls.robot` (hermetic settings via `PLATYNUI_INSPECTOR_SETTINGS_PATH`, `@Id` locators): window buttons resolve uniquely; pointer-drag on empty menu-bar space (right of the menus, left of the buttons, coordinates from the menu bar's rectangle) changes the window's screen-rectangle position; `window-maximize` grows the rectangle and restores on second activation; `window-close` removes the window from the tree — last test, teardown tolerant of the app being gone
- [ ] 4.2 Run the full egui compositor lane; existing menu/toolbar suites green proves the grip steals no clicks; judge via `robotcode results`

## 5. Verification and docs

- [ ] 5.1 `just check` and `just test` (workspace gates)
- [ ] 5.2 Manual pass under niri: drag from empty menu-bar space, double-click maximize, button placement/tooltips in light and dark theme
- [ ] 5.3 Update `dev-docs/inspector.md`: decoration policy per platform (Wayland headerbar model, declare-don't-detect rationale), window buttons and their IDs, move grip, known trade-offs (KDE SSD replaced, no pointer resize edges on GNOME floating)
