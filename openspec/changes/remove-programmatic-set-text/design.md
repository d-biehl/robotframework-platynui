# Design — Remove Programmatic Set-Text

## Context

Verified current state:

- **Core** defines `TextEditablePattern` + `TextEditableAction` (`crates/core/src/ui/pattern.rs:405-455`), doc'd as "programmatic text replacement … as opposed to synthesizing keystrokes", re-exported via `ui/mod.rs`.
- **JAB** is the only implementor: `pattern_by_name` returns a `TextEditableAction` calling `client.set_text_contents` (`crates/provider-jab/src/node.rs:565-574`); the write path spans `client.rs:367-378`, the `setTextContents` DLL binding (`dll.rs:117,166`), and the `TextTooLong` error variant (`error.rs:28-31`). The live fixture exercises the round trip (`tests/live_fixture.rs:320-339`).
- **No other caller exists.** UIA and AT-SPI never wired the action. `packages/native` exposes no set-text binding (verified by grep). The Python `patterns.TextEditable` ABC's `set_text` is implemented by the keyboard-driven proxies (`src/PlatynUI/ui/proxies/text.py:53-59`), which `Adapter.get_pattern` resolves preferentially (`core/adapter_proxy.py:206-221`) — so even `ui/text.py`'s `get_pattern(patterns.TextEditable).set_text(...)` lands on keyboard synthesis today.
- **The contract already supports marker-only patterns**: `runtime_pattern_without_instance_is_allowed` (`crates/core/src/ui/contract.rs`).
- The advertisement itself is load-bearing: clients derive editability from `TEXT_EDITABLE` in `SupportedPatterns` (archived textcontent-pattern design D4) plus `IsReadOnly`; the companion change `uia-common-attributes` builds on exactly this marker semantics for UIA.

## Goals / Non-Goals

**Goals:**
- No provider can (or is invited to) implement programmatic text writes; the core vocabulary for it disappears.
- `TEXT_EDITABLE` stays as advertisement + editability metadata (`IsReadOnly`, optional `MaxLength`), with the marker semantics written down where the next provider author will look.
- JAB keeps advertising correctly; only the action and its transport die.

**Non-Goals:**
- No change to the Python pattern ABCs' public shape (`set_text` stays on the ABC — it is the keyboard-synthesized client operation).
- No new keyboard-based text-entry implementation (already exists in the proxy layer).
- No `Clearable` rework (its `clear` is likewise proxy-synthesized keyboard input; nothing native exists).
- No removal of `pattern_names::TEXT_EDITABLE` or the `text_editable::*` attribute names.

## Decisions

**D1 — Delete the trait and action from core rather than deprecating.** `TextEditablePattern`/`TextEditableAction` have exactly one implementor and zero external consumers; the crates are internal workspace members with no semver commitment. Keeping a deprecated trait would preserve the invitation to misuse it. Alternative — repurpose the trait doc to "marker only" — rejected: a trait with a `set_text` method *is* the API surface that misleads; docs alone don't remove the affordance.

**D2 — Marker semantics documented at the pattern-name constant.** The authoritative note ("advertised capability, no action instance; text entry is keyboard-driven") goes on `pattern_names::TEXT_EDITABLE` (`crates/core/src/ui/identifiers.rs`) — the one symbol every provider must touch to advertise the pattern — plus the architecture.md pattern-catalog row. This is where `uia-common-attributes` D4 already points.

**D3 — JAB advertisement and `IsReadOnly` stay exactly as-is.** The gate (text interface ∧ `editable`, `node.rs:322-324`) and the attribute (`node.rs:436`) are the reference semantics the UIA change mirrors. Only the `pattern_by_name` branch is removed; `TEXT_EDITABLE` then falls through to `None` naturally, which is contract-conform for advertised patterns.

**D4 — Remove the whole transport chain, not just the wiring.** `client.set_text_contents`, the DLL binding, and `TextTooLong` become dead code the moment the action goes; leaving them "for later" contradicts the policy decision this change exists to record. If a sanctioned programmatic write ever returns (it should not), it re-enters through a new proposal, not through leftover plumbing.

**D5 — Live-fixture replacement asserts the policy.** The `setTextContents` round trip (`live_fixture.rs:320-339`) becomes: the editable stage-1 field advertises `TEXT_EDITABLE` and exposes `IsReadOnly=false`, and `pattern_by_name(TEXT_EDITABLE)` returns `None`. End-to-end keyboard text entry is covered where it belongs: the Swing acceptance suite drives real input devices.

**D6 — In-flight Java-agent changes are aligned by reference, not edited here.** The `provider-java-swing`/`-swt`/`-javafx` proposals mention TextEditable; this change's spec (`text-input-policy`) becomes the constraint they must conform to. Their artifacts are living documents in the same repo — a coordination task points there rather than this change quietly rewriting another change's design.

## Risks / Trade-offs

- [A future workflow genuinely needs programmatic writes (huge paste, IME-hostile fields)] → The policy spec records the decision explicitly; re-adding is a deliberate proposal with the trade-offs on the table, and the git history preserves the removed transport for reference. Keyboard entry via `type_keys` handles the known cases.
- [JAB live fixture runs only on Windows with a live Swing app; the removal could silently break advertisement] → The marker assertion in the replaced fixture test plus the Swing acceptance suite (`tests/acceptance/swing`) both pin it; unit-level gating logic (`supported_patterns_for`) is pure and already unit-testable.
- [Old layer callers (`ui/text.py`, `ui/combobox.py`, `ui/item.py`) look like they use a native pattern] → They resolve to proxies today (verified above); no behavior change. Docstring clarifications reduce the future misreading risk.

## Migration Plan

Behavioral only inside Rust (an action that nothing outside a Rust test invokes disappears); additive doc changes elsewhere. No native rebuild needed for Python behavior. Verification: `just check`, `just test-crate platynui-provider-jab`; on Windows optionally the JAB live fixture and Swing acceptance lane. Rollback: revert the commit — no data, config, or API migration.

## Open Questions

- Should the `text-input-policy` spec also constrain `Clearable` (currently proxy-synthesized `<Ctrl+A><Delete>`) explicitly? Leaning yes-by-mention (it follows the same keyboard principle) without adding requirements about its implementation.
