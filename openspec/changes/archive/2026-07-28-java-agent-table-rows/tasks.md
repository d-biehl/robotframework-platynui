## 1. Coverage first

- [x] 1.1 In `crates/provider-java/tests/live_fixture.rs`, assert the nested shape in the **agent** test: the fixture's `main-table` has **4 children, each a row**, and each row has **3 cells**. The bridge test's `cells.len() == 12` stays exactly as it is — the flat shape is what JAB reports and must keep reporting (spec: *A table's children are its rows*).
- [x] 1.2 Keep every existing cell assertion, one level deeper: `TableCell.Row`/`Column`/`Index`/`IsSelected`/`IsEditable`, the model-derived `@Name`, real bounds, and `control:Id` staying absent (spec: *A cell still knows where it is*).
- [x] 1.3 Add row assertions: role `TableRow`, bounds spanning the row's cells, selection state true for the preselected row 2 and false otherwise, and an identity that is byte-identical across two enumerations of the unchanged table (spec: *A row is addressable and locatable*).
- [x] 1.4 Extend the agent hit-test live test so a point inside a cell returns a chain that reaches the cell **via its row**, and the picked cell is the same node the enumeration produced (spec: *Hit-testing passes through the row*).
- [x] 1.5 Assert `control:SelectedItems` on the table resolves: every published id must be findable in the tree, and the set must name the selected **rows** (spec: *Selected cells are still named correctly*).
- [x] 1.6 Author a new agent-facing acceptance suite `tests/acceptance/swing/agent_table.robot` alongside the existing bridge suites — started **with** the agent (no `providers.java.agent.enabled = False` override), asserting `@Technology = "JavaAgent"`, the row-then-cell structure, and a row-scoped cell locator. Authored here, run in 6.3 (it needs the rebuild from 5.1).

## 2. Rows in the Swing tree

- [x] 2.1 `SwingTree`: intern rows on a `(table, row)` key alongside the cells' `(table, row, column)` key, reusing the existing per-owner weak map so a row dies with its table (design 3).
- [x] 2.2 `SwingTree.childrenOf`: a `JTable` yields one row per model row; a row `VirtualChild` yields its cells left to right. Cells stop being direct children of the table.
- [x] 2.3 `SwingTree.accessibleChildCount` / `childAt`: narrow the alignment guarantee — for a `JTable` the accessible child index no longer addresses a direct child, so `childAt` declines rather than inventing one, which is the behaviour it already has for owners whose orders cannot be shown to agree (design 4).
- [x] 2.4 `SwingTree.chainAt`: a point inside a table appends the interned **row** and then the interned **cell**, both from the same interning table as the enumerated nodes (design 5).

## 3. The row payload

- [x] 3.1 `SwingElement`: describe a row virtual child — role `table row`, **no name** (design 3), `enabled`/`visible`/`showing` inherited from the table.
- [x] 3.2 A row's rectangle is the union of its cells' `getCellRect`s — `SwingTree.rowRect`, next to the interning it belongs with, since it is table structure rather than coordinate conversion; the conversion and the `hasArea` suppression stay in `SwingGeometry.boundsWithin` as for every other element.
- [x] 3.3 Publish `native:TableRow.Index` and `native:TableRow.IsSelected` on the row (resolves design open question 1 — yes: the index is free and is what a user reads off the screen).
- [x] 3.4 `SwingElement.selectionOf`: for a `JTable`, re-derive the selection from `getSelectedRows()` instead of walking `AccessibleSelection` by accessible index, so the published ids name rows that exist (design 4). Cell-level selection stays on the cell via `isCellSelected`.
- [x] 3.5 Extend the agent's JUnit tests for the new tree shape and row payload (`just test-java-agent`).

## 4. Provider mapping

- [x] 4.1 `crates/provider-java/src/agent/element.rs`: add the row block to the wire payload and map role `table row` → `item:TableRow`.
- [x] 4.2 Confirm `map_role`'s cell path still fires now that a cell's parent role is `table row` rather than `table`, and cover both parent roles in the existing role-mapping unit tests.
- [x] 4.3 `crates/provider-java/src/agent/node.rs`: `push_table_row` for `native:TableRow.*`, mirroring `push_table_cell`'s shape.
- [x] 4.4 Verify the node contract still holds on the new level — `control:SupportedPatterns` and the rest of `COMMON_ATTRIBUTES` present on a row — via the existing testkit check.

## 4b. A realistic fixture table, and what it exposed

- [x] 4b.1 Grow `apps/test-app-swing`'s `TablePanel` to **100×6** in a viewport it does not fit (auto-resize off, both scrollbars), keeping the names, the `r<row>c<column>` content scheme and the preselected row 2 — so every existing locator still resolves and only the counts change.
- [x] 4b.2 Update the bridge-facing assertions for the new size: `native:Table.RowCount`/`ColumnCount`, the row-major cell positions (`row*6 + col + 1`), and the live fixture's flat 600-cell expectation.
- [x] 4b.3 **Clip cell and row geometry to the viewport** (`SwingTree.visiblePart`): off-view content reports no bounds and `IsInView = false`, partially scrolled content is clipped so its centre stays clickable (design 5). Without this the fixture measured a row at `y = 2808` — an instruction to click another application.
- [x] 4b.4 Cover it: a headless JUnit test for `visiblePart`, live-fixture assertions that row 90 and its cells are in the tree with names and coordinates but have no rectangle, and the matching acceptance scenario.
- [x] 4b.5 Measure what the bigger fixture costs the bridge, which walks all 600 cells: ten full walks in `hygiene.robot` take **18 s**, so the flat backend pays for it without the suite becoming impractical.
- [x] 4b.6 Fix the latent flakiness the volume exposed: `hygiene.robot`'s walk signature compared **cell names**, which the bridge's renderer aliasing makes volatile — the suite's own documentation already said so, and with 12 cells it never fired. The count still covers every cell (that is the handle-churn signal); the names now exclude the table's children. Confirmed stable over three consecutive runs.

## 5. Delivery and docs

- [x] 5.1 `just install-provider-java` to restage the JAR, then `just build-native` — the agent change is invisible to the Rust and Python consumers until both run.
- [x] 5.2 `dev-docs/platform-windows.md`: state in the agent-backend section that a table's **shape follows the backend** — nested through the agent, flat through the bridge — and that `@Technology` is how a consumer tells them apart (design 2).
- [x] 5.3 `java/agent/README.md`: note the row level in the Swing adapter's tree description.

## 6. Verification

- [x] 6.1 `just check` and `just test`.
- [x] 6.2 `just test-java-agent`, then the live fixture lane for the agent tests from group 1 (real JVM required).
- [x] 6.3 Run the acceptance lane on Windows with an **unlocked** session: the new `agent_table.robot` plus the existing Swing suites, which must stay green unchanged because they run the bridge. **Result: 34/34, the 8 new agent tests included**, against the provisioned Java 8 — so the row shape and the in-place attach both hold on the oldest JVM the agent targets.
