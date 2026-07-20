## MODIFIED Requirements

### Requirement: Reveal-ready hit result
A node returned by JAB hit-testing SHALL carry the same `RuntimeId` that top-down traversal produces for that element (app-scoped, `jab://app/<pid>/…`) and a walkable parent chain up to its `app:Application`, so a consumer can reveal-and-select it in the tree by walking `parent()`. Because `isSameObject` cannot match virtual wrapper children (JTable cells alias the shared cell renderer; `JTableHeader` entries are freshly allocated per lookup), the enumeration-index mapping SHALL fall back to the target wrapper's `indexInParent` — accepted only within the level's child count — when identity matching fails at a level. When no index can be recovered either, the hit SHALL resolve to the deepest matched ancestor (at minimum the claimed window) rather than a parentless node; a parentless best-effort node remains only for hits whose parent chain never reaches the window.

#### Scenario: Picked node reveals to the same tree node
- **WHEN** a control is resolved by point and then located by top-down XPath
- **THEN** both carry the identical `RuntimeId`, and the picked node's ancestors resolve up to the `app:Application` node for the fixture's PID

#### Scenario: Picking a JTable cell reveals the cell's tree node
- **WHEN** the pointer hovers a data cell of the fixture `JTable` (the JVM has seen the mouse) and the point is hit-tested
- **THEN** the resolved node carries the cell's tree `RuntimeId` (the table's path plus the row-major enumeration index) and a walkable chain up to `app:Application`

#### Scenario: Picking a column header reveals the header entry
- **WHEN** the pointer hovers a column header of the fixture `JTable` and the point is hit-tested
- **THEN** the resolved node carries the header entry's tree `RuntimeId` and a walkable chain up to `app:Application`

#### Scenario: Unrecoverable levels degrade to the deepest matched ancestor
- **WHEN** neither identity matching nor the index fallback can map a level of the hit's ancestor chain
- **THEN** the hit resolves to the deepest ancestor matched so far (at minimum the claimed window), with tree `RuntimeId` and walkable chain — not to a parentless node
