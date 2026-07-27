## 1. Fixture

- [ ] 1.1 Add `apps/test-app-swing/src/platynui/testapp/TreePanel.java` modelled on `TablePanel`: fixed accessible names (`tree-panel`, `tree-scroll`, `main-tree`), a deterministic shape with an expanded branch and **one branch deliberately left collapsed**, and one preselected node — both of which tests must not change (design 7).
- [ ] 1.2 Wire `TreePanel` into `Main.createAndShow` and document the panel's names, shape and fixed expansion/selection state in `apps/test-app-swing/README.md`.
- [ ] 1.3 Verify the existing Swing acceptance suites still pass with the larger window — they assert names and structure, not frame dimensions, but the fixture grew and that is worth confirming rather than assuming (design risk 4).

## 2. Coverage first

- [ ] 2.1 In `crates/provider-java/tests/live_fixture.rs`, assert the nested shape: the tree's node has children, and those children have children — replacing today's measured one-child stub (spec: *A tree reports its nested structure*).
- [ ] 2.2 Assert a collapsed node reports `Expandable.CanExpand = true`, `IsExpanded = false` and **no children**, and that no node anywhere in the tree lacks an on-screen rectangle (spec: *A collapsed branch hides its contents but says it has some*).
- [ ] 2.3 Assert that expanding the collapsed branch and re-enumerating reveals its children with names, bounds and their own expansion state (spec: *Expanding a branch reveals its nodes*).
- [ ] 2.4 Assert identity survives structure changes: take a node's id, expand or collapse a branch above it so its screen position shifts, and confirm the id still resolves to the same node (spec: *A node keeps its identity across structural changes*).
- [ ] 2.5 Assert the selection: the preselected node reports `Selectable.IsSelected`, and the tree's `control:SelectedItems` publishes an id that resolves to it (spec: *Selection reflects the model*).
- [ ] 2.6 Assert a hover-based hit-test over a tree row returns a chain reaching that node through its ancestors, and that the node is the same one the enumeration produced (spec: *Hit-testing reaches the node*).
- [ ] 2.7 Assert every tree node's role is `TreeItem`, not `Label` (spec: *Tree nodes are recognizable as tree items*).
- [ ] 2.8 Author an agent-facing Robot suite `tests/acceptance/swing/agent_tree.robot` — agent enabled, `@Technology = "JavaAgent"` — covering locate-a-node, expand, then locate-what-was-hidden. Authored here, run in 6.3 after the rebuild.

## 3. The tree reader

- [ ] 3.1 `SwingTree`: widen the per-owner interning key from `Long` to `Object` so a `TreePath` can be a key alongside the packed cell coordinates, keeping the weak-owner discipline (design 3).
- [ ] 3.2 `SwingTree.VirtualChild`: add the path-carrying flavour beside the cell and indexed ones, with an intern helper for `(tree, TreePath)`.
- [ ] 3.3 `SwingTree.childrenOf`: a `JTree` yields the root path (or the root's children when `isRootVisible()` is false, design open question 2); a tree-node virtual child yields its children **only when the node is expanded**, so the blanket "virtual children are leaves" rule no longer swallows the hierarchy (design 2).
- [ ] 3.4 `SwingTree.childCountOf`: the matching count without building the children.
- [ ] 3.5 `SwingTree.chainAt`: a point over a tree resolves via `getPathForLocation`, and the chain is that path's ancestors, each interned (design 6).
- [ ] 3.6 `SwingTree.childAt`: for a `JTree` the accessible child index does not address what `childrenOf` produces, so decline rather than guess — the same narrowing the table-row work applies to `JTable`.

## 4. The node payload

- [ ] 4.1 `SwingElement`: describe a tree-node virtual child — a node kind the provider can map without inference (design 4), name from the model's rendered value, `enabled`/`visible`/`showing` from the tree.
- [ ] 4.2 `SwingGeometry`: a node's rectangle from `JTree.getPathBounds`, through the existing `hasArea` guard.
- [ ] 4.3 Publish expansion state from `JTree.isExpanded` and the model's leaf test, so `control:Expandable.IsExpanded`/`CanExpand` are model-derived rather than renderer-derived.
- [ ] 4.4 Publish per-node selection from `isPathSelected`, and re-derive the tree's selection from `getSelectionPaths()` instead of the accessible selection view, which answers for the root only (design 5).
- [ ] 4.5 Extend the agent's JUnit tests for the tree reader and the node payload.

## 5. Provider mapping

- [ ] 5.1 `crates/provider-java/src/agent/element.rs`: carry the tree-node kind in the payload and map it to `item:TreeItem`.
- [ ] 5.2 Retire the inference path for tree items (parent role `tree` plus `selectable`), which measurement shows never fires, and update the role-mapping unit tests to cover the new kind.
- [ ] 5.3 Confirm the node contract holds on tree nodes — `control:SupportedPatterns` and the rest of `COMMON_ATTRIBUTES` — via the existing testkit check.

## 6. Docs, delivery and verification

- [ ] 6.1 `dev-docs/platform-windows.md`: the agent's tree handling and the fact that a tree's shape follows the backend — realized nodes only through the agent, the accessible view's shape including undescribable collapsed children through the bridge.
- [ ] 6.2 `just install-provider-java`, then `just build-native`.
- [ ] 6.3 `just check`, `just test`, `just test-java-agent`, the live fixture lane, and the acceptance lane on Windows with an unlocked session — `agent_tree.robot` plus the full existing Swing suite set.
