## Why

A `JTree` served by the in-JVM agent is **one node deep**. Measured against a live JDK 21 tree (six visible rows, one collapsed branch): the agent reports the tree with exactly **one child** — a `Label` named after the root — and nothing beneath it. Everything below the first level is gone.

The cause is two innocuous rules meeting. `SwingTree.childrenOf` falls through to the accessible view for a `JTree`, because its only child component is an invisible `CellRendererPane`; the accessible view then hands back one `AccessibleJTreeNode` for the root. And `childrenOf` treats **every** virtual child as a leaf — a rule written for table cells, where it is exactly right, and fatal here.

The accessible view is not the answer either, and three measurements say so:

- **It announces children it cannot describe.** The collapsed `branch-b` reports one accessible child whose name is `null`, whose role is `unknown`, and whose bounds are `null` — a node that exists only as a claim.
- **Its selection is degenerate.** `AccessibleJTree.getAccessibleSelectionCount()` answers `0` while `getSelectionPath()` correctly reports `[root, branch-a, a-leaf-1]`; the JDK only ever checks whether the **root** is selected.
- **Its roles come from the renderer.** A tree node's accessible role is `label`, and its states carry no `selectable` — so the provider's `TreeItem` rule (parent role `tree` **and** selectable) never fires, and tree nodes map to `control:Label`.

The `TreeModel` has all of it: the hierarchy, the expansion state, the selection paths, and a stable identity per node. That is the same move `java-agent-table-rows` makes for tables, for the same reason — the agent reads the toolkit's own model, the bridge only ever saw the accessibility projection of it.

Roles need no negotiation this time: `provider-atspi`, `provider-windows-uia` and the Java role table all already spell it `Tree` / `TreeItem`.

**There is no `JTree` in the Swing fixture** (nor a `JList` or `JTabbedPane`), which is why this went unnoticed and why the change carries a fixture stage — the precedent being `5334e21`, where the JAB interface-attribute work added `TablePanel` to carry its own acceptance surface.

## What Changes

- **The agent reads the tree from `TreeModel`/`JTree`, not from the accessible view.** A tree reports its root (or the root's children when the root is hidden), and each node reports its own children — a real hierarchy instead of a one-level stub.
- **Only realized nodes are in the tree.** A collapsed node advertises that it can expand and reports no children until it is expanded (design 2) — so no node in the tree is one the user cannot see, click or hit-test.
- **Nodes are identified by their `TreePath`**, not by a row index, so an identity survives an expand or collapse somewhere above it (design 3).
- **Tree nodes map to `item:TreeItem`.** The agent says a node is a tree node rather than leaving the provider to infer it from a renderer-derived `label` role.
- **Expansion state is published from the model**: `control:Expandable.IsExpanded`/`CanExpand`, which the existing state mapping already carries. The action half stays where it belongs — `Expandable.expand()` is synthesised by the Python proxy from input, per [`src/PlatynUI/core/patterns/expandable.py`](../../../src/PlatynUI/core/patterns/expandable.py).
- **Selection is re-derived** from `JTree.getSelectionPaths()`, since the accessible selection view reports only the root.
- **Hit-testing reaches the node**: a point over a row resolves through the node's ancestors down to the node itself.
- **A `TreePanel` joins the Swing fixture** with fixed accessible names, one deliberately collapsed branch and one preselected node.

## Capabilities

### New Capabilities

_None._ This changes what an existing capability reports.

### Modified Capabilities

- `java-provider`: the agent backend serves hierarchical tree content — nodes with children, expansion state, path-stable identity and model-derived selection — where it previously served a single level of accessibility wrappers.

## Impact

- **Fixture** (`apps/test-app-swing`): new `TreePanel`, wired into `Main`, documented in the app README. Existing accessible names are untouched, per the fixture's own rule.
- **Agent** (`java/agent`): `SwingTree` (tree children, `TreePath` interning, `chainAt`), `SwingElement` (the node payload, expansion state, selection), `SwingGeometry` (a node's row rectangle).
- **Provider** (`crates/provider-java/src/agent`): the node block in the wire payload and the `TreeItem` role mapping.
- **Tests**: `crates/provider-java/tests/live_fixture.rs`, plus agent-facing Robot coverage alongside what `java-agent-table-rows` introduces.
- **Docs**: `dev-docs/platform-windows.md` (agent-backend section), `dev-docs/java-toolkits.md` if the coverage table names trees.
- **Not affected**: the JAB backend — through the bridge a tree keeps the accessible view's shape, including the undescribable collapsed children, which is the floor this change deliberately does not try to raise.
- **Relation to `java-agent-table-rows`**: independent, same theme. Both widen `SwingTree`'s interning key and both re-derive a selection the accessible view gets wrong; if the tables change lands first, this one inherits those two mechanics rather than restating them.
- **Not breaking** in the ordinary sense: nothing could depend on the current shape, because the current shape is a truncated stub and the fixture never contained a tree.
- Needs `just install-provider-java` plus `just build-native` to be visible, as any agent change does.
