## Context

Accessibility APIs carry a description next to the accessible name and id. PlatynUI currently surfaces it only as a provider-specific `native:` attribute:

- AT-SPI: `native:Accessible.Description` is already fetched — the native prop is registered in `AttrsIter::new` (`crates/provider-atspi/src/node.rs:937`) and resolved via `proxy.description()` in `LazyNativeAttr::fetch_accessible` (`crates/provider-atspi/src/node.rs:1892`). *(verified)*
- UIA: `FullDescription` appears only through the dynamic native-property enumeration (`collect_native_properties`, `crates/provider-windows-uia/src/map.rs:749`); there is no dedicated constant or typed attribute. *(verified: no `FullDescription` identifier exists in the repo)*

The common attribute set (`Role`, `Name`, `Id`, `RuntimeId`, `Technology`, `SupportedPatterns`) is defined as constants in `crates/core/src/ui/attributes.rs:5-13` *(verified)*. There is no global attribute registry: each provider emits its own `UiAttribute` values, and the XPath engine, runtime bridge, and Inspector treat attributes fully generically.

## Goals / Non-Goals

**Goals:**

- `control:Description` as a common attribute available on every provider whose platform exposes an accessible description, usable in XPath locators (`//control:Button[@Description='…']`) and via `Get Attribute`.
- Strict, documented per-platform mapping; provider-independent semantics.
- First-class accessor parity with `Name`/`Id` through the Rust trait, native bindings, and Python API.
- End-to-end verification against a real provider (AT-SPI via the egui acceptance lane), not just the mock.

**Non-Goals:**

- No `HelpText`/tooltip attribute — that is a semantically different concept and, if ever wanted, a separate change.
- No macOS AX implementation (provider is a stub, `crates/provider-macos-ax/src/lib.rs`); this change only records the open mapping question.
- No change to the CLI default snapshot attribute set (`crates/cli/src/commands/snapshot.rs` guarantees only `Name`+`Id` in `AttrMode::Default`) — descriptions are verbose and often absent; `--attrs` already exposes everything.
- No Inspector/runtime/XPath changes — attribute handling there is generic and picks the new attribute up automatically.

## Decisions

### D1: Strict source mapping — no fallback chain

`control:Description` maps 1:1 to the platform's accessible-description property:

| Platform | Source | Explicitly excluded |
|---|---|---|
| AT-SPI2 | `Accessible.Description` | `Accessible.HelpText` |
| Windows UIA | `FullDescription` (`UIA_FullDescriptionPropertyId`) | `HelpText`, `LegacyIAccessible.Description` |
| macOS AX | open (stub provider; `AXHelp` is help text, not a description) | — |

Rationale: HelpText is not the same thing as a description (tooltip/help semantics). A `FullDescription → HelpText` fallback was considered and rejected: it would return plausible-looking but semantically wrong values, and the excluded properties remain reachable under `native:`. This was an explicit user decision during exploration.

### D2: Emit only when non-empty (the `control:Id` precedent)

Descriptions are empty for most elements. Emitting empty strings would flood the Inspector attribute view and make `[@Description]` existence checks meaningless. UIA already gates `IdAttr` on a present value (`crates/provider-windows-uia/src/node.rs:1579` + iterator gating); `Description` follows the same rule on all providers: empty/absent platform value → attribute not present.

Consequence: the attribute is *optional* in the contract sense — `validate_control_or_item` (`crates/core/src/ui/contract.rs`) does not enforce per-attribute presence, so no contract change is needed.

### D3: First-class accessor parity

Add `fn description(&self) -> Option<String>` to `UiNode` with a `None` default (`crates/core/src/ui/node.rs` — `id()` at line 21 is the template *(verified)*), a getter in `packages/native/src/runtime.rs` (next to `id()`/`name()`, lines 41-56 *(verified)*), and `description` properties on the Python `Adapter` (`src/PlatynUI/core/adapter.py`) and `Context` (`src/PlatynUI/core/context.py`) mirroring `name`.

Rationale: `name()`/`id()`/`role()` all have trait methods plus Python properties; leaving `Description` attribute-lookup-only would be an inconsistency users trip over. A default of `None` keeps the trait change non-breaking for all existing `UiNode` implementors.

### D4: Provider wiring follows each provider's existing idiom

- **AT-SPI** (`crates/provider-atspi/src/node.rs`): new `StdAttrKind::Description` variant, a new slot in `AttrsIter::next` after `Id`, name mapping to `common::DESCRIPTION`, and a `resolve_description()` on the lazy node context calling `proxy.description()` — the same call the native attr already uses (`node.rs:1892` *(verified)*). Apply D2 (map empty → `None`).
- **UIA** (`crates/provider-windows-uia/src/node.rs`, `map.rs`): new `DescriptionAttr` struct modeled on `NameAttr` (`node.rs:635` *(verified)*), a `map::get_description` reading `UIA_FullDescriptionPropertyId` via `read_uia_property` (`map.rs:432` *(verified)*), a gated iterator slot like `IdAttr`, **and** an arm in the hardcoded `attribute()` fast-path (`node.rs:285` *(verified)*) — this fast-path is easy to miss and would make `attribute()` and `attributes()` disagree. *(assumed: `UIA_FullDescriptionPropertyId` is exposed by the `windows` crate bindings — verify at implementation time; it is a Win10+ property, older targets simply return nothing → attribute absent per D2.)*
- **Mock** (`crates/provider-mock`): description comes from the node spec (the tree DSL already supports arbitrary attributes); wire it so `common::DESCRIPTION` is emitted when the spec provides one and `UiNode::description()` returns it. Per project convention the mock is deliberately partial and not authoritative — it exists here so the fast lane (`just test-python`) covers the plumbing.

### D5: egui test app sets descriptions via AccessKit

`apps/test-app-egui` sets a description on one or two stable widgets using egui's public `Context::accesskit_node_builder` (egui 0.34, `context.rs:3699` *(verified in vendored source)*) and AccessKit's `Node::set_description` (accesskit 0.24, generated accessor *(verified)*). The AT-SPI adapter used by eframe (`accesskit_atspi_common`) forwards `description()` to `Accessible.Description` *(verified in vendored source)*, so the acceptance lane exercises the full chain: egui → AccessKit → AT-SPI → provider → `control:Description`.

### D6: Documentation gets an explicit Common Attributes table

`dev-docs/architecture.md` describes the common set only in prose. Add:
- a "Common Attributes" table in §6.3 listing `Role`/`Name`/`Id`/`RuntimeId`/`Technology`/`SupportedPatterns`/`Description` with presence semantics,
- a `Description` subsection in §5 parallel to "Developer Id (`control:Id`)" with the D1 mapping table (explicitly naming the excluded HelpText-like sources and the open macOS question),
- a `Description` row in the relevant §6.4 platform-mapping tables and entries in the §7 provider checklists (Windows, Linux, Mock).

Per project doc conventions: normative prose, no Rust signatures, point to `crates/core/src/ui/attributes.rs` as source of truth.

## Risks / Trade-offs

- [UIA `FullDescription` requires Win10 1703+ and app support; many apps never set it] → Acceptable: attribute absent per D2 is correct behavior, and `native:` still exposes whatever the app provides. Document that absence is expected.
- [AT-SPI `description()` is an extra D-Bus round-trip per node] → Mitigated by the existing lazy-attribute design (`LazyStdAttr` resolves on access only); the iterator slot itself is cheap.
- [egui/AccessKit description behavior could change on the pinned 0.34 family] → egui is pinned (see repo memory/Cargo constraints); the acceptance test targets specific widgets in our own test app, so breakage is local and visible.
- [Trait method addition could conflict with a provider defining its own `description`] → Checked: no `UiNode` implementor currently defines `description()`; default method keeps it non-breaking.
- [Mock passing while real providers regress] → Standing project rule: mock is not authoritative; the AT-SPI acceptance test (D5) is the authoritative Linux check, Windows verification is manual/Inspector until a UIA lane exists.

## Migration Plan

Purely **additive** — no existing attribute, locator, or API changes behavior. New optional attribute + new accessors with `None`/absent defaults.

- Needs a **native rebuild** for the Python surface (`just test-python` rebuilds with `mock-provider`).
- Rollback: revert the change; nothing persists (no settings, no schema, no serialized state).

## Open Questions

- macOS AX mapping (`AXDescription` vs `AXHelp` semantics differ per role) — deliberately deferred until the macOS provider is real; recorded in the architecture doc as open.
- Exact placement/wording of the new Common Attributes table in §6.3 — decide while editing `architecture.md` (follow the existing pattern-catalog table style).
