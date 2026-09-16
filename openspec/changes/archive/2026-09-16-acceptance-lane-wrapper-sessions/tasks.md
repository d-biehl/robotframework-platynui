## 1. Wrapper wiring (prototype — already implemented)

- [x] 1.1 `robot.toml`: give `real-x11` and `real-wayland` a `wrapper` that chains the matching session script to `scripts/platynui-robot-session.sh`, with a comment stating what is wrapped and what is not
- [x] 1.2 `scripts/platynui-robot-session.sh`: drop the `XDG_SESSION_TYPE`→profile default and the `uv run --no-sync robotcode "$@"` tail; end with `exec "$@"`, fail a bare invocation with the profile-based command to use (guard sits *before* the accessibility setup and the fixture builds, so a misuse has no side effects), and rewrite the header for the new entry point
- [x] 1.3 `justfile`: turn `test-acceptance-compositor` / `test-acceptance-x11` into plain `robotcode --profile … run` invocations and carry the headless toggle as `PLATYNUI_BACKEND` — set only when `headless=true`, so a `PLATYNUI_BACKEND` the caller exported still reaches the session scripts
- [x] 1.4 `scripts/startcompositor.sh` / `scripts/startxsession.sh`: export `ROBOTCODE_WRAPPER_APPLIED=1` in the session environment so a run started inside an established session does not nest a second one
- [x] 1.5 Drop the `paths` narrowing from the `real` and `mock` profiles so every profile shares one suite tree and longnames stay stable (the editor selects a single test by a longname it resolved without the profile); record the rule in `robot.toml` so it is not re-added

## 2. Dependency floor

- [x] 2.1 Raise the RobotCode pin in `pyproject.toml` from `>=2.6.2` to `>=2.7.0` (the version that introduced `wrapper`) and refresh the lockfile — the comment states why, `uv lock` resolved without moving any other package
- [x] 2.2 Confirm the resolved environment still satisfies the other RobotCode extras in use (`analyze`, `repl`, `runner`) — `robotcode --version` reports 2.7.0 and `just check` passes (fmt, clippy, ruff, mypy)

## 3. Documentation follow-up

- [x] 3.1 `.claude/skills/robot-test-style/SKILL.md`: replace the two session-script command lines with the profile-based lane commands
- [x] 3.2 `apps/test-app-qml/README.md`: the X11 session is no longer "Xephyr + the session script" but what the `real-x11` profile brings up. The `PLATYNUI_TEST_APP_*` hand-over sentences in both fixture READMEs stay as they are — the script still exports those variables, only its position in the chain changed
- [x] 3.3 Grep for remaining descriptions of the old chain: fixed the `real` profile's header comment in `robot.toml` and the two lane-entry-point passages in `dev-docs/testing-strategy.md` (§ acceptance "Needs", § platform scoping), the usage header of `scripts/startxsession.sh` (its example chained the session script bare, which now errors) and the "outer `uv run`" remark in `scripts/platynui-robot-session.sh` (the editor starts RobotCode without one). Left alone: `CHANGELOG.md` and `openspec/changes/archive/` (history), and the passing mention in `openspec/specs/qml-test-app/spec.md` (names the Linux lane, no requirement about the entry point)

## 4. Verification

- [x] 4.1 Spec scenario "Commands that do not execute Robot Framework start no session": `robotcode --profile real-x11 discover tests` returns the lane's 74 tests immediately, no session
- [x] 4.2 Spec scenario "The session script refuses a bare invocation": exits 2 naming the profile command, before any accessibility or build side effect
- [x] 4.3 Spec scenarios "Selecting the lane profile establishes its session" / "Backend selection remains available": `just headless=true test-acceptance-compositor` PASS 73/73 and `just headless=true test-acceptance-x11` PASS 74/74, read via `robotcode results summary` (real-provider lanes only — the compositor lane's exit code is not a pass signal)
- [x] 4.4 Spec scenario "Interactive commands run inside the session too": `robotcode --profile real-wayland run-debug --break tests/acceptance/egui/hit_test.robot:19 -bl "Tests.Acceptance.Egui.Hit Test.Element Under The Cursor Is Resolved"` stops at `(rdb)` on `BM.Pointer Move To` at that line, inside the (headless) compositor session; `.abort` ends the run and no compositor is left behind (re-run with the stable longname after 1.5)
- [x] 4.5 Spec scenario "A run inside an already established session does not nest": inside a manually opened compositor session `ROBOTCODE_WRAPPER_APPLIED=1` is present and a robotcode run there leaves the compositor process count unchanged
- [x] 4.6 Spec scenarios "Longnames do not depend on the selected profile" / "A single test selected by the editor runs in its session": the same test discovers as `Tests.Acceptance.…` under no profile, `real-wayland` and `mock` alike, and the editor's generated command line (`-I <file> -N Tests -s "Tests.…" -bl "Tests.…"`) runs that one test green inside the compositor session
- [x] 4.7 Editor half, manual (only the user can run it): trigger the same run and a debug session from the VS Code Test Explorer with the `real-wayland` profile, and confirm test discovery does *not* start a session — confirmed by the user: the Test Explorer run executes inside the compositor session, Debug Test halts at a breakpoint in `hit_test.robot` inside that session, and refreshing tests / saving a `.robot` file starts no compositor
- [x] 4.8 Regression: `just test-baremetal` (mock lane, no wrapper configured) PASS 113/113, `just check` passes; all three lanes re-run after the `paths` change with unchanged selection (73 / 74 / 113)
