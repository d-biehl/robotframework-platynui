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

**D3 — Focusable gate on an *explicitly denied* `IsKeyboardFocusable`.** New `OnceLock<bool>` cache on `UiaNode` (same shape as `has_window_surface`, `node.rs:210-219`). Gate applied in both `supported_patterns()` and `pattern_by_name()`.

*Corrected during implementation.* The section originally proposed reading `CurrentIsKeyboardFocusable` and defaulting to `false` when it cannot be read, and assumed top-level windows report focusable. Measurement against live elements shows both halves are wrong, and the accessor itself is unusable here:

- `UIA_IsKeyboardFocusablePropertyId` has a **documented default value of `FALSE`** ([Automation Element Property Identifiers](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-automation-element-propids)). `CurrentIsKeyboardFocusable` therefore returns `false` both when the provider denies focusability and when it never implements the property — the two are indistinguishable. Reading with `GetCurrentPropertyValueEx(id, ignoreDefaultValue = TRUE)` returns the NotSupported sentinel for the second case and separates them.
- Measured on real elements: static `Text` labels and title-bar min/max/close buttons report an **explicit `false`** (this is the bug the change fixes); buttons, edit controls and the egui/AccessKit top-level window report an **explicit `true`** — so the assumption that windows are non-focusable came from a single bad sample; a VS Code (Electron, accessibility not switched on) window and several plain Win32 panes **supply nothing at all**.

Hence the rule: an explicit value is taken at face value, and when the provider is silent the answer comes from whether the element is a top-level window — a window can be focused, an element deeper in the tree gets no benefit of the doubt. Defaulting silence to "not focusable" across the board would have stripped `Focusable` from an entire Electron window; defaulting it to "focusable" across the board would have kept the advertisement on every silent container pane, which is exactly the looseness this change set out to remove.

`has_window_surface()` (WindowPattern ∨ TransformPattern, already cached) stands in for "top-level window". Measured, it separates the observed cases exactly: the silent VS Code window has it; the silent inner panes (`DesktopWindowXamlSource`, a Program Manager child) do not — although both carry a native `HWND`, so an `HWND`-based test would not distinguish them. Where it is loose (a modal dialog, a resizable non-window pane) it errs toward advertising, which costs a failed action rather than a lost capability.

*Alternative considered:* deriving focusability from Win32 window styles (`WS_TABSTOP`, `WS_DISABLED`) for elements with a native handle. Rejected for now — it answers nothing the top-level rule does not already answer for the one silent case measured, while adding a per-node Win32 probe and a heuristic that would need its own validation. Worth revisiting only if a real app surfaces silent *inner* controls that genuinely take focus.

**D4 — Editability gate: `supports_text_content` ∧ ¬read-only.** `supports_text_content` (`map.rs:499-502`: TextPattern or ValuePattern available) is the established text gate. Read-only resolution: `ValuePattern.CurrentIsReadOnly` when ValuePattern is available; an element with only TextPattern (document viewers) counts as read-only. Mirrors JAB's text-interface ∧ `editable` gate with UIA vocabulary. `TEXT_EDITABLE` is advertised iff this predicate holds; no action instance is returned from `pattern_by_name` (capability marker only, per companion change).

**D5 — `IsReadOnly` exposed whenever `supports_text_content`.** Value from the same read-only resolution as D4 (lazy attribute, per-read COM call like the other UIA attrs). JAB does the same: `IS_READ_ONLY` accompanies every text-interface element (`provider-jab/src/node.rs:436`). Clients derive "editable" as `TextContent ∧ ¬IsReadOnly` or directly via the `TEXT_EDITABLE` marker.

**D6 — Both `attributes()` and `attribute()` are extended.** UIA's `attribute()` is an explicit name-match without fallback scan (`node.rs:343-412`), so `Technology`, `SupportedPatterns`, and `IsReadOnly` must be added there too. New `AttrsIter` indices come after `Text` (index 18) and before the native-property stream (index 19 → shifts to the end).

**D7 — Contract testkit gains a common-attribute expectation.** A `common_attributes()` helper in `platynui_core::ui::contract::testkit` returning the required set (`Role`, `Name`, `RuntimeId`, `Technology`, `SupportedPatterns` required; `Id`, `Description` conditional) as `AttributeExpectation`s, verified through the existing `verify_node` machinery (`testkit.rs:86-127`). Unit-tested against the mock node in core. *Revised during implementation:* a live-COM unit lane does exist after all — `com::uia().GetRootElement()` reaches the desktop root without a fixture app, so `node.rs` now carries `#[cfg(test)]` tests that run the real UIA surface (attribute/enumeration agreement, `Technology`, `SupportedPatterns`, the common-attribute contract). They panic rather than skip when UIA is unreachable, so an environment that cannot reach it fails loudly instead of reporting a green run that checked nothing — which is how they immediately exposed a latent provider bug, see D8.

Acceptance-level verification remains the cross-platform check: Swing dedup asserts `@Technology="UIAutomation"` on the UIA shell with the kill switch active, and the egui smoke suite asserts non-empty `@Technology`/`@SupportedPatterns` on the app window platform-agnostically (value differs per platform: `AT-SPI2` on Linux, `UIAutomation` on Windows).

**D8 — The UIA client library's first initialization is serialized process-wide (`com.rs`).** Found while implementing, not planned: the new live tests failed on 7 of 8 threads with `GetRootElement` returning `E_FAIL` whenever their first ever UIA use overlapped. Measured behaviour — `CoCreateInstance(CUIAutomation)` succeeds on every thread, but the library is not usable until one real call has completed, and racing that first call breaks all but one thread. Priming it once makes every thread work, and it stays warm for the process lifetime.

`uia()` now takes a process-wide `Mutex` for the creation path and, on first arrival only, performs one `GetRootElement()` to finish the initialization. Both halves are load-bearing: serializing `CoCreateInstance` alone only reduces the failures from 7/8 to 1-2/8. The lock is taken once per thread (each keeps its own instance in the existing thread-local), so steady-state cost is nil.

This is a provider bug rather than a test artefact — the provider is entered from several threads (tree streaming, the Inspector, hit-testing), so a cold parallel start is reachable in production. `com.rs` carries a regression test that fans out eight threads on a cold process; it must stay the only test in its module, because once the library is warm the assertion proves nothing.

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
