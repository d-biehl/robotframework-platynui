## Why

Accessibility APIs expose a description alongside the accessible name and id (AT-SPI `Accessible.Description`, UIA `FullDescription`), but PlatynUI only surfaces it as a provider-specific `native:` attribute. Test authors cannot locate or assert on descriptions in a provider-independent way (`@Description`), even though both real providers already fetch the value.

## What Changes

- Add `Description` to the common attribute set (`control:` namespace), alongside `Role`, `Name`, `Id`, `RuntimeId`, `Technology`, `SupportedPatterns`.
- Strict per-platform mapping to the true accessible-description source only: UIA `FullDescription`, AT-SPI `Accessible.Description`. **No fallback** to `HelpText`, `LegacyIAccessible.Description`, or other tooltip-like properties — those stay in `native:` (a help/tooltip concept would be a separate future attribute).
- Follow the `control:Id` precedent: the attribute is emitted only when the platform value is non-empty.
- Add a first-class `UiNode::description()` accessor (parallel to `name()`/`id()`) and expose it through the native bindings and the Python `Adapter`/`Context` API.
- Mock provider supports spec-driven descriptions so the fast test lane covers the attribute.
- Document the mapping in `dev-docs/architecture.md`, including a new explicit "Common Attributes" table (the common set is currently only described in prose).
- macOS AX: no change (provider is a stub); the mapping is recorded as open — `AXHelp` is help text, not a description.

## Capabilities

### New Capabilities

- `description-attribute`: The common `control:Description` attribute — its per-platform sourcing, empty-value semantics, node accessor, and Python/Robot Framework exposure.

### Modified Capabilities

<!-- none — no existing spec covers the common attribute set -->

## Impact

- **Rust core**: `crates/core/src/ui/attributes.rs` (new `common::DESCRIPTION` constant), `crates/core/src/ui/node.rs` (new default `description()` trait method).
- **Providers**:
  - `crates/provider-atspi/src/node.rs` — new `StdAttrKind::Description` slot + resolver (value already fetched for `native:Accessible.Description`).
  - `crates/provider-windows-uia/src/node.rs`, `map.rs` — new `DescriptionAttr` reading `UIA_FullDescriptionPropertyId`, including the hardcoded `attribute()` fast-path.
  - `crates/provider-mock` — spec-driven description emission.
  - `crates/provider-macos-ax` — untouched (stub; see README platform table).
- **Python boundary**: `packages/native/src/runtime.rs` getter; `src/PlatynUI/core/adapter.py` / `context.py` properties. Requires a native rebuild (`just test-python` builds with `mock-provider`).
- **Test app**: `apps/test-app-egui` sets AccessKit descriptions on selected widgets (egui `Context::accesskit_node_builder` + AccessKit `Node::set_description`) so the AT-SPI acceptance lane can verify end-to-end.
- **Docs**: `dev-docs/architecture.md` §5 (new Description subsection next to "Developer Id"), §6.4 platform-mapping tables, §7 provider checklists.
- **Tests**: Rust unit (`crates/core/tests/`), mock-provider expectations, pytest (`packages/native/tests/`), RF acceptance (egui lane). XPath engine, runtime bridge, Inspector, and CLI handle attributes generically — no changes needed there (CLI default snapshot attribute set intentionally stays Name+Id).
- **Not breaking**: purely additive; existing locators and `native:` attributes are unchanged.
