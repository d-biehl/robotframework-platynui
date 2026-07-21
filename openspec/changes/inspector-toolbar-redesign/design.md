## Context

The Inspector window stacks, top to bottom: menu bar, an unpanelled picker row, the search bar, then tree/attributes/results/status panels. Three verified defects drive this redesign:

- **Unthemed picker strip.** The picker toggle and its inline state text are drawn with a bare `ui.horizontal` directly on the root `Ui` (`apps/inspector/src/lib.rs:697-718`) — the only UI outside any `egui::Panel`. Panels paint their themed `panel_fill`; this strip paints nothing, so the window clear color (black) shows through regardless of theme.
- **Clipped search-row controls.** The search field reserves space via `desired_width(ui.available_width() - 320.0)` (`apps/inspector/src/view/toolbar.rs:190`), a fixed budget for Search/Stop, Refresh, Subtree, and the Always On Top checkbox. Real widths (font/emoji dependent) exceed 320 px, clipping the checkbox label at the panel edge.
- **Misplaced picker status.** Picker state hints ("armed — hold …", "picking…") render inline next to the toggle, while the existing status bar (`apps/inspector/src/view/status_bar.rs`) only ever shows search/result status (`status_bar_text()` reads only `result_status`, `apps/inspector/src/viewmodel/inspector_vm.rs:521-523`).

Constraints, verified against the code and project memory:

- egui family is pinned at 0.34 (`egui-system-fonts` has no 0.35).
- `egui_extras` 0.34 is already a dependency (tables); the `image` crate is already present for the embedded window icon (`lib.rs:94-100`).
- Persisted settings live in `inspector.ron` under the config dir via eframe storage, `PersistedSettings` in `lib.rs:51-62`; acceptance suites isolate them via `PLATYNUI_INSPECTOR_SETTINGS_PATH`.
- The acceptance suite `tests/acceptance/egui/inspector_picker.robot:21` locates the pick toggle by accessible name: `//Button[contains(@Name,"Pick Element")]`.
- The in-flight `inspector-xpath-history` change will add a history dropdown and completion popup to the search bar in `toolbar.rs`.

## Goals / Non-Goals

**Goals:**

- Every header element sits inside a themed panel; no unpainted strips in either theme.
- One toolbar hosting picker toggle + node actions + always-on-top, with a display style setting (`IconsOnly` default, `IconsAndText`), consistent height in both modes.
- Search row reduced to field + Search/Stop with no fixed-width magic number.
- Status bar with a transient left segment (existing statuses + picker events) and a persistent right segment (picker state).
- Theme-following SVG icons (Phosphor) with stable, mode-independent accessible names.

**Non-Goals:**

- No menu-bar changes (all actions stay reachable via menus with shortcuts).
- No new theme system or light/dark override setting (the Inspector keeps following the system theme; a preference can be a later change).
- No icon usage outside the toolbar (menus, context menus, dialogs stay text-only).
- No changes to picker semantics, search semantics, or any spec'd picker behavior.
- Not adopting the `inspector-xpath-history` work — only staying compatible with it.

## Decisions

### D1: Toolbar is an `egui::Panel::top`, not a styled strip

A real panel paints `panel_fill` itself, fixing the black strip with zero color management on our side and staying correct for future theme work. Alternative — keeping the bare row and painting a background rect manually — rejected: it duplicates what panels already do and would silently drift from theme changes.

### D2: Icons are embedded monochrome SVGs rendered by `egui_extras`' `svg` feature

- Enable `features = ["svg"]` on the existing `egui_extras` dependency (pulls `resvg`; compile-time cost consciously accepted).
- Install loaders once at startup (`egui_extras::install_image_loaders`).
- Assets: individual Phosphor SVGs (MIT) committed under `apps/inspector/assets/icons/`, normalized to white fill/stroke at check-in time, plus a `LICENSE` note in that directory. White is load-bearing: egui's image tint is multiplicative, so only white sources take the tint color fully.
- Buttons use `egui::include_image!` + `Button::image`/`image_and_text` with `image_tint_follows_text_color(true)`, which yields correct light/dark/disabled coloring for free.
- Active toggles (picker armed, pin on) switch to the Phosphor *fill* weight of the same glyph, in addition to egui's selected styling.

Alternatives rejected: icon font crates (egui-phosphor) — interacts with the `egui-system-fonts` font-merging setup and only helps if icons must live inline in text; hand-painted `Painter` icons (precedent: `toolbar.rs:350`) — fine for one-offs, unmaintainable for a consistent six-icon set; PNGs — blurry under fractional DPI scaling.

### D3: Toolbar contents and layout

Left-to-right: Pick Element (selectable toggle), separator, Refresh Node, Refresh Subtree, Highlight Node, then right-aligned (right-to-left layout) the Always-on-Top pin toggle. Enablement mirrors today's rules (node actions need a selection; pick toggle needs `picker_supported`). Highlight is promoted into the toolbar because it is a primary action currently buried in menu/context menu — same `AppCommand::HighlightNode` path, no new logic. The Always-on-Top checkbox becomes a pin toggle: an icon toggle cannot clip, and the pin is the established idiom in inspector-type tools. The existing apply-on-change guard for the window level (`lib.rs:736-743`) is kept.

### D4: Display style is a persisted enum; controls carry stable IDs, tooltips, and mode-independent accessible names

- `ToolbarStyle { IconsOnly, IconsAndText }`, default `IconsOnly`, stored as a new field in `PersistedSettings` (serde `#[serde(default)]` already tolerates old files), edited in the existing Settings dialog next to the picker combo.
- **Stable element IDs are the test contract.** Every toolbar control gets an AccessKit `author_id` (e.g. `picker-toggle`, `refresh-node`, `refresh-subtree`, `highlight-node`, `always-on-top`), set via `ctx.accesskit_node_builder(response.id, |n| n.set_author_id(…))`. The chain is verified in source: accesskit 0.24 exposes `author_id` (accesskit-0.24.1 `src/lib.rs:1929`); the AT-SPI adapter surfaces it as `AccessibleId` (accesskit_atspi_common-0.18.0 `src/node.rs:821`); the Windows adapter as UIA `AutomationId` (accesskit_windows-0.32.1 `src/node.rs:1249`); our providers read exactly those (`crates/provider-atspi/src/node.rs:665`, UIA `AutomationId` in `crates/provider-windows-uia/src/node.rs:183`) and expose them as the common `@Id` attribute (`crates/core/src/ui/attributes.rs:9`). Acceptance locators use `@Id` — rename- and localization-proof, provider-independent. What remains to verify end-to-end (not evident from source alone) is only that egui forwards our node-builder mutation for these widgets — a quick live check, no longer a gating spike.
- Every toolbar control also carries a tooltip "Action — Shortcut" (e.g. "Refresh Node — F5") and a human-readable AccessKit name equal to the action name, identical in both modes (`Image::alt_text` on the icon, `accesskit_node_builder` as fallback). Names serve humans and screen readers; tests target `@Id`. The picker suite's existing name locator (`inspector_picker.robot:21`) is updated to the new `@Id`.
- Both modes render the same button height (icon sized to font height; text adds width only), so toggling the setting does not shift the layout vertically.

### D5: Search row slimming

The row keeps the 🔍 glyph, the multiline field (same id from `search_field_id()`, same focus-lock and Enter/Shift+Enter handling, same error icon/popup), and Search/Stop. Layout builds right-to-left: button first, then the field takes `available_width()` — the 320 px constant disappears. This deliberately leaves the field's right edge free for the `inspector-xpath-history` dropdown to slot in beside the button.

### D6: Status bar becomes two segments

- Left (transient): existing activity indicator + `status_bar_text()` messages, extended with picker events — at minimum "Picked: <element label>" on a completed pick resolution.
- Right (persistent, right-aligned in the same panel row): picker state derived from existing viewmodel accessors (`picker_armed()`, `picker_active()`, `picker_combo_label()`, `picker_supported`): "Picker: off" / "armed — hold <combo>" / "picking…" / the unavailability reason. This is state, not an event: it must not compete with or be overwritten by transient messages.
- The inline hint labels next to the toggle (`lib.rs:707-717`) are removed; the gesture also lives in the toggle tooltip.

The transient/persistent split is the design point: today's single `status_bar_text()` slot cannot host a permanent state without suppressing result messages.

### D7: Testing posture

Per the crate's existing posture (view = thin rendering, no view unit tests), logic added here stays minimal and testable where it exists: settings round-trip for `toolbar_style`, and any pure formatting helper for the picker-state text. Behavior verification is acceptance-level via the existing BareMetal egui lane, which reads the Inspector's own AccessKit tree: toolbar buttons resolvable by their stable `@Id` under the icons-only default, pin toggle switchable, picker suite (locator updated to `@Id`) stays green, picker-state segment text present in the tree. Suites stay hermetic via `PLATYNUI_INSPECTOR_SETTINGS_PATH`; per project convention they use provider-independent locators (`*:Role` wildcards, no `native:` attributes).

## Risks / Trade-offs

- [`alt_text` may not map to the AccessKit label in egui 0.34] → no longer test-critical since locators target `@Id`; still checked during implementation for screen-reader quality, with an explicit `accesskit_node_builder` label as fallback, kept in one helper so all buttons share the mechanism.
- [egui might not forward `accesskit_node_builder` mutations (`author_id`) for these widgets] → verified live as the first implementation step by inspecting the Inspector with BareMetal; the adapter-side mapping is already confirmed in source (see D4), so the residual risk is narrow.
- [resvg dependency grows compile time for the inspector crate] → accepted explicitly (user decision); scoped to `apps/inspector` only, not the workspace-wide gates' hot path.
- [Both this change and `inspector-xpath-history` edit `toolbar.rs`] → search-field id, key handling, and error-popup code are intentionally untouched; whichever change lands second rebases. The slimmed row leaves room beside the field for the history dropdown.
- [Icon-only default reduces discoverability for first-time users] → tooltips with shortcuts on every button, all actions remain in the menu bar with shortcut hints, and the style can be switched to `IconsAndText` in Settings.
- [White-normalized SVGs look wrong if tinting is ever disabled] → normalization is a documented check-in rule for `assets/icons/`; `image_tint_follows_text_color(true)` is set at the single toolbar-button construction helper.
- [Picker spec's discoverability requirement relocates its "status hint"] → the spec does not prescribe location; proposal flags the relocation explicitly, and the acceptance check moves with it.

## Migration Plan

Additive/behavioral UI change confined to the `platynui-inspector` binary; no native (PyO3) rebuild, no Python/RF surface change, no data migration. `inspector.ron` gains one field with a serde default — old files load unchanged; a file written by the new version and read by an old binary fails RON deserialization of unknown fields only if serde is strict (`#[serde(default)]` struct-level without `deny_unknown_fields` — verified tolerant). Rollback = revert the commits; settings files remain compatible in both directions.

## Open Questions

- Exact Phosphor glyph choices (e.g. `crosshair` vs `target` for picking, `arrows-clockwise` vs `arrow-clockwise` for refresh variants) — decide during implementation with a visual check; not spec-relevant.
- Whether the pick-completed transient message should include the element's role in addition to its name — decide by eyeballing real picks.
