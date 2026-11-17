## 1. Tests first (model layer)

- [x] 1.1 Extend the test-only `MockNode` in `apps/inspector/src/model/tree_data.rs` with a constructor that sets namespace, role (`&'static str`) and name; keep `MockNode::new(id)` at `Control` / `"Mock"` / name = id so existing `tree_data` and `async_tasks` tests stay unchanged
- [x] 1.2 Add unit tests for the node label, one per spec scenario: `item:ListItem "Item 1"`, `Button "OK"`, `app:Application "MyApp"`, `native:Widget "Custom"`, empty name → `item:TableRow`, line feed in name → `item:ListItem "Line 1 Line 2"`
- [x] 1.3 Add a unit test that the result-list label (`SearchResultItem::from_evaluation_item` with `EvaluationItem::Node`) equals the tree label for the same `item` node
- [x] 1.4 Run `just test-crate platynui-inspector` and confirm the new tests fail for the expected reason (missing prefix)

## 2. Implementation

- [x] 2.1 Add `xpath_role_name(namespace: Namespace, role: &str) -> String` below `xpath_attribute_name`, with a doc comment stating that `control` is the default element namespace
- [x] 2.2 Use it in `format_node_label`
- [x] 2.3 Replace the inline node label in `from_evaluation_item` with a call to `format_node_label`
- [x] 2.4 Leave `namespace_cache`, the "Picked: …" status message and the attributes table unchanged

## 3. Docs

- [x] 3.1 Update `dev-docs/inspector.md`: the "UI Tree" and "XPath Search" feature bullets describe the qualified role label (`control` unprefixed); the `SearchResultItem` model note says it reuses the tree label; add a short tip: if an element is visible in the Inspector but a locator finds nothing, check the namespace prefix

## 4. Verification

- [ ] 4.1 `openspec validate inspector-namespace-prefixed-labels`
- [ ] 4.2 `just test-crate platynui-inspector` and `just check`
- [ ] 4.3 Manual check against a real provider: start the Inspector with an application that exposes list or table items, confirm prefixed labels in the tree and the result list, and that `//item:ListItem[@Name="…"]` copied from a label finds the node
