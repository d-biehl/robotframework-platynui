## ADDED Requirements

### Requirement: Tabular content served by the agent is structured by row
A table surfaced through the agent backend SHALL place its cells beneath **row** nodes rather than directly beneath the table, so a row is addressable in its own right. Each row SHALL carry its own identity, its own selection state, and — when it is in view — its own on-screen rectangle; each cell SHALL remain reachable and keep the coordinates it reports today, so a cell's position stays knowable both structurally and by attribute.

A table is routinely larger than the viewport showing it, and the toolkit answers geometry questions from the model regardless of what is scrolled into view. Rows and cells that are **not** on screen SHALL therefore report no rectangle at all and SHALL NOT claim to be in view, rather than publishing the position they would occupy — a rectangle outside the window would aim pointer input at whatever is there instead.

The row level SHALL come from the toolkit's own model rather than from the accessibility view — which for Swing offers only a flat list of cells, and is the reason the flat shape existed at all. This aligns the Java tree with what the other providers already surface for tabular content, and the alignment is the point: a table should not have a different shape merely because of which technology reads it.

#### Scenario: A table's children are its rows
- **WHEN** a Swing table with four rows and three columns is enumerated through the agent backend
- **THEN** the table node has four children, each a row, and each row has three cells — not twelve cells directly under the table

#### Scenario: A cell still knows where it is
- **WHEN** a cell inside a row is inspected
- **THEN** it reports the same row and column coordinates as before the row level existed, and its name, bounds and selection state are unchanged

#### Scenario: A row is addressable and locatable
- **WHEN** a row that is in view is inspected
- **THEN** it has an on-screen rectangle spanning its cells, a selection state that reflects whether the row is selected, and an identity that stays the same across repeated enumerations of the unchanged table

#### Scenario: Content scrolled out of view is present but has no place on screen
- **WHEN** a row far below the visible part of a scrolling table, and one of its cells, are inspected
- **THEN** both are in the tree with their names, coordinates and identity intact
- **AND** neither reports a rectangle or anything to aim pointer input at, and both report that they are not in view

#### Scenario: Hit-testing passes through the row
- **WHEN** a point inside a cell is hit-tested through the agent backend
- **THEN** the returned chain reaches the cell by way of its row, so a consumer revealing the result can place it in the tree

#### Scenario: Selected cells are still named correctly
- **WHEN** a table reports its selection while its cells sit beneath rows
- **THEN** the identifiers it publishes for the selected items resolve to nodes that exist in the tree, or are omitted — never identifiers assembled from a position that no longer addresses what it used to
