## 1. Icon assets and rendering foundation

- [ ] 1.1 Enable the `svg` feature on `egui_extras` in `apps/inspector/Cargo.toml` and call `egui_extras::install_image_loaders` once at app startup
- [ ] 1.2 Add the chosen Phosphor SVGs (regular + fill weights for the two toggles) under `apps/inspector/assets/icons/`, normalized to white fill/stroke, with a `LICENSE` note (MIT, Phosphor) and a short README line documenting the white-normalization rule
- [ ] 1.3 Add a toolbar-button construction helper (icon via `include_image!`, `image_tint_follows_text_color(true)`, tooltip "Action — Shortcut", accessible name via `Image::alt_text`, stable `author_id` via `accesskit_node_builder`) used by every toolbar control
- [ ] 1.4 Spike: verify with a minimal build that the helper's `author_id` surfaces as `@Id` in the Inspector's own AccessKit tree (inspect the Inspector with BareMetal) — the adapter mappings are confirmed in source (see design D4), this checks only that egui forwards the node-builder mutation; also check whether `alt_text` lands as `@Name` (nice-to-have, not gating)

## 2. Acceptance tests first (BareMetal egui lane)

- [ ] 2.1 Extend `tests/acceptance/egui/` with a toolbar suite (hermetic via `PLATYNUI_INSPECTOR_SETTINGS_PATH`): toolbar buttons resolvable by stable `@Id` in the icons-only default (`picker-toggle`, `refresh-node`, `refresh-subtree`, `highlight-node`, `always-on-top`), Refresh controls absent from the search row
- [ ] 2.2 Add status-bar checks to the suite: with the picker armed, the persistent segment text (armed + combo) is on the Inspector's a11y tree; after a completed pick, a transient "Picked: …" message appears alongside the persistent segment
- [ ] 2.3 Update `inspector_picker.robot` to locate the pick toggle by `@Id` instead of `contains(@Name,"Pick Element")` (run the suite; it may fail until implementation lands — that is the red state driving 3–5)

## 3. Toolbar implementation

- [ ] 3.1 Add a `toolbar` view function rendering an `egui::Panel::top` with: Pick Element toggle, separator, Refresh Node, Refresh Subtree, Highlight Node, and a right-aligned Always-on-Top pin toggle; wire it in `lib.rs` and remove the bare picker `ui.horizontal` row
- [ ] 3.2 Wire enablement (node actions need selection, pick toggle needs `picker_supported`) and route clicks through the existing `AppCommand`s; toggles use the fill-weight icon + selected styling when active; keep the apply-on-change window-level guard for Always-on-Top
- [ ] 3.3 Add `ToolbarStyle { IconsOnly, IconsAndText }` (default `IconsOnly`) to `PersistedSettings`, render both modes with identical button height, and add the style selector to the Settings dialog
- [ ] 3.4 Unit tests: `PersistedSettings` round-trip including `toolbar_style`, and loading a pre-existing RON without the field yields the icons-only default

## 4. Search row slimming

- [ ] 4.1 Rework `show_search_bar`: remove Refresh/Subtree/Always-on-Top, lay out right-to-left (Search/Stop button, then the field taking `available_width()`), delete the 320 px constant; leave field id, focus lock, Enter/Shift+Enter/Escape handling, and the error icon/popup untouched
- [ ] 4.2 Manually verify field behavior (Enter evaluates, Shift+Enter newline, Escape cancels, error popup) and that no control clips at default window size

## 5. Segmented status bar

- [ ] 5.1 Extend `show_status_bar` to two segments: left keeps the activity indicator + transient text (existing styling incl. error color), right renders a right-aligned persistent picker-state string
- [ ] 5.2 Expose a picker-state string from the viewmodel (disarmed / armed with combo / picking… / unavailable reason) as a pure, unit-tested formatting helper; remove the inline hint labels next to the former picker toggle
- [ ] 5.3 Emit a transient "Picked: <element name>" status on completed pick resolution through the existing `result_status`/status-text path

## 6. Verification and docs

- [ ] 6.1 `just check` and `just test` (workspace gates)
- [ ] 6.2 Run the egui acceptance lane (real compositor/X session) including the new toolbar/status-bar suite and the existing picker suite; judge results via `robotcode results`, not the console exit code
- [ ] 6.3 Visual pass in a real session: light and dark theme (no unthemed strip, icons tinted correctly), both toolbar styles at identical height, active-toggle fill icons
- [ ] 6.4 Update `dev-docs/inspector.md` (toolbar, display-style setting, status-bar segments, icon-asset conventions)
