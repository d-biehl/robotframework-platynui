## Context

`provider-java-swing` gave the agent a Swing tree reader and, with it, correct table cells: real names from the model, real rectangles from `getCellRect`, identity-stable ids from an interning table keyed by `(table, row, column)`. What it kept was the *shape* the Access Bridge had — twelve cells hanging directly off the fixture's 4×3 table, ordered row-major. That was never a decision; it is what `AccessibleContext.getAccessibleChild(i)` offers, and the bridge has nothing else to offer.

The agent does. It reads the toolkit's own model, where a row is a first-class thing with a rectangle, a selection state and a stable position. And the shape it would produce is the one the other providers already produce: `provider-atspi` maps `TableRow` to `item:TableRow`, and `provider-windows-uia` maps `UIA_DataItemControlTypeId` to `DataItem` — a grid row with its cells beneath. Java is the outlier.

One constraint from the existing implementation is load-bearing here and easy to miss. `SwingTree.childAt(owner, index)` exists so that anything answering in terms of *accessible child indices* can name the very objects the tree hands out — it is what lets `control:SelectedItems` publish ids that resolve. It works today because for a `JTable` the accessible child index and the position in `childrenOf` are the same row-major sequence. A row level breaks that identity: `AccessibleSelection` still reports selected **cells** by accessible index, while the table's children have become rows.

## Goals / Non-Goals

**Goals:**

- Rows between a table and its cells, with identity, geometry and selection of their own.
- Cells unchanged in everything they already answer, one level deeper.
- Hit-test chains that pass through the row.
- Selection that keeps naming nodes which exist.
- The first Robot coverage that drives the **agent** rather than the bridge.

**Non-Goals:**

- **Changing the JAB backend.** The bridge cannot produce rows — it has only the flat accessible list — and pretending otherwise by synthesising rows in the provider would invent structure the target never reported.
- **Trees.** Same theme, different mechanics and prerequisites: a `TreePath` identity rather than an interned coordinate pair, and a fixture that has no `JTree` at all. Its own proposal.
- **A core-vocabulary alignment.** UIA says `DataItem` where AT-SPI says `TableRow`; they already disagree, and settling that is not this change's business (see decision 1).
- **Column-major or transposed views.** Swing tables are row-major; nothing here generalises to a table that is not.

## Decisions

1. **The role is `TableRow` (decided).** It matches `provider-atspi`, it is self-describing, and the role vocabulary is open — unmapped roles fall back to PascalCase, so nothing in the core has to learn it. UIA's `DataItem` is a third spelling for the same idea; aligning the two is a separate conversation, and adopting UIA's name here would align with one provider by diverging from the other. Namespace `item:`, consistent with `TableCell` and with AT-SPI.

2. **The same table has different shapes depending on the backend — accept it, and say so (decided).** Through the agent a table is nested; through the Access Bridge it stays flat. This is the change's real cost and it should not be buried.

    It is defensible because the divergence is already there and is not gratuitous: the two backends differ in `@Technology`, in the RuntimeId scheme, in whether a cell's name is its model value or whatever the shared renderer last rendered, and in whether a cell has bounds at all. A consumer that cares already has to know which backend served a node. The agent is the default path and the bridge is the floor, so the better shape belongs on the path most runs take.

    What it costs is real: a locator written against one backend does not work on the other, and an environment where the agent is sometimes unavailable sees both. The honest mitigation is not to hide it but to make it observable — `@Technology` already says which backend answered — and to say plainly in the docs that a table's shape follows the backend.

    *Alternative considered:* holding the flat shape until the bridge can match it. Rejected: it never can, so that is a permanent freeze at the worse shape for the benefit of the weaker path.

3. **A row's identity, name and geometry come from the table, not from the accessible view (decided).** Identity: the existing interning table, keyed by `(table, row)` alongside the cells' `(table, row, column)` — same weak-owner discipline, so a row dies with its table and the same row is the same object across enumerations. Geometry: the union of the row's cell rectangles, which is what `getCellRect` composes to and what a user would point at. Name: **none by default.** A row is addressed by position or by what it contains, not by a label the toolkit never gave it; synthesising one by joining cell values would invent an identifier that changes when any cell does. Rows therefore rely on structural locators and on their cells — which is exactly how the other providers' rows behave.

4. **Selection is re-derived from the table, not from the accessible index (decided).** `AccessibleSelection` reports selected cells by accessible child index, and after this change that index no longer addresses a direct child. Rather than patch the index arithmetic, take the answer from where it is unambiguous: `JTable.getSelectedRows()`/`getSelectedColumns()` already travel in the payload's `table` block, and `isCellSelected(row, column)` is the per-cell truth already used for `native:TableCell.IsSelected`. `SelectedItems` on a table then names the selected **rows** — which is what row selection means and what a user asks for — while cell-level selection stays visible per cell. `SwingTree.childAt`'s alignment guarantee is narrowed accordingly: it keeps its contract of never inventing an id, and simply declines for tables, which is the behaviour it already has for any owner whose orders cannot be shown to agree.

5. **Geometry is clipped to the viewport; content with nothing on screen reports no bounds (decided during implementation).** This was not in the original design and it should have been. The fixture's table was 4×3 and fitted its viewport, so nothing exercised the normal case: a table larger than the space showing it, where `getCellRect` still answers from the model for every cell. Measured once the fixture grew to 100×6 with a scrolling viewport, row 90 published a rectangle at `y = 2808` — roughly two thousand pixels below the window — and claimed to be in view. Pointer input aims at a rectangle's centre, so that is not a cosmetic inaccuracy; it is an instruction to click a different application.

    A cell's and a row's rectangle is therefore intersected with the table's `getVisibleRect()`. Nothing visible means **no bounds** and `IsInView = false`, which is the same doctrine `SwingGeometry.hasArea` already applies to unlaid-out components one level up: absent is the honest answer. Partially scrolled content is **clipped rather than dropped**, so the rectangle's centre stays inside the part the user can actually see and a click lands where it looks like it should.

    The node itself is unaffected — it keeps its name, its coordinates and its identity, and it is still live. Only its claim to a place on screen goes away. *Alternative considered:* publish the full model rectangle alongside `IsInView = false` and let consumers filter. Rejected: every consumer would have to remember to, and the one that forgets misclicks.

6. **Hit-testing composes the chain rather than special-casing it (decided).** `chainAt` already appends a virtual child after the deepest component; with rows it appends the row and then the cell. Both come from the same interning table as the enumerated nodes, so the picked cell is the *same object* the tree hands out and reveal matching keeps working by construction — which is the property that makes an in-JVM hit-test worth having.

## Risks / Trade-offs

- [A locator written against the agent breaks on the bridge and vice versa] → inherent to decision 2 and mitigated by making it explicit, not by hiding it: `@Technology` distinguishes them, and the docs will state that shape follows backend.
- [`SelectedItems` changes meaning from cells to rows] → it currently names cells; after this it names rows. Both are defensible, and the change is only safe because nothing released depends on either. The cell-level answer does not disappear — it moves to where it belongs, on the cell.
- [One more level on every table walk] → a row node per row, which is cheap next to the cells it contains, and it *reduces* the per-frame cost of `ui/children` on a table's top level from all cells to just the rows. The known large-table frame-size limitation (see `provider-java-swing`'s design) gets better, not worse: descending is now opt-in per row.
- [~~The fixture's table is small (4×3), so the shape is proven but not stressed~~] → **resolved during implementation**: the fixture table is now 100×6 in a viewport it does not fit, with both scrollbars. That is what surfaced decision 5, and it is also the first measurement of the walk cost — ten full bridge-side walks over all 600 cells take 18 s, so the flat backend pays for the bigger fixture without the suite becoming impractical.

## Migration Plan

Additive to the tree, breaking to positional locators on agent-served tables — of which none exist outside this repository, since the agent backend has not been released. Inside the repository the affected assertions are the Rust live fixtures, which are updated with the change; the Swing acceptance suites run with the agent disabled and are untouched.

Rollback is the ordinary one: revert the agent's `childrenOf` for `JTable` and the row block in the payload; cells return to being direct children.

## Open Questions

- **Should a row expose `native:TableRow.Index`, or is structural position enough?** Leaning yes for the index — it is free, and it is what a user reads off the screen — but it duplicates something the tree already encodes.
- **Do column headers belong under a header row?** Today they are virtual children of the `JTableHeader` component, which is a sibling of the table inside the scroll pane. That is faithful to Swing's own structure and this change does not touch it; whether a consumer would rather find them as a header row of the table is a question worth asking once rows exist.
