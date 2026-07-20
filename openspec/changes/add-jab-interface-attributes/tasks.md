## 1. FFI + client bindings for interface getters

- [ ] 1.1 `ffi.rs`: add out-param structs (`AccessibleTableInfo`, `AccessibleTableCellInfo`, `AccessibleTextInfo`, `AccessibleActions`, `AccessibleKeyBindings`, `AccessibleRelationSetInfo`) with `#[repr(C)]` layouts and size/offset asserts against `AccessBridgeCalls.h`
- [ ] 1.2 `ffi.rs`/`dll.rs`: bind the getters — `getAccessibleTableInfo`, `getAccessibleTableCellInfo`, `getAccessibleTableRow/ColumnSelectionCount`, `getCurrent/Maximum/MinimumAccessibleValueFromContext`, `getAccessibleTextInfo`, `getAccessibleTextSelectionInfo`, `getAccessibleActions`, `getAccessibleHypertextExt`, `getAccessibleKeyBindings`, `getAccessibleRelationSet` (loud `GetProcAddress` on missing symbols)
- [ ] 1.3 `client.rs`: typed `JabClient` methods wrapping each getter, running on the pump thread under `call_timeout_ms`, returning owned Rust values; release every embedded `JOBJECT64` (header tables, relation targets) after extraction via `JabObject` RAII

## 2. Property catalog + projection

- [ ] 2.1 New catalog (interface → `[(property-name, reader-closure)]`) as the single source of truth for names, using the `<Interface>.<Property>` PascalCase-dotted convention (`Table.RowCount`, `Value.Current`, `Text.CaretIndex`, `Action.Names`, `Hypertext.LinkCount`, …)
- [ ] 2.2 Mock-lane unit test: every catalog name matches `<KnownInterface>.<Property>` and there are no collisions (the only JVM-free check)
- [ ] 2.3 `node.rs` `attributes()`: for each interface present in `info.interfaces`, append its **container-level** `native:*` attributes with live reader closures (no per-cell reads here); bitfield-gated so unsupported interfaces emit nothing and issue no bridge call
- [ ] 2.4 `node.rs` `attribute(Namespace::Native, name)`: resolve **expensive per-cell** `TableCell.*` on demand — derive the cell's coordinate from the parent table context + the child's enumeration index; documented fallback to omitting `TableCell.*` when the parent is not a table
- [ ] 2.5 Confirm live-read semantics (no sticky cache) and that a degraded/unresponsive `vmID` yields no interface attributes rather than hanging

## 3. Swing fixture: JTable stage

- [ ] 3.1 Add a `JTable` stage to the Swing test app (fixed, known dimensions + a designated data cell and selection), wired into the existing stage-switching harness
- [ ] 3.2 `tests/acceptance/swing` locators/resource wiring for the table stage

## 4. Acceptance & verification

- [ ] 4.1 Windows real-provider scenario: assert `@native:Table.RowCount`/`@native:Table.ColumnCount` equal the fixture table's dimensions and a data cell's `@native:TableCell.Row`/`Column`/`IsSelected`
- [ ] 4.2 Windows real-provider scenario: assert `@native:Value.*` on the slider and `@native:Text.CharCount`/`CaretIndex` on the text field reflect live state (read back on the same runtime)
- [ ] 4.3 Robustness: a full walk of the table stage stays within the deadline budget and does not issue per-cell calls for unrequested cells; extend the handle-hygiene repeat-walk guard over the table; frozen-JVM lane shows interface attributes absent without hang
- [ ] 4.4 `dev-docs/platform-windows.md`: JAB interface-attribute projection note (parallel to UIA `collect_native_properties`); then `just check`, `just test`, `just build-native`, and the Windows acceptance run green
