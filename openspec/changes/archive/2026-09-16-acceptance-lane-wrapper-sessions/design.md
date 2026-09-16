## Context

Until now the Linux acceptance lanes nested RobotCode *inside* the session: `just` ran `uv run scripts/startcompositor.sh --backend … -- scripts/platynui-robot-session.sh [args]`, and the session script's last act was `uv run --no-sync robotcode "$@"`. Because the caller could not pass the profile through that chain conveniently, the script derived it from the `XDG_SESSION_TYPE` its own session wrapper had exported (`wayland` → `real-wayland`, `x11` → `real-x11`, otherwise a warning and the unfiltered `real`). That worked for `just` and for hand-typed shell chains, and for nothing else — the editor, the debugger and the REPL all start `robotcode` themselves and therefore could never reach the lane.

RobotCode 2.7.0 (released 2026-07-14) added the `wrapper` profile option, which inverts exactly that nesting. Verified against the installed package rather than the docs alone:

- `robotcode/cli/__init__.py:53-143` re-executes the process once through the configured command prefix. The appended command is `[*wrapper, sys.executable, *entry, *sys.argv[1:]]` (`:130`) — i.e. the venv interpreter plus `-m robotcode.cli` plus the original arguments — so the wrapper tail needs no `uv run` of its own.
- Only commands marked `@wrappable` are wrapped (`:74-83`, `plugin/click_helper/wrappable.py`); `discover`, `libdoc` and the language server are not.
- Recursion is prevented by the `ROBOTCODE_WRAPPER_APPLIED` environment variable, set before the exec (`:137`). Its docstring states explicitly that an outer layer may set it itself to suppress wrapping — the supported hook for a session that is already established.
- On POSIX the process is *replaced* (`os.execvp`, `:141`), so PID, stdio, signals and exit code stay with the wrapper chain; Windows spawns and forwards instead.
- The selected profile's `env` is applied before the wrapper runs (`:96-102`), so a wrapper may rely on it.

Everything the chain needs on our side already existed: both session scripts split their own options from the session command at `--`, and both take the display backend from `PLATYNUI_BACKEND` (`scripts/startcompositor.sh:36`, `scripts/startxsession.sh:30`) — so the `--backend` flag `just` used to interpolate is not the only way in.

## Goals / Non-Goals

**Goals:**

- One entry point per lane: the profile selects the suites *and* the session, in one place.
- The lane is reachable from anything that starts RobotCode — shell, `just`, VS Code Test Explorer, `run-debug`, `repl`.
- Remove the profile guessing from the session script; keep the script's real job (accessibility bus, fixture builds, fixture hand-over).
- No behavior change to what the lanes select or how the suites run.

**Non-Goals:**

- Fixing the compositor lane's exit-code propagation (a run failure still cannot fail the process; results are read via `robotcode results`). Unchanged in either direction by this change.
- Moving the Windows lane's fixture environment into a PowerShell wrapper. Possible later — the mechanism supports it — but it would touch the JAB/Swing plumbing and belongs in its own change.
- Any Rust or Python source change; no native rebuild is involved.

## Decisions

**1. The wrapper is configured per lane profile, not passed at the call site.** `robot.toml` carries `wrapper = ["scripts/startxsession.sh", "--", "scripts/platynui-robot-session.sh"]` (`robot.toml:56`) and the compositor equivalent (`:62`). The `--` is the session scripts' own option terminator; RobotCode appends its command line directly after it, which is exactly the "passing options to the wrapper" form the RobotCode docs describe. *Alternative rejected:* `--wrapper` on the command line or `ROBOTCODE_WRAPPER` in the `just` recipes — that would keep the pairing outside the configuration and would not reach the editor, which is the main point of the change.

**2. `platynui-robot-session.sh` becomes a pure wrapper tail ending in `exec "$@"` (`:111`).** The profile default block and the `uv run --no-sync robotcode` call are gone; a bare invocation now fails with a message naming the profile command (`:103`). `exec` rather than run-and-forward is correct here because the script has nothing to tear down — the session scripts own the session — and it makes stdio, signals and the exit code flow without any extra handling, as the wrapper contract requires. *Alternative rejected:* keeping a "no args → guess the profile" fallback for backwards compatibility; it would preserve the very heuristic this change removes and would silently mask a misconfigured wrapper.

**3. The headless toggle moves to `PLATYNUI_BACKEND`.** `just` sets it as an environment prefix (`justfile:317`, `:322`) instead of interpolating `--backend headless` into a script call. Both session scripts already read it, so this is a call-site change only. *Alternative rejected:* separate `*-headless` profiles, or a computed `{ expr }` env value in `robot.toml` reading `CI` — both duplicate a decision that belongs to the caller and would double the profile count.

**4. Established sessions suppress wrapping by setting the guard themselves.** `startcompositor.sh:199` and `startxsession.sh:152` export `ROBOTCODE_WRAPPER_APPLIED=1` into the session environment, so a `robotcode --profile real-x11 run` typed inside a manually opened session runs *in* that session instead of nesting a second one. This uses the mechanism as documented in the RobotCode source rather than inventing a project-specific variable.

**5. The `real` parent profile stays wrapper-free.** It remains the select-everything parent for discovery and dry runs; only the concrete lane profiles know a session.

**6. Profiles select within the suite tree; they no longer narrow `paths`.** `real` scoped `paths = ["tests/acceptance"]` and `mock` scoped `paths = ["tests/BareMetal"]`. That makes the narrowed directory the root suite, so the same test is `Tests.Acceptance.Egui.Inspector Picker.…` without a profile and `Acceptance.Egui.Inspector Picker.…` with one — measured, not assumed. An editor builds its test tree from the plain configuration and then runs a single test by longname (`-s` / `-bl` plus `-N`), so under the profile that selection matches nothing and Robot Framework aborts with "Suite 'Acceptance' contains no tests after model modifiers". Dropping `paths` from both profiles leaves the selection identical (`real-x11` 74, `real-wayland` 73, `mock` 113 tests, unchanged) because the tags already carve out exactly those sets, and it costs only the parsing of the sibling directory — which the editor's `--parseinclude` narrows away anyway. *Alternatives rejected:* configuring the editor to discover with the lane profile (`robotcode.profiles`) — it makes the visible test tree one lane at a time and pushes a repo-level defect into every contributor's editor settings; or teaching callers to use profile-specific longnames — the same defect, moved.

This is a pre-existing defect, not one the wrapper introduced: any profile run from the editor hit it, `mock` included. It is fixed here because the wrapper is what makes running from the editor worth doing.

**Verified by running, not by reasoning:** both lanes green through the new entry point — compositor 73/73, X11 74/74, read from `output.xml` via `robotcode results summary` rather than from the exit code; `robotcode --profile real-wayland discover tests` returns the same 73 tests instantly and starts no session; `--wrapper echo` shows the appended command line to be the venv interpreter plus the original arguments.

**Assumed, not verified here:** that the VS Code Test Explorer and debug launcher pick the wrapper up (documented behavior plus the 2.7.0 fix "Make the wrapper work in VS Code"; needs a manual editor check, listed as a task).

## Risks / Trade-offs

- **A relative wrapper path is resolved against the current working directory** → runs must start from the project root. `just` does; the editor uses the workspace root. If a different working directory ever becomes relevant, the entries can be made absolute with a `{ expr = "str(Path.cwd() / …)" }` value.
- **Silent loss of the session on an old RobotCode**: a `wrapper` key is simply ignored by RobotCode < 2.7, and the suites would then run against the *host* desktop while reporting normally. → Raise the `robotcode` floor in `pyproject.toml` to `>=2.7.0`; this is the reason the spec carries a scenario for it.
- **The wrapper also applies to `repl` and `repl-server`** → an interactive `robotcode --profile real-wayland repl` now brings a whole compositor session up. That is the intended payoff, but it makes an accidental profile selection more expensive than before. Discovery and analysis remain unaffected.
- **Environment must survive the session boundary** for the guard variable to work. Verified for the compositor path: the session child inherits the environment and only `DISPLAY` is removed (`apps/wayland-compositor/src/child.rs:51`); the X11 path re-exports its own variables inside `dbus-run-session` and passes the session command through a `printf '%q'`-serialized variable (`scripts/startxsession.sh:57`). Both survived a real run.
- **Direct invocation of `platynui-robot-session.sh` is now an error** (**BREAKING** for anyone with that command in their shell history or notes). → The error message names the replacement command, and the script header documents the new form; docs that carried the old chain are updated in this change.

## Migration Plan

Behavioral for the entry points, not for the tests: same suites, same selection, same providers. No native rebuild — nothing outside `robot.toml`, three shell scripts, the `justfile` and the dependency pin is touched; `just build-native` is still the prerequisite of both lane recipes as before. CI needs no workflow edit because the recipe names are unchanged.

Rollback is a plain revert of the change's files: restoring the `XDG_SESSION_TYPE` default and the `uv run --no-sync robotcode "$@"` tail in `scripts/platynui-robot-session.sh`, the two `just` recipes, and dropping the `wrapper` keys. A leftover `ROBOTCODE_WRAPPER_APPLIED` export in the session scripts is harmless under the old chain (no wrapper is configured), so a partial revert cannot leave the lanes broken.
