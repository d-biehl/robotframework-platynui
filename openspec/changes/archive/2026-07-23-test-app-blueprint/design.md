## Context

The repo has three fixture apps (egui, Qt Widgets, Swing) and two proposed ones (SWT, JavaFX), each scenario-driven with its own ad-hoc control set and naming. The semantic keyword layer designed in `dev-docs/python-library-design.md` (§5a: default proxies synthesizing actions via mouse/keyboard, plus technology-specific proxies shipped by PlatynUI and selected via the `@pattern_proxy_for` registry's `framework_id`/`class_name` weighting) needs a systematic acceptance surface: the same pattern behaviors proven per technology, with the observed differences driving which shipped proxies must exist. More technologies are planned (QML next; WPF, Avalonia, Win32 later). This change writes the blueprint those fixtures implement; it deliberately ships no code.

## Goals / Non-Goals

**Goals:**

- One canonical, tiered control catalog with fixed names, derived from the semantic layer's pattern families (Activatable, Toggleable, TextEditable, Selectable/MultiSelectable, Expandable, Value, Tab, Menu, Window/Dialog, Table).
- A uniform fixture CLI and observability discipline so any catalog action is assertable through the tree by name.
- Conventions for one shared catalog acceptance suite that onboards a new technology with launch configuration only.
- Alignment of `dev-docs/testing-strategy.md` §5 with the technology-matrix reality.

**Non-Goals:**

- No retrofit of existing fixtures (egui, Qt Widgets, Swing) — separate follow-up changes per app.
- No executable suite or fixture code here — the first runnable blueprint instance is `add-qml-test-app`, which delivers the shared resource and the first `catalog.robot` as reference implementation.
- No semantic-keyword or proxy implementation — the blueprint only prepares their acceptance surface.
- No re-deciding per-fixture toolchains (Gradle scaffold for Java, PEP 723/uv for Python, Cargo for Rust fixtures stay as established).

## Decisions

1. **The blueprint lives in `testing-strategy.md` §5.1; the capability spec is its acceptance-criteria form.** Dev-docs are the repo's authoritative convention documentation, so the developer-facing blueprint (catalog table, names, observables, CLI, suite conventions) is written out there — fixture READMEs and future fixture changes link a file that exists today, not an OpenSpec path that only materializes at archive time. The `test-app-blueprint` capability spec expresses the same contract as testable requirements (one scenario per acceptance criterion) so per-fixture changes stay anchored to something verifiable; both are updated together when the blueprint evolves. *Alternatives:* spec-only with a doc pointer — rejected (dead link until archive, and conventions belong in dev-docs per AGENTS.md); doc-only — rejected (nothing would anchor per-fixture changes to a verifiable contract).

2. **Catalog derived from pattern families, split into core and extended tier.** Core = what the default-proxy layer needs first (activation, toggling, text editing, selection, expansion, menus, dialogs); extended = Table, Value (slider/progress), Tab. Full definition now, staged adoption per fixture — the spec stays complete while first instances stay buildable. Core omissions must surface as documented skips, so tier compliance is observable, not aspirational.

3. **One canonical name set, kebab-case, accessible-name based.** The accessible name is the only addressing mechanism every targeted technology shares (UIA Name, AT-SPI name, JAB name, AX title); AutomationId/objectName equivalents are technology-private and excluded from the contract. Names follow `<control>-<qualifier>` (`button-basic`, `list-item-3`) — close to the existing Qt fixture's style, and pairwise-unique app-wide so name-only locators need no hierarchy. Existing fixtures keep their historic names until their retrofit change.

4. **Observables standardized on two mechanisms.** A counter label (`clicks-<n>` on `status-label`, the Swing/SWT pattern) for the button, and rename-on-activate (`<ident>-activated` / `-clicked`, the Qt pattern) for menu items and dialog buttons — the dialog rename doubles as the bounds-correctness proof established by the Qt fixture. State-bearing controls rely on provider read attributes (`IsSelected`, `IsExpanded`, value), matching the read/action pattern split in the library design; a fixture only adds extra observables where a bridge provably drops state, and documents it.

5. **Shared suite as resource + thin per-technology suites, no variable files.** Robot keywords for the catalog live once under `tests/acceptance/resources/`; each `tests/acceptance/<tech>/catalog.robot` supplies launch configuration (env-var convention `PLATYNUI_TEST_APP_<TECH>_*`, as wired today by `scripts/platynui-robot-session.sh` and the `test-acceptance-windows` recipe) and declares the tests. This keeps per-technology CI lanes independent (a lane needs only its own fixture built), matches the existing `tests/acceptance/<tech>/` layout and robot.toml profiles, and makes technology skips visible in that technology's suite file. *Alternative:* one parameterized suite driven by `--variablefile` per technology — rejected; it couples all lanes to one run and hides which technology skips what.

6. **Custom-controls chapter is optional per technology.** Hand-built controls (self-drawn button with manually wired accessibility, plus a deliberately non-exposed element as negative case) probe the default-proxy lower bound that real-world QML/WPF/Avalonia apps exhibit. Optional because not every toolkit has a meaningful "raw" tier (e.g. SWT), but normed so implementing technologies share tests.

## Risks / Trade-offs

- [Doc (§5.1) and capability spec express the same contract in two forms and can drift] → accepted for the benefit of a live dev-doc; the sync rule is stated in both places, and blueprint changes go through OpenSpec changes whose tasks include the doc alignment.
- [Blueprint decided before any fixture implements it — details may not survive contact with reality] → the QML fixture is the immediate proving ground; expected friction points (in-scene popups, bridge state gaps) already have escape hatches (documented skips, documented extra observables). Spec adjustments ride along with `add-qml-test-app` if needed.
- [Canonical names diverge from existing fixtures until retrofits land] → accepted; the shared suite only runs against conforming fixtures, existing suites keep working unchanged.
- [A catalog this wide tempts fixtures into becoming UI kitchen sinks] → tiering plus "additions never rename" keeps growth monotonic and reviewable; scenario-specific controls (e.g. Qt's bounds dialogs) live beside the catalog, not inside it.

## Migration Plan

Purely additive: new capability spec + `testing-strategy.md` §5 rewrite. No fixture, suite, or CI behavior changes until the follow-up changes (`add-qml-test-app` first, then per-app retrofits) adopt the blueprint. Rollback = archive the spec and revert the doc section.

## Open Questions

- Whether radio groups need a dedicated group container name (`radiogroup-basic`) on technologies whose bridges expose the group as its own node — decide when the first two fixtures implement it; the spec can add the container name without renaming the buttons.
- Exact keyword surface of the shared resource (which Given/When/Then keywords, how launch/teardown is shared with the existing per-tech resources) — decided in `add-qml-test-app`, where the reference implementation lands.
