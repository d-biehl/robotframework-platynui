# Design: Tag-Based Acceptance Lane Selection

## Context

Acceptance suites under `tests/acceptance` are all tagged `real` (suite `Test Tags` in each `__init__.robot` / suite file) and selected by the single `[profiles.real]` in `robot.toml` (`includes = ["real"]`, verified at `robot.toml:20-23`). Platform fitness is then decided *inside* the run:

- `tests/acceptance/egui/config_display.robot:44` and `tests/acceptance/egui/coexisting_runtimes.robot:54` — suite-setup `Skip If '%{XDG_SESSION_TYPE=}' != 'x11'` (X11-only scenarios).
- `tests/acceptance/egui/inspector_window_controls.robot:150` — one Wayland-only test with an inline `Skip If ... != 'wayland'`.
- `tests/acceptance/swing/resources/testapp.resource:35-42` — `Require Swing Prerequisites` skips on non-Windows, missing Java launcher, or unbuilt fixture classes.

Everything a guard checks is known before Robot starts: `startcompositor.sh:192` / `startxsession.sh:145` export `XDG_SESSION_TYPE` themselves, and `just test-acceptance-windows` (justfile, `[windows] test-acceptance-windows`) already `Test-Path`-checks the Swing classes for the JAB Rust live tests. `dev-docs/testing-strategy.md` §2.6/§5/§8 names `platform:*` tags as the intended selection mechanism, but no test carries one — the doc records intent, the code diverged (this design resolves toward the doc, per AGENTS.md's flag-the-divergence rule).

Verified mechanics (not assumed): `robotcode config info list` confirms `[profile].inherits`, `excludes`, and `extend-excludes` are valid `robot.toml` keys; `platynui-robot-session.sh:91` is where the default `--profile real run` command is chosen.

## Goals / Non-Goals

**Goals:**
- Platform requirements are declared on suites/tests as tags; lanes select by profile. Reports contain no environment skips in normal operation.
- A suite that no lane selects is visible in `robotcode discover` output instead of hiding behind a permanent skip.
- The Windows lane treats the Swing fixture as a hard prerequisite; fixture-missing turns from yellow to red.
- `dev-docs/testing-strategy.md` becomes the normative reference for test placement, tags, profiles, and how to author platform-dependent suites.

**Non-Goals:**
- No change to the mock lane (`tests/BareMetal`, `mock` profile) beyond documenting it in the taxonomy.
- No new CI jobs. A Windows acceptance CI job is planned but out of scope here; this change prepares for it (`real-windows` profile, hard-fail semantics — a CI gate must go red, not skip-green, when the fixture is broken). The Linux matrix keeps calling the same `just` recipes.
- No relocation of suites; directory layout stays as-is. Tags scope *within* the existing tree.
- No macOS tags yet — no macOS lane exists; the vocabulary is extensible when one appears.

## Decisions

### D1: Tag vocabulary `platform:x11` / `platform:wayland` / `platform:windows`

Matches the `platform:*` shape testing-strategy.md already promises, so doc and code converge instead of inventing a second convention (alternative considered: bare `x11-only`-style tags — rejected because the prefixed form groups in tag statistics and is already documented). Semantics: **no platform tag = runs on every lane; a platform tag = runs only on lanes that don't exclude it.** Suite-wide requirements go in `Test Tags` (e.g. the whole `swing/` tree via its `__init__.robot`); single platform-bound tests use `[Tags]` (the one Wayland-only window-controls test).

### D2: Lane profiles via `inherits` + excludes, not per-lane includes

```toml
[profiles.real-wayland]
inherits = ["real"]
excludes = ["platform:x11", "platform:windows"]

[profiles.real-x11]
inherits = ["real"]
excludes = ["platform:wayland", "platform:windows"]

[profiles.real-windows]
inherits = ["real"]
excludes = ["platform:x11", "platform:wayland"]
```

Exclude-the-others rather than include-mine (`includes = ["real AND platform:x11"]`-style) because untagged suites must keep running everywhere — include-based selection would force tagging every suite with every platform it supports, which does not scale and buries the common case. The enumeration grows by one line per future platform; acceptable. `real` stays as the parent and remains directly runnable for "select everything" (useful for discover/dry-run; on a concrete machine, foreign-platform suites will fail rather than skip — that is the intended honesty, and the lane profiles are the supported entry points).

### D3: The session script picks the lane profile from the session it created

`platynui-robot-session.sh:91` currently defaults to `--profile real run`. It runs *inside* the session wrapper, which exports `XDG_SESSION_TYPE` (`startcompositor.sh:192`, `startxsession.sh:145`), so the default becomes `--profile real-wayland run` or `--profile real-x11 run` keyed off that variable (fallback: `real` with a warning if unset/unknown). Explicit user args still override entirely — unchanged pass-through. `test-acceptance-windows` switches its default from `--profile real` to `--profile real-windows`. Alternative considered: teach the `just` recipes to pass the profile — rejected because ad-hoc `startcompositor.sh -- platynui-robot-session.sh` invocations (documented in the script header) would silently lose the selection.

### D4: Swing prerequisites become hard failures; the recipe builds the fixture unconditionally

- The `os.name != "nt"` skip is deleted — `platform:windows` on the swing suites' `__init__.robot` covers it at selection time.
- `just test-acceptance-windows` drops the `-` soft-fail prefix on `build-test-app-swing`: Gradle auto-provisions daemon JVM, JDK 21 toolchain, and Java 8 runtime (per the `swing-test-app` spec), so a build failure is a real defect or a first-run-offline machine — both should stop the lane, not skip a third of it. The `Test-Path` fallback warning for the JAB Rust live tests goes with it.
- `Require Swing Prerequisites` keeps its launcher/classes checks (they still produce far better messages than a raw `Start Process` failure) but converts `Skip If` → `Fail`-style hard errors with the same actionable text ("run `just build-test-app-swing` …"). This also protects people running `--profile real-windows` outside the recipe.

Trade-off accepted (marked BREAKING-dev in the proposal): offline first run on Windows now fails loudly instead of passing with skips. The soft alternative — recipe detects the failed build and appends `--exclude platform:windows`-style flags — was considered and rejected: it reintroduces "green without Swing" as a silent state, which is exactly the failure mode this change exists to remove. The hard semantics are also a prerequisite for the planned Windows acceptance CI job: a gate that skips a broken fixture is no gate.

### D5: Documentation lands in testing-strategy.md as normative taxonomy

One section (extending §2.6/§5, updating the §8 quick-reference table) documents: the directory→lane map, the complete tag vocabulary (`mock`, `real`, `platform:*`), the profile table (`mock`, `real`, `real-{x11,wayland,windows}`) and which recipe/script invokes which, and an authoring checklist for a new acceptance suite (own your app instance; tag the lane; add `platform:*` only for genuinely platform-bound behavior; never gate on environment probes inside the suite — if you can only know it at runtime, it's a prerequisite *failure*, not a skip). Written as durable rules, not current-state inventory; suite counts and today's file names stay out except as examples.

### D6 (added during apply): Scope the no-skip rule against the fixture blueprint

Interim commits (`e559e35` QML fixture app, `1d5c91c` blueprint archive) landed the fixture blueprint into `testing-strategy.md` §5.1 and the `test-app-blueprint` capability, which *requires* an explicitly skipped test for a documented technology limitation in the shared catalog suite. That is not the skip class this change removes: environment/prerequisite skips vary per machine and can hide dead suites; a limitation skip is deterministic per lane, per-test, and carries a tracking pointer — it is the auditable representation of "this bridge cannot do X". The no-skip rule is therefore scoped to environment fitness and runtime prerequisites, with the blueprint limitation skip named as the single sanctioned exception (spec delta and §2.6/§7 updated accordingly). QML suites themselves are untagged (cross-platform by design) and need no `platform:*` tag.

## Risks / Trade-offs

- [Wrong/missing tag silently changes coverage — an untagged X11-only suite now *fails* on the Wayland lane instead of skipping] → That failure is loud and immediate (the point of the change); the authoring checklist in testing-strategy.md makes tagging a review item. The reverse error (over-tagging) shows up in `robotcode discover` per profile.
- [`inherits` merge semantics differ from expectation (e.g. `paths`/`includes` not carried over)] → Verify with `robotcode --profile real-x11 discover tests` during implementation before touching any suite; fall back to repeating `paths`/`includes` per profile if inheritance surprises.
- [Ad-hoc `robotcode --profile real run` on a dev box now fails foreign-platform suites] → Documented as intended; the lane profiles are one word away and printed by the session script (`Running: robotcode --profile real-… run`).
- [CI behavior shift] → None expected: CI calls `just test-acceptance-{x11,compositor}` (ci.yml matrix `backend: [x11, compositor]`), which route through the session script defaults. The previously-skipped suites were not providing signal on those lanes anyway.

## Migration Plan

Additive-plus-behavioral, test-infrastructure only, **no native rebuild**. Order: robot.toml profiles → session script/justfile defaults → tags + guard removal per suite → swing skip→fail conversion → docs. Each step keeps the tree runnable (tags without profiles are inert; profiles without tags select everything, matching today). Rollback: revert the commits — no persisted state, no schema, no wheel involvement.

## Open Questions

- None blocking. If a Linux Swing lane (AT-SPI via `java-atk-wrapper`, anticipated by the swing-test-app spec) materializes later, the swing suites' `platform:windows` tag moves from the `__init__.robot` down to genuinely JAB-specific suites — the vocabulary already supports it.
