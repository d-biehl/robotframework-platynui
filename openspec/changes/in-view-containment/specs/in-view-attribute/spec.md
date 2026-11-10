# Spec Delta

## Purpose

What `control:IsInView` says about an element: whether it lies fully inside the area its ancestors show, so that it can be seen in full. Users read it to scroll until an element is in view before they act on it.

## ADDED Requirements

### Requirement: On AT-SPI, IsInView is true only for an element fully inside its ancestors' visible area

On the AT-SPI provider, `control:IsInView` SHALL be `true` exactly when all of the following hold:

- the element's `control:IsVisible` is `true`;
- no ancestor of the element reports `control:IsVisible` as `false`;
- the element's `control:Bounds` are not empty (width and height greater than 0);
- the element's bounds lie inside its visible area.

The visible area SHALL be the intersection of the bounds of the element's ancestors, up to and including the desktop. An ancestor that has no `control:Bounds`, or whose bounds are empty, SHALL NOT narrow the visible area. When the bounds of the ancestors do not overlap, the visible area SHALL be empty and no element SHALL be in view. An edge of the element that coincides with an edge of the visible area SHALL count as inside.

`IsInView` SHALL be listed for the same nodes as before: nodes with the AT-SPI `Component` interface.

#### Scenario: A tab inside the tab list is in view

- **GIVEN** a tab list with bounds x 100..400, y 20..50, below ancestors that contain it
- **AND** a visible tab with bounds x 160..220, y 20..50
- **WHEN** the tab's `IsInView` is read
- **THEN** it SHALL be `true`
- **NOTE:** Verified by a unit test of the containment check and against a real Qt application with a scrolling `QTabBar`.

#### Scenario: A tab that ends exactly at the edge is in view

- **GIVEN** the tab list above
- **AND** a visible tab with bounds x 340..400 whose right neighbour starts at 400
- **WHEN** the tab's `IsInView` is read
- **THEN** it SHALL be `true`

#### Scenario: A tab scrolled partly out of the tab list is not in view

- **GIVEN** the tab list above
- **AND** a visible tab with bounds x 50..110, fifty pixels left of the tab list
- **WHEN** the tab's `IsInView` is read
- **THEN** it SHALL be `false`

#### Scenario: A tab scrolled entirely out of the tab list is not in view

- **GIVEN** the tab list above
- **AND** a visible tab with bounds x 460..520
- **WHEN** the tab's `IsInView` is read
- **THEN** it SHALL be `false`

#### Scenario: An element beyond the desktop is not in view

- **GIVEN** a desktop with bounds 0,0 1920×1080
- **AND** a visible element whose ancestors contain it but which extends to x 1990
- **WHEN** the element's `IsInView` is read
- **THEN** it SHALL be `false`

#### Scenario: An ancestor without bounds does not narrow the visible area

- **GIVEN** a visible element below an application node that has no `control:Bounds`
- **AND** all other ancestors contain the element
- **WHEN** the element's `IsInView` is read
- **THEN** it SHALL be `true`

#### Scenario: An element with empty bounds is not in view

- **GIVEN** a visible element whose bounds are 0×0
- **WHEN** its `IsInView` is read
- **THEN** it SHALL be `false`

### Requirement: On AT-SPI, an element below a hidden ancestor is not in view

On the AT-SPI provider, `control:IsInView` SHALL be `false` for an element when any of its ancestors reports `control:IsVisible` as `false`, whatever the element itself and the ancestor's bounds report.

#### Scenario: Web content of a closed dialog is not in view

- **GIVEN** a closed dialog that reports `IsVisible` as `false` and bounds 0,0 0×0
- **AND** web content below it that reports `IsVisible` as `true` and bounds 10,100 800×600
- **WHEN** the web content's `IsInView` is read
- **THEN** it SHALL be `false`
- **NOTE:** Verified by a unit test of the containment check and against a real Qt application with web content in a closed dialog.

#### Scenario: A list entry of a closed combo box is not in view

- **GIVEN** a closed combo box whose list reports `IsVisible` as `false` and bounds 0×0
- **AND** a list entry below it that reports `Showing`, `Visible` and bounds 0,0 300×25
- **WHEN** the list entry's `IsInView` is read
- **THEN** it SHALL be `false`

### Requirement: On AT-SPI, a popup is judged against the desktop

On the AT-SPI provider, a node SHALL count as a popup when its role is `PopupMenu`, `Menu`, `ToolTip`, `Window` or `Dialog`, or when its role is `List` and its parent is a `ComboBox`. For a popup and its descendants, the visible area SHALL start again from the desktop: the bounds of the ancestors between the popup and the desktop SHALL NOT narrow it. The popup's own bounds SHALL narrow it, except for the list of a combo box, whose own bounds SHALL NOT. An ancestor that reports `control:IsVisible` as `false` SHALL still leave nothing in view.

#### Scenario: The entries of an open menu are in view

- **GIVEN** a menu bar entry `File` at y 24..46
- **AND** its open menu, a `PopupMenu` below that entry at y 46..121
- **WHEN** a menu entry's `IsInView` is read
- **THEN** it SHALL be `true`
- **NOTE:** Verified by the Qt acceptance suite `tests/acceptance/qt/in_view.robot`.

#### Scenario: The entries of an open combo box are in view

- **GIVEN** an open combo box at y 300..329 in a dialog whose top edge is at y 250
- **AND** its popup list, reported at x 400..450, y 240..327, no wider than the combo box
- **AND** entries reported 120 px wide, the first at y 240..269
- **WHEN** an entry's `IsInView` is read
- **THEN** it SHALL be `true`
- **NOTE:** Verified against a real Qt application and by the Qt acceptance suite.

#### Scenario: The entries of a closed combo box stay out of view

- **GIVEN** a closed combo box whose list reports `IsVisible` as `false`
- **WHEN** an entry's `IsInView` is read
- **THEN** it SHALL be `false`

### Requirement: On AT-SPI, IsInView allows for the rounding of reported coordinates

AT-SPI reports positions and sizes as whole pixels. A toolkit that lays out in fractions of a pixel rounds each box on its own, so a child that fits its parent can be reported up to one pixel beyond it on each side. The containment check on the AT-SPI provider SHALL allow the element to exceed its visible area by up to one pixel on each side, and SHALL NOT allow more.

#### Scenario: Inline text that exceeds its parent by one pixel is in view

- **GIVEN** a visible label with bounds y 40..57 whose ancestors contain it
- **AND** a visible text child with bounds y 39..58
- **WHEN** the text child's `IsInView` is read
- **THEN** it SHALL be `true`
- **NOTE:** Verified by a unit test and against inline text in web content.

#### Scenario: An element that exceeds its parent by two pixels is not in view

- **GIVEN** a visible parent with bounds y 40..57 whose ancestors contain it
- **AND** a visible child with bounds y 38..57
- **WHEN** the child's `IsInView` is read
- **THEN** it SHALL be `false`

### Requirement: On AT-SPI, IsInView follows the current geometry in every query

On the AT-SPI provider, `control:IsInView` SHALL be judged from the `IsVisible` and `Bounds` the element and its ancestors report when the value is read. The provider MAY share what it reads of an ancestor between the elements of one query. It SHALL NOT reuse it in a later query, and SHALL NOT reuse it for a value read outside a query, even when the runtime keeps its snapshot of the tree between queries.

#### Scenario: A tab is judged by where its dialog is now

- **GIVEN** a runtime that keeps its snapshot between queries
- **AND** a tab fully inside its tab list, read as in view by a query
- **WHEN** the dialog that holds the tab list is moved, and a second query reads the tab's `IsInView`
- **THEN** it SHALL be `true` while the tab is fully inside the moved tab list
- **NOTE:** Verified against a real Qt application by dragging the dialog away and back within one runtime.

#### Scenario: A node held from an earlier query is judged by the current geometry

- **GIVEN** a node returned by an earlier query
- **AND** its ancestors moved after that query
- **WHEN** the node's `IsInView` is read directly, outside a query
- **THEN** it SHALL be judged by the bounds its ancestors report at that moment
