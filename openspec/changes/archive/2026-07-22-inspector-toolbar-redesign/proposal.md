## Why

The Inspector's header area grew piecemeal and it shows: the picker toggle sits in a bare strip outside any panel, so its background is the raw window clear color — permanently black, ignoring the light/dark theme the rest of the app follows. The search row crams four unrelated controls (Search/Stop, Refresh Node, Refresh Subtree, Always On Top) behind a hard-coded 320 px budget, which clips the checkbox label and makes the placement look arbitrary. Picker state hints render inline next to the toggle even though a status bar already exists. This change redesigns the header as a proper toolbar and the status bar as a segmented strip, fixing all three defects with one coherent structure.

## What Changes

- **New toolbar** as a real `egui::Panel::top` directly below the menu bar (a panel paints its themed background — this alone fixes the black strip). It hosts, left to right: the Pick Element toggle, Refresh Node, Refresh Subtree, Highlight Node (promoted from menu/context-menu only), and — right-aligned — an Always-on-Top pin toggle replacing the clipped checkbox. Buttons are disabled without a node selection exactly as today; every button carries a tooltip with its action name and shortcut (e.g. "Refresh Node — F5").
- **Slimmed search row**: only the XPath field and the Search/Stop button remain. The row is laid out right-to-left (button first, field takes the remaining width), removing the magic `available_width() - 320.0`. Existing field behavior (multiline growth, Enter/Shift+Enter, error icon/popup, focus lock) is unchanged.
- **Segmented status bar**: the left segment keeps the activity indicator and transient messages (search/result status as today, plus picker events such as a completed pick); a new right-aligned segment shows the persistent picker *state* ("off", "armed — hold Ctrl+Alt+Shift", "picking…", or the unavailability reason). The inline state text next to the picker toggle is removed; the gesture hint moves to the toggle's tooltip and the state segment.
- **SVG icons**: toolbar buttons get Phosphor icons (MIT), committed as white-normalized SVG files under `apps/inspector/assets/icons/` with a license note, embedded via `egui::include_image!`, rendered through `egui_extras`' `svg` feature, and tinted with the current text color (`image_tint_follows_text_color`) so they follow the theme and disabled state automatically. Toggles (picker, pin) switch to the Phosphor fill variant when active.
- **Configurable toolbar style**: a new `toolbar_style` setting — `IconsOnly` (default) or `IconsAndText` — added to the persisted Inspector settings (`inspector.ron`) and editable in the existing Settings dialog. Toolbar height does not change between modes.
- **Stable element IDs and mode-independent accessible names**: every toolbar control gets a stable AccessKit `author_id` (set via `accesskit_node_builder`), which surfaces as the provider-independent common `@Id` attribute (UIA `AutomationId` / AT-SPI `AccessibleId`) — the acceptance-test contract. In addition, each control exposes a human-readable accessible name identical in both display modes (via `Image::alt_text`, with `accesskit_node_builder` as fallback).

Not BREAKING for users of the toolkit: this is Inspector-only UI. The picker acceptance suite's name-based locator (`contains(@Name,"Pick Element")` in tests/acceptance/egui/inspector_picker.robot) is switched to the new stable `@Id` as part of this change — IDs are rename- and localization-proof, names are not.

## Capabilities

### New Capabilities
- `inspector-toolbar`: the header-area contract — a themed toolbar panel hosting the picker toggle and node/window actions, the slimmed search row, the IconsOnly/IconsAndText display setting with its persistence and default, tooltips, and mode-independent accessible names.
- `inspector-status-bar`: the segmented status bar — transient activity/result/picker-event messages on the left, the persistent picker-state segment on the right.

### Modified Capabilities

None. `inspector-live-mouse-picker`'s discoverability requirement ("visible toggle communicates the gesture; a status hint communicates what is happening") is still satisfied — the gesture moves from an inline label to the toggle tooltip and the status-bar state segment; the spec does not prescribe the hint's location. Flagged here so the relocation is a conscious call, not an accident.

## Impact

- **Layer:** Rust only, entirely within `apps/inspector` (view: `toolbar.rs`, `status_bar.rs`, new `view` code for the toolbar; `lib.rs` layout and `PersistedSettings`; viewmodel: picker state/event exposure for the status bar, `settings_dialog.rs`). No native rebuild — the Inspector is a standalone binary, not the `packages/native` PyO3 module. No Python/Robot Framework API surface changes. UI is platform-agnostic across the backends in the README support table.
- **Dependencies:** enable the `svg` feature on the existing `egui_extras` dependency (pulls `resvg`; compile-time cost accepted). New committed assets: a handful of Phosphor SVGs plus their MIT license notice. The egui family stays pinned at 0.34.
- **Coordination:** the in-flight `inspector-xpath-history` change also edits the search bar in `toolbar.rs` (history dropdown beside the field, completion popup). The redesign keeps the field, its id, and its key handling intact and only removes the unrelated buttons, so both changes compose; whichever lands second rebases its `toolbar.rs` edits.
- **Tests:** Rust unit tests where logic is extractable (toolbar action wiring stays thin view code per the crate's posture; settings round-trip for `toolbar_style`). Acceptance coverage via the existing PlatynUI.BareMetal egui lane (`tests/acceptance/egui/`), which already drives the Inspector's own AccessKit tree: extend/keep the picker suite green with the icons-only default and add checks for the toolbar buttons' accessible names and the status-bar picker-state segment. Suites must stay hermetic via `PLATYNUI_INSPECTOR_SETTINGS_PATH`.
- **Docs:** `dev-docs/inspector.md` (UI structure/features).
