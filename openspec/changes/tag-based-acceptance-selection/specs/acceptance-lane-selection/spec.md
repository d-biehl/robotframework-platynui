# acceptance-lane-selection Delta

## ADDED Requirements

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

#### Scenario: No runtime environment guards remain
- **WHEN** the egui acceptance suites run on their matching lane
- **THEN** no test or suite setup evaluates `XDG_SESSION_TYPE` (or any equivalent environment probe) to skip

### Requirement: Lane profiles select by excluding foreign platforms
`robot.toml` SHALL define lane profiles `real-x11`, `real-wayland`, and `real-windows` that inherit the `real` profile and exclude the *other* platforms' tags, so untagged suites always remain selected. The plain `real` profile SHALL remain the runnable parent that selects every acceptance suite.

#### Scenario: Lane profile filters exactly the foreign platform tags
- **WHEN** `robotcode --profile real-x11 discover tests` runs
- **THEN** every suite/test tagged `platform:wayland` or `platform:windows` is absent, and every untagged or `platform:x11`-tagged one is present

#### Scenario: Dead suites are discoverable, not hidden
- **GIVEN** a suite whose platform tag no lane includes (e.g. a typo like `platform:x12`)
- **WHEN** each lane profile's discovery output is compared against the full `real` profile
- **THEN** the suite is visibly missing from every lane's selection — instead of running everywhere and permanently skipping

### Requirement: Lane entry points choose the matching profile
The acceptance entry points SHALL pass the lane profile that matches the session they establish: `scripts/platynui-robot-session.sh` SHALL default to `real-wayland` or `real-x11` based on the `XDG_SESSION_TYPE` exported by the session wrapper (falling back to `real` with a warning when it is unset or unknown), and the Windows acceptance recipe SHALL default to `real-windows`. Explicitly supplied robotcode arguments SHALL continue to override the default entirely.

#### Scenario: Compositor lane selects the Wayland profile
- **WHEN** `startcompositor.sh -- platynui-robot-session.sh` runs with no robotcode args
- **THEN** the session script invokes `robotcode --profile real-wayland run` (echoed in its "Running:" line), and X11-only suites do not appear in the result

#### Scenario: Explicit arguments override the lane default
- **WHEN** the session script is invoked with explicit robotcode arguments (e.g. `--profile real run-debug`)
- **THEN** those arguments are passed through unchanged and no default profile is injected

#### Scenario: Unknown session type falls back loudly
- **WHEN** the session script runs with `XDG_SESSION_TYPE` unset
- **THEN** it warns on stderr and defaults to the plain `real` profile rather than guessing a lane

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
