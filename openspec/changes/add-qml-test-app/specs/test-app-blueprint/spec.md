## MODIFIED Requirements

### Requirement: Tiered control catalog
The blueprint SHALL define a control catalog in two tiers. The **core tier** is mandatory for every conforming fixture app and SHALL contain: a main window, a push button with a click-counter observable, a status label, a checkbox, a group box titled with its canonical name that groups a radio-button group (two buttons), a single-line text field, a multi-line text area, a static label, a plain static text, an image, a combo box with fixed items, a list with fixed items, a tree with nested nodes (at least three levels), a menu bar with three menus (one carrying items only, one carrying items plus a nested submenu, one minimal), a context menu with items and a nested submenu, a modeless dialog, and a modal dialog (opened via `--open-modal`). The **extended tier** SHALL contain: a table/grid with fixed rows and cells, a slider (writable value), a progress bar (read-only value), and a tab control with at least two tabs. A fixture MAY adopt extended-tier controls incrementally; core-tier omissions SHALL be declared as documented limitations (see catalog-suite requirement), not silently absent.

The catalog covers every standard-role family of the semantic layer's v1 set (`python-library-design.md` §5a.2). Deliberate gaps: role variants whose patterns are already exercised by a catalog control (Link → button, ToggleButton → checkbox, PasswordBox → text field, Spinner → slider) and scrolling surfaces (ScrollBar / the `Scrollable` pattern, deferred post-Phase-4 in the library design). Closing such a gap later adds a control under a new canonical name per the no-rename rule.

#### Scenario: Core tier enumerable on a conforming fixture
- **GIVEN** a fixture app that declares blueprint conformance is running
- **WHEN** the platform provider walks the accessibility tree under the fixture's main window (real-provider verification)
- **THEN** every core-tier control is found under its canonical name, and no interactive control reports an empty or duplicate accessible name

#### Scenario: Extended tier is optional but named identically when present
- **WHEN** a fixture implements an extended-tier control (e.g. the table)
- **THEN** the control and its items carry the canonical extended-tier names, identical across all technologies that implement it

### Requirement: Canonical control names
The catalog controls SHALL carry the same kebab-case accessible names in every technology, so one locator set drives all fixtures. The canonical names are: `main-window`, `button-basic`, `status-label`, `checkbox-basic`, `groupbox-basic` (grouping `radio-first` / `radio-second`), `textfield-basic`, `textarea-basic`, `label-basic`, `text-basic`, `image-basic`, `combobox-basic` (items `combo-item-1` … `combo-item-3`), `list-basic` (items `list-item-1` … `list-item-5`), `tree-basic` (roots `tree-node-a` / `tree-node-b`, children `tree-node-a-1` / `tree-node-a-2`, grandchild `tree-node-a-1-i`), menu bar `main-menubar` with menus `menu-file` (items `menu-file-new`, `menu-file-open`, `menu-file-quit`), `menu-edit` (items `menu-edit-undo`, `menu-edit-redo`; submenu `menu-edit-more` with `menu-edit-sub-one`, `menu-edit-sub-two`), and `menu-help` (item `menu-help-about`), context menu `context-menu` (items `ctx-cut`, `ctx-copy`, `ctx-paste`; submenu `ctx-more` with `ctx-sub-alpha`, `ctx-sub-beta`), dialogs `dialog-modeless` / `dialog-modal` (each containing `<dialog-ident>-button` and `<dialog-ident>-label`); extended tier: `table-basic` (cells `table-cell-<row>-<col>`, 1-based), `slider-basic`, `progress-basic`, `tabs-basic` (tabs `tab-one` / `tab-two`). Names SHALL be stable: later additions to a fixture SHALL NOT rename or repurpose existing catalog names. The accessible name is the locator contract — fixtures SHALL NOT rely on technology-private IDs (AutomationId, objectName) for catalog addressing, and shared catalog locators SHALL address controls by `@Name` alone (names are pairwise-unique app-wide; roles differ across bridges and are not part of the shared contract). Where a technology derives a **window's** accessible name from its title and the name cannot be set independently (verified reality on Qt Quick, where `Accessible` does not attach to windows), the main window SHALL be matched via the launch configuration's window matching instead of the `main-window` name, and child windows SHALL carry their canonical name as their title so it still surfaces as `@Name`.

#### Scenario: Same locator resolves on two technologies
- **GIVEN** two conforming fixture apps of different technologies are each running
- **WHEN** the same name-based locator (e.g. for `list-item-3`) is resolved against each app's tree
- **THEN** it resolves to the corresponding control in both apps without technology-specific adjustments

#### Scenario: Name verified against the real tree before encoding
- **WHEN** a fixture implementation maps a canonical name onto a technology's accessibility API
- **THEN** the surfaced `@Name` is verified against the running app through a real provider (Inspector or `Get Attribute`) before the acceptance suite encodes it, per the testing strategy's verify-against-reality rule

#### Scenario: Window naming falls back to launch configuration
- **GIVEN** a technology whose bridge reports a window's title as its accessible name and offers no independent window name
- **WHEN** the catalog suite locates the main window
- **THEN** it matches via the launch configuration (title/app id/process pinning), the fixture README documents the deviation, and dialog child windows still resolve by their canonical names (used as titles)

### Requirement: Fixture CLI contract
Every fixture app SHALL support: `--title <text>` (main-window title, default `PlatynUI <Technology> TestApp`), `--auto-close <seconds>` (self-terminate for CI; 0 or absent = never), and `--open-modal` (additionally open `dialog-modal` at startup so no pointer interaction is needed to reach modal state). Unknown arguments SHALL fail with a usage message and a non-zero exit code. Fixtures MAY add technology-specific flags (e.g. `--app-id` where the platform matches windows by application id, `--log-level`, popup-mode switches), but defaults SHALL always produce the conforming catalog under the canonical names.

#### Scenario: Title and auto-close honored
- **WHEN** a fixture starts with `--title "Catalog Fixture" --auto-close 5`
- **THEN** the main window's title is "Catalog Fixture" and the process exits with code 0 no later than a few seconds after the 5-second deadline without user interaction

#### Scenario: Modal state reachable without interaction
- **WHEN** a fixture starts with `--open-modal`
- **THEN** `dialog-modal` and its contents are present in the tree without any prior pointer or keyboard input, with modal state exposed where the technology's bridge surfaces it — where it does not (verified reality for Qt Quick's in-scene `Dialog` on UIA, which carries no window pattern), the documented-deviation rule of the observables requirement applies and presence + interactability are the asserted facts

#### Scenario: Unknown argument fails fast
- **WHEN** a fixture starts with `--bogus`
- **THEN** it prints a usage message naming the unknown argument and exits with a non-zero code
