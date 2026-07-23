# test-app-blueprint Specification

## Purpose

PlatynUI maintains a fixture technology matrix — one fixture app per UI technology (egui, Qt Widgets, Qt Quick/QML, Swing, SWT, JavaFX, later WPF/Avalonia/Win32) — because toolkits implement accessibility differently and the semantic keyword layer ships per-technology proxies that must be proven against each of them. This blueprint is the contract every fixture app implements: a tiered control catalog under canonical names, name-based action observables, a common CLI contract, shared catalog acceptance-suite conventions, and an optional custom-controls chapter. The developer-facing description lives in `dev-docs/testing-strategy.md` §5.1; this spec is the acceptance-criteria form of the same contract — keep both in sync when the blueprint evolves.

## Requirements

### Requirement: Tiered control catalog
The blueprint SHALL define a control catalog in two tiers. The **core tier** is mandatory for every conforming fixture app and SHALL contain: a main window, a push button with a click-counter observable, a status label, a checkbox, a radio-button group (two buttons), a single-line text field, a static label, a combo box with fixed items, a list with fixed items, a tree with nested nodes (at least three levels), a menu bar with a menu and items, a context menu with items and a nested submenu, a modeless dialog, and a modal dialog (opened via `--open-modal`). The **extended tier** SHALL contain: a table/grid with fixed rows and cells, a slider (writable value), a progress bar (read-only value), and a tab control with at least two tabs. A fixture MAY adopt extended-tier controls incrementally; core-tier omissions SHALL be declared as documented limitations (see catalog-suite requirement), not silently absent.

The catalog covers every standard-role family of the semantic layer's v1 set (`python-library-design.md` §5a.2). Deliberate gaps: role variants whose patterns are already exercised by a catalog control (Link → button, ToggleButton → checkbox, PasswordBox and multi-line text → text field, Spinner → slider, Image → label) and scrolling surfaces (ScrollBar / the `Scrollable` pattern, deferred post-Phase-4 in the library design). Closing such a gap later adds a control under a new canonical name per the no-rename rule.

#### Scenario: Core tier enumerable on a conforming fixture
- **GIVEN** a fixture app that declares blueprint conformance is running
- **WHEN** the platform provider walks the accessibility tree under the fixture's main window (real-provider verification)
- **THEN** every core-tier control is found under its canonical name, and no interactive control reports an empty or duplicate accessible name

#### Scenario: Extended tier is optional but named identically when present
- **WHEN** a fixture implements an extended-tier control (e.g. the table)
- **THEN** the control and its items carry the canonical extended-tier names, identical across all technologies that implement it

### Requirement: Canonical control names
The catalog controls SHALL carry the same kebab-case accessible names in every technology, so one locator set drives all fixtures. The canonical names are: `main-window`, `button-basic`, `status-label`, `checkbox-basic`, `radio-first` / `radio-second`, `textfield-basic`, `label-basic`, `combobox-basic` (items `combo-item-1` … `combo-item-3`), `list-basic` (items `list-item-1` … `list-item-5`), `tree-basic` (roots `tree-node-a` / `tree-node-b`, children `tree-node-a-1` / `tree-node-a-2`, grandchild `tree-node-a-1-i`), menu bar `main-menubar` with menu `menu-file` (items `menu-file-new`, `menu-file-open`, `menu-file-quit`), context menu `context-menu` (items `ctx-cut`, `ctx-copy`, `ctx-paste`; submenu `ctx-more` with `ctx-sub-alpha`, `ctx-sub-beta`), dialogs `dialog-modeless` / `dialog-modal` (each containing `<dialog-ident>-button` and `<dialog-ident>-label`); extended tier: `table-basic` (cells `table-cell-<row>-<col>`, 1-based), `slider-basic`, `progress-basic`, `tabs-basic` (tabs `tab-one` / `tab-two`). Names SHALL be stable: later additions to a fixture SHALL NOT rename or repurpose existing catalog names. The accessible name is the locator contract — fixtures SHALL NOT rely on technology-private IDs (AutomationId, objectName) for catalog addressing.

#### Scenario: Same locator resolves on two technologies
- **GIVEN** two conforming fixture apps of different technologies are each running
- **WHEN** the same name-based locator (e.g. for `list-item-3`) is resolved against each app's tree
- **THEN** it resolves to the corresponding control in both apps without technology-specific adjustments

#### Scenario: Name verified against the real tree before encoding
- **WHEN** a fixture implementation maps a canonical name onto a technology's accessibility API
- **THEN** the surfaced `@Name` is verified against the running app through a real provider (Inspector or `Get Attribute`) before the acceptance suite encodes it, per the testing strategy's verify-against-reality rule

### Requirement: Name-based action observables
Every catalog action SHALL have an effect observable through the accessibility tree by name or text — never by screenshot. The blueprint mechanisms are: activating `button-basic` updates `status-label` so its text and accessible name end with `clicks-<n>` (n = cumulative click count); activating a menu, context-menu, or submenu item renames it to `<ident>-activated`; activating a dialog's `<dialog-ident>-button` renames it to `<dialog-ident>-button-clicked`. State-bearing controls (checkbox, radio, list/tree selection, expansion, slider value) SHALL expose their state through the provider's read attributes; where a technology's bridge does not surface a required state, the fixture SHALL add a name-based observable and document the deviation.

#### Scenario: Click counter accumulates
- **WHEN** `button-basic` is activated twice
- **THEN** `status-label`'s text and accessible name end with `clicks-2`

#### Scenario: Menu activation observable
- **WHEN** the menu item `menu-file-new` is activated via the fixture's real UI
- **THEN** an element named `menu-file-new-activated` is resolvable in the tree, proving activation rather than hover or hit-test

#### Scenario: Dialog click lands inside the dialog
- **WHEN** `dialog-modeless-button` is clicked at its reported bounds
- **THEN** it renames to `dialog-modeless-button-clicked`, proving the pointer action landed inside the dialog and not in the window below

### Requirement: Fixture CLI contract
Every fixture app SHALL support: `--title <text>` (main-window title, default `PlatynUI <Technology> TestApp`), `--auto-close <seconds>` (self-terminate for CI; 0 or absent = never), and `--open-modal` (additionally open `dialog-modal` at startup so no pointer interaction is needed to reach modal state). Unknown arguments SHALL fail with a usage message and a non-zero exit code. Fixtures MAY add technology-specific flags (e.g. `--app-id` where the platform matches windows by application id, `--log-level`, popup-mode switches), but defaults SHALL always produce the conforming catalog under the canonical names.

#### Scenario: Title and auto-close honored
- **WHEN** a fixture starts with `--title "Catalog Fixture" --auto-close 5`
- **THEN** the main window's title is "Catalog Fixture" and the process exits with code 0 no later than a few seconds after the 5-second deadline without user interaction

#### Scenario: Modal state reachable without interaction
- **WHEN** a fixture starts with `--open-modal`
- **THEN** `dialog-modal` is present in the tree with modal state exposed, without any prior pointer or keyboard input

#### Scenario: Unknown argument fails fast
- **WHEN** a fixture starts with `--bogus`
- **THEN** it prints a usage message naming the unknown argument and exits with a non-zero code

### Requirement: Shared catalog acceptance suite
The catalog acceptance tests SHALL be written once as technology-neutral Robot Framework keywords in a shared resource under `tests/acceptance/resources/` (Given/When/Then style per the testing strategy), addressing controls exclusively by canonical name. Each technology SHALL add a thin suite `tests/acceptance/<tech>/catalog.robot` that imports the shared resource, provides only its launch configuration (fixture command from `PLATYNUI_TEST_APP_<TECH>_*` environment variables, window matching), and declares the catalog tests. There SHALL be no per-technology variable files; per-technology CI lanes and robot.toml profiles stay as they are. A technology limitation SHALL surface as an explicitly skipped test whose skip message names the limitation and where it is tracked — a conforming fixture never silently omits a core-tier test.

#### Scenario: New technology onboards without new test logic
- **WHEN** a new conforming fixture's `catalog.robot` is added with only launch configuration
- **THEN** the shared catalog tests run against it unchanged, and failures indicate real behavioral differences of that technology, not suite adaptation gaps

#### Scenario: Known limitation is a documented skip
- **WHEN** a technology cannot satisfy a core catalog behavior (e.g. its bridge exposes no modal state)
- **THEN** the corresponding test is skipped with a message naming the limitation and its tracking location, and the remaining catalog tests still run

### Requirement: Custom-controls chapter (optional)
A fixture MAY implement the custom-controls chapter to probe the lower bound of what default proxies can drive on hand-built widgets: at least one self-drawn activatable control (`custom-button`) with manually wired accessibility (explicit role, name, and action observability equivalent to `button-basic` with counter label `custom-status-label`), and one deliberately non-exposed drawn element (documented in the fixture README) as a negative case. When the chapter is implemented, its names and observables SHALL follow the same canonical scheme so custom-control tests are shared across technologies that implement it.

#### Scenario: Custom control drivable like a native one
- **WHEN** `custom-button` is activated twice via pointer input
- **THEN** `custom-status-label`'s text and accessible name end with `clicks-2`

#### Scenario: Non-exposed element is honestly absent
- **WHEN** the accessibility tree of a fixture implementing the chapter is searched for the documented non-exposed element
- **THEN** the element is not resolvable (or carries no actionable pattern), and the suite asserts that absence as the expected lower-bound behavior
