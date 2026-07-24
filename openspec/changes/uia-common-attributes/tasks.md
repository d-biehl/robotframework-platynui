# Tasks — UIA Common Attributes

## 1. Core testkit

- [ ] 1.1 Add a `common_attributes()` expectation helper to `platynui_core::ui::contract::testkit` (Role, Name, RuntimeId, Technology, SupportedPatterns required; Id/Description conditional) with unit tests for the pass and missing-Technology cases
- [ ] 1.2 Run the mock-provider node through the new expectation in an existing core/runtime test to prove the helper against a conforming provider

## 2. UIA element nodes

- [ ] 2.1 Add `TechnologyAttr` (static, `Namespace::Control`, value `UIAutomation`) and owner-backed `SupportedPatternsAttr` (reads `supported_patterns()` via `supported_patterns_value`) to `crates/provider-windows-uia/src/node.rs`
- [ ] 2.2 Extend `AttrsIter` with the two new attributes (after `Text`, before the native-property stream) and extend the `UiaNode::attribute()` name match accordingly
- [ ] 2.3 Add a cached keyboard-focusability check (`OnceLock`, `CurrentIsKeyboardFocusable`, error ⇒ false) and gate both `supported_patterns()` and `pattern_by_name(Focusable)` on it
- [ ] 2.4 Add the editability predicate (`supports_text_content` ∧ ¬read-only via `ValuePattern.CurrentIsReadOnly`; TextPattern-only ⇒ read-only) in `map.rs`/`node.rs` and advertise `TEXT_EDITABLE` as a capability marker (no action from `pattern_by_name`)
- [ ] 2.5 Expose `control:IsReadOnly` for all `supports_text_content` elements in `AttrsIter` and `attribute()`

## 3. UIA application node

- [ ] 3.1 Add `Technology` and `SupportedPatterns` to `AppAttrsIter` (Control namespace, mirroring the element-node attributes)

## 4. Verification

- [ ] 4.1 Unit test in `provider-windows-uia` pinning that every name yielded by the new `AttrsIter` entries is also resolvable via `attribute()` (guards the dual-path drift)
- [ ] 4.2 Tighten `tests/acceptance/swing/dedup.robot`: assert the kill-switch UIA shell via `@Technology="UIAutomation"` and update the workaround docstring
- [ ] 4.3 Add a platform-agnostic assertion to the egui acceptance smoke suite: the app window exposes non-empty `@Technology` and `@SupportedPatterns`
- [ ] 4.4 Run `just check` and `just test-crate platynui-provider-windows-uia`; on Windows run `just build-native` and the Swing/egui acceptance lanes (expect the three known pre-existing menu failures on main)

## 5. Docs

- [ ] 5.1 Correct the `Technology` example value in dev-docs/architecture.md §6.3 (`UIA` → `UIAutomation`)
- [ ] 5.2 Update dev-docs/platform-windows.md if it describes the UIA attribute surface (verify; flag divergence rather than silently rewriting)
