# Design

## Context

The motivation is in proposal.md (Why), and the contract is in `specs/id-attribute/spec.md`. This section records the current state that shapes the approach. **Verified** marks what was read on `main` at `077a9d5` on 2026-09-29. That tree includes the landed code of `atspi-application-level` (`9ce921b`), which introduced the level marker `Level` in the AT-SPI provider. It also includes the landed code of `application-process-attributes` (`3c151f1` to `4a07527`). That change gave the UIA `ApplicationNode` an `attribute()` override and a listing that skips absent attributes, and it takes the node's name from `app:ProcessName`. It left `id()` as it was, so the UIA `Id` follows that name.

**Two paths answer "the id" of a node.** **Verified.**

- The accessor `UiNode::id()` feeds:
  - `element.id` in Python (`packages/native/src/runtime.rs:71-75`);
  - the `#…` suffix of every one-line description (`crates/core/src/ui/describe.rs:10-22`), which shows up in BareMetal action lines and log records;
  - the CLI's `element-at-point` output (`crates/cli/src/lib.rs:171-180`).
- The attribute `control:Id` feeds XPath `@Id` (`crates/runtime/src/xpath.rs:637-655`, a named lookup through `UiNode::attribute`), `Get Attribute … Id`, the Inspector's attribute pane, `platynui-cli query` and snapshots.
- Nothing inside PlatynUI keys on either value:
  - window lookups walk up to `control:ProcessId`;
  - the Inspector's tree identity is the runtime id (`apps/inspector/src/model/tree_data.rs:151-160`).

**Where the providers stand.** **Verified.**

| Provider | Controls: `id()` / `@Id` | Application node: `id()` / `@Id` |
|---|---|---|
| UI Automation | `AutomationId`; `@Id` only when `id()` is set, and its value delegates to `id()` (`crates/provider-windows-uia/src/node.rs:380-392`, `:428-434`, `:1024-1032`, `IdAttr` `:1898-1919`) | the node's name, `app:ProcessName` (`id()` `:1824-1827` over `name()` `:1809-1820`); `AppAttrsIter` slot 3 lists it as `@Id` when it is not empty (`:1452-1454`), and the `attribute()` override answers `control:Id` from the same cached name (`:1872-1874`) |
| AT-SPI | accessible-id (`resolve_id`, `crates/provider-atspi/src/node.rs:829-838`, with the object-attribute fallback); `@Id` always listed, as `""` without an accessible-id (`AttrsIter` slot 2, `:1284-1288`; value `:1755`) | `id()` is the process ID (`fn id` `:346-348` → `node_id` `:1959-1970` → `application_id` `:1972-1982`); `@Id` is the root's accessible-id or `""` |
| Java agent | `Element::stable_id()`: `Component.getName()` for components and windows, set only when AWT's `nameExplicitlySet` is true (`crates/provider-java/src/agent/element.rs:253-267`, `java/agent/.../SwingElement.java:389-426`); accessor and attribute agree | none (`crates/provider-java/src/agent/app.rs`) |
| Java Access Bridge | none (spec `jab-provider`: "`control:Id` SHALL never be emitted") | none |
| Mock | none; the mock tree's `control:AutomationId` on two windows is a free attribute, not `Id` | none |
| macOS | stub, lists no elements | — |

**AT-SPI reads.** **Verified.**
- `UiNode::attribute` runs the attribute iterator and returns the first match (`node.rs:443-452`). It already switches off the `app:*` block for names outside it (`attrs.process.process_table = None`), because that block decides presence by reading.
- Slot 3 (`Description`) is gated on a read; the gate is cached on the iterator's context (`:1289-1302`).
- Gating slot 2 on the accessible-id therefore puts a read in front of every enumeration, and in front of every named lookup that walks past slot 2, unless the lookup is told not to.

**Tests.** **Verified.**
- The rule as it stands is pinned by `crates/provider-atspi/src/node.rs:2508-2529`, the three `application_id` tests of the archived `atspi-process-identity`. On UIA it is pinned by tests of `application-process-attributes` in `crates/provider-windows-uia/src/node.rs`:
  - `an_application_node_is_named_after_its_process_name` (`:2464-2475`) asserts that `control:Id` equals `app:ProcessName`;
  - `a_nameless_application_node_keeps_its_process_id_and_common_attributes` (`:2397-2421`) asserts that `control:Id` is absent because the name is ("no name, so no developer id").
- `atspi-application-level` (`9ce921b`) added two `node_id` tests at `:2542-2556`. Below the level the accessible-id answers (`below_the_application_level_the_id_is_the_accessible_id`). At the level the process ID answers (`at_the_application_level_the_process_is_reported`, which also asserts `process_attributes_at`).
- The pidns harness records and asserts `{peer}.id` (`crates/provider-atspi/src/pidns_harness.rs:145`, and in `assert_unresolved` `:304-311`).
- The contract testkit treats `control:Id` as optional and never compares it with `id()` (`crates/core/src/ui/contract/testkit.rs:131-146`, `:163-192`).
  - Its callers are the mock (`crates/provider-mock/src/tests.rs:209`), the UIA desktop root (`crates/provider-windows-uia/src/node.rs:2113-2117`) and the Java agent's window subtree (`crates/provider-java/tests/live_fixture.rs:1056`).
  - None of them reaches an application node or an AT-SPI node.
- No acceptance suite reads an application's `id` or selects `app:*` by `@Id`. The suites address widgets by `@Id` only where egui sets an author id, and applications by `@ProcessId` only.

## Goals / Non-Goals

**Goals:**

- One decision per provider answers both the accessor and the attribute, so they cannot drift apart again.
- AT-SPI decides the `Id` without a bus for the application level, and with exactly the read it needs below it.
- The rule is guarded where each provider can be reached: provider unit tests, the contract testkit, and the egui acceptance suite on every real lane.

**Non-Goals:**

- Changing where an identifier comes from on any provider (D1), or filtering names the toolkit sets itself.
- Giving the mock provider an `Id` model. The mock is deliberately partial; the real providers and lanes are authoritative here.
- Making the other gated slot (`Description`) cheaper for named lookups. That is observed below (Risks), not changed.
- The runtime id, `native:` attributes, and AT-SPI's `native:Application.Id`: all unchanged.

## Decisions

### D1: The Id is taken by source

A provider reports what its toolkit's automation-identifier source holds, whoever put it there. This includes:

- Qt's generated object path;
- Avalonia's `PART_*` template names;
- Swing's own `setName` calls (`null.contentPane`, `Spinner.nextButton`, the non-unique `OptionPane.button`);
- AT-SPI's object-attribute fallback, including a bare `id` key such as a web engine's DOM id.

Alternatives considered:

- **Filter toolkit-set names.** For Swing this would take a list of `JRootPane` and plaf names, or a guess about which class called `setName`. Every other toolkit would need its own list. It breaks the one property a locator needs, that `@Id` shows what the toolkit shows in its own tools, and the maintainer ruled against it.
- **Drop the AT-SPI object-attribute fallback.** Toolkits that expose their id only there would lose it; there is no gain.

### D2: The application level carries no Id, on every provider

An application node's identity is `@ProcessId` (spec `sidecar-deployment`). Applications have no automation identifier on any toolkit:

- UI Automation, JAB and the Java agent build the application node themselves.
- On AT-SPI the root object's accessible-id, where a toolkit sets one, is no automation identifier of the application. Qt reports its application name there, or the class name `QApplication` when the application sets none.

So the application level answers none, even when the root reports an accessible-id, which stays visible as `native:Accessible.AccessibleId`.

Alternatives considered:

- **Delegate `@Id` to the process ID**, as the accessor does today on AT-SPI. That reports one fact under two names, which `application-process-attributes` forbids. A process ID is also neither set by a developer nor stable across runs.
- **Use the root's accessible-id on AT-SPI.** That is a different value per toolkit, often empty and never unique among applications. The spec `id-attribute` scenario *An application root's own identifier is not its Id* pins the decision.

### D3: AT-SPI: one pure decision for accessor and attribute

A pure function of the level and the accessible-id read decides the `Id`:

- at `Level::Application` it answers none and does not ask;
- below it, it answers the read.

`AtspiNode::id()` calls it with `resolve_id`. The attribute iterator calls it through its context's cached `resolve_id`, and lists slot 2 only when it answers a value, which is then the attribute's value. `node_id` and `application_id` are removed; `process_attributes_at` (from `atspi-application-level`) stays.

This replaces the Id half of `atspi-application-level`'s D4. There the level decided between the process ID and the accessible-id; now the level decides between nothing and the accessible-id. Unit tests cover the decision without a bus, as they covered `application_id`.

`resolve_id` asks the object-attribute fallback when the `AccessibleId` read is empty, fails or times out (`node.rs:829-838`), and answers none only when that yields nothing as well. The decision then answers none, and the attribute is absent for that iterator; the next enumeration reads again.

Removing the process-ID input also removes the only caller of `AtspiNode::resolve_process_id` (`node.rs:308-311`, called only from `id()`), so it goes too. The doc comment of `LazyNodeData::resolve_process_id` (`:1564-1566`) no longer links it. `resolve_peer` stays for `process_attributes`. The pidns harness calls `application_id` in the same test binary (`crates/provider-atspi/src/lib.rs:17-18`, `pidns_harness.rs:145`), so its `{peer}.id` record goes in the same step. So do its assertion in `assert_unresolved` (`:310`) and that function's doc-comment clause about a node identifier (`:304-305`).

### D4: AT-SPI: a named lookup answers `Id` directly

`UiNode::attribute(Control, "Id")` answers from D3's decision without running the iterator. For every other name the iterator is told to skip slot 2's gate, the same way it already skips the `app:*` block. Only an enumeration of all attributes, or a lookup of `Id` itself, pays the accessible-id read. An `@Id` predicate in XPath pays it, as it does today when it reads the value.

The skip is carried like the `app:*` skip, as an `Option` (for example the level, with none meaning "skip slot 2"), not as another `bool`: `AttrsIter` already has three, and a fourth trips `clippy::struct_excessive_bools` under `just clippy`.

Alternative considered: **gate slot 2 and let named lookups walk past it.** That adds one or two bus calls (`AccessibleId`, then `GetAttributes` as the fallback) to every named lookup of a later attribute, `@Bounds` and `@IsEnabled` included.

### D5: UIA: the application node inherits the trait default

- `ApplicationNode::id()` is removed, so the trait default (none) applies.
- The `Id` slot leaves the application node's attribute listing (`crates/provider-windows-uia/src/node.rs:1452-1454`). Since `application-process-attributes`, `AppAttrsIter::next` skips an absent attribute (`:1446-1477`), but an index without an arm reaches `_ => return None` and ends the listing. So the later arms are renumbered; deleting the arm alone would hide `ProcessId` and everything after it. The iterator's `name()` helper (`:1435-1442`) serves only that slot and goes with it. The struct's doc comment (`:1396-1402`) then no longer names `Id` as a reason to read the process. The shared `name` cell stays, because the process read still seeds it.
- `application-process-attributes` gave `ApplicationNode` an `attribute()` override (its task 3.2, `:1854-1885`), and that override answers `control:Id` from the cached name (`:1872-1874`). That arm goes, so the lookup answers nothing, and the override's doc comment (`:1854-1856`) names only `Name` as answered from the cached name. Its test `an_application_node_is_named_after_its_process_name` (`:2464-2475`) keeps its `control:Name` assertion and asserts that `control:Id` is absent. The guarantee is task 2.3's assertions (`attribute(Control, "Id")` finds nothing, and the D7 check), not construction.
- `@Name` keeps the process name that `application-process-attributes` gives it (its design D9), and this change leaves that alone. Its D9 also calls that name the developer id; this change drops that half, as D3 drops the Id half of `atspi-application-level`'s D4.

### D6: Java agent, JAB and mock: no code change

They already follow the rule. The agent's docs are made precise about D1, because several call the name the developer's own:
- `crates/provider-java/src/agent/element.rs:226-227` and `:529` ("the developer's own identifier"), and `:253-257` ("which the application set deliberately");
- `java/agent/.../SwingElement.java:101-102` ("only when the developer actually set it"), the Javadoc of `explicitNameOf` (`:389-390`, "only when it is the developer's own name") and of `nameWasExplicitlySet` (`:418`, "set by the application").

All become "set through `setName`, by the application or by Swing itself". The Java comments change nothing the agent sends, so no agent version bump and no JAR rebuild is needed.

The agent reads `Component.nameExplicitlySet` reflectively; that read is the guarantee. If it is not readable, the agent's fallback drops the name for windows only (`SwingElement.java:406-416`), so a heavyweight AWT component could report its generated name (`button0`). The agent opens `java.awt` to itself, so the fallback is not expected to run. It stays best effort, and the docs say so rather than the agent changing.

### D7: The contract testkit checks the Id both ways

The testkit gains a check that, for any node:
- `control:Id` is listed exactly when `id()` is set, with the same non-empty value;
- a named lookup agrees with the listing;
- a node in the `app` namespace has neither.

It is wired where nodes are reachable in tests:
- the mock's application node, main window and button (`crates/provider-mock/src/tests.rs`, `contract_expectations_for_button_hold`);
- on Windows: the UIA desktop root, the test window, `ApplicationNode::orphan`, and one test-window button that gets a control ID. UIA's Win32 proxy reports the control ID as `AutomationId`, so the button carries an identifier, and the check covers both halves;
- the Java live fixture on Windows, ignored by default:
  - the JAB window tree (`live_fixture_contract_and_interaction`, `:317-338`);
  - the agent's window tree (next to `verify_common_attributes`, `:1056`);
  - both application nodes through `fixture_application`.

AT-SPI has no in-crate live tree, so D3's unit tests and the acceptance suite cover it.

### D8: Acceptance: the egui query suite, on every real lane

`tests/acceptance/egui/query.robot` runs in the X11, compositor and Windows lanes. It gets cases for:
- the application node pinned by `@ProcessId`:
  - `${app.id}` is `None`;
  - `Get Attribute … Id` fails with "attribute not found";
  - `count(@*[local-name()="Id"])` is `0`;
  - `${app.describe()}` carries no `#`;
- `/app:*[@Id]` selecting nothing;
- a fixture element without an author id: no `@Id`, no `${el.id}`, and no `Id` in its enumerated attributes. egui sets no author id itself, so the `Buttons` heading is the candidate; it is checked on both bridges first;
- `btn-click-me`: the enumerated `Id`, `@Id` and `${el.id}` agree.

`@*` enumerates through `attributes()`, while `@Id`, `Get Attribute` and `[@Id]` use the named lookup. So the cases reach both AT-SPI paths of D4, on real trees. The cases use no `native:` attributes. They follow the pattern of the existing `Widget Without A Description Has No control:Description` case.

## Risks / Trade-offs

- **[Users read an application's process ID through `${el.id}`]** It becomes `None`. → The release notes name it, and `@ProcessId` has been the documented identity since `atspi-process-identity`. No suite uses it. The docs that describe an application's id change with this change (tasks 4.1 and 4.2):
  - `dev-docs/platform-linux.md:134`, `:154` and `:157` for the process ID;
  - `dev-docs/platform-windows.md:77` and `dev-docs/architecture.md:577`, which `application-process-attributes` wrote, for the UIA application's `control:Id` as the process name.
- **[Log lines lose `#<pid>` / `#<program>` for applications]** A reader loses the process ID at a glance. → Action lines name the application; the process ID is one query away. The description form (spec `diagnostic-logging`) is unchanged.
- **[Locators relying on `@Id=""` or on `[@Id]` being true on AT-SPI]** They change meaning. → Named in the release notes. A locator that needs "has no id" writes `not(@Id)`, which now works on every provider.
- **[Enumerating all attributes on AT-SPI costs one or two more calls per node]** Examples are the Inspector's attribute pane, `@*` in XPath and `platynui-cli query`. → D4 keeps named lookups free of it. `Description` already costs the same in enumeration.
  - Observed, not changed: a named lookup of an attribute after slot 3 still pays the `Description` read today.
- **[The Java agent's fallback can report a generated AWT name]** Only when `nameExplicitlySet` cannot be read (D6). → Documented as best effort; not expected to run.
- **[The UIA part is Windows-only]** → Its unit tests and the Windows lane run on real Windows (the libvirt VM); Wine does not count. The Windows tasks stay open until they ran there.
- **[`application-process-attributes` left the UIA application's `control:Id` following its name]** Its code is on `main`. As agreed, it did not touch `ApplicationNode::id()`, which returns the node's name, and that name is now `app:ProcessName` (its design D9). So `control:Id` follows `ProcessName` through `id()`, `AppAttrsIter` slot 3 and the `control:Id` arm of its `attribute()` override. Its test `an_application_node_is_named_after_its_process_name` asserts that value. → This change removes all three paths and changes that test (D5). Its spec clause "a `control:Id` that carries the name SHALL carry the same value" is removed by this change's delta. That delta restates the requirement, so it is re-synced with the archived text once `application-process-attributes` is archived, before this change is archived.
- **[Overlap with `gate-uia-window-patterns` and `evaluate-pidns-tests-in-ci`]** They touch `ApplicationNode` construction and the pidns harness. → Whichever lands later rebases; neither depends on the `Id`.

## Migration Plan

- **The change is behavioral, not additive.** It changes what `id()` and `@Id` report on AT-SPI and UIA application nodes, and on AT-SPI nodes without an accessible-id (proposal: Behavior changes that users see).
- **It needs a native rebuild** (`just build-native`), because the providers are linked into `packages/native`.
- **There is no configuration and no stored data.** Rolling back means reverting the change.
- **Order:** on top of the code of `atspi-application-level` (`9ce921b`: the level marker and `node_id`) and of `application-process-attributes` (`3c151f1` to `4a07527`: the UIA `attribute()` override and the skipping `AppAttrsIter`), both on `main`. `application-process-attributes` is archived before this change, since its archived spec is what the second delta modifies.
