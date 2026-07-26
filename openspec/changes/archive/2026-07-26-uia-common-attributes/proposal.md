# UIA Common Attributes

## Why

The architecture contract (dev-docs/architecture.md §6.3) declares `Technology` and `SupportedPatterns` as always-present common attributes on every `control:`/`item:` node. The AT-SPI2, JAB, and mock providers honor this; the Windows UIA provider does not — its nodes expose neither attribute, so XPath filters like `@Technology="UIAutomation"` silently match nothing. The Swing acceptance suite (`tests/acceptance/swing/dedup.robot`) already works around exactly this gap. Two adjacent defects surface with the fix: the UIA provider advertises `Focusable` unconditionally (every static label claims to be focusable), and it gives clients no editability signal (`TextEditable` marker / `IsReadOnly`) even though JAB set the precedent and the Python proxy layer needs it to gate keyboard-based text entry.

## What Changes

- The UIA provider emits `control:Technology` (`"UIAutomation"`) and `control:SupportedPatterns` on element nodes — in the `attributes()` enumeration, in the `attribute()` single lookup, and on the synthetic `ApplicationNode`.
- `Focusable` is advertised (and its action returned) only when the element is actually keyboard-focusable (`CurrentIsKeyboardFocusable`), matching the AT-SPI/JAB gating. This narrows `SupportedPatterns` on non-focusable elements — technically a behavior change, but a bug fix against the contract ("keep SupportedPatterns consistent with available pattern instances").
- The UIA provider advertises `TextEditable` as a pure capability marker (no programmatic set-text action — text entry stays keyboard-driven, see the companion change `remove-programmatic-set-text`) and exposes `control:IsReadOnly` for text-bearing elements, mirroring JAB.
- The shared contract testkit (`platynui_core::ui::contract::testkit`) gains a common-attribute expectation (Role, Name, RuntimeId, Technology, SupportedPatterns always present) that provider test suites reuse; the Swing dedup acceptance test drops its workaround and asserts `@Technology="UIAutomation"` directly.
- Doc fix: architecture.md's example value `UIA` is corrected to the actual registered id `UIAutomation`.

## Capabilities

### New Capabilities

- `uia-common-attributes`: the Windows UIA provider's common-attribute surface — Technology and SupportedPatterns presence, focusability-gated pattern advertisement, TextEditable capability marker with IsReadOnly.

### Modified Capabilities

<!-- none: no existing spec covers the UIA provider node surface; jab-provider and textcontent-pattern requirements are untouched -->

## Impact

- **Rust**: `crates/provider-windows-uia` (node.rs: `AttrsIter`, `attribute()`, `supported_patterns()`, `pattern_by_name()`, `AppAttrsIter`), `crates/core` (contract testkit helper only — no trait changes).
- **Windows-only behavior**: the affected provider is Windows-native (README platform table: Windows UIA = supported). Linux/macOS providers are untouched; the mock provider already conforms.
- **Tests**: new/extended unit coverage in `provider-windows-uia`, testkit helper tests in `core`, and a tightened `tests/acceptance/swing/dedup.robot` (Windows acceptance lane). Note the three pre-existing menu-related acceptance failures on main are unrelated.
- **Python/RF surface**: no API change — attributes flow through the existing generic attribute pipeline. Robot suites gain reliable `@Technology`/`@SupportedPatterns` filtering on Windows.
- **Native rebuild**: needed to see the effect from Python on Windows (`just build-native`); pure-Rust verification via `just test-crate platynui-provider-windows-uia`.
- **Breaking**: none for public APIs. `SupportedPatterns` narrowing (Focusable gating) can change XPath results for queries that relied on the buggy always-on advertisement; none exist in-tree. The narrowing hits elements whose provider states `IsKeyboardFocusable = false` (measured: static text labels, title-bar min/max/close buttons) plus silent inner elements; a silent top-level window keeps the advertisement, since that property defaults to `FALSE` and absence must not be read as denial for a window that plainly takes focus.
