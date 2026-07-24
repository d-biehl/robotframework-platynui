# Remove Programmatic Set-Text

## Why

PlatynUI's interaction philosophy is simulating genuine user input: text is entered through synthesized keyboard events (focus + select-all + type), which the Python proxy layer already implements (`src/PlatynUI/ui/proxies/text.py`). The JAB provider nevertheless implements `TextEditable` as a programmatic write (`setTextContents`), and the core trait `TextEditablePattern` even documents programmatic replacement as intended ("as opposed to synthesizing keystrokes"). That was an oversight: no Python/Robot code path can reach the action (the native bindings expose no set-text), it bypasses the application's real input handling, and it invites every future provider (UIA `ValuePattern.SetValue`, AT-SPI `set_text_contents`, the upcoming Java agents) to repeat the mistake. `TextEditable` must be settled as a pure capability marker before those providers land.

## What Changes

- **BREAKING (Rust-internal)**: Remove `TextEditablePattern` and `TextEditableAction` from `platynui_core::ui::pattern`. The `pattern_names::TEXT_EDITABLE` identifier stays and is documented as a capability marker (advertised without an action instance — explicitly allowed by the node contract).
- JAB provider: remove the `TextEditableAction` wiring from `pattern_by_name` and the now-dead write path (`client.set_text_contents`, the `setTextContents` DLL binding, the `TextTooLong` error variant). The `TEXT_EDITABLE` advertisement (text interface ∧ `editable` state) and the `IsReadOnly` attribute stay unchanged.
- JAB live fixture: replace the `setTextContents` round-trip test with a marker assertion (`TextEditable` advertised on the editable field, `pattern_by_name` returns no action).
- Docs: reword the `TextEditable` description in dev-docs/architecture.md's pattern catalog to capability-marker semantics; clarify the Python `TextEditable` ABC docstring (`core/patterns/text.py`) that implementations synthesize keyboard input.
- Coordination: the in-flight `provider-java-agent-swing`/`-swt`/`-javafx` changes reference `TextEditable`/set-text — align their designs with the marker-only stance (no agent-side programmatic writes).

## Capabilities

### New Capabilities

- `text-input-policy`: text entry across all providers is keyboard-driven; `TextEditable` is a capability marker (advertisement + editability metadata, never a programmatic write action).

### Modified Capabilities

- `jab-provider`: the "Core interaction patterns" requirement changes — `TextEditable` drops its `setTextContents` write and becomes advertisement-only.

## Impact

- **Rust**: `crates/core` (`ui/pattern.rs` trait/action removal, `ui/mod.rs` re-exports, marker-semantics docs), `crates/provider-jab` (`node.rs`, `client.rs`, `dll.rs`, `error.rs`, `tests/live_fixture.rs`). No other provider implements the action (verified: UIA and AT-SPI never wired it).
- **Python/RF surface**: none — `patterns.TextEditable.set_text` resolves to the keyboard-based proxies (`get_pattern` prefers proxy implementations), and `packages/native` exposes no set-text binding. Docstring updates only.
- **Platforms**: JAB is Windows-only (Swing/AWT); behavior change is the removal of an action no test-external caller uses. Linux/macOS untouched.
- **Native rebuild**: not required for Python behavior (nothing reachable changes); `just test-crate platynui-provider-jab` plus the JAB live fixture (needs a live Swing app, Windows) cover the removal.
- **Specs**: `openspec/specs/jab-provider/spec.md` requirement "Core interaction patterns" (delta included); new `text-input-policy` capability records the cross-provider stance.
