## Why

A Swing table served by the in-JVM agent is one flat, row-major list of cells: the fixture's 4×3 `JTable` has **twelve direct children** and no rows. That shape is not a PlatynUI model — it is what the Java accessibility view happens to offer, inherited from the Access Bridge era.

The other providers already have rows. `provider-atspi` maps `TableRow` to `item:TableRow` (and `TableRowHeader` alongside it); `provider-windows-uia` maps `UIA_DataItemControlTypeId` to `DataItem`, which in UIA *is* a grid row with its cells beneath it. So Java is the outlier, and adding rows brings it into line rather than inventing something. No core change is needed for that: the role vocabulary is open — an unmapped role falls back to PascalCase and becomes a usable XPath name — so this is a provider deciding what it reports, which is the provider's business.

The agent can do it because it reads the toolkit's own model, where a row is a first-class thing. The Access Bridge never could: it only ever offered the accessible view's flat child list, which is exactly why the flat shape exists.

## What Changes

- **The agent reports a table as `Table` → `TableRow` → `TableCell`** instead of `Table` → cells. Cells keep everything they have today, including `native:TableCell.Row`/`Column` — the coordinates stay true, they simply stop being the *only* way to know where a cell sits.
- **Rows become addressable in their own right**: a row node with its own identity, bounds and selection state, so `"the third row"` and `"the row whose first cell says X"` become locators rather than arithmetic over a flat list.
- **Hit-testing gains the level**: a pick inside a table returns `… → Table → TableRow → TableCell`.
- **Selection has to be re-derived.** `AccessibleSelection` on a `JTable` reports selected *cells*, and the agent's `control:SelectedItems` currently names them by mapping an accessible child index onto the direct child at that index. A row level breaks that correspondence, so the mapping needs a deliberate answer rather than an accidental one (design 4).
- **New agent-facing acceptance coverage.** The existing Swing suites do **not** break: since `provider-java-swing` they run with the agent disabled and exercise the Access Bridge, whose flat shape is unchanged. What is missing is coverage of the agent's own tree, which today exists only as Rust live fixtures.

## Capabilities

### New Capabilities

_None._ This changes what an existing capability reports, not what capabilities exist.

### Modified Capabilities

- `java-provider`: the agent backend's tree gains a row level between a table and its cells, with rows carrying identity, geometry and selection; the requirement that cells resolve with correct name, bounds and selection is unchanged and continues to hold one level deeper.

## Impact

- **Agent** (`java/agent`): `SwingTree` (children of a `JTable`, the interning key for a row, `childAt`), `SwingElement` (the row payload), `SwingGeometry` (a row's rectangle), and the hit-test chain.
- **Provider** (`crates/provider-java/src/agent`): the wire payload's row block, role mapping, and the `SelectedItems` derivation.
- **Tests**: `crates/provider-java/tests/live_fixture.rs` (the agent's tree assertions move a level deeper), plus the first Robot suite that drives the **agent** rather than the bridge.
- **Docs**: `dev-docs/platform-windows.md`'s agent-backend section.
- **Not affected**: the JAB backend, the existing Swing acceptance suites (agent disabled), `native:Table.*` on the table, and `native:TableCell.*` on the cell.
- **Inherited later**: `provider-java-javafx` and `provider-java-swt` reuse this mapping layer, so their adapters get the same shape without restating it.
- No native rebuild semantics change; `just install-provider-java` is needed to see it, as for any agent change. Windows-verified, but the agent-side work is platform-neutral by construction.
- **BREAKING for anyone addressing agent-served cells positionally** as direct children of a table. There is no released version where that was possible — the agent backend shipped in the same development cycle — so this is a shape decision taken before anyone can depend on it, which is precisely why it should be taken now rather than later.
