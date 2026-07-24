# Tasks — Remove Programmatic Set-Text

## 1. Core vocabulary

- [ ] 1.1 Remove `TextEditablePattern` and `TextEditableAction` from `crates/core/src/ui/pattern.rs` (incl. their tests) and drop the re-exports in `crates/core/src/ui/mod.rs`
- [ ] 1.2 Document the capability-marker semantics on `pattern_names::TEXT_EDITABLE` in `crates/core/src/ui/identifiers.rs` (advertised without an action instance; text entry is keyboard-driven)

## 2. JAB provider

- [ ] 2.1 Remove the `TEXT_EDITABLE` branch from `pattern_by_name` in `crates/provider-jab/src/node.rs` (advertisement in `supported_patterns_for` and the `IsReadOnly` attribute stay unchanged)
- [ ] 2.2 Remove `client.set_text_contents` (`client.rs`), the `setTextContents` DLL binding (`dll.rs`), and the `TextTooLong` error variant (`error.rs`); fix any remaining references
- [ ] 2.3 Replace the `setTextContents` round-trip block in `tests/live_fixture.rs` with the marker assertion (TextEditable advertised, `IsReadOnly` = false, `pattern_by_name` returns `None`) and update the fixture module docs
- [ ] 2.4 Add/extend a unit test for `supported_patterns_for` pinning that an editable text field still advertises `TEXT_EDITABLE` after the action removal

## 3. Docs and Python clarifications

- [ ] 3.1 Reword the `TextEditable` entry in dev-docs/architecture.md's pattern catalog to capability-marker semantics (attributes stay: Text, IsReadOnly, optional MaxLength)
- [ ] 3.2 Clarify the docstrings in `src/PlatynUI/core/patterns/text.py` (`TextEditable.set_text`, `Clearable.clear`) that implementations synthesize keyboard input, never programmatic writes
- [ ] 3.3 Check dev-docs/platform-windows.md and openspec/specs/jab-provider references for `setTextContents` mentions and update them

## 4. Coordination and verification

- [ ] 4.1 Align the in-flight `provider-java-agent-swing`/`-swt`/`-javafx` change artifacts with the text-input-policy capability (no agent-side programmatic writes; flag rather than silently rewrite if their designs conflict)
- [ ] 4.2 Run `just check` and `just test-crate platynui-provider-jab`; on Windows optionally run the JAB live fixture and the Swing acceptance lane (expect the three known pre-existing menu failures on main)
