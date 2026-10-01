# acceptance-lane-selection Specification

## Purpose

Acceptance suites declare their platform requirements as tags (`platform:x11`, `platform:wayland`, `platform:windows`; no tag = every lane), and the acceptance lanes select suites via `robot.toml` profiles (`real-x11`, `real-wayland`, `real-windows`) that exclude the foreign platforms' tags. Environment fitness is decided by selection before Robot Framework starts — runtime-unknowable prerequisites fail with actionable messages, they never skip — so reports carry no environment-skip noise and a permanently dead suite cannot hide behind an always-true skip condition. The taxonomy is documented normatively in `dev-docs/testing-strategy.md`.

## Requirements

### Requirement: Platform requirements are declared as tags
Acceptance suites and tests SHALL declare platform-bound behavior via the tag vocabulary `platform:x11`, `platform:wayland`, `platform:windows` — suite-wide via `Test Tags`, per-test via `[Tags]`. A suite or test without a platform tag SHALL run on every acceptance lane. Acceptance suites SHALL NOT probe the environment (session type, OS) at runtime to decide whether to skip: environment fitness is a selection concern, decided before Robot Framework starts.

#### Scenario: Untagged suite runs on all lanes
- **GIVEN** an acceptance suite tagged only `real`
- **WHEN** the X11, Wayland, and Windows lanes each run
- **THEN** the suite executes on all three, with no environment skip in any report

#### Scenario: Platform-bound test excluded, not skipped
- **GIVEN** a test tagged `platform:wayland` inside an otherwise untagged suite
- **WHEN** the X11 lane runs
- **THEN** the test is not selected at all — the report contains neither a skip nor any trace of an attempted run (verifiable only on a real session lane, not the mock lane)

#### Scenario: Suites carry no runtime environment guards
- **WHEN** the egui acceptance suites run on their matching lane
- **THEN** no test or suite setup evaluates `XDG_SESSION_TYPE` (or any equivalent environment probe) to skip

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

#### Scenario: Backend selection is available to the caller
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

### Requirement: Runtime-only prerequisites fail, they do not skip
When an acceptance suite has a prerequisite that genuinely cannot be known before the run (a fixture binary, a launcher), its prerequisite check SHALL fail the suite with an actionable message naming the fixing command — it SHALL NOT skip. A selected suite that cannot run is a defect of the lane setup. The single exception remains the fixture blueprint's documented technology limitation (capability `test-app-blueprint`): a shared catalog test a technology's bridge provably cannot satisfy SHALL stay an explicitly skipped test with a message naming the limitation and its tracking location — that skip is deterministic per lane and machine-independent, unlike the environment and prerequisite conditions this capability bans from skipping.

#### Scenario: Missing fixture is a red failure with guidance
- **GIVEN** a lane selected a suite whose fixture is absent
- **WHEN** the suite's prerequisite check runs
- **THEN** the suite fails (not skips) with a message naming the `just` recipe that provisions the fixture

#### Scenario: Documented technology limitation stays a skip
- **GIVEN** a shared catalog test that a technology's accessibility bridge provably cannot satisfy
- **WHEN** that technology's onboarding catalog suite runs on its lane
- **THEN** exactly that test is skipped with a message naming the limitation and where it is tracked, on every run of that lane alike — while environment- or prerequisite-conditioned skips remain absent
