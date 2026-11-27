## Why

PlatynUI XPath puts element roles in namespaces. `control` is the default element namespace, so `//Button` matches control nodes, but roles in `item`, `app` or `native` need a prefix (`//item:ListItem`). The Inspector labels nodes with the bare role in both the UI tree and the XPath result list (`ListItem "Item 1"`). A locator written from what the user sees (`//ListItem[@Name="Item 1"]`) finds nothing, and nothing in the Inspector says why. The attributes panel already avoids this problem for attribute names by showing them XPath-qualified.

## What Changes

- Tree rows and node entries in the XPath result list show the role as it must be written in XPath: `ns:Role "Name"` for the `item`, `app` and `native` namespaces, unchanged `Role "Name"` for `control`.
- Nodes with an empty name show only the qualified role (`item:TableRow`), as today without quotes.
- The result list builds its node labels with the same function as the tree, so the two cannot drift apart.
- Everything that already reuses the tree label picks up the prefix too: the attributes panel header and the result list's copy actions.
- Behavior change (not BREAKING): users see different labels. There is no API or settings change.

## Capabilities

### New Capabilities
- `inspector-node-labels`: how the Inspector labels a node in the UI tree and in the XPath result list — the XPath-qualified role, the quoted, escaped name, and the same label in both places.

### Modified Capabilities

None.

## Impact

- **Layer:** Rust only, within `apps/inspector` (`src/model/tree_data.rs`). No native (PyO3) rebuild, no Python/Robot Framework surface change.
- **Platforms/providers:** all the same. The label reads `UiNode::namespace()`, which every provider implements, so the change does not depend on the platform or provider.
- **Tests:** new unit tests in `tree_data.rs`. The test-only `MockNode` gets a settable namespace, role and name; existing callers keep the current defaults. No existing unit or acceptance test depends on the label format.
- **Docs:** `dev-docs/inspector.md` (label format, a short troubleshooting tip).
