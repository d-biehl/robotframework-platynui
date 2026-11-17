## Context

All references are to `main` at `4165ce0` and were read in the source unless marked as assumed.

**Verified:**

- The XPath engine's default element namespace is `control` (`crates/runtime/src/xpath.rs:364`, `.with_default_element_namespace(CONTROL_NS_URI)`). An unprefixed element name test matches control nodes only.
- `Namespace` has `as_str()` (`control|item|app|native`) and `is_default()`, true only for `Control` (`crates/core/src/ui/namespace.rs:23-36`).
- The tree label comes from `format_node_label` (`apps/inspector/src/model/tree_data.rs:390-394`): `Role "Name"`, or just `Role` when the escaped name is empty. It is called only from `UiNodeData::preload_label` (`tree_data.rs:174-177`), which `preload_row_caches` (`tree_data.rs:265-269`) and the selection task (`apps/inspector/src/viewmodel/async_tasks.rs:315`) run in the background. The cached value reaches `VisibleRow.label` (`apps/inspector/src/viewmodel/tree_vm.rs:270`).
- `SearchResultItem::from_evaluation_item` builds the node label a second time, inline, with the same logic (`tree_data.rs:43-51`). It runs in the search task (`async_tasks.rs:351`).
- `xpath_attribute_name` (`tree_data.rs:403-412`) already drops the `control` prefix for attribute names and prefixes every other namespace. The new rule mirrors it for roles.
- The label is reused by the attributes panel header (`selected_label`, `apps/inspector/src/viewmodel/inspector_vm.rs:499-520`, rendered from `apps/inspector/src/lib.rs:1078`) and by the result list's `display_label()` / `full_copy_text()` (`tree_data.rs:66,99`).
- The "Picked: …" status message builds its own text from `node.name()`, falling back to the bare `node.role()` when the name is empty (`inspector_vm.rs:313-315`). It does not use the tree label.
- `UiNodeData.namespace_cache` (`tree_data.rs:129`) is filled by `preload_namespace` (`tree_data.rs:168-171`) and cleared by `refresh` (`tree_data.rs:370`), but nothing reads it.
- The test-only `MockNode` (`tree_data.rs:525-572`) has a fixed namespace (`Control`), role (`"Mock"`) and name (= id). `async_tasks` tests use it too (`async_tasks.rs:456-466`).
- No unit test and no acceptance suite under `tests/acceptance/egui/` checks the tree or result label text.
- `UiNode::namespace()` is resolved at most once per node and then cached by every real provider:
  - AT-SPI, JAB and the Java agent resolve namespace and role in the same cached step that `role()` already triggers (`crates/provider-atspi/src/node.rs:323-331`, `crates/provider-java-jab/src/node.rs:353-357`, `crates/provider-java/src/agent/node.rs:235-239`).
  - Windows UIA caches it in a `OnceLock` (`crates/provider-windows-uia/src/node.rs:356-365`). Nodes created from a UIA cache request get it prefilled together with the control type (`node.rs:227-238`); any other node pays one or two property reads on the first call.
  - Tree rows already call it in `preload_row_caches` before `preload_label`. A node from an XPath search has usually been read during evaluation, because an element name test such as `//Button` or `//item:ListItem` compares the namespace. So the label adds at most one cached call per node.

**Assumed:**

- None.

## Goals / Non-Goals

**Goals:**

- Tree rows and XPath node results show the XPath-qualified role, `control` unprefixed.
- One label function for both places.

**Non-Goals:**

- No change to which namespace a role belongs to (providers, `menu-item-namespace`).
- No change to the "Picked: …" status message, the attributes table, or settings (no toggle).
- No XPath generation or "Copy XPath" action.
- No cleanup of `namespace_cache`.

## Decisions

### D1: A role helper next to `xpath_attribute_name`

Add a private `fn xpath_role_name(namespace: Namespace, role: &str) -> String` in `tree_data.rs`, directly below `xpath_attribute_name`. It returns `role` when `namespace.is_default()` and `"{namespace}:{role}"` otherwise. It takes `Namespace` instead of `&str` because the caller has a `Namespace` from `UiNode::namespace()` and `is_default()` states the rule directly. It stays private because only `format_node_label` uses it; `xpath_attribute_name` is `pub` only because the attributes view imports it (`apps/inspector/src/view/attributes.rs:12`).

Alternative rejected: generalizing `xpath_attribute_name` to cover roles. The two rules happen to match today, but they have different reasons (attributes: unprefixed attributes are in no namespace; elements: `control` is the default element namespace). Keeping two named helpers keeps each one's doc comment accurate.

### D2: `format_node_label` uses the helper

`format_node_label` builds `xpath_role_name(node.namespace(), node.role())` and keeps its empty-name and escaping behavior unchanged.

### D3: The result list reuses `format_node_label`

The `EvaluationItem::Node` branch of `from_evaluation_item` calls `format_node_label(node.as_ref())` instead of building the label inline. This removes the duplicate and makes "same label as the tree" hold by construction.

### D4: `namespace_cache` stays untouched

Reading the namespace from `namespace_cache` inside `preload_label` would avoid one `namespace()` call per row. It would also tie `preload_label` to `preload_namespace` running first and would not help `from_evaluation_item`, which has no `UiNodeData`. KISS: call `node.namespace()` directly. Whether to remove or use the unused cache is left as an open question.

### D5: Testing posture

Model-layer unit tests in `tree_data.rs`, one per spec scenario. `MockNode` gains a constructor that sets namespace, role and name; `MockNode::new(id)` keeps `Control` / `"Mock"` / name = id, so existing callers in `tree_data.rs` and `async_tasks.rs` stay unchanged. Because `role()` returns `&'static str`, the role field is a `&'static str`. No acceptance test: the change is pure string formatting, which the unit tests cover completely. That the shown role works as an XPath name test is not a requirement of this capability: which nodes `//item:ListItem` matches is decided by the XPath engine and already required by the `menu-item-namespace` spec ("SHALL still be matched by `//item:ListItem`"). Task 4.3 checks the whole flow once by hand against a real provider.

## Risks / Trade-offs

- [Users or scripts that read the old unprefixed labels from the tree or clipboard] → Low risk: the labels are display text, not an API, and no suite depends on them. Mention the change in the commit body.
- [The "Picked: …" message still shows the bare role for unnamed nodes] → Accepted, out of scope. That message shows the name first and only falls back to the role.
- [Wider labels for `item` nodes] → A few extra characters; the tree already scrolls horizontally.

## Migration Plan

Behavioral, display-only change in the `platynui-inspector` binary. No native (PyO3) rebuild, no settings or file-format change. Rollback = revert the commit.

## Open Questions

- Should the unused `UiNodeData.namespace_cache` be removed or used (for example as a per-row namespace for future features)? Not part of this change.
