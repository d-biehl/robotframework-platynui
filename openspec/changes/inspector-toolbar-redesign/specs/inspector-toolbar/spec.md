# inspector-toolbar Delta

## ADDED Requirements

### Requirement: All header UI is rendered inside themed panels

Every element of the Inspector's header area (menu bar, toolbar, search row) SHALL be rendered inside an `egui` panel that paints the theme's panel background. No header region SHALL show the raw window clear color in either the light or the dark theme.

#### Scenario: No unthemed strip in light theme

- **WHEN** the Inspector runs with the system in light theme
- **THEN** the region hosting the picker toggle SHALL show the light panel background, not a black strip
- **NOTE** Visual/theme rendering is verifiable only against a real session (screenshot or manual check), not the mock.

#### Scenario: No unthemed strip in dark theme

- **WHEN** the Inspector runs with the system in dark theme
- **THEN** the region hosting the picker toggle SHALL show the dark panel background consistent with the surrounding panels

### Requirement: The toolbar hosts the primary actions

The Inspector SHALL show a toolbar directly below the menu bar containing, in order: the Pick Element toggle, Refresh Node, Refresh Subtree, and Highlight Node buttons, and — right-aligned — an Always-on-Top toggle. The node actions (Refresh Node, Refresh Subtree, Highlight Node) SHALL be disabled while no tree node is selected and enabled while one is. The Pick Element toggle SHALL be disabled where live picking is unsupported. All of these actions SHALL remain reachable through the menu bar as well.

#### Scenario: Node actions disabled without a selection

- **WHEN** the Inspector starts and no tree node is selected
- **THEN** the Refresh Node, Refresh Subtree, and Highlight Node toolbar buttons SHALL be disabled

#### Scenario: Node actions enabled by selecting a node

- **WHEN** the user selects a node in the tree
- **THEN** the Refresh Node, Refresh Subtree, and Highlight Node toolbar buttons SHALL be enabled

#### Scenario: Toolbar buttons trigger the same actions as the menu

- **WHEN** the user clicks Refresh Node in the toolbar with a node selected
- **THEN** the selected node SHALL be refreshed exactly as via the Node menu's Refresh Node entry

### Requirement: Always-on-Top is a toolbar toggle that controls the window level

The Always-on-Top control SHALL be a toggle in the toolbar (replacing the former checkbox). Turning it on SHALL raise the Inspector window to always-on-top; turning it off SHALL restore the normal window level. The toggle SHALL visibly reflect its current state and SHALL render fully inside the toolbar without clipping in both display styles.

#### Scenario: Enabling always-on-top raises the window

- **WHEN** the user turns the Always-on-Top toggle on
- **THEN** the Inspector window SHALL be set to the always-on-top level
- **NOTE** The actual window level is verifiable only against a real window manager (X11/compositor lane), not the mock.

#### Scenario: Disabling always-on-top restores the window level

- **WHEN** the user turns the Always-on-Top toggle off
- **THEN** the Inspector window SHALL return to the normal window level

### Requirement: Toolbar display style is configurable and persisted

The Inspector SHALL support two toolbar display styles — icons only (the default) and icons with text labels — selectable in the Settings dialog and persisted with the Inspector's other settings so the choice survives restarts. Switching the style SHALL NOT change the toolbar's height.

#### Scenario: Default style is icons only

- **WHEN** the Inspector starts with no persisted settings file
- **THEN** the toolbar SHALL render icon-only buttons

#### Scenario: Selected style survives a restart

- **WHEN** the user selects the icons-with-text style in Settings and restarts the Inspector with the same settings file
- **THEN** the toolbar SHALL render icons with text labels

#### Scenario: Settings file from an older version loads

- **WHEN** the Inspector loads a persisted settings file that predates the display-style setting
- **THEN** the settings SHALL load successfully and the toolbar SHALL use the icons-only default

### Requirement: Toolbar controls expose stable element IDs and mode-independent accessible names

Every toolbar control SHALL expose a stable element ID, queryable as the provider-independent common `@Id` attribute (backed by AccessKit `author_id`, surfacing as UIA `AutomationId` on Windows and AT-SPI `AccessibleId` on Linux). IDs SHALL be identical in both display styles and SHALL NOT change when a control's visible label or icon changes. In addition, every toolbar control SHALL expose a human-readable accessible name equal to its action name, identical in both display styles, and SHALL show a hover tooltip stating the action name and, where one exists, its keyboard shortcut.

#### Scenario: Buttons resolvable by ID in icons-only mode

- **WHEN** the toolbar renders in the default icons-only style
- **THEN** each toolbar control SHALL be resolvable in the Inspector's accessibility tree by its stable ID (e.g. `//*[@Id="refresh-node"]`)
- **NOTE** Verified via the BareMetal egui acceptance lane reading the Inspector's own AccessKit tree.

#### Scenario: IDs and names identical in both display styles

- **WHEN** the display style is switched from icons-only to icons-with-text
- **THEN** every toolbar control SHALL expose the same `@Id` and the same accessible name as before

#### Scenario: Picker suite locates the toggle by ID

- **WHEN** the picker acceptance suite (with its locator updated to the pick toggle's `@Id`) runs against the redesigned Inspector with default settings
- **THEN** it SHALL resolve the pick toggle and pass

### Requirement: Toolbar icons follow the active theme

Toolbar icons SHALL be rendered from embedded monochrome vector assets and tinted with the current text color, so they adapt to light theme, dark theme, and the disabled state without per-icon color handling. Active toggles (Pick Element armed, Always-on-Top on) SHALL be visually distinguishable from their inactive state.

#### Scenario: Icons legible in both themes

- **WHEN** the Inspector runs in light theme and then in dark theme
- **THEN** toolbar icons SHALL render in a color derived from the theme's text color in each case
- **NOTE** Verifiable only visually against a real session.

#### Scenario: Armed picker toggle is visually distinct

- **WHEN** the user arms the Pick Element toggle
- **THEN** the toggle SHALL render in a visibly active state distinct from the disarmed state

### Requirement: The search row contains only the search field and the Search/Stop button

The search row SHALL contain the XPath input field and the Search/Stop button, with the field taking all remaining row width (no fixed-width reservation). The field's existing behavior SHALL be preserved: plain Enter evaluates, Shift+Enter inserts a newline, Escape cancels a running search, and the parse-error icon and popup continue to work.

#### Scenario: No clipped controls in the search row

- **WHEN** the Inspector window is at its default size
- **THEN** every control in the search row SHALL be fully visible (no truncated labels)

#### Scenario: Field behavior unchanged after the redesign

- **WHEN** the user types an XPath expression and presses Enter
- **THEN** the expression SHALL be evaluated exactly as before the redesign

#### Scenario: Refresh controls no longer in the search row

- **WHEN** the redesigned Inspector renders
- **THEN** the Refresh Node and Refresh Subtree controls SHALL be located in the toolbar and SHALL NOT appear in the search row
