## Context

`add-jab-provider` (archived) exposes Swing/AWT trees on Windows with normalized roles, standard attributes, and interaction patterns. It records *which* JAB interfaces an element supports as a name-list attribute `native:Interfaces` (from `AccessibleContextInfo.accessibleInterfaces`, mapped by `ffi::interface_names` in [crates/provider-jab/src/node.rs](../../../crates/provider-jab/src/node.rs)), but never reads the *data* behind those interfaces.

The UIA provider does the opposite already: [`collect_native_properties`](../../../crates/provider-windows-uia/src/map.rs) queries a catalog of properties grouped by pattern, checks each pattern's `Is<Pattern>AvailablePropertyId` (one cheap boolean COM call), and reads a pattern's properties only when it is available — emitting them as `native:<ProgrammaticName>` attributes. A companion `get_native_property_by_name` does a single lazy lookup by name for `attribute()` calls. This change gives JAB the equivalent, keyed on the interface bitfield instead of UIA availability properties.

JAB's relevant getters (`AccessBridgeCalls.h`): `getAccessibleTableInfo` / `getAccessibleTableCellInfo` / `getAccessibleTableRowSelectionCount` / `getAccessibleTableColumnSelectionCount` / `getAccessibleTableRowSelections` / `getAccessibleTableColumnSelections` (`AccessibleTable`), `getCurrentAccessibleValueFromContext` / `getMaximumAccessibleValueFromContext` / `getMinimumAccessibleValueFromContext` (`AccessibleValue`), `getAccessibleTextInfo` / `getAccessibleTextSelectionInfo` (`AccessibleText`), `getAccessibleActions` (`AccessibleAction`), `getAccessibleHypertextExt` (`AccessibleHypertext`), `getAccessibleKeyBindings` (`AccessibleKeyBindings`), `getAccessibleRelationSet` (`AccessibleRelationSet`).

## Goals / Non-Goals

**Goals:**

- Every supported JAB interface's data is visible as `native:<Interface>.<Prop>` attributes, so Inspector and XPath selectors see the same richness UIA gives.
- Zero cost for unsupported interfaces (bitfield gate) and bounded cost for supported-but-expensive ones (lazy per-cell resolution).
- Live reads, pump-thread + per-call-deadline discipline, no new hang surface — identical to the base provider.

**Non-Goals:**

- **No new `Table`/`Grid` core pattern.** This is raw attribute projection only; it does not introduce a cross-provider tabular abstraction. (That would be a separate, larger change.)
- No duplication of existing patterns: where a pattern already models an interface (StatefulValue over `AccessibleValue`, TextContent over `AccessibleText`), the pattern stays; `native:*` exposure is orthogonal, exactly as UIA emits both a pattern and `native:*` properties.
- Events/push (`event_capabilities` stays `None`).
- Full text-attribute-run extraction, hyperlink target resolution, and relation graph traversal beyond the summary properties named in the spec.

## Decisions

1. **Bitfield-gated projection, mirroring UIA's availability gate.** `node.rs` iterates the interfaces present in `info.interfaces` and, for each, appends that interface's attributes. This replaces UIA's per-pattern `Is…AvailablePropertyId` boolean probe with a free in-memory bitfield check (the info is already fetched for the node), so the gate is cheaper than UIA's. Unsupported interface ⇒ no attribute, no bridge call. *Alternative considered:* probe every getter and swallow failures — rejected (needless bridge round-trips, and JAB getters on unsupported interfaces have undefined behavior).

1a. *Discovered during implementation — JTable cells alias the shared renderer through the bridge.* The JDK-side AccessBridge special-cases table children: `getAccessibleChildFromContext(table, i)` (and the `accessibleContext` embedded in `getAccessibleTableCellInfo`) returns the accessible context of the *one shared cell-renderer component*, configured for the requested cell at call time — not a per-cell `AccessibleJTableCell`. Consequently every cell node aliases every other cell of its table: a cell's `Name`/bounds read whatever cell the renderer was configured for last, and `isSameObject` answers TRUE for any two cells. Three responses: (a) `TableCell.*` resolution is keyed off the **tree** parent's context (captured in `JabNode` at construction) plus the cell's enumeration index — never the cell's own bridge context, whose bridge parent is the `CellRendererPane`, not the table; (b) the coordinate-based `getAccessibleTableCellInfo` fields (row/column/extents/selection) are computed from the `AccessibleTable` interface and are the stable cell identity; (c) fixture tests address cells by row-major child position, never by `@Name`.

2. **Two-tier eagerness: cheap eager, expensive lazy.** `attributes()` (full enumeration during a tree walk) emits only **container/element-level** properties — a bounded, constant number of calls per node (`Table.RowCount`, `Value.Current`, `Text.CharCount`, `Action.Names`, …). **Per-cell** `TableCell.*` (which would be O(rows×cols) if done eagerly) is emitted **only** through the targeted `attribute(Namespace::Native, "TableCell.Row")` path — the same split UIA uses between `collect_native_properties` (enumeration) and `get_native_property_by_name` (single lookup). This keeps a full walk of a large `JTable` bounded. *Alternative considered:* emit all cell attributes eagerly — rejected (a 1000-row table would issue thousands of blocking IPC calls per walk).

2a. *Amended during implementation — per-cell attributes are additionally **listed** on table children.* Lookup-only `TableCell.*` was invisible to attribute *consumers that enumerate* (the Inspector's attribute panel iterates `attributes()`), so cells showed none of their per-cell facts there. The expensive part was never the listing but the value reads, so the split is refined: `attributes()` on a child of a table also lists the `TableCell.*` entries — gated by the parent role captured at node construction (a free in-memory check, no bridge call) — while every value keeps resolving lazily at read time. A walk that does not read per-cell values still issues zero per-cell calls; a consumer that reads them has explicitly asked for exactly those cells.

3. **Naming: `<Interface>.<Property>` dotted, PascalCase.** Matches UIA's dotted programmatic names (`Grid.RowCount` ⇢ `Table.RowCount`) so selectors feel consistent across providers. A single Rust catalog (interface → list of `(property-name, reader)`) is the source of truth and is unit-tested for well-formedness (known prefix, no collisions) in the mock lane — the one part of this change verifiable without a JVM.

4. **Live reads on the pump thread under the deadline.** Each attribute's value is produced by a reader closure that runs the bridge getter on the pump thread with `call_timeout_ms`, exactly like the existing attributes. No sticky cache; a degraded `vmID` yields no interface attributes for that node (consistent with `invalidate()` clearing only calibration). This inherits the frozen-JVM robustness already proven for the base provider.

5. **FFI surface.** Add the getters and their out-param structs (`AccessibleTableInfo`, `AccessibleTableCellInfo`, `AccessibleTextInfo`, `AccessibleActions`, `AccessibleKeyBindings`, `AccessibleRelationSetInfo`) to `ffi.rs` with layout asserts, bind them in `dll.rs`, and wrap them as typed `JabClient` methods returning owned Rust values (handles released via the existing `JabObject` RAII where the getter returns a `JOBJECT64`, e.g. header tables). Structs that embed child-context handles (table headers, relation targets) must release those handles after extracting the summary data this change needs.

## Risks / Trade-offs

- [A large table walked with attribute enumeration still costs one `getAccessibleTableInfo` per table node] → bounded (one call per table, not per cell); acceptable and matches UIA's per-node pattern-property cost.
- [Per-cell `attribute()` lookups have no natural cell identity, so mapping "which cell is this node?" requires the node's enumeration path] → resolve the cell coordinate from the node's own context via `getAccessibleTableCellInfo` on the *parent table's* context using the child's index, reusing the existing enumeration-index model; documented fallback to omitting `TableCell.*` when the parent is not a table.
- [Some JAB getters return handle-bearing structs (header tables, relation targets) that leak if not released] → each typed client method releases embedded `JOBJECT64`s after extraction; covered by the existing handle-hygiene walk-repeat guard extended over a table.
- [Property catalog drift vs. real JAB field availability across JDK versions] → the catalog is data-driven and the mock-lane test only checks well-formedness; the real-provider scenario pins the concrete values against the fixture, and missing optional fields are simply omitted rather than erroring.

## Migration Plan

Additive and behind the existing provider. New FFI + client methods, plus the projection in `node.rs`; no config surface change (the interface bitfield already exists). A `JTable` stage is added to the Swing fixture as the acceptance carrier. Requires `just build-native` for Python/Robot/Inspector to see the new attributes. Rollback: `providers.jab.enabled=false` removes JAB entirely; there is no separate kill switch because the attributes are inert on elements that don't support the interface.

## Open Questions

- Whether `Text.*` should eventually include attribute-run and word/line boundary data — deferred; only caret/char-count/selection here.
- Whether relation targets should be exposed as RuntimeId references (enabling cross-links in the Inspector) rather than a count/summary — deferred to a follow-up if the Inspector grows relation navigation.
- Final concrete property list per interface (names in the catalog) — pinned during implementation against the fixture and the mock-lane catalog test.
