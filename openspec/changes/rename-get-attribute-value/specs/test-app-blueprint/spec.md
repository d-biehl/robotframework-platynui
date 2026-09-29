# Spec Delta

## MODIFIED Requirements

### Requirement: Canonical control names
The catalog controls SHALL carry the same kebab-case accessible names in every technology, so one locator set drives all fixtures. The canonical names are: `main-window`, `button-basic`, `status-label` (surfacing as `status-label-clicks-<n>`), the last-action label (surfacing as `last-action-<ident>`, initially `last-action-none`), `checkbox-basic`, `groupbox-basic` (grouping `radio-first` / `radio-second`), `textfield-basic`, `textarea-basic`, `label-basic`, `text-basic`, `image-basic`, `combobox-basic` (items `combo-item-1` … `combo-item-3`), `list-basic` (items `list-item-1` … `list-item-5`), `tree-basic` (roots `tree-node-a` / `tree-node-b`, children `tree-node-a-1` / `tree-node-a-2`, grandchild `tree-node-a-1-i`), menu bar `main-menubar` with menus `menu-file` (items `menu-file-new`, `menu-file-open`, `menu-file-quit`), `menu-edit` (items `menu-edit-undo`, `menu-edit-redo`; submenu `menu-edit-more` with `menu-edit-sub-one`, `menu-edit-sub-two`), and `menu-help` (item `menu-help-about`), context menu `context-menu` (items `ctx-cut`, `ctx-copy`, `ctx-paste`; submenu `ctx-more` with `ctx-sub-alpha`, `ctx-sub-beta`), dialogs `dialog-modeless` / `dialog-modal` (each containing `<dialog-ident>-button` and `<dialog-ident>-label`); extended tier: `table-basic` (cells `table-cell-<row>-<col>`, 1-based), `slider-basic`, `progress-basic`, `tabs-basic` (tabs `tab-one` / `tab-two`). Names SHALL be stable: later additions to a fixture SHALL NOT rename or repurpose existing catalog names. The accessible name is the locator contract — fixtures SHALL NOT rely on technology-private IDs (AutomationId, objectName) for catalog addressing, and shared catalog locators SHALL address controls by `@Name` alone (names are pairwise-unique app-wide; roles differ across bridges and are not part of the shared contract). Where a technology derives a **window's** accessible name from its title and the name cannot be set independently (verified reality on Qt Quick, where `Accessible` does not attach to windows), the main window SHALL be matched via the launch configuration's window matching instead of the `main-window` name, and child windows SHALL carry their canonical name as their title so it still surfaces as `@Name`.

#### Scenario: Same locator resolves on two technologies
- **GIVEN** two conforming fixture apps of different technologies are each running
- **WHEN** the same name-based locator (e.g. for `list-item-3`) is resolved against each app's tree
- **THEN** it resolves to the corresponding control in both apps without technology-specific adjustments

#### Scenario: Name verified against the real tree before encoding
- **WHEN** a fixture implementation maps a canonical name onto a technology's accessibility API
- **THEN** the surfaced `@Name` is verified against the running app through a real provider (Inspector or `Get Attribute Value`) before the acceptance suite encodes it, per the testing strategy's verify-against-reality rule

#### Scenario: Window naming falls back to launch configuration
- **GIVEN** a technology whose bridge reports a window's title as its accessible name and offers no independent window name
- **WHEN** the catalog suite locates the main window
- **THEN** it matches via the launch configuration (title/app id/process pinning), the fixture README documents the deviation, and dialog child windows still resolve by their canonical names (used as titles)
