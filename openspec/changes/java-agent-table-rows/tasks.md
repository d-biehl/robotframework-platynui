## 1. Coverage first

- [ ] 1.1 In `crates/provider-java/tests/live_fixture.rs`, rewrite the agent table assertions for the nested shape: the fixture's `main-table` has **4 children, each a row**, and each row has **3 cells** — replacing the current `cells.len() == 12` expectation (spec: *A table's children are its rows*).
- [ ] 1.2 Keep every existing cell assertion, one level deeper: `TableCell.Row`/`Column`/`Index`/`IsSelected`/`IsEditable`, the model-derived `@Name`, real bounds, and `control:Id` staying absent (spec: *A cell still knows where it is*).
- [ ] 1.3 Add row assertions: role `TableRow`, bounds spanning the row's cells, selection state true for the preselected row 2 and false otherwise, and an identity that is byte-identical across two enumerations of the unchanged table (spec: *A row is addressable and locatable*).
- [ ] 1.4 Extend the agent hit-test live test so a point inside a cell returns a chain that reaches the cell **via its row**, and the picked cell is the same node the enumeration produced (spec: *Hit-testing passes through the row*).
- [ ] 1.5 Assert `control:SelectedItems` on the table resolves: every published id must be findable in the tree, and the set must name the selected **rows** (spec: *Selected cells are still named correctly*).
- [ ] 1.6 Author a new agent-facing acceptance suite `tests/acceptance/swing/agent_table.robot` alongside the existing bridge suites — started **with** the agent (no `providers.java.agent.enabled = False` override), asserting `@Technology = "JavaAgent"`, the row-then-cell structure, and a row-scoped cell locator. Authored here, run in 6.3 (it needs the rebuild from 5.1).

## 2. Rows in the Swing tree

- [ ] 2.1 `SwingTree`: intern rows on a `(table, row)` key alongside the cells' `(table, row, column)` key, reusing the existing per-owner weak map so a row dies with its table (design 3).
- [ ] 2.2 `SwingTree.childrenOf`: a `JTable` yields one row per model row; a row `VirtualChild` yields its cells left to right. Cells stop being direct children of the table.
- [ ] 2.3 `SwingTree.accessibleChildCount` / `childAt`: narrow the alignment guarantee — for a `JTable` the accessible child index no longer addresses a direct child, so `childAt` declines rather than inventing one, which is the behaviour it already has for owners whose orders cannot be shown to agree (design 4).
- [ ] 2.4 `SwingTree.chainAt`: a point inside a table appends the interned **row** and then the interned **cell**, both from the same interning table as the enumerated nodes (design 5).

## 3. The row payload

- [ ] 3.1 `SwingElement`: describe a row virtual child — role `table row`, **no name** (design 3), `enabled`/`visible`/`showing` inherited from the table.
- [ ] 3.2 `SwingGeometry`: a row's rectangle is the union of its cells' `getCellRect`s, suppressed by `hasArea` when the row has no extent.
- [ ] 3.3 Publish `native:TableRow.Index` and `native:TableRow.IsSelected` on the row (resolves design open question 1 — yes: the index is free and is what a user reads off the screen).
- [ ] 3.4 `SwingElement.selectionOf`: for a `JTable`, re-derive the selection from `getSelectedRows()` instead of walking `AccessibleSelection` by accessible index, so the published ids name rows that exist (design 4). Cell-level selection stays on the cell via `isCellSelected`.
- [ ] 3.5 Extend the agent's JUnit tests for the new tree shape and row payload (`just test-java-agent`).

## 4. Provider mapping

- [ ] 4.1 `crates/provider-java/src/agent/element.rs`: add the row block to the wire payload and map role `table row` → `item:TableRow`.
- [ ] 4.2 Confirm `map_role`'s cell path still fires now that a cell's parent role is `table row` rather than `table`, and cover both parent roles in the existing role-mapping unit tests.
- [ ] 4.3 `crates/provider-java/src/agent/node.rs`: `push_table_row` for `native:TableRow.*`, mirroring `push_table_cell`'s shape.
- [ ] 4.4 Verify the node contract still holds on the new level — `control:SupportedPatterns` and the rest of `COMMON_ATTRIBUTES` present on a row — via the existing testkit check.

## 5. Delivery and docs

- [ ] 5.1 `just install-provider-java` to restage the JAR, then `just build-native` — the agent change is invisible to the Rust and Python consumers until both run.
- [ ] 5.2 `dev-docs/platform-windows.md`: state in the agent-backend section that a table's **shape follows the backend** — nested through the agent, flat through the bridge — and that `@Technology` is how a consumer tells them apart (design 2).
- [ ] 5.3 `java/agent/README.md`: note the row level in the Swing adapter's tree description.

## 6. Verification

- [ ] 6.1 `just check` and `just test`.
- [ ] 6.2 `just test-java-agent`, then the live fixture lane for the agent tests from group 1 (real JVM required).
- [ ] 6.3 Run the acceptance lane on Windows with an **unlocked** session: the new `agent_table.robot` plus the existing Swing suites, which must stay green unchanged because they run the bridge.
