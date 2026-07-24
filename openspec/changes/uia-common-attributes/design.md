# Design — UIA Common Attributes

## Context

The common-attribute contract (dev-docs/architecture.md §6.3) requires `Technology` and `SupportedPatterns` on every `control:`/`item:` node. Current provider state, verified against the code:

- **AT-SPI2**: `TechnologyAttr` (`crates/provider-atspi/src/node.rs:1643`) and a lazy `SupportedPatterns` attribute (`node.rs:1563-1584`, computed from focusable state + window-surface patterns).
- **JAB**: static `common::TECHNOLOGY` and `common::SUPPORTED_PATTERNS` attributes (`crates/provider-jab/src/node.rs:418,423`), the latter via `supported_patterns_value(&self.supported_patterns_for(&info))`.
- **Mock**: both attributes (`crates/provider-mock/src/node.rs:44,137`).
- **Runtime desktop node**: both attributes in `Namespace::Control` (`crates/runtime/src/runtime/desktop.rs:35-44`).
- **UIA**: neither attribute exists anywhere in `crates/provider-windows-uia/src/node.rs` — not in `AttrsIter` (`node.rs:852-995`), not in `UiaNode::attribute()` (`node.rs:343-412`), not in `AppAttrsIter` (`node.rs:1288-1316`). The `TechnologyId` constant already exists (`provider.rs:18`, value `"UIAutomation"`).

Two adjacent defects, also verified:

- `UiaNode::supported_patterns()` (`node.rs:414-427`) starts with `vec![FocusableAction::static_pattern_name()]` unconditionally, and `pattern_by_name()` (`node.rs:432`) returns the `FocusableAction` without any gate. AT-SPI gates on the `Focusable`/`Focused` states (`provider-atspi/src/node.rs:345`), JAB on `states.focusable || focused` (`provider-jab/src/node.rs:319`).
- UIA exposes no editability signal. JAB advertises `TEXT_EDITABLE` (text interface ∧ `editable` state, `provider-jab/src/node.rs:322-324`) and exposes `text_editable::IS_READ_ONLY` (`node.rs:436`). The Python proxy layer sets text via keyboard (`src/PlatynUI/ui/proxies/text.py:53-59`) and needs the marker/attribute to gate and verify that flow. Per the companion change `remove-programmatic-set-text`, `TextEditable` is a capability marker only — the contract explicitly allows advertised patterns without an action instance (`crates/core/src/ui/contract.rs`, test `runtime_pattern_without_instance_is_allowed`).

`tests/acceptance/swing/dedup.robot:29-30` documents the Technology gap verbatim and works around it.

## Goals / Non-Goals

**Goals:**
- UIA element nodes and the synthetic `ApplicationNode` expose `control:Technology` and `control:SupportedPatterns`, consistent with `supported_patterns()` by construction.
- `Focusable` advertisement and action are gated on real keyboard focusability.
- `TEXT_EDITABLE` capability marker + `control:IsReadOnly` on text-bearing UIA elements.
- A reusable common-attribute expectation in the core contract testkit; tightened Swing dedup acceptance assertions; platform-agnostic presence assertion in the egui suite.
- architecture.md example value corrected (`UIA` → `UIAutomation`).

**Non-Goals:**
- No programmatic set-text (no `ValuePattern.SetValue`) — that stance is `remove-programmatic-set-text`.
- No Toggleable/StatefulValue/Selectable/SelectionProvider/Expandable attributes for UIA (separate future change; AT-SPI lacks them too).
- No change to AT-SPI/JAB/mock behavior, no `UiNode` trait changes.
- No decision on advertising `Application`/`Element`/`TextContent` ClientPatterns in `supported_patterns()` (pre-existing cross-cutting question, see archived textcontent-pattern design D5).

## Decisions

**D1 — `SupportedPatterns` reads through `supported_patterns()` at value time.** A `SupportedPatternsAttr { owner: Weak<dyn UiNode> }` calls `owner.supported_patterns()` and converts with `supported_patterns_value()` (same helper as JAB/AT-SPI). This keeps the attribute consistent with `pattern_by_name` gating by construction (architecture.md:552) instead of duplicating the pattern list. Alternative — a static snapshot at iterator construction — rejected: it can go stale and duplicates logic. The existing `IdAttr`/`DescriptionAttr` owner-backed shape (`node.rs:1692-1713`) is the template.

**D2 — `Technology` is a static attribute.** Value `"UIAutomation"`, sourced from the existing constant semantics (`provider.rs:18`); `Namespace::Control`, matching all other providers and the runtime desktop node. Emitted for element nodes and `ApplicationNode` (whose standard attrs already live in `Namespace::Control`, `node.rs:1061-1117`). The app node also gets `SupportedPatterns` (currently an empty list — `ApplicationNode::supported_patterns()` returns `Vec::new()`); whether app nodes should advertise the `Application` ClientPattern stays open (Open Questions).

**D3 — Focusable gate via cached `CurrentIsKeyboardFocusable`.** New `OnceLock<bool>` cache on `UiaNode` (same shape as `has_window_surface`, `node.rs:210-219`), read via `CurrentIsKeyboardFocusable`, defaulting to `false` on COM error (an element whose properties cannot be read cannot be focused either). Gate applied in both `supported_patterns()` and `pattern_by_name()`. Top-level windows report keyboard-focusable under UIA, so window activation flows are unaffected.

**D4 — Editability gate: `supports_text_content` ∧ ¬read-only.** `supports_text_content` (`map.rs:499-502`: TextPattern or ValuePattern available) is the established text gate. Read-only resolution: `ValuePattern.CurrentIsReadOnly` when ValuePattern is available; an element with only TextPattern (document viewers) counts as read-only. Mirrors JAB's text-interface ∧ `editable` gate with UIA vocabulary. `TEXT_EDITABLE` is advertised iff this predicate holds; no action instance is returned from `pattern_by_name` (capability marker only, per companion change).

**D5 — `IsReadOnly` exposed whenever `supports_text_content`.** Value from the same read-only resolution as D4 (lazy attribute, per-read COM call like the other UIA attrs). JAB does the same: `IS_READ_ONLY` accompanies every text-interface element (`provider-jab/src/node.rs:436`). Clients derive "editable" as `TextContent ∧ ¬IsReadOnly` or directly via the `TEXT_EDITABLE` marker.

**D6 — Both `attributes()` and `attribute()` are extended.** UIA's `attribute()` is an explicit name-match without fallback scan (`node.rs:343-412`), so `Technology`, `SupportedPatterns`, and `IsReadOnly` must be added there too. New `AttrsIter` indices come after `Text` (index 18) and before the native-property stream (index 19 → shifts to the end).

**D7 — Contract testkit gains a common-attribute expectation.** A `common_attributes()` helper in `platynui_core::ui::contract::testkit` returning the required set (`Role`, `Name`, `RuntimeId`, `Technology`, `SupportedPatterns` required; `Id`, `Description` conditional) as `AttributeExpectation`s, verified through the existing `verify_node` machinery (`testkit.rs:86-127`). Unit-tested against the mock node in core. Live verification for UIA runs at acceptance level (no live-COM unit lane exists for windows-uia — the crate has no `tests/` dir): Swing dedup asserts `@Technology="UIAutomation"` on the UIA shell with the kill switch active, and the egui smoke suite asserts non-empty `@Technology`/`@SupportedPatterns` on the app window platform-agnostically (value differs per platform: `AT-SPI2` on Linux, `UIAutomation` on Windows).

## Risks / Trade-offs

- [`SupportedPatterns` narrows on non-focusable elements] → Contract-conform bug fix; no in-tree consumer relies on the always-on `Focusable`. Called out as behavior change in the proposal.
- [Extra COM calls per node: `CurrentIsKeyboardFocusable`, ValuePattern probe for `IsReadOnly`] → Both cached per node (`OnceLock`) or evaluated only when the attribute/pattern is actually read; the traversal cache request (`populate_cached_properties`, `node.rs:186-206`) is unchanged, so tree walks do not slow down.
- [`attribute()` and `AttrsIter` drift apart again] → The testkit expectation catches missing enumeration entries; a small unit test comparing `attribute()` lookups against enumerated names for the new attrs pins the single-lookup path.
- [Read-only heuristic for TextPattern-only elements misclassifies an editable rich-text control lacking ValuePattern] → Accepted for now; such controls still expose `Text`, and the keyboard path works regardless of the marker. Revisit if a real app surfaces the case.

## Migration Plan

Additive plus one intended behavioral narrowing (Focusable gating); no API/ABI change. Pure-Rust verification via `just test-crate platynui-provider-windows-uia` and `just check`; Python-visible effect on Windows requires `just build-native`. Acceptance: Swing suites on Windows (expect the three pre-existing menu failures on main, unrelated). Rollback: revert the commit — no data or config migration involved.

## Open Questions

- Should `app:`-namespace nodes advertise the `Application` ClientPattern in `SupportedPatterns` (and should the contract formally cover `app:` nodes)? Deferred; this change only makes the app node carry the two attributes.
- Cross-provider namespace inconsistency: AT-SPI attaches standard attributes to the node's own namespace (`item:` nodes included), JAB/UIA always to `Control`. Flagged for a separate alignment decision; this change follows the JAB/UIA convention.
