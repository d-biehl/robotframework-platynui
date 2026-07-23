# Testing Strategy

<!-- Living document. For history see CHANGELOG.md and git log. -->

> **Scope of this document.** This is the *strategy*: what we test, **how**, and
> **where**, and how tests should be built. It is normative and meant to stay stable as
> code lands. It deliberately does **not** track counts, coverage, or a backlog of what
> is not yet covered — CI and the issue tracker own current status.

## 1. Philosophy

PlatynUI is tested in **two complementary lanes**. Neither is "more correct"; they
answer different questions, and most behavior wants both.

- **Mock lane — fast, deterministic, cross-platform feedback.** The mock
  provider/platform (`platynui-provider-mock` + `platynui-platform-mock`) is
  **deliberately partial**: kept simple on purpose so logic and wiring can be exercised
  quickly, deterministically, and on any OS without a real desktop. It is the default
  lane while developing, and it is meant to be **extended** when a behavior needs
  covering (§4). A green mock test proves *logic and wiring* — not real-platform behavior.

- **Acceptance lane — real platforms, no mock.** The mock cannot stand in for real
  accessibility providers, real input, timing/waits, or OS quirks. Behavior that depends
  on the platform must **also** be proven by tests that run **without the mock**, against
  the real provider (UIA / AT-SPI / AX), on the actual platform(s) and across platforms.

This leads to a few standing rules:

- **Prove behavior at the lowest layer that can**, then add the acceptance test for the
  platform-specific part. Don't reach for a heavyweight layer when a cheaper one suffices.
- **For attribute namespaces/values, node liveness, or real input, the source of truth
  is the real provider or the PlatynUI Inspector — never the mock** (§7 says how). The
  mock deliberately simplifies some of this; that is expected, not something to rely on.
- **Tests are behavior-first.** A suite or scenario states the expected behavior; the
  implementation makes it pass. Add the test with (or before) the change, not after.
- **For the Robot Framework library, write the keyword test first.** When a change adds or
  reshapes a `PlatynUI.BareMetal` keyword, write its RF test before implementing the
  keyword — a mock-lane suite (§2.5), plus an acceptance suite when the behavior needs a
  real platform (§2.6). Exercising the keyword and its arguments *from Robot's side* first
  is how an awkward API surfaces before it is built, not after; the implementation then
  makes the test pass.
- **The mock is dev-only.** Neither the mock nor the `use_mock` argument appears in
  user-facing documentation (it is a normal part of these contributor-facing test docs).

## 2. The test layers

Every layer below states **what it is for**, **where it lives**, **how a test there is
built**, and **what it needs to run**. Pick the lowest layer that can demonstrate the
behavior under test.

### 2.1 Rust unit — pure logic

- **For:** deterministic logic with no platform dependency — the XPath engine, value
  types, parsing, model invariants.
- **Where:** in-crate `#[cfg(test)]` modules; per-crate integration tests under `tests/`.
- **How:** `rstest` for fixtures/parametrization; small, deterministic, no I/O. Use
  property-based tests where the input space is large (the XPath engine does).
- **Needs:** nothing special — the mock crates are unconditional `dev-dependencies`.

### 2.2 Rust integration — runtime, CLI, providers

- **For:** behavior of the runtime and CLI commands, and provider/platform conformance.
- **Where:** the `runtime`, `cli`, `platform-*`, and `provider-*` crates.
- **How:** drive through the **mock** platform/provider. Assert *effects* via the mock
  platform's introspection logs in `platynui-platform-mock` (pointer, keyboard, highlight,
  window-manager, and screenshot logs). Assert *provider conformance* with the shared
  `core::ui::contract::testkit` — written provider-agnostic by design, so the same
  pattern/attribute contract suite runs against the mock and against live providers.
- **Needs:** nothing special (mock via dev-deps). The CLI gets a mock backend through the
  same opt-in `mock-provider` cargo feature the native package uses — it is a build
  feature, not a runtime flag.

### 2.3 Python — native bindings

- **For:** the PyO3 surface — runtime, geometry types, iterators, overrides, value
  semantics, screenshots, focus.
- **Where:** `packages/native/tests`.
- **How:** pytest driving the **native mock provider**. Acquire the runtime through the
  shared fixture (`rt_mock_platform` in `packages/native/tests/conftest.py`), which builds
  it via `Runtime.new_with_mock()` and always shuts it down in teardown — follow that
  pattern rather than constructing a runtime ad hoc.
- **Needs:** a `mock-provider` native build (`just test-python` builds it first). Without
  it, `Runtime.new_with_mock()` raises `ProviderError`.

### 2.4 Python — high-level library

- **For:** the logic of the high-level `PlatynUI.ui` control model (buttons, lists, tree,
  combobox, table, tabs, menus, text, window, …) and its adapter/proxy/pattern machinery.
- **Where:** `tests/PlatynUI`.
- **How:** pytest against **pure-Python pattern stubs** — no native code and no provider.
  Build the unit under test with the `make_adapter` factory and the pattern stubs in
  `tests/PlatynUI/_ui_helpers.py` (`ElementStub`, `ActivatableStub`, `SelectableStub`, …),
  then assert on the library's behavior via stub call counts and predicates. This isolates
  library logic from the runtime.

### 2.5 Robot Framework — mock lane

- **For:** deterministic, cross-platform behavior of the RF keyword surface
  (`PlatynUI.BareMetal`) against the mock tree.
- **Where:** `tests/BareMetal`; tagged `mock`, selected by the `mock` profile.
- **How:** import the library in mock mode and tag the suite so the profile picks it up —
  `Library  PlatynUI.BareMetal  use_mock=${True}` (pass import-time `query_settings` here
  too), with the `mock` tag set on the suite or its `__init__.robot`. Write behavior-first:
  state expectations as assertions — query results, attribute values, focus, scoping
  (`Set Root`), query settings, explicit waits — and cover error paths too (§7).
  Deterministic, no display required.
- **Needs:** a `mock-provider` build (`just test-baremetal`).

### 2.6 Robot Framework — acceptance lane

- **For:** proving the full stack on a real platform — real provider, real input, real
  timing — what the mock cannot reproduce.
- **Also guards per-runtime isolation.** Because each suite builds and tears down its own
  runtime, a *multi-suite* run exercises what a single suite cannot: a runtime constructed
  after an earlier one has been dropped must establish a fresh platform connection. This is
  the guard for the per-runtime platform architecture — the mock cannot stand in for it
  (it shares process-global state and holds no real connection), so the real `…-x11` /
  `…-compositor` lanes run *several* suites in one process on purpose.
- **Where:** `tests/acceptance` (egui app); tagged `real`, selected by the `real` profile.
- **How:** unlike a mock suite (static tree, no setup), an acceptance suite **owns its
  app instance**: launch it in `Suite Setup` and terminate it in `Suite Teardown`, pin the
  window root by `ProcessId` (not title, to survive stacking/ambiguity), and keep launch
  and locator keywords in a shared page-object resource
  (`tests/acceptance/egui/resources/testapp.resource`) rather than inline. Assert on
  observable results through the real provider; use poll-based waits (§7) and `platform:*`
  tags for selective runs.
- **Needs:** the **non-mock** native build and an isolated session via
  `scripts/platynui-robot-session.sh` (`just test-acceptance*`).

> Not a strategy layer: `tests/playground` holds manual `.robot` experiments against
> pre-installed desktop apps. They have no assertions and are not run in CI — a scratchpad,
> not part of any lane.

## 3. Where tests live, and how to run them

```
crates/**/src (#[cfg(test)]), crates/*/tests   Rust unit + integration
packages/native/tests                          Python — native bindings (native mock)
tests/PlatynUI                                  Python — high-level library (stubs)
tests/BareMetal                                 RF mock lane      (tag: mock)
tests/acceptance                                RF acceptance lane (tag: real)
tests/playground                                manual experiments (no CI)
```

| Goal | Command |
|---|---|
| Rust suite | `just test` · `just test-crate <pkg>` |
| Python bindings + RF mock | `just test-python` · `just test-baremetal` |
| RF acceptance (real provider) | `just test-acceptance` (this OS) · `…-compositor` / `…-x11` (Linux) · `…-windows` |
| Full local mock gate | `just pre-commit` |

`just`/`CONTRIBUTING.md` are the source of truth for exact recipe names; the durable point
is the **build duality**: the mock and real lanes need **different native builds and
cannot share one** — a `mock-provider` build makes `Runtime()` resolve the built-in mock
tree instead of the real desktop. `mock-provider` is opt-in, never a default feature. So a
lane's tag pairs with its build — `mock` ↔ mock-provider build, `real` ↔ plain build — and
each lane runs as its own job. Acceptance suites must support headless execution so they
can run in CI.

## 4. The mock provider

The mock is a complete, deterministic *simulation* with introspection logging, not a real
backend. It is the right tool for logic, wiring, keyword behavior, and anything that must
run identically on every OS.

**The shared fixture.** All three mock-driven layers (§2.2, §2.3, §2.5) query one shared
tree, `crates/provider-mock/assets/mock_tree.xml`. Because suites depend on specific nodes
in it, treat it as a shared fixture: **prefer adding new nodes over mutating existing
ones**, and when a change to the tree is unavoidable, update every affected suite in the
same change.

**Deliberately partial, and designed to grow.** When a behavior needs mock coverage,
extend the mock rather than skipping the test: add the pattern's trait in
`core::ui::pattern`, implement it in the mock following the shape of
`crates/provider-mock/src/focus.rs` or `window.rs`, and add the supporting elements to the
mock tree.

**The hard boundary.** Some things the mock cannot cover by design, and which therefore
belong in the acceptance lane: real input actually changing an application, real
timing/wait behavior (the mock responds instantly), live provider events, and platform
quirks (UIA COM lifecycle, AT-SPI2 D-Bus timing, AX permissions).

## 5. Testing against real UIs

The acceptance lane needs real applications that expose themselves to the OS accessibility
APIs. Because UI toolkits implement accessibility differently (different bridges, tree
shapes, and pattern coverage) — and because the semantic keyword layer will ship
per-technology proxies that must be proven against each toolkit — PlatynUI maintains a
**fixture technology matrix**: one fixture app per UI technology, all implementing the
same blueprint.

- **Blueprint conformance.** Fixture apps implement the fixture blueprint defined in §5.1
  below: a tiered control catalog under canonical names, name-based action observables, a
  common CLI contract, and onboarding to the shared catalog acceptance suite.
  Scenario-specific surfaces (e.g. the Qt Widgets dialog-bounds reproduction) live beside
  the catalog, not inside it.
- **Current matrix.** egui (`apps/test-app-egui`) — Rust/AccessKit — stays the
  **lightweight smoke target**: it renders its own widgets and exposes a flatter tree with
  a limited role set (details in the
  **[egui Accessibility API Guide](egui-accessibility-guide.md)**), is trivial to run in
  CI, and behavior reproducible on that tree belongs there. Qt Widgets (`apps/test-app-qt`,
  PySide6 — chosen for its LGPL license) is the **native-widget tier**: real native
  controls with full patterns. Swing (`apps/test-app-swing`) covers the Java Access Bridge
  surface. Qt Quick/QML (`apps/test-app-qml`) is the **first full blueprint instance**:
  scene-graph rendering, `Accessible` attached properties, in-scene popups — and the home
  of the shared catalog suite's reference onboarding (`tests/acceptance/qml/catalog.robot`).
  Planned rows: SWT, JavaFX, and later WPF, Avalonia, and native Win32. Fixtures that
  predate the blueprint conform after their per-app retrofit changes; until then their
  historic names and suites stay valid.
- **Choosing a target.** Prove behavior on the cheapest row that can show it: egui for
  provider smoke, a native-widget or toolkit-specific fixture when the behavior needs that
  technology's real controls or bridge. Per-technology lanes remain separate CI jobs, so a
  lane needs only its own fixture built.

### 5.1 The fixture blueprint

The blueprint is the contract every fixture app implements. The `test-app-blueprint`
capability spec carries it as testable requirements (one scenario per acceptance
criterion); this section is the developer-facing description — keep both in sync when the
blueprint evolves.

**Tiered control catalog.** The catalog derives from the pattern families of the semantic
keyword layer (`python-library-design.md` §5a). The **core tier** is mandatory for a
conforming fixture; the **extended tier** is adopted incrementally. Canonical names are
kebab-case, pairwise-unique app-wide, and identical across technologies — one locator set
drives every fixture. The accessible **name** is the locator contract; technology-private
IDs (AutomationId, objectName) are never relied on.

| Tier | Control | Canonical names |
|---|---|---|
| core | main window | `main-window` (title from `--title`) |
| core | push button + status label | `button-basic`, `status-label` |
| core | checkbox | `checkbox-basic` |
| core | group box + radio group | `groupbox-basic` (title = name), grouping `radio-first`, `radio-second` |
| core | single-line text field | `textfield-basic` |
| core | multi-line text area | `textarea-basic` |
| core | static label | `label-basic` |
| core | plain static text | `text-basic` |
| core | image | `image-basic` |
| core | combo box | `combobox-basic`, items `combo-item-1`…`combo-item-3` |
| core | list | `list-basic`, items `list-item-1`…`list-item-5` |
| core | tree (3 levels) | `tree-basic`; `tree-node-a`/`tree-node-b`, `tree-node-a-1`/`tree-node-a-2`, `tree-node-a-1-i` |
| core | menu bar (3 menus, 1 submenu) | `main-menubar`; `menu-file` (`menu-file-new`/`-open`/`-quit`); `menu-edit` (`menu-edit-undo`/`-redo`; submenu `menu-edit-more` with `menu-edit-sub-one`/`-two`); `menu-help` (`menu-help-about`) |
| core | context menu + submenu | `context-menu`, items `ctx-cut`/`ctx-copy`/`ctx-paste`; `ctx-more` with `ctx-sub-alpha`/`ctx-sub-beta` |
| core | dialogs (modeless, modal) | `dialog-modeless`, `dialog-modal`; each with `<ident>-button`, `<ident>-label` |
| extended | table/grid | `table-basic`, cells `table-cell-<row>-<col>` (1-based) |
| extended | slider / progress bar | `slider-basic`, `progress-basic` |
| extended | tabs | `tabs-basic`, tabs `tab-one`/`tab-two` |

Names are stable: additions never rename or repurpose existing catalog names. Deliberate
gaps (role variants whose patterns a catalog control already exercises — Link → button,
ToggleButton → checkbox, PasswordBox → text field, Spinner → slider — and scrolling
surfaces, whose `Scrollable` pattern is post-Phase-4 in the library design) are closed
later by adding controls under new names. Shared catalog
locators address controls by `@Name` **alone** — roles differ across bridges (Qt Quick
exposes lists/trees as `Group` and tree rows as `ListItem`) and are not part of the shared
contract. Where a technology derives a *window's* name from its title and the name cannot
be set independently (Qt Quick), the main window is matched via the launch configuration
instead of the `main-window` name, and dialog child windows carry their canonical name as
their title.

**Observables.** Every catalog action has an effect observable through the tree by name or
text — never by screenshot: activating `button-basic` updates `status-label` to end with
`clicks-<n>`; activating a menu/context-menu item renames it to `<ident>-activated`;
activating a dialog button renames it to `<ident>-button-clicked` (which doubles as the
bounds-correctness proof that a click landed inside the dialog). State-bearing controls
(toggle, selection, expansion, value) expose state via the provider's read attributes;
where a bridge provably drops a state, the fixture adds a name-based observable and
documents the deviation in its README.

**CLI contract.** `--title <text>` (default `PlatynUI <Technology> TestApp`),
`--auto-close <seconds>`, `--open-modal` (opens `dialog-modal` at startup so modal state
needs no interaction; where a bridge exposes no modal state — Qt Quick's in-scene `Dialog`
on UIA — the deviation rule applies and presence + interactability are the asserted
facts); unknown arguments fail with a usage message and non-zero exit.
Technology-specific flags (e.g. `--app-id`, `--popup-mode`) are allowed, but defaults
always produce the conforming catalog.

**Shared catalog suite.** Catalog tests are written once as technology-neutral keywords
under `tests/acceptance/resources/` (Given/When/Then, canonical names only). Each
technology onboards with a thin `tests/acceptance/<tech>/catalog.robot` that supplies only
launch configuration (`PLATYNUI_TEST_APP_<TECH>_*` env vars) and declares the tests — no
per-technology variable files. A technology limitation is an explicitly skipped test whose
message names the limitation and where it is tracked; core-tier behavior is never silently
absent. Before a suite encodes a name or state, verify it against the real tree
(Inspector / `Get Attribute`) per §7.

**Custom-controls chapter (optional).** A fixture may add hand-built controls to probe the
default-proxy lower bound: `custom-button` (self-drawn, manually wired accessibility,
counter on `custom-status-label`) and one deliberately non-exposed drawn element whose
absence from the tree is asserted as expected behavior.

## 6. CI

Tests are written to be CI-gated: **every automated lane runs in CI**. Because of the
build duality (§3), the mock and real lanes run as **separate jobs**. A direct consequence
for how live suites are built: any acceptance suite must support headless execution and
self-contained session setup so it can run unattended in CI. The CI configuration owns the
concrete job list; the strategy only requires that no lane is left ungated.

## 7. Conventions for writing tests

- **Behavior-first and deterministic.** State the expected behavior as assertions. Mock
  layers are instant; live layers must not race — wrap any assertion that can race in a
  poll (`Wait Until Keyword Succeeds` / `Wait Until …` with an interval and a generous
  timeout, and gate readiness by polling the accessibility tree). Never use a bare `Sleep`.
- **Test error paths, not just happy paths.** Asserting on failures is first-class: in RF
  mock suites use `Run Keyword And Expect Error` against the user-facing message (with
  short timeouts), and cover per-call vs scoped override precedence and rejected/invalid
  selectors; in Rust/Python assert the typed error. `tests/BareMetal/wait_keywords.robot`
  is a good model.
- **Lowest layer first.** Prove logic in Rust/Python/RF-mock; reserve the acceptance lane
  for what only a real platform can show.
- **Tag by build and platform.** `mock` / `real` select the lane; `platform:*` scopes
  platform-specific acceptance suites.
- **Verify platform facts against reality.** Before encoding an attribute namespace/value
  or a liveness expectation, read the *actual* value from a real target — run the
  **Inspector** (see [dev-docs/inspector.md](inspector.md)) or a `Query` / `Get Attribute`
  against a non-mock build of the running app — and assert that exact value. Do not infer
  it from the mock tree.
- **Robot Framework:** keyword names in Title Case; return values instead of `print`.
- **Mirror spec scenarios as Given/When/Then.** A delta-spec scenario is the acceptance
  criterion; realize it as one test, one behavior per test. New RF acceptance suites write
  that flow in Robot Framework's Given/When/Then (BDD) style — RF ignores a leading
  `Given`/`When`/`Then`/`And`/`But` when matching keywords — so the test reads as the
  scenario. This is RF-native BDD, **not** full Gherkin (no `.feature` files).
- **Keep the mock out of user docs.** It is a development tool only.

## 8. Decisions

| Choice | Decision |
|---|---|
| Native Qt binding | **PySide6** (LGPL; functionally equivalent to PyQt6/GPL) |
| Lane selection | `robot.toml` profiles `mock` / `real`, plus `platform:*` tags |
| Mock vs real builds | Separate native builds per lane; one CI job each |
| Live-test waits | Poll-based (`Wait Until …`) — preferred for robustness against timing variation |
