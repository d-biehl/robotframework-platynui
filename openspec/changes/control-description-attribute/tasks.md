## 1. Core attribute definition

- [x] 1.1 Add `crates/core/tests/test_description_attribute.rs` asserting `attribute_names::common::DESCRIPTION == "Description"` (template: `test_id_attribute.rs`)
- [x] 1.2 Add `pub const DESCRIPTION: &str = "Description";` to `pattern::common` in `crates/core/src/ui/attributes.rs`
- [x] 1.3 Add `fn description(&self) -> Option<String>` with `None` default to `UiNode` in `crates/core/src/ui/node.rs` (parallel to `id()`)

## 2. Mock provider (fast lane)

- [x] 2.1 Add mock tests for the spec scenarios "Mock node with description" and "Mock node without description" (attribute present only when the spec sets it; `description()` accessor returns it) in `crates/provider-mock/src/tests.rs`
- [x] 2.2 Implement spec-driven description in the mock provider: tree/`NodeSpec` support, emit `common::DESCRIPTION` when non-empty, override `description()` (`crates/provider-mock/src/node.rs`, `tree.rs`)

## 3. AT-SPI provider

- [x] 3.1 Add `StdAttrKind::Description`, its `AttrsIter::next` slot (gated on non-empty), name/value mapping, and `resolve_description()` calling `proxy.description()` in `crates/provider-atspi/src/node.rs`; implement `UiNode::description()` there
- [x] 3.2 Verify empty-description gating: elements with empty `Accessible.Description` expose no `control:Description` while `native:Accessible.Description`/`HelpText` behavior stays unchanged (covered end-to-end by task 6)

## 4. Windows UIA provider

- [x] 4.1 Add `map::get_description` reading `UIA_FullDescriptionPropertyId` via `read_uia_property` in `crates/provider-windows-uia/src/map.rs` (confirm the constant exists in the `windows` crate bindings)
- [x] 4.2 Add `DescriptionAttr` (modeled on `NameAttr`), a gated `AttrsIter` slot (like `IdAttr`), the `"Description"` arm in the `attribute()` fast-path (`node.rs:285`), and the `UiNode::description()` override in `crates/provider-windows-uia/src/node.rs`
- [x] 4.3 Compile-check the Windows provider (cross-check lane / `cargo check` for the windows target); note in the change that runtime behavior needs manual Inspector verification on Windows (no automated UIA lane)

## 5. Python bindings and API (native rebuild before Python tests)

- [x] 5.1 Add pytest cases first: mock tree with a description → `//control:Button[@Description='…']` matches, `attribute('Description')` returns it, and the node-level `description` getter returns it / `None` when absent (`packages/native/tests/test_mock_tree_content.py`)
- [x] 5.2 Expose `description` getter in `packages/native/src/runtime.rs` (next to `name`/`id`)
- [x] 5.3 Add read-only `description` properties to `Adapter` (`src/PlatynUI/core/adapter.py`) and `Context` (`src/PlatynUI/core/context.py`) mirroring `name`, with tests in `tests/PlatynUI/`
- [x] 5.4 Run `just test-python` (rebuilds native with mock-provider) and make the new tests pass

## 6. egui test app and acceptance suite

- [x] 6.1 Write the RF acceptance test first: locate the description-bearing widget with a provider-independent locator and assert `Get Attribute    Description` returns the configured string; add a negative check that a widget without a description has no `Description` attribute (egui acceptance suite, `tests/acceptance/egui/`)
- [x] 6.2 Set an AccessKit description on one or two stable widgets in `apps/test-app-egui` via `Context::accesskit_node_builder` + `Node::set_description` (egui 0.34 / accesskit 0.24)
- [x] 6.3 Run the compositor/egui acceptance lane and confirm pass via `robotcode results` (lane exit code is not trustworthy)

## 7. Documentation

- [x] 7.1 Add a "Common Attributes" table to `dev-docs/architecture.md` §6.3 (Role/Name/Id/RuntimeId/Technology/SupportedPatterns/Description with presence semantics)
- [x] 7.2 Add a `Description` subsection in §5 parallel to "Developer Id (`control:Id`)": strict mapping table (UIA `FullDescription`, AT-SPI `Accessible.Description`), explicit no-HelpText-fallback rationale, open macOS mapping
- [x] 7.3 Add `Description` rows to the §6.4 platform-mapping tables and entries in the §7 provider checklists (Windows, Linux, Mock)

## 8. Verification

- [x] 8.1 Run `just check` and `just test` (workspace fmt/clippy/ruff/mypy + nextest)
- [x] 8.2 Run `just test-python`
- [x] 8.3 Confirm the acceptance lane result via `robotcode results`; run `just pre-commit` before handing over
