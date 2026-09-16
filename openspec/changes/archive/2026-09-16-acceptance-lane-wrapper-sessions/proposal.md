## Why

The Linux acceptance lanes are entered from the outside: a session script starts the display stack and RobotCode is the innermost process (`startcompositor.sh -- platynui-robot-session.sh` → `robotcode`). Two costs follow from that nesting. The lane profile cannot be chosen by the caller, so the session script guesses it from the `XDG_SESSION_TYPE` its own wrapper exported — the profile↔session pairing lives in a shell heuristic instead of the configuration that defines both. And nothing that starts RobotCode itself can reach the lane: running or debugging a single acceptance test from the editor, or using `run-debug`/`repl` against the real providers, requires reproducing the whole script chain by hand.

RobotCode 2.7.0 (2026-07-14) added the `wrapper` profile option, which inverts the nesting: RobotCode re-executes itself through a configured command prefix and appends its own command line to it. That lets `robotcode --profile real-wayland run` bring its session up itself — the same command from the shell, from `just`, and from the editor.

## What Changes

- The acceptance profiles `real-x11` / `real-wayland` in `robot.toml` gain a `wrapper` that runs the lane through its session script chain, making the profile the single place that pairs a session with the suites selected for it.
- `scripts/platynui-robot-session.sh` becomes the tail of that chain: it prepares the environment (AT-SPI, fixture builds, `PLATYNUI_TEST_APP_*`) and `exec`s the RobotCode command line appended to it. Its `XDG_SESSION_TYPE`→profile default and its own `robotcode` invocation are removed — **BREAKING** for anyone invoking the script directly, which now errors with the profile-based command to use instead.
- The `just test-acceptance-compositor` / `test-acceptance-x11` recipes become plain `robotcode --profile … run` invocations; the headless toggle moves from the scripts' `--backend` flag to the `PLATYNUI_BACKEND` environment variable both session scripts already honor.
- `startcompositor.sh` / `startxsession.sh` mark their session as wrapper-established (`ROBOTCODE_WRAPPER_APPLIED`), so a RobotCode run started *inside* a manually opened session does not nest a second one.
- The `paths` narrowing is removed from the `real` and `mock` profiles; selection stays purely tag-based. A profile-specific `paths` renames the root suite (`Acceptance` instead of `Tests`), so every longname shifts with the profile — and an editor, which builds its test tree without a profile and then selects by longname, selects nothing ("Suite 'Acceptance' contains no tests after model modifiers"). This predates the wrapper but blocks exactly the editor workflow the wrapper unlocks, so it is fixed here.
- New, without extra work: the acceptance lane is reachable from the VS Code Test Explorer and debugger, and `run-debug` / `repl` run inside the session by selecting the profile. Commands that do not execute Robot Framework (`discover`, `libdoc`, the language server) are never wrapped, so editor discovery still costs nothing.
- Documentation that names the old invocation chain follows: the `robot-test-style` skill and the fixture-app READMEs.

## Capabilities

### New Capabilities

None — this changes how an existing capability's entry points work, not what the lanes select.

### Modified Capabilities

- `acceptance-lane-selection`: one requirement is replaced, one gains a rule.
  - "Lane entry points choose the matching profile" is removed and replaced by the new requirement "Lane profiles establish their session": the lane profile is no longer derived inside the session script from `XDG_SESSION_TYPE`; the profile selects both the suites (tag excludes, unchanged) and the session (its `wrapper`), and the session script is a pure environment-preparing wrapper tail. It is a replacement rather than a modification because the old requirement's scenarios (profile default, argument pass-through, fallback to `real`) describe exactly the behavior this change removes.
  - "Lane profiles select by excluding foreign platforms" gains the rule that a profile selects within the suite tree and never reshapes it — no profile narrows `paths`, so longnames are stable across profiles.

## Impact

- **Configuration / tooling only — no Rust or Python source, no native rebuild.** Touched: `robot.toml` (profile `wrapper`), `scripts/platynui-robot-session.sh`, `scripts/startcompositor.sh`, `scripts/startxsession.sh`, `justfile` (both Linux acceptance recipes).
- **Dependency floor**: `wrapper` requires RobotCode ≥ 2.7.0; `pyproject.toml` currently pins `robotcode[analyze,repl,runner]>=2.6.2` and must be raised, otherwise a resolvable-but-too-old environment silently runs the lane with no session at all.
- **Platforms**: Linux only (X11 + the PlatynUI Wayland compositor). The Windows lane (`real-windows`, no isolated session) keeps its current recipe and is unaffected; moving its fixture environment into a PowerShell wrapper is possible later but out of scope here.
- **Lane behavior is unchanged** — same suites, same selection, same build duality (`just build-native`, non-mock, per `dev-docs/testing-strategy.md`). Only the process nesting and the entry point change.
- **CI**: the `just` recipe names stay, so workflow steps need no edit; the compositor lane's known inability to propagate a failing exit code is neither fixed nor worsened by this change.
- **Docs**: `.claude/skills/robot-test-style/SKILL.md`, `apps/test-app-qt/README.md`, `apps/test-app-qml/README.md`.
