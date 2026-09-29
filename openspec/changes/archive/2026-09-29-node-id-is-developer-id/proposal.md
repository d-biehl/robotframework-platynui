# Proposal

## Why

A node's `Id` is meant to be one thing: the identifier a developer or toolkit gives an element for automation. The core contract says so (`crates/core/src/ui/node.rs:19`, "developer-provided stable identifier (control:Id)"), and so does `dev-docs/architecture.md` §5.5: UIA `AutomationId`, AT-SPI `accessible_id`, macOS `AXIdentifier`, emitted only when set. Two providers break this, and no spec pins the rule:

- **AT-SPI application nodes report their process ID as `id()`.** `element.id` / `${el.id}` and the `#…` in log lines show the PID. The `@Id` attribute on the same node shows the toolkit's accessible-id instead, for example `""` or Qt's `QApplication`. A user who copies `#3768` from a log line into `/app:Application[@Id="3768"]` finds nothing.
  - This came from code (babe8704, 2026-02-12), not from a design.
  - The archived change `atspi-process-identity` later wrote it into `sidecar-deployment` and flagged the conflict with the core contract without resolving it.
- **UIA application nodes report their name as `id()` and as `@Id`** (`crates/provider-windows-uia/src/node.rs:1824-1827`; listed by `AppAttrsIter` at `:1452-1454`, answered by name at `:1872-1874`). `application-process-attributes` left `id()` as it was, so the `Id` still follows the name, which is now `app:ProcessName` (its design D9). Its spec keeps a clause that a `control:Id` carrying the name carries the same value.
- **AT-SPI emits `@Id` on every node, as `""` when the toolkit sets no accessible-id** (`crates/provider-atspi/src/node.rs:1284-1288`, `:1755`).
  - This contradicts architecture.md §5.5 and the presence rule `description-attribute` cites ("the same presence rule as `control:Id`").
  - It makes `[@Id]` true for every AT-SPI node, and `not(@Id)` never true.
  - It started as a regression: 58004c9d emitted `@Id` only when an id existed, and 87eb514d dropped that condition.

UIA controls, JAB, the Java agent and the mock already follow the rule. An application is identified by `@ProcessId` on every provider already (`sidecar-deployment`: "The process-ID attribute of an application node … is the application's identity").

## What Changes

- **`Id` is the toolkit's identifier, taken by source.**
  - UIA: `AutomationId`.
  - AT-SPI: `Accessible.AccessibleId`, with the object-attribute fallback the provider already reads.
  - AccessKit: its author id reaches both of the above.
  - Java agent (Swing/AWT): `Component.getName()` when it was set through `setName`, by the application or by Swing itself, and only for components and windows.
  - JAB: none.
  - macOS: `AXIdentifier` once the provider exists.
  - Who set the value does not matter: Qt's generated object path and Swing's own names (`null.contentPane`, `Spinner.nextButton`) count as the id.
- **An element without an id has no `Id`.**
  - `UiNode::id()` is none, and no `control:Id` attribute is listed or found by name — never an empty string.
  - The attribute and the accessor always carry the same value.
- **Application nodes have no `Id` on any provider.**
  - This covers every node at the application level, whatever its role (after `atspi-application-level` an AT-SPI root can be `app:Frame`).
  - AT-SPI no longer derives `id()` from the process ID, and an application root's own accessible-id stays visible only as `native:Accessible.AccessibleId`.
  - UIA drops the node's name, its process name, from `id()` and `@Id`.
  - An application's identity is `@ProcessId`, unchanged.
- **Specs.**
  - The rule gets its own capability, `id-attribute`, next to `description-attribute`.
  - `sidecar-deployment` drops the node-identifier paragraph and its two scenarios.
  - `application-process-attributes` drops the `control:Id` clause once that change is archived.
- **Docs.** All of the following are aligned with the rule:
  - the core trait and `attribute_names`, the Python `UiNode.id` docstring and stub;
  - the Java agent's id docs, in Rust and in the agent's comments;
  - `architecture.md` §5.1, §5.5, §6.3, §6.4 and the §7.3 checklist;
  - `platform-linux.md`, `platform-windows.md`, `planning.md`;
  - the QML fixture README;
  - the `@AutomationId` examples in `src/PlatynUI/core/locator.py`, `dev-docs/python-library-design.md` and `dev-docs/python-migration-status.md`.

**Behavior changes that users see.** PlatynUI is at 0.x, so they are not marked as breaking. The release notes name each of them:

- **`element.id` / `${el.id}` of an application is `None`.** On AT-SPI it was the process ID, on UIA the process name. `@ProcessId` stays the way to identify an application.
- **`@Id` disappears from UIA application nodes.** `app:Application[@Id="notepad"]` no longer matches; `@Name` and `@app:ProcessName` still do.
- **One-line descriptions of applications lose their `#…` suffix.** This affects BareMetal action lines, log records and `describe()`: `Application "gedit" #3768` becomes `Application "gedit"`.
- **On AT-SPI, a node without an accessible-id no longer lists `@Id=""`.**
  - `[@Id]` is then true only for nodes with an id.
  - A locator `[@Id=""]` stops matching.
  - `Get Attribute … Id` fails with "attribute not found" instead of returning `""`.
  - Snapshots, `platynui-cli query` and the Inspector's attribute pane lose the empty `Id` lines.
- **On AT-SPI, an application whose root object reports an accessible-id no longer carries it as `@Id`.** Qt's root reports its application name, or `QApplication` when the application sets none, so `app:Application[@Id="QApplication"]` stops matching. The value stays readable as `native:Accessible.AccessibleId`.

## Capabilities

### New Capabilities

- `id-attribute`: what `Id` is on every provider, when it is present, and that the attribute and the accessor agree. It also states that application nodes carry no `Id`.

### Modified Capabilities

- `sidecar-deployment`: the requirement *An application reports its own process ID, and process-table data only through a process ID valid in the runtime's namespace* is replaced by *An application reports its own process ID, and reads process-table data only through a process ID valid in the runtime's namespace*. OpenSpec cannot drop scenarios from a modified requirement, hence the new name. The new requirement:
  - keeps the text and scenarios of the old one;
  - replaces the node-identifier paragraph with a pointer to `id-attribute`;
  - drops the scenarios *An application's node identifier follows its process ID* and *An application without a process ID has no PID-derived node identifier*;
  - no longer mentions the node identifier in *On an ordinary desktop the process attributes are unchanged*.
- `application-process-attributes`: the requirement *Process attributes have fixed names and namespaces* drops the clause "and a `control:Id` that carries the name SHALL carry the same value". This delta can be archived only after `application-process-attributes` itself is archived.

## Impact

- **Rust:**
  - `crates/provider-atspi`:
    - `src/node.rs`: `id()` decides from the level alone. `node_id`, `application_id` and `AtspiNode::resolve_process_id` are removed. The `Id` attribute follows the same decision and is listed only when present, and a named lookup answers `Id` directly.
    - `src/pidns_harness.rs`: the `.id` probe field and its assertion go.
  - `crates/provider-windows-uia/src/node.rs`: `ApplicationNode::id()` is removed, and so is every `Id` path of the application node: the `Id` slot of `AppAttrsIter` with its `name()` helper, which only that slot uses, and the `control:Id` arm of the `attribute()` override that `application-process-attributes` added. Their doc comments stop naming `Id`. `IdAttr` stays for the element nodes.
  - `crates/core`:
    - the `UiNode::id()` contract doc and `attribute_names::common::ID`;
    - a contract-testkit check that the attribute and the accessor agree, with none on `app` nodes.
  - `crates/provider-java`: doc comments only (`src/agent/element.rs`). JAB, the Java agent's nodes and the mock need no code change.
- **Java agent:** comments in `java/agent/.../SwingElement.java` only (`explicitNameOf`, `nameWasExplicitlySet`). No behavior change and no version bump.
- **Python / Robot Framework:**
  - No keyword or API signature changes.
  - The `UiNode.id` docstring (`packages/native/src/runtime.rs`) and the `.pyi` stub are updated.
  - The `locator.py` docstring examples are updated.
- **Tests:**
  - AT-SPI unit tests for the `Id` decision replace the three `application_id` tests and the two `node_id` tests of `atspi-application-level`.
  - The UIA test `an_application_node_is_named_after_its_process_name`, which `application-process-attributes` added, asserts that `control:Id` equals `app:ProcessName`. It keeps its `control:Name` assertion and asserts instead that no `control:Id` is found. The nameless-node test keeps asserting that no `Id` is listed; its reason becomes that an application has none.
  - The contract testkit's new `Id` check runs on the mock, on UIA nodes (the application node, the desktop root, the test window and a test-window button that gets a control ID) and on the Java live fixture's window trees and application nodes.
  - `tests/acceptance/egui/query.robot` gets cases that run on every real lane (X11, compositor and Windows): the application has no `Id` and is described without one, no application is selected by `Id`, and enumeration, named lookup and accessor agree for an element with and one without an author id.
- **Docs:** as listed under What Changes.
- **Build:** a native rebuild, because the providers are linked into `packages/native`.
- **Platforms:**
  - Linux AT-SPI (X11 and the compositor) and Windows UIA change behavior.
  - JAB, the Java agent, the mock and macOS (stub) keep theirs.
  - The UIA part is verified only on real Windows; Wine is not Windows verification.
- **Order and coordination:**
  - This change builds on `atspi-application-level` (`9ce921b`), which introduced the level marker `Level`, `node_id` and the two `node_id` tests.
  - It also builds on `application-process-attributes`, whose code and docs have landed (`3c151f1` to `4a07527`) and which is not archived yet. As agreed, that change left `control:Id` alone: `ApplicationNode::id()` still returns the node's name. Because the name now comes from `ProcessName` (its design D9), the UIA `Id` follows it. Its `attribute()` override answers `control:Id` from that name, a UIA test pins the value, and `platform-windows.md` and the Windows checklist of `architecture.md` §7.3 describe it. This change removes those paths, changes that test and those docs, and removes the spec clause by delta.
  - `gate-uia-window-patterns` and `evaluate-pidns-tests-in-ci` touch the same files (`ApplicationNode`, the pidns harness). Whichever lands later rebases.
  - `provider-java-javafx` plans `Node.getId()` as an addressable attribute through the agent. If it becomes `control:Id`, that change carries a MODIFIED delta of `id-attribute` naming the JavaFX source.
