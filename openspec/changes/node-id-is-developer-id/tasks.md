# Tasks

The behavior changes on AT-SPI (Linux) and UI Automation (Windows). Provider unit tests and the Linux lanes run here. The UIA unit tests, the Java live fixture and the Windows lane run only on real Windows; Wine does not count. Those tasks stay open until they ran there.

## 1. Prerequisites

- [ ] 1.1 Start from a tree that contains the code of both changes this one builds on: `atspi-application-level` (`9ce921b`) and `application-process-attributes` (`3c151f1`, `bfd9297`, `6c85cb8`, `ca4a765`, `ca499f1`, `4a07527`). `application-process-attributes` stays unarchived until its Windows tasks 3.2, 3.3 and 7.1 have run (see 6.1).
  - That change left `ApplicationNode::id()` alone, as agreed: the UIA `Id` is the node's name, which now comes from `ProcessName` (its design D9), so `control:Id` follows it. This change removes that `Id` in 2.3 and 3.3.
  - Its `ApplicationNode` answers named lookups in its own `attribute()` (`crates/provider-windows-uia/src/node.rs:1854-1885`), and answers `control:Id` there from the cached name (`:1872-1874`). Task 3.3 removes that arm.

  Verify: `git log` shows those commits below this branch, and `just test-crate platynui-provider-atspi` and `just check` are green before any edit.

## 2. Tests first

- [ ] 2.1 In `crates/provider-atspi/src/node.rs` (tests module), replace five tests with a table test of the `Id` decision of design D3:
  - the three `application_id` tests (archived `atspi-process-identity` task 2.7, `:2508-2529`);
  - the two `node_id` tests of `atspi-application-level` (`:2542-2556`). Keep the process-attribute assertion of `at_the_application_level_the_process_is_reported` as a test of its own.

  The rows:
  - **At the application level**: one row whose read closure panics if it is called; its would-be value is `QApplication`. It answers none.
    - The decision takes no process ID, so *An application without a process ID has no Id either* holds by construction.
    - It does not read the accessible-id at that level, so neither does *An application root's own identifier is not its Id*.
    - The row pins both.
  - **Below the application level**:
    - the read `btn-ok` answers `btn-ok`; this replaces `below_the_application_level_the_id_is_the_accessible_id` and keeps its spec comment;
    - a read that answers nothing answers none (*An identifier that does not answer in time is absent*, *An empty accessible-id is no Id*).

  Verify: `just test-crate platynui-provider-atspi` fails to compile before 3.1.
- [ ] 2.2 In `crates/core/src/ui/contract/testkit.rs`, add the `Id` check of design D7:
  - `control:Id` is listed exactly when `id()` is set, with the same non-empty value;
  - the named lookup agrees with the listing;
  - a node in the `app` namespace has neither.

  Unit-test the check itself with stub nodes: agreeing, disagreeing, empty value, and an `app` node with an `Id`. In `crates/provider-mock/src/tests.rs` (`contract_expectations_for_button_hold`), run it on the mock's application node, main window and button.

  Verify: `just test-crate platynui-core` and `just test-crate platynui-provider-mock` are green, since the mock has no `Id` anywhere.
- [ ] 2.3 In `crates/provider-windows-uia/src/node.rs` (tests module, Windows only):
  - Change `an_application_node_is_named_after_its_process_name` (`:2464-2475`), which `application-process-attributes` added for its design D9. It keeps the `control:Name` assertion, and its `control:Id` lookup now finds nothing; its doc comment drops "and its developer id is the same name". In `a_nameless_application_node_keeps_its_process_id_and_common_attributes`, the message of the `Id` assertion (`:2416`, "no name, so no developer id") states the rule instead: an application node has no `Id`.
  - Extend `application_node_carries_the_common_attributes`:
    - `id()` is none;
    - no `control:Id` is listed, and `attribute(Control, "Id")` finds nothing;
    - `platynui_core::ui::describe` of the node carries no ` #`;
    - `ProcessId` is still listed.
  - Give one test-window button a control ID, for example `1001` as the `hMenu` of its `CreateWindowExW`. Assert that enumeration, `attribute(Control, "Id")` and `id()` all return it (*Enumeration and lookup agree*).
  - Run the D7 check on `ApplicationNode::orphan`, the desktop root, the test window and that button.

  Verify on real Windows: `just test-crate platynui-provider-windows-uia` fails the application-node assertions before 3.3.
- [ ] 2.4 In `crates/provider-java/tests/live_fixture.rs` (Windows only, `#[ignore]`d), run the D7 check on:
  - the JAB window tree, in the node loop of `live_fixture_contract_and_interaction` (`:317-338`);
  - the agent's window tree, next to `verify_common_attributes` (`:1056`);
  - both application nodes from `fixture_application`, where `id()` is also none (*Java application nodes have no Id*).

  Through the agent, locate the increment button structurally, as a child of the node named `stage2-spinner`, and assert its `@Id` is `Spinner.nextButton` (*An identifier Swing sets itself counts as the Id*). *A table cell has no Id* is covered by the accessor assertion at `:902` and by the D7 check over the agent's tree, which includes the cells.

  Verify on real Windows, after `just build-java-agent` and `just build-test-app-swing`, with `PLATYNUI_TEST_APP_SWING_JAVA` pointing at the Java 8 launcher as `just test-acceptance-windows` sets it: `cargo nextest run -p platynui-provider-java --run-ignored ignored-only` passes. These nodes already follow the rule.
- [ ] 2.5 In `tests/acceptance/egui/query.robot`, following the `robot-test-style` skill, add four cases. None uses `native:` attributes. `@*` enumerates through `attributes()`; `@Id`, `Get Attribute` and `[@Id]` use the named lookup.
  - *Application Has No Id*: pin the application with `/app:Application[@ProcessId=${pid}]`, where the process ID comes from `${TEST_APP_HANDLE}`.
    - `${app.id}` is `${None}`.
    - `Get Attribute … Id` fails with "attribute not found", as in `Widget Without A Description Has No control:Description`.
    - `count(@*[local-name()="Id"])` on it is `0`.
    - `${app.describe()}` contains no `#`.
  - *No Application Is Selected By Id*: `/app:*[@Id]` selects nothing.
  - *Element Without An Author Id Has No Id*: no `@Id`, no `${el.id}`, and `count(@*[local-name()="Id"])` is `0`.
    - Use an element that carries no author id. egui sets none itself, so the `Buttons` heading is the candidate.
    - First check on X11 and, on real Windows, that it has no identifier on either bridge. Name the choice in the case's documentation.
  - *Author Id Is The Same Through Every Read*: for `btn-click-me`, the enumerated `Id`, `@Id` and `${el.id}` are all `btn-click-me`.

  Verify with `just headless=true test-acceptance-x11 --suite '*.Egui.Query'` before 3.1: the first three cases fail and the fourth passes. The reasons: the application's `${app.id}` is its process ID and its description ends in `#<pid>`, and AT-SPI lists `@Id=""` on the application and on the element without an author id. On Windows before 3.3, the first two cases fail, because of the process name.

## 3. Implementation

- [ ] 3.1 In `crates/provider-atspi`, implement design D3 and D4:
  - `src/node.rs`:
    - A pure decision of the level and the accessible-id read. `AtspiNode::id()` calls it with `resolve_id`.
    - Remove `node_id` and `application_id` with their doc comments.
    - Remove `AtspiNode::resolve_process_id`, whose only caller was `id()`, and reword the doc comment of `LazyNodeData::resolve_process_id` that links it. `resolve_peer` stays for `process_attributes`.
    - The attribute iterator carries the level. It lists slot 2 only when the decision answers a value, which is then the value (no `unwrap_or_default`).
    - `UiNode::attribute` answers `control:Id` from the decision directly. It tells the iterator to skip slot 2 for every other name, as an `Option` like the existing `app:*` skip, not as a fourth `bool`.
  - `src/pidns_harness.rs`: drop the `{peer}.id` record (`:145`), its assertion in `assert_unresolved` (`:310`) and the doc-comment clause about a node identifier (`:304-305`). The harness is compiled into the same test binary.

  Verify: 2.1 passes, and `just test-crate platynui-provider-atspi` and `just check` are green.
- [ ] 3.2 Run the pidns harness where the machine provides the daemons: `just test-atspi-pidns dbus-daemon` and `just test-atspi-pidns dbus-broker`. Verify: both pass, or the run notes say which daemon is missing on this machine.
- [ ] 3.3 In `crates/provider-windows-uia/src/node.rs`, apply design D5 to the application node as `application-process-attributes` left it:
  - Remove `ApplicationNode::id()` (`:1824-1827`).
  - Remove the `Id` slot of `AppAttrsIter` (`:1452-1454`) without ending the listing early. `next()` already skips an absent attribute (`:1446-1477`), but an index without an arm reaches `_ => return None`. So renumber the later arms, or `ProcessId`, the `app` attributes, `Technology` and `SupportedPatterns` stop being listed.
  - Remove `AppAttrsIter::name()` (`:1435-1442`), whose only caller is that slot, and reword the struct's doc comment (`:1396-1402`): the process is read for the first `app` attribute only.
  - Remove the `common::ID` arm of `ApplicationNode::attribute()` (`:1872-1874`), so that `Id` falls through to `_ => None`. Reword the method's doc comment (`:1854-1856`): only `Name` answers from the cached name. `IdAttr` stays, because `UiaNode` uses it.

  Verify:
  - `just check-windows` and `just clippy-windows` are clean;
  - on real Windows, 2.3 passes and `just test-crate platynui-provider-windows-uia` is green.
- [ ] 3.4 Make the contract docs state the rule. Verify: `just check`.
  - `crates/core/src/ui/node.rs:19-20`: `id()` is the identifier the toolkit reports, by source. It is none when there is none, never `Some("")`, always none on application nodes, and always equal to `control:Id`.
  - `crates/core/src/ui/attributes.rs`:
    - `common::ID` (`:8`): "the identifier the toolkit reports; absent when unset, never on application nodes", instead of "developer-provided".
    - `application::PROCESS_ID`: an application node's identity.
  - `packages/native/src/runtime.rs:71-75` and `packages/native/python/platynui_native/_native.pyi` (`UiNode.id`): the toolkit's identifier, the same value as `@Id`, `None` when unset and always `None` for applications.
- [ ] 3.5 Make the Java agent's id docs precise (design D6): "set through `setName`, by the application or by Swing itself". The reflective read is the guarantee, and the fallback is best effort.
  - `crates/provider-java/src/agent/element.rs:226-227`, `:253-257` and `:529`.
  - `java/agent/src/main/java/platynui/agent/SwingElement.java:101-102`, and the Javadoc of `explicitNameOf` (`:389-390`) and of `nameWasExplicitlySet` (`:418`).

  Comment only: no agent version bump, no JAR rebuild. Verify: `just check`.
- [ ] 3.6 Run `just build-native`, so the lanes use the new providers. Verify: `just headless=true test-acceptance-x11 --suite '*.Egui.Query'` passes all four cases from 2.5.

## 4. Documentation

- [ ] 4.1 In `dev-docs/architecture.md`:
  - §5.1 (`:186`): "an optional developer-set stable id" becomes the identifier the toolkit reports.
  - §5.5 *Developer Id*: Id by source, whoever set it. Application nodes carry none, and `@ProcessId` is their identity. The attribute and the accessor agree.
  - §6.3 *Common Attributes*, the `Id` row (`:328`): the same source wording, and `app:` nodes carry none.
  - §6.4 *Pattern-per-Platform Attribute Mapping*:
    - The common-attribute row for `Id` adds JAB (none) and the Java agent (`Component.getName()` for components and windows).
    - The *Application* table (`:501-513`) states that there is no `Id` and that `ProcessId` is the identity.
  - §7.3 *Provider compliance checklist* (`:558`): never on application nodes, `id()` none rather than `""`, equal to `control:Id`. Its Windows line (`:577`) says today that `control:Name` and `control:Id` are the process name. It keeps `control:Name` and states that the UIA application node carries no `Id`.

  Keep it explanatory; no type signatures. Verify by reading against `specs/id-attribute/spec.md`.
- [ ] 4.2 Platform docs:
  - `dev-docs/platform-linux.md`:
    - `:117`: optional `Id` from the accessible-id, absent when not set, never on application nodes.
    - `:134`, `:154`, `:157`: drop `element.id` as the application's identity; `@ProcessId` alone.
  - `dev-docs/platform-windows.md`:
    - `:74-77`: the application node lists no `Id`. `:77` names `control:Id` as the process name today; it keeps `control:Name`.
    - `:93`: `Component.getName()` surfaces as `Id` on components and windows.

  Verify: `rg -n 'element\.id|control:Id' dev-docs` finds no statement that an application's id is its process ID or its process name.
- [ ] 4.3 In `dev-docs/planning.md`, by item text, since lines may shift before this change is applied:
  - Open Design Question 4, "Windows AUMID as Application Id": decided. Applications have no `Id`, and AUMID could at most become an `app:` attribute.
  - Reverse "[x] Windows/ApplicationNode: `id()` returns process name".
  - Close "Application nodes: platform-appropriate stable identifier" and "Windows option: evaluate AUMID as application Id" as decided.
  - Update the `Id` test items in §10.26 ("Core contract tests for `Id`" and the two after it) and in §7 ("UiNode `Id` tests …", "CLI/Python example queries for `Id` documented") to the tests of section 2.

  Verify by reading.
- [ ] 4.4 Replace `@AutomationId` presented as the developer-id locator key with `id=` / `@Id`:
  - the docstrings of `src/PlatynUI/core/locator.py`: `:17` keeps a free-form attribute that exists, and `:492`, `:496` and `:601` use `id=`;
  - the examples in `dev-docs/python-library-design.md`, which already opens with an English summary;
  - `dev-docs/python-migration-status.md:616`, adding a brief English summary at its top per AGENTS.md;
  - `apps/test-app-qml/README.md:75` and `:136-143`: `@Id` is absent, not empty, without `Accessible.id`.

  Verify: `just check` (ruff, mypy), and `rg -n 'AutomationId' src dev-docs apps/*/README.md` shows it only as a UIA source name or a `native:` attribute.

## 5. Verification

- [ ] 5.1 Run `just check` and `just test`. Both are green.
- [ ] 5.2 Run the Linux lanes one after the other, and inspect each right after its run with `uv run --no-sync robotcode results summary --failed`, because the next lane overwrites `results/output.xml`:
  - `just headless=true test-acceptance-x11`;
  - `just headless=true test-acceptance-compositor`.

  Both lanes are green, including the four cases from 2.5.
- [ ] 5.3 On real Windows (the libvirt VM, not Wine):
  - `just test-crate platynui-provider-windows-uia`;
  - the Java live fixture from 2.4;
  - `just test-acceptance-windows`, including the four cases from 2.5.

  Record the runs in the run notes. This task stays open until it ran there.
- [ ] 5.4 Check by hand in `scripts/startxsession.sh --backend headless -- <script>` with the egui and Qt test apps running. Use `target/debug/platynui-cli` from `just build`:
  - `platynui-cli query '/app:*'` lists no `@Id` line;
  - `platynui-cli query 'count(//*[@*[local-name()="Id"] = ""])'` is `0`;
  - `native:Accessible.AccessibleId` of the Qt application root still reads the root's own accessible-id; Qt reports `QApplication`.

  Record the output in the run notes.

## 6. Before archiving

- [ ] 6.1 Once `application-process-attributes` is archived, re-sync both delta specs with the specs they modify, changing only what this change changes:
  - `specs/application-process-attributes/spec.md` restates *Process attributes have fixed names and namespaces* from the archived text, minus the `control:Id` clause;
  - `specs/sidecar-deployment/spec.md` restates the current main requirement under its new name, minus the node identifier.

  Verify: `openspec validate node-id-is-developer-id --strict` reports the change valid, without the note that the `application-process-attributes` target spec does not exist.

## 7. Commit

- [ ] 7.1 When the maintainer asks, commit with a Conventional Commit subject of at most 72 characters, for example `fix(providers): report Id only as the toolkit's identifier`.
  - No `!` and no BREAKING footer, since PlatynUI is at 0.x.
  - The body lists every bullet of the proposal's *Behavior changes that users see*, because the changelog is generated from commit bodies (git-cliff), and names the testkit's new `Id` check as an addition.
  - Tests go in the same commit, because `test:` commits are left out of the changelog.

  Verify: `git log -1 --format=%B`.
