---
name: robot-test-style
description: "Use when writing or reviewing Robot Framework test suites in this repo — any *.robot or *.resource file, wherever it lives (currently under tests/, more directories will follow). Keywords: robot framework, .robot, .resource, test suite, acceptance test, BareMetal, locator, Set Root, Wait Until Exists, VAR, keyword, suite setup, RF test, testfall, testfälle"
user-invocable: false
---

# Robot Framework Suite Style

Normative rationale lives in `dev-docs/testing-strategy.md` (§2.5 mock lane, §2.6
acceptance lane, §5.1 fixture blueprint, §7 waiting). This is the authoring
checklist. Reference implementation: `tests/acceptance/qml/`.

## Suite anatomy

| Rule | How |
|------|-----|
| Suites own their app instance | Launch in `Suite Setup`, terminate in `Suite Teardown`; a directory `__init__.robot` launches nothing |
| Pin by ProcessId, not title | The launcher keyword does `BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=SUITE` (or `TEST` for per-test instances — auto-clearing, no teardown reset) |
| Shared resources hold the launch/teardown flow ONLY | Never a page-object locator layer, never wrappers around BareMetal keywords |
| Single-use keywords live in the suite file | Or inline in the test; a suite-local keyword needs ≥2 uses AND real logic (retry flows, bounds predicates, computed points) |
| Tests are self-contained | No one-liner test delegating to a same-named keyword; the body IS the test. Cross-technology consistency comes from the spec'd canonical test set (blueprint), not shared keyword code |

## Locators

- Relative and inline at the point of use: `.//*[@Name="button-basic"]` / `.//*[@Id="btn-click-me"]`. No locator variables for single-use fragments.
- Key on the fixture's contract: `@Name` (canonical, pairwise-unique by blueprint rule) or `@Id` (AccessKit author id, egui/Inspector). Wildcard `*` role — roles differ per bridge and stay out of the contract.
- Role steps only where the role is the point: window nodes (`.//(Frame|Window)[@Name=…]` — bounds/window-ops need the window, unions cover AT-SPI vs UIA) and role-assertion tests.
- Never `native:` attributes in shared suites; absolute `/…` locators only when deliberately escaping the root (desktop-level checks, second instances).

## Waiting — the keywords already do it

| Need | Use |
|------|-----|
| Element appears | `BM.Wait Until Exists` (returns it; `query_overrides={'timeout': N}` to tune, default 30 s) |
| Element disappears / stays absent briefly | `BM.Wait Until Gone` (instant when already absent) |
| Permanent absence | `BM.Query … only_first=${True}` + `Should Be Equal    ${node}    ${None}` |
| Attribute reaches a value | `BM.Get Attribute    <loc>    <attr>    ==    <value>` — it retries until the assertion passes |
| Computed condition | `BM.Wait Until Query    <xpath>    <op>    <value>` |
| Before an action | Nothing — `Pointer Click` etc. wait for their target themselves |

Never wrap these in `Wait Until Keyword Succeeds` or hand-rolled `Node Is Present`
predicates. WUKS is legitimate only for conditions no BM keyword can express
(e.g. Bounds-field predicates after async window ops, whole-flow retries).
`Sleep` is legitimate only when asserting a NON-event (nothing observable will
ever change) or documented app-frame timing.

## Observables

- Activation effects are name-based: counter labels (`status-label-clicks-<n>`) and the always-visible `last-action-<ident>` label (`last-action-none` initially).
- Controls NEVER change their own accessible names on activation.

## RF ≥ 7 idioms

- `VAR` for all variable creation, incl. `scope=SUITE|TEST` — not `Create List`/`Create Dictionary`/`Set Variable`/`Set Suite Variable`.
- Inline evaluation `${{ … }}` instead of `Evaluate` for expressions; `$var` accesses objects. Gotcha: 2+ consecutive spaces split arguments even inside quotes — keep expressions single-spaced.
- `RETURN`, `TRY/FINALLY`, `IF/ELSE` — never `[Return]`, `Run Keyword If`.

## Tags, platforms, verification

- Tags are three orthogonal dimensions: test level (`acceptance`), build requirement (`real` vs `mock` — which native build the run needs), platform binding (`platform:x11|wayland|windows`). Acceptance suites inherit `acceptance    real` from their `__init__.robot`. Tags select — never `Skip If` on environment probes.
- Lanes: the profile brings its own session (robot.toml `wrapper`), so run headless — nested sessions inherit the desktop size and break geometry tests —
  `PLATYNUI_BACKEND=headless uv run robotcode --profile real-x11 run` and
  `… --profile real-wayland run` (or `just headless=true test-acceptance-x11` / `-compositor`). Same for `run-debug`/`repl`; `discover` starts no session.
- Build duality: real lanes need `just build-native`; the `tests/BareMetal` mock lane is `just test-baremetal` (mock build). Rebuild when switching.
- Judge runs with `uv run --no-sync robotcode results summary --failed` — lane exit codes are unreliable. Static check: `robotcode analyze code <suite dirs>` (clear its cache on stale diagnostics).
- `BM.Take Screenshot` filenames must be relative (land under `results/`).
