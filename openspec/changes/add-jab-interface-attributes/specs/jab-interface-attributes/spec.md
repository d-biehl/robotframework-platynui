## ADDED Requirements

### Requirement: Interface-gated native attribute projection
The JAB provider SHALL expose the properties of the accessibility interfaces an element actually supports as `native:*` attributes, gated by the element's `interfaces` bitfield (the same source that already produces the `native:Interfaces` name-list). An element that does not support an interface SHALL NOT carry that interface's attributes, and no bridge call for that interface SHALL be issued. Attribute names SHALL follow the dotted convention `<Interface>.<Property>` (e.g. `Table.RowCount`, `Value.Current`, `Text.CaretIndex`), mirroring the UIA provider's programmatic-name convention. (All scenarios in this spec are real-provider-only: they require a live JVM with the bridge enabled and run in the Windows acceptance lane against the Swing fixture app, unless marked mock-lane.)

#### Scenario: Only supported interfaces contribute attributes
- **WHEN** a plain label node (no `Table`/`Value`/`Text` interface) is inspected
- **THEN** it carries none of the `native:Table.*`, `native:Value.*`, or `native:Text.*` attributes, and the existing `native:Interfaces` name-list still lists exactly its supported interfaces

#### Scenario: Attribute-name catalog is well-formed
- **WHEN** the property-name mapping unit test runs over the interface property catalog
- **THEN** every emitted name matches `<Interface>.<Property>` with a known interface prefix, and no two entries collide (mock-lane verifiable — pure mapping test)

### Requirement: Table interface attributes
For an element supporting `AccessibleTable`, the provider SHALL expose container-level table properties as `native:Table.*` attributes derived from `getAccessibleTableInfo` and the selection/header calls: at least `Table.RowCount`, `Table.ColumnCount`, and the selected-row/column counts, plus caption/summary presence where available. Cells within the table SHALL expose `native:TableCell.*` attributes derived from `getAccessibleTableCellInfo` — at least `TableCell.Row`, `TableCell.Column`, `TableCell.RowExtent`, `TableCell.ColumnExtent`, and `TableCell.IsSelected`.

#### Scenario: JTable reports its dimensions
- **WHEN** the fixture's `JTable` node is inspected
- **THEN** `@native:Table.RowCount` and `@native:Table.ColumnCount` equal the fixture table's actual row and column counts

#### Scenario: A cell reports its coordinates
- **WHEN** a specific data cell of the fixture `JTable` is located and inspected
- **THEN** `@native:TableCell.Row` and `@native:TableCell.Column` equal that cell's actual position, and `@native:TableCell.IsSelected` reflects whether the cell is currently selected

### Requirement: Value, text, and action interface attributes
For an element supporting `AccessibleValue`, the provider SHALL expose `native:Value.Current`, `native:Value.Minimum`, and `native:Value.Maximum`. For `AccessibleText`, it SHALL expose at least `native:Text.CharCount` and `native:Text.CaretIndex` (and selection bounds where a selection exists). For `AccessibleAction`, it SHALL expose the list of available action names as `native:Action.Names`. For `AccessibleHypertext`, it SHALL expose `native:Hypertext.LinkCount`. `AccessibleKeyBindings` and `AccessibleRelationSet` SHALL be surfaced where present (key-binding list, relation targets).

#### Scenario: Slider reports its value range
- **WHEN** the fixture's slider (an `AccessibleValue` element) is inspected
- **THEN** `@native:Value.Current`, `@native:Value.Minimum`, and `@native:Value.Maximum` reflect the slider's live state

#### Scenario: Text field reports caret and length
- **WHEN** the caret is placed in the fixture's text field and the node is inspected
- **THEN** `@native:Text.CharCount` equals the field's character count and `@native:Text.CaretIndex` equals the caret position, read live on the same runtime

### Requirement: Live reads and lazy resolution of expensive properties
Interface attributes SHALL be read live from the bridge per access (no sticky cache), consistent with the provider's live-read model. Enumerating a node's attributes (`attributes()`) SHALL emit only cheap container-level interface properties; expensive per-cell table info SHALL be resolved on demand via a targeted `attribute()` lookup, so that a full tree walk of a large table does not query every cell. All interface reads SHALL run on the provider's pump thread under the per-call deadline; a degraded/unresponsive `vmID` SHALL yield no interface attributes for the affected node rather than hang.

#### Scenario: Tree walk of a large table stays bounded
- **WHEN** the fixture window containing a multi-row `JTable` is walked and each node's attributes are enumerated
- **THEN** the walk completes within the normal per-call deadline budget and does not issue a per-cell `getAccessibleTableCellInfo` call for cells whose attributes were not explicitly requested

#### Scenario: Live value after a change
- **WHEN** the fixture slider's value is changed and its node's `@native:Value.Current` is read again on the same runtime
- **THEN** the attribute reflects the new value (no stale cached reading)

#### Scenario: Frozen JVM yields no interface attributes without hanging
- **WHEN** the fixture app's event-dispatch thread is suspended and a node's interface attributes are requested
- **THEN** the request returns within the configured deadline margin with those attributes absent (or an error surfaced for that node), and a concurrent UIA query completes normally
