# Tag-Based Acceptance Lane Selection

## Why

Acceptance suites currently decide *at runtime* whether they can run on the current system, via `Skip If` guards on session type (`XDG_SESSION_TYPE` in `tests/acceptance/egui/config_display.robot`, `coexisting_runtimes.robot`, `inspector_window_controls.robot`) and on OS/fixture availability (`Require Swing Prerequisites` in `tests/acceptance/swing/resources/testapp.resource`). All of these conditions are already known *before* Robot Framework starts — the lane (`just test-acceptance-compositor` / `-x11` / `-windows`) chooses the session, and the Windows recipe builds the Swing fixture with Gradle-provisioned toolchains. Runtime skips therefore pollute reports with yellow noise, make `robotcode discover` lie about what a lane covers, and — worst — can silently hide a permanently-dead suite behind an always-true skip condition. `dev-docs/testing-strategy.md` already *documents* `platform:*` tags as the intended mechanism (§2.6, §5, §8), but no test uses them: a doc/code divergence this change resolves in the doc's favor.

## What Changes

- Introduce a platform tag vocabulary for acceptance suites: `platform:x11`, `platform:wayland`, `platform:windows` (suite-level via `Test Tags`, or per-test for single platform-bound tests). Suites/tests without a platform tag run on every lane.
- Add lane profiles to `robot.toml` — `real-x11`, `real-wayland`, `real-windows` — each including `real` and excluding the other platforms' tags. The plain `real` profile stays as the "everything" parent.
- `scripts/platynui-robot-session.sh` defaults to the lane profile matching the session it set up (compositor → `real-wayland`, X11 → `real-x11`); `just test-acceptance-windows` runs `--profile real-windows`.
- Remove the environment `Skip If` guards from the egui suites; replace with tags.
- Swing suites: tag `platform:windows`; **behavior change** — the soft-skip on a missing fixture is removed. `just test-acceptance-windows` builds the Swing fixture as a hard prerequisite (Gradle auto-provisions the JVMs, so a failure means a real problem, not a missing local install), and `Require Swing Prerequisites` turns its remaining runtime checks (launcher usable, classes present) into hard failures with the same actionable messages instead of skips.
- Document the test taxonomy normatively in `dev-docs/testing-strategy.md`: where tests live, the full tag vocabulary (`mock`/`real` lane tags, `platform:*` scoping), which profile selects what, and how to write a new platform-dependent acceptance suite — replacing the currently vague `platform:*` mentions.

## Capabilities

### New Capabilities
- `acceptance-lane-selection`: how acceptance suites declare their platform requirements via tags and how lanes select them via `robot.toml` profiles — including the guarantee that environment fitness is decided by selection, not by runtime skips.

### Modified Capabilities
- `swing-test-app`: the "Unavailable build fails fast, acceptance skips softly" requirement changes — the Windows lane now fails hard when the fixture cannot be built, and suite-level prerequisite checks fail (with actionable messages) instead of skipping.

## Impact

- **Tests (RF only, no Rust/Python code):** `tests/acceptance/egui/*.robot` (2 suite guards, 1 test guard, tags), `tests/acceptance/swing/*.robot` + `resources/testapp.resource` (tags, skip→fail), `tests/acceptance/qt/*.robot` (untouched semantics; gains nothing to remove — currently no skips).
- **Config/tooling:** `robot.toml` (new profiles), `scripts/platynui-robot-session.sh` (profile default per session type), `justfile` (`test-acceptance-windows`: hard Swing build, `real-windows` profile).
- **CI:** the Linux acceptance matrix (`x11`, `compositor` backends in `.github/workflows/ci.yml`) picks up the new defaults through the session script — no workflow edit needed unless profiles are named explicitly. The Windows lane is currently run manually (a Windows acceptance CI job is planned); the hard-fail behavior change lands there first and carries over to the future CI job, where fail-not-skip is exactly what a gate needs.
- **Docs:** `dev-docs/testing-strategy.md` (tag/profile taxonomy, authoring guidance); `CONTRIBUTING.md` only if it names the affected recipes' behavior.
- **No native rebuild** and no provider/platform behavior change — this is test-infrastructure only. Platform support status is unaffected.
- **BREAKING (dev workflow only):** an offline-first-run Windows lane that previously went green with skipped Swing suites now fails at the Gradle build step. Accepted deliberately: a missing fixture should be loud.
