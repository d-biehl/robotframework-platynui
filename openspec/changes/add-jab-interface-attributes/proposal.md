## Why

The JAB provider knows *which* accessibility interfaces an element supports but does not expose their *data*. Today a node carries only a name-list `native:Interfaces` (`["Table","Value","Text",…]`) plus the basic `AccessibleContextInfo` fields ([crates/provider-jab/src/node.rs](../../../crates/provider-jab/src/node.rs)); the actual properties behind those interfaces — a table's row/column counts and headers, a slider's value range, a text field's caret — are invisible. The UIA provider already surfaces every supported pattern's properties as `native:*` attributes ([crates/provider-windows-uia/src/map.rs](../../../crates/provider-windows-uia/src/map.rs) `collect_native_properties`), so Swing/AWT elements look impoverished by comparison in the Inspector and to XPath selectors.

## What Changes

- `provider-jab` projects the properties of each **supported** JAB interface onto the node as `native:<Interface>.<Prop>` attributes, data-driven from the element's `interfaces` bitfield (the same gate that already produces `native:Interfaces`), mirroring UIA's availability-gated `collect_native_properties`.
- Interfaces covered: **`AccessibleTable`** (container: row/column count, caption, summary, row/column header presence, selection counts + selected rows/columns) and **table-cell** info for cells (`getAccessibleTableCellInfo`: index, row, column, row/column extent, selected); **`AccessibleValue`** (current/min/max); **`AccessibleText`** (char count, caret index, selection range); **`AccessibleAction`** (available action names); **`AccessibleHypertext`** (link count); **`AccessibleKeyBindings`**; **`AccessibleRelationSet`**.
- Reads are **live per access** (no sticky cache — consistent with the `add-jab-provider` live-read model) and **lazy per `attribute()` lookup** where expensive: a large `JTable` must not query every cell during a tree walk. Enumeration (`attributes()`) emits cheap container-level properties; per-cell table info is resolved on demand.
- Naming follows UIA's dotted programmatic-name convention (`native:Table.RowCount`, `native:Value.Current`, `native:Text.CaretIndex`).
- New FFI bindings for the interface getters (`ffi.rs`/`dll.rs`/`client.rs`).
- The Swing fixture app gains a **`JTable` stage** as the acceptance carrier; a Windows real-provider scenario asserts the projected attributes.

Not breaking: this is purely additive attribute exposure; existing `native:*` and `control:*` attributes and all patterns are unchanged.

## Capabilities

### New Capabilities

- `jab-interface-attributes`: exposure of JAB accessibility-interface data (table, value, text, action, hypertext, key-bindings, relations) as `native:*` attributes on Java Swing/AWT nodes, gated by the element's supported-interface set, read live and resolved lazily for expensive per-cell data.

### Modified Capabilities

<!-- none — this adds a new capability; jab-provider's existing requirements (native:Interfaces name-list, base AccessibleContextInfo) are unchanged and remain valid. -->

## Impact

- **Modified crate**: `crates/provider-jab` — new FFI bindings (`ffi.rs`, `dll.rs`) and typed client methods (`client.rs`) for the interface getters; `node.rs` `attributes()`/`attribute()` gain the interface-property projection (eager for container-level, lazy for per-cell).
- **Test app**: `tests/acceptance/swing` fixture — a new `JTable` stage (and its locators/resource wiring).
- **Tests**: new Windows real-provider acceptance scenario (needs a JDK + the bridge; skips otherwise); a role/interface-mapping unit test for the property-name catalog is mock-lane verifiable.
- **Native rebuild**: `just build-native` for Python/Robot/Inspector consumers to see the new attributes.
- **Docs**: `dev-docs/platform-windows.md` (JAB interface-attribute projection, parallel to the UIA `collect_native_properties` note).
- **Depends on**: `add-jab-provider` (archived — base provider, node model, live-read discipline, interface bitfield). **Platform scope**: Windows only. No BREAKING changes; other providers unaffected.
