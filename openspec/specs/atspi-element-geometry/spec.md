# atspi-element-geometry Specification

## Purpose

Defines how the AT-SPI provider places a node on the desktop: relative to the node's window, read in the coordinate type the node's toolkit implements consistently, and anchored at the window's origin as the platform reports it. A popup the platform shows as a surface of its own counts as such a window.

## Requirements

### Requirement: An element is placed relative to its window

The bounds of a node inside a window SHALL be the window's coordinate origin plus the node's position relative to that window, in desktop coordinates. The window is the nearest ancestor that is a top-level window or a popup matched to the platform's popup list (see *A popup is placed where the platform shows it*). A node's bounds SHALL NOT depend on the extents its ancestors between it and its window report.

#### Scenario: A click on a nested button lands on it

- **GIVEN** a fixture window positioned away from the screen origin, with a control inside a group, such as the blueprint's `radio-first` inside `groupbox-basic`
- **WHEN** `Pointer Click` is called with that control's locator
- **THEN** the control SHALL report the click through its observable
- **NOTE** Real provider only, on the X11 lane and the compositor lane, for each Linux fixture: egui, Qt Widgets, Qt Quick, and the GTK3 and GTK4 fixtures.

#### Scenario: The hit-test resolves a point inside the reported bounds back to the element

- **GIVEN** a button inside a window positioned away from the screen origin
- **WHEN** the element at the center of the button's `Bounds` is resolved
- **THEN** the result SHALL be that button or one of its descendants, never its window or a sibling
- **NOTE** Real provider only, on both Linux lanes.

#### Scenario: Items of a grafted popup are placed relative to the popup

- **GIVEN** an open Qt context menu, surfaced as a popup directly under the application
- **WHEN** one of its items is clicked by locator
- **THEN** the click SHALL land on that item
- **NOTE** Real provider only. The popup's own bounds come from the source `compositor-popup-geometry` specifies; the Qt context-menu acceptance suite covers this on both lanes.

### Requirement: A popup is placed where the platform shows it

A node SHALL be a popup when it is a transient popup grafted under its application or when it reports the AT-SPI popup layer, as GTK 4 popovers do. A popup matched to a listed popup of the platform (see *A popup is matched to the platform's popup by its surface*) SHALL have the listed visible rectangle as its own bounds. Its descendants SHALL be placed at the listed surface's origin plus their position relative to the popup node.

#### Scenario: An item of a GTK 4 submenu is clicked where it is drawn

- **GIVEN** the GTK 4 fixture with its `menu-edit` menu and the nested submenu `menu-edit-more` open
- **WHEN** an item of the submenu is clicked by locator
- **THEN** the item SHALL report the click through the `last-action-<ident>` label
- **NOTE** Real provider only, on both Linux lanes, with GTK 4.18 or later. GTK 4 measures a nested popover from the item that opened it, about 100 px from where it is drawn.

#### Scenario: An item of a GTK 4 drop-down is selected where it is drawn

- **GIVEN** the GTK 4 fixture's `combobox-basic` with its list open
- **WHEN** an item of the list is clicked by locator
- **THEN** the list SHALL close with that item selected
- **NOTE** Real provider only, on both Linux lanes, with GTK 4.18 or later.

#### Scenario: A popup with a shadow is matched by its surface

- **GIVEN** a node on the popup layer of size 204×128, and a listed popup of the same process whose visible rectangle is `(265,179,158,82)` on a surface `(242,158,204,128)`
- **WHEN** the bounds of the node and of a descendant 23 px right of and 21 px below the node are read
- **THEN** the node's bounds SHALL be `(265,179,158,82)`
- **AND** the descendant SHALL start at `(265,179)`
- **NOTE** Unit test with geometry sources that answer directly; the numbers are a measured GTK 4 submenu under the PlatynUI compositor.

### Requirement: A popup is matched to the platform's popup by its surface

A popup's geometry SHALL come from the platform's popup list for its process. The popup SHALL be matched by its own size against each listed popup's surface, or else its visible rectangle, and the first match in the list SHALL win: the most recently opened or topmost popup. Its descendants SHALL count from the origin of the rectangle it matched. A popup that matches no listed popup SHALL be placed like any other node of its window.

#### Scenario: A GTK 3 menu is matched by its visible size

- **GIVEN** a GTK 3 menu node on the popup layer of size 177×93, and a listed popup of its process whose visible rectangle is `(34,105,177,93)` on a surface `(28,100,189,105)`
- **WHEN** the node's bounds are read
- **THEN** they SHALL be `(34,105,177,93)`, and its descendants SHALL count from `(34,105)`
- **NOTE** Unit test with geometry sources that answer directly; the numbers are a measured GTK 3 menu under the PlatynUI compositor.


#### Scenario: Popups of equal size resolve to the first one listed

- **GIVEN** two listed popups of one process with surfaces of the same size, and one node on the popup layer of that size
- **WHEN** the node's bounds are read
- **THEN** they SHALL come from the popup listed first
- **NOTE** Unit test.

#### Scenario: A popup the platform does not list keeps its window placement

- **GIVEN** a node on the popup layer whose size matches no listed popup of its process, or a platform without a popup list
- **WHEN** its bounds are read
- **THEN** it SHALL be placed relative to its top-level window like any other node
- **NOTE** Unit test.

### Requirement: A popup listed beside its items places them

A popup that matches no listed popup SHALL place its children from a listed popup among those children, when there is one. GTK 3 lists a menu's items, and the node of the menu itself, below the item that opened the menu.

#### Scenario: A GTK 3 menu places the items listed beside it

- **GIVEN** the GTK 3 fixture under the PlatynUI compositor with its `menu-file` menu open, whose items GTK 3 lists below `menu-file` beside the menu's own node
- **WHEN** `menu-file-new` is clicked by locator
- **THEN** the item SHALL report the click through the `last-action-<ident>` label
- **NOTE** Real provider only (GTK3 fixture's catalog, compositor lane); the reference handed from `menu-file` to its items is also unit-tested.

### Requirement: A toolkit is read in the coordinate type it implements consistently

The position relative to the window SHALL come from the toolkit's screen coordinates, except for a toolkit that reports no screen position, which SHALL be read in window coordinates. GTK 4 is such a toolkit. So is any node whose screen position equals its window's screen position while its window position does not. Parent-relative coordinates SHALL NOT be used, because toolkits implement them inconsistently.

#### Scenario: A GTK 4 element is read in window coordinates

- **GIVEN** a GTK 4 window positioned away from the screen origin, whose elements all report the screen position `(0,0)`
- **WHEN** the bounds of a button inside it are read
- **THEN** they SHALL be the window's origin plus the button's window-relative position
- **NOTE** Real provider only (GTK4 fixture); the choice itself is also unit-tested with geometry sources that answer directly.

#### Scenario: A toolkit that reports screen values as parent-relative ones is not added up

- **GIVEN** a GTK 3 window, whose parent-relative extents are its screen extents, with a control several levels below the window
- **WHEN** the control's bounds are read
- **THEN** they SHALL be the control's real position, not the sum of its ancestors' positions
- **NOTE** Real provider only (GTK3 fixture); also unit-tested.

#### Scenario: An element inside a Qt MDI subwindow is placed by screen coordinates

- **GIVEN** a Qt Widgets window with an MDI subwindow offset inside the main window, and a button in that subwindow
- **WHEN** the button is clicked by locator
- **THEN** the click SHALL land on the button
- **NOTE** Real provider only (the Qt Widgets fixture's MDI scenario), on both Linux lanes. Qt's window coordinates count from the subwindow, its screen coordinates from the real window.

#### Scenario: An unknown toolkit without screen positions falls back to window coordinates

- **GIVEN** a node of a toolkit not known to PlatynUI, whose screen position equals its window's screen position while its window position is `(40,60)`
- **WHEN** its bounds are read
- **THEN** they SHALL be its window's origin plus `(40,60)`
- **NOTE** Unit test with geometry sources that answer directly; no fixture has an unknown toolkit.

#### Scenario: A node with consistent screen coordinates keeps them

- **GIVEN** a node of a toolkit not known to PlatynUI, whose screen position is `(140,160)` inside a window whose screen position is `(100,100)`
- **WHEN** its bounds are read
- **THEN** its position relative to the window SHALL be `(40,60)`, whatever its window coordinates say
- **NOTE** Unit test.

### Requirement: Screen coordinates are re-based where they are window-relative

Where the platform states that toolkit screen coordinates are desktop coordinates, as on X11 with a window manager, a node's screen extents SHALL be used as its bounds. Elsewhere, such as under the PlatynUI compositor or on X11 without a window manager, a node's position SHALL be its window's surface origin plus the difference between the node's screen position and its window's screen position.

#### Scenario: A Qt window under the compositor is not placed at the screen corner

- **GIVEN** a Qt 6 window under the PlatynUI compositor, placed away from the screen origin, whose toolkit reports the screen's top-left corner as its own screen position
- **WHEN** a button inside it is clicked by locator
- **THEN** the click SHALL land on the button
- **NOTE** Real provider only (Qt Widgets fixture, compositor lane).

#### Scenario: A window whose toolkit pins it to its output's origin keeps its elements

- **GIVEN** a window whose toolkit reports `(1920,0)`, the origin of a second output, as its screen position, and a node inside it at screen position `(1960,60)`
- **WHEN** the node's bounds are read on a platform whose toolkit screen coordinates are window-relative
- **THEN** the node SHALL be placed at the window's surface origin plus `(40,60)`
- **NOTE** Unit test; the lanes have one output.

#### Scenario: An element of a window moved without a window manager is placed where it is drawn

- **GIVEN** the egui fixture on an X11 display without a window manager, whose window another client moved away from where it opened, so that its toolkit's screen positions still count from the old origin
- **WHEN** a button inside it is clicked by locator
- **THEN** the click SHALL land on the button
- **NOTE** Real provider only, X11 lane without a window manager; the egui auto-activate suite and the activation test of the bare-mode suite move windows before they click.

#### Scenario: On X11 screen extents are the bounds

- **GIVEN** a Qt Widgets window on the X11 lane with a window manager
- **WHEN** the bounds of a button inside it are read
- **THEN** they SHALL equal the button's screen extents (`native:Component.Extents.Screen`)
- **NOTE** Real provider only (X11 lane).

### Requirement: A container without extents does not break placement

A node whose extents cannot be read, because it has no Component interface or reports no geometry, SHALL have no bounds of its own. Its descendants SHALL still be placed, because their placement does not go through it.

#### Scenario: An egui button below a pane without extents is clickable under the compositor

- **GIVEN** the egui fixture under the PlatynUI compositor, whose buttons sit below panel nodes that report no extents
- **WHEN** a button is clicked by locator
- **THEN** the click SHALL land on the button and its counter SHALL increase
- **NOTE** Real provider only; the existing egui acceptance suites on the compositor lane cover it.

#### Scenario: A node below a container without geometry is placed

- **GIVEN** a node whose parent reports no extents, inside a window at origin `(10,40)`, with the node at window-relative position `(30,20)`
- **WHEN** the node's bounds are read
- **THEN** they SHALL start at `(40,60)`
- **AND** the parent SHALL have no bounds
- **NOTE** Unit test.

### Requirement: A window's own bounds are its visible geometry

A top-level window's bounds SHALL be the window as the user sees it. That includes decorations the toolkit draws, such as a header bar, and excludes a client-side shadow, so position and size describe the same rectangle. The origin its elements are placed from SHALL be the surface origin their toolkit counts from, which can differ from the visible geometry's origin.

#### Scenario: A GTK 3 window with a header bar reports its visible frame

- **GIVEN** a GTK 3 window with a header bar under the PlatynUI compositor, whose surface carries a shadow inset
- **WHEN** the window's bounds are read
- **THEN** they SHALL equal the window geometry the compositor reports for it, with the shadow excluded from both position and size
- **AND** a button inside the window SHALL still be clicked where it is drawn
- **NOTE** Real provider only (GTK3 fixture, whose main window has a header bar; compositor lane).

#### Scenario: A GTK 4 window with a header bar places its elements from its visible frame

- **GIVEN** a GTK 4 window with a header bar under the PlatynUI compositor, whose surface carries a shadow inset
- **WHEN** a button inside it is clicked by locator
- **THEN** the click SHALL land on the button
- **NOTE** Real provider only (GTK4 fixture, whose main window has a header bar; compositor lane).

#### Scenario: A window with server-side decorations keeps its client area

- **GIVEN** a window with server-side decorations under the PlatynUI compositor
- **WHEN** its bounds are read
- **THEN** they SHALL be its client area, without the compositor's frame
- **NOTE** Real provider only (egui fixture, compositor lane).
