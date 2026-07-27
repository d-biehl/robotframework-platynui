## Context

Everything below was measured against a live JDK 21 (Temurin 21.0.11, Windows) `JTree` with a visible root, one expanded branch, one collapsed branch and a selected leaf. It is worth recording, because most of it contradicts what the table work would lead one to assume.

- `JTree.getComponents()` is one **invisible** `CellRendererPane`. `SwingTree.visibleComponentChildren` filters it out, so a tree takes the accessible-view fallback.
- `AccessibleJTree.getAccessibleChildrenCount()` is `1` with a visible root, `3` (the root's children) with a hidden root. The accessible view of a tree **is** hierarchical — unlike `JTable`, which is why the flat-list intuition from tables does not transfer.
- What truncates it is ours: `childrenOf` returns `Collections.emptyList()` for **every** `VirtualChild`. Correct for a table cell, fatal for a tree node. **Net effect today: a tree has one child and no grandchildren.**
- A collapsed branch reports `children = 1`, and that child has `name = null`, `role = unknown`, `bounds = null`, `states = [collapsed]`. The JDK announces a node it cannot describe.
- `AccessibleJTree.getAccessibleSelectionCount()` returned `0` with `[root, branch-a, a-leaf-1]` selected — the JDK implementation only tests whether the **root** is selected.
- A node's accessible role is `label` (the default renderer is a `JLabel`) and its states contain no `selectable`. The provider's rule *parent role is `tree` and states are selectable → `item:TreeItem`* therefore never fires; nodes map to `control:Label`.
- States otherwise are good: `expandable,expanded` and `expandable,collapsed` come through correctly, and the provider already maps them to `control:Expandable.*`.
- `AccessibleJTreeNode` is freshly allocated per lookup — `==` and `equals()` both false across two calls for the same node. Same pattern as `JTableHeader` entries.

So the accessible view is not a flat projection to be re-nested; it is a hierarchy with holes. The model underneath has no holes.

## Goals / Non-Goals

**Goals:**

- A tree that is a tree: nodes with children, to whatever depth is on screen.
- Node identity that survives an expand or collapse above it.
- Expansion state and selection read from the model, not from the accessible view.
- Hit-test chains that reach the node.
- A `JTree` in the fixture, so any of this can be asserted at all.

**Non-Goals:**

- **The JAB backend.** Through the bridge a tree keeps the accessible view's shape, ghosts included. Raising that floor is a separate question and a much worse cost/benefit than it was for tables.
- **An expand/collapse action in the provider.** The Read/Action split already decided this: `IsExpandable` is native state, `Expandable` is synthesised by the Python proxy from input. Adding a programmatic expand would repeat the mistake corrected in `remove-programmatic-set-text`.
- **`JList` and `JTabbedPane`**, which the fixture also lacks. Same fixture gap, different mechanics; naming them here would turn this into "all item-bearing controls".
- **`TreeTable` / `JTree` inside a table column.** AT-SPI has a `TreeTable` role; Swing has no such component, and composing one is application-specific.
- **Lazily-loading models as a supported case.** Decision 2 makes them harmless rather than supported — see the risk note.

## Decisions

1. **Read the tree from `JTree`/`TreeModel`; keep the accessible wrapper only for what the model does not carry (decided).** Structure, names, expansion and selection come from the model and the view's path methods. The accessible wrapper stays available for the states the payload already publishes, but it is no longer the source of the tree's *shape*. This is the same split table cells already use: model for the truth, wrapper for the trimmings.

2. **Only realized nodes are in the tree; a collapsed node advertises `CanExpand` and reports no children (decided).** This is the central call, and three independent things point the same way.

    *The user's tree is what is on screen.* `SwingTree` already says so for components — "invisible children are the cards of a tab or a collapsed panel: real objects, but not part of the UI a user or a test is looking at" — and a collapsed subtree is exactly that case one level down.

    *An unrealized node cannot answer.* Measured: no name, no role, no bounds. It could not be clicked, hit-tested, or checked for visibility; a locator finding it would produce a node that fails at the first interaction. The JDK demonstrates the alternative, and the demonstration is not encouraging.

    *It is the only answer that terminates.* A `TreeModel` may be lazy and effectively unbounded — a filesystem browser is the canonical example. Enumerating the model means walking it; enumerating what is realized means walking what the UI has already committed to.

    The cost is that a test must expand before it can find, which is what a user does and what `TreeItem.expand()` in the Python surface already exists for. *Alternative considered:* the whole model, with collapsed nodes marked "offscreen". Rejected on all three counts above — it does not terminate, it produces nodes that cannot be used, and it makes "found it" mean less.

3. **Identity is the `TreePath`, interned (decided).** A row index shifts whenever anything above it expands or collapses, so it cannot be an identity. `TreePath` is the model's own identity for a node and compares by its components, which for the usual `DefaultMutableTreeNode` is reference identity — stable for as long as the node lives. Concretely, `SwingTree`'s per-owner interning map widens its key from `Long` to `Object`, so a path can be a key alongside the existing packed cell coordinates; the weak-owner discipline is unchanged, so nodes die with their tree. `VirtualChild` gains a path-carrying flavour beside the cell and the indexed one.

4. **The agent declares the node kind; the provider stops inferring it (decided).** The payload gains a tree-node kind, and the provider maps it to `item:TreeItem` directly. The existing inference — parent role `tree` plus a `selectable` state — is measurably wrong on both halves: nested nodes see a parent role of `label`, and no node reports `selectable`. Inferring a role from what a renderer happens to be is the same class of mistake as reading a cell's name off the shared renderer, and the fix is the same one: let the side that knows say so. The role name itself needs no decision — `TreeItem` is what all three providers already use.

5. **Selection re-derived from `getSelectionPaths()` (decided).** The accessible selection view answers for the root only, so there is nothing to patch. `control:SelectedItems` on the tree names the selected nodes, and each node reports its own `Selectable.IsSelected` from `isPathSelected`. Mechanically identical to the table-row decision, and for the same reason: the model is unambiguous where the accessible index is not.

6. **Geometry and hit-testing come from the view's path methods (decided).** A node's rectangle is `getPathBounds`, suppressed by the existing `hasArea` guard when it is null or empty — which by decision 2 should not happen, so a null there is a bug signal rather than a routine case. Hit-testing is better here than for tables: `getPathForLocation(x, y)` returns the **full path**, so the chain is that path's ancestors, each interned, with no per-level descent to get wrong.

7. **The fixture gets a `TreePanel` modelled on `TablePanel` (decided).** Fixed accessible names that never change, a deterministic shape, and — the part that carries this change's whole point — **one branch deliberately left collapsed** and one node preselected. Tests must not change either, exactly as `TablePanel`'s preselected row works today.

## Risks / Trade-offs

- [A test must expand before it can locate a node] → inherent to decision 2, and the intended behaviour: it is what a user does, and the Python `TreeItem.expand()` already exists for it. The alternative costs more.
- [`TreePath` identity is only as stable as the model's node objects] → a model that rebuilds its nodes on every change (a common `DefaultTreeModel` misuse) will produce new identities. Unavoidable — that model has genuinely replaced the node — and no worse than any other identity scheme could do.
- [Deep or wide realized trees make one `ui/children` frame large] → the same known frame-size concern as large tables, and mitigated the same way: children are fetched per node, so depth is paid for only where it is walked.
- [Adding a fixture panel changes the window's size and layout] → existing suites assert names and structure, not the frame's dimensions, and `TablePanel` set the precedent for growing the fixture without disturbing them. Still worth running the full Swing lane rather than assuming.
- [The bridge and the agent now disagree about trees too] → yes, and more sharply than for tables: the bridge shows ghost nodes the agent omits. The mitigation is the one already stated for tables — `@Technology` says which backend answered, and the docs say the shape follows the backend.

## Migration Plan

Additive throughout. Nothing can depend on the current behaviour: the shape it produces is a one-level stub, and the fixture has never contained a tree, so there are no assertions to migrate — only new ones to write. The JAB path is untouched and its suites keep passing unchanged.

Rollback is by reverting the agent's tree branch in `childrenOf` and the node block in the payload; the fixture panel can stay either way, since it is inert unless something looks at it.

## Open Questions

- **Should a node expose its depth or its path as a native attribute?** The path is the identity and a user might reasonably want to assert on it, but publishing it invites locators built from string-joined paths, which are brittle in exactly the way structural locators are not. Leaning no.
- **Should the root be reported when `isRootVisible()` is false?** The measurement says the accessible view already skips it and starts at the root's children, which matches what is on screen. Following that is almost certainly right; it is called out only because it is the one place where the accessible view and the model disagree in the *helpful* direction.
