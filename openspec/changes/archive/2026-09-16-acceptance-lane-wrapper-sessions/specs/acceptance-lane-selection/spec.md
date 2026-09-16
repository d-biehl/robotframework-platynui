## MODIFIED Requirements

### Requirement: Lane profiles select by excluding foreign platforms
`robot.toml` SHALL define lane profiles `real-x11`, `real-wayland`, and `real-windows` that inherit the `real` profile and exclude the *other* platforms' tags, so untagged suites always remain selected. The plain `real` profile SHALL remain the runnable parent that selects every acceptance suite. Profiles SHALL select *within* the suite tree and SHALL NOT reshape it: no profile may narrow `paths`, because a narrower root directory renames the root suite and shifts every longname with the profile — which breaks any caller that resolved a longname without it, an editor test tree above all.

#### Scenario: Lane profile filters exactly the foreign platform tags
- **WHEN** `robotcode --profile real-x11 discover tests` runs
- **THEN** every suite/test tagged `platform:wayland` or `platform:windows` is absent, and every untagged or `platform:x11`-tagged one is present

#### Scenario: Dead suites are discoverable, not hidden
- **GIVEN** a suite whose platform tag no lane includes (e.g. a typo like `platform:x12`)
- **WHEN** each lane profile's discovery output is compared against the full `real` profile
- **THEN** the suite is visibly missing from every lane's selection — instead of running everywhere and permanently skipping

#### Scenario: Longnames do not depend on the selected profile
- **WHEN** the same test is discovered with no profile, with `real-wayland`, and with `mock`
- **THEN** it carries the identical longname in all three, so a longname taken from one selection selects that test under any other

## ADDED Requirements

### Requirement: Lane profiles establish their session
The lane profile SHALL be the entry point of an acceptance run: selecting it SHALL select both the suites (via the platform-tag excludes) and the session they need. Each Linux lane profile in `robot.toml` (`real-x11`, `real-wayland`) SHALL therefore carry a `wrapper` command prefix that establishes its session, and RobotCode SHALL execute the run through it. `scripts/platynui-robot-session.sh` SHALL be the tail of that wrapper chain: it prepares the environment (accessibility bus, fixture builds, fixture hand-over variables) and executes the RobotCode command line appended to it in the foreground with stdio passed through and its exit code propagated. It SHALL NOT choose a profile itself and SHALL NOT be invoked directly. The Windows acceptance recipe, which establishes no isolated session, SHALL keep defaulting to `real-windows`.

#### Scenario: Selecting the lane profile establishes its session
- **GIVEN** the project root as the working directory
- **WHEN** `robotcode --profile real-wayland run` runs
- **THEN** the suites execute inside a PlatynUI compositor session and no X11-only suite appears in the result (verifiable only on the real lane, not the mock lane)

#### Scenario: The same command works from the shell, `just`, and the editor
- **WHEN** the lane is started as `just test-acceptance-x11`, as a bare `robotcode --profile real-x11 run`, or from the editor's test runner
- **THEN** each of them brings the X11 session up the same way, with no session script named at the call site

#### Scenario: A single test selected by the editor runs in its session
- **GIVEN** the editor's generated command line, which selects one test by longname (`-s` / `-bl`) and narrows parsing to its file
- **WHEN** it runs with the lane profile
- **THEN** that one test executes inside the lane's session and passes — the selection is not emptied by the profile

#### Scenario: Interactive commands run inside the session too
- **WHEN** `robotcode --profile real-wayland run-debug -bl "<test longname>"` runs
- **THEN** the run halts at the debugger prompt with the test executing inside the compositor session, against the real providers

#### Scenario: Commands that do not execute Robot Framework start no session
- **WHEN** `robotcode --profile real-x11 discover tests` runs
- **THEN** the test list is returned without starting an X server or accessibility stack, so editor discovery and analysis stay session-free

#### Scenario: Backend selection remains available to the caller
- **GIVEN** `PLATYNUI_BACKEND=headless` in the environment
- **WHEN** a Linux lane runs
- **THEN** its session uses the windowless backend (compositor headless / Xvfb) and the suites pass exactly as with a visible backend

#### Scenario: A run inside an already established session does not nest
- **GIVEN** an interactive session opened by `startcompositor.sh` or `startxsession.sh`
- **WHEN** an acceptance run is started from inside that session
- **THEN** it executes in the existing session and no second session is created

#### Scenario: The session script refuses a bare invocation
- **WHEN** `scripts/platynui-robot-session.sh` is executed with no appended command
- **THEN** it exits non-zero with a message naming the profile-based command to use instead, rather than guessing a profile or a session

#### Scenario: An environment without wrapper support cannot run the lane silently
- **GIVEN** the project's declared Python dependencies
- **WHEN** the environment is resolved
- **THEN** a RobotCode version that ignores `wrapper` is excluded, so no lane can run without its session while reporting a normal result

## REMOVED Requirements

### Requirement: Lane entry points choose the matching profile
**Reason**: The session script no longer picks a profile. Deriving the lane from the `XDG_SESSION_TYPE` its session wrapper exported only worked for callers that start the script chain themselves, so the editor, `run-debug` and `repl` could never reach a lane. The profile now selects both the suites and the session through its `wrapper` — see "Lane profiles establish their session".
**Migration**: Run a lane by profile instead of through the session scripts: `robotcode --profile real-wayland run` / `robotcode --profile real-x11 run` (or the unchanged `just test-acceptance-compositor` / `just test-acceptance-x11`). Arguments go directly to `robotcode` (e.g. `robotcode --profile real-wayland run-debug`). A direct invocation of `scripts/platynui-robot-session.sh` now fails with the profile command to use; there is no fallback to the unfiltered `real` profile.
