# Spec Delta

## MODIFIED Requirements

### Requirement: Platform requirements are declared as tags
Acceptance suites and tests SHALL declare platform-bound behavior via the tag vocabulary `platform:x11`, `platform:wayland`, `platform:windows`, and `platform:linux` for behavior that needs Linux but either Linux session — suite-wide via `Test Tags`, per-test via `[Tags]`. A suite or test without a platform tag SHALL run on every acceptance lane. Acceptance suites SHALL NOT probe the environment (session type, OS) at runtime to decide whether to skip: environment fitness is a selection concern, decided before Robot Framework starts.

#### Scenario: Untagged suite runs on all lanes
- **GIVEN** an acceptance suite tagged only `real`
- **WHEN** the X11, Wayland, and Windows lanes each run
- **THEN** the suite executes on all three, with no environment skip in any report

#### Scenario: Platform-bound test excluded, not skipped
- **GIVEN** a test tagged `platform:wayland` inside an otherwise untagged suite
- **WHEN** the X11 lane runs
- **THEN** the test is not selected at all — the report contains neither a skip nor any trace of an attempted run (verifiable only on a real session lane, not the mock lane)

#### Scenario: A Linux-bound suite runs on both Linux lanes only
- **GIVEN** an acceptance suite tagged `platform:linux`
- **WHEN** the X11, Wayland, and Windows lanes each run
- **THEN** the suite executes on the X11 and Wayland lanes and is not selected on the Windows lane

#### Scenario: Suites carry no runtime environment guards
- **WHEN** the egui acceptance suites run on their matching lane
- **THEN** no test or suite setup evaluates `XDG_SESSION_TYPE` (or any equivalent environment probe) to skip

### Requirement: Lane profiles select by excluding foreign platforms
`robot.toml` SHALL define lane profiles `real-x11`, `real-wayland`, and `real-windows` that inherit the `real` profile and exclude the *other* platforms' tags, so untagged suites always remain selected. `platform:linux` is foreign only to `real-windows`. The plain `real` profile SHALL remain the runnable parent that selects every acceptance suite. Profiles SHALL select *within* the suite tree and SHALL NOT reshape it: no profile may narrow `paths`, because a narrower root directory renames the root suite and shifts every longname with the profile — which breaks any caller that resolved a longname without it, an editor test tree above all.

#### Scenario: Lane profile filters exactly the foreign platform tags
- **WHEN** `robotcode --profile real-x11 discover tests` runs
- **THEN** every suite/test tagged `platform:wayland` or `platform:windows` is absent, and every untagged, `platform:x11`- or `platform:linux`-tagged one is present

#### Scenario: Dead suites are discoverable, not hidden
- **GIVEN** a suite whose platform tag no lane includes (e.g. a typo like `platform:x12`)
- **WHEN** each lane profile's discovery output is compared against the full `real` profile
- **THEN** the suite is visibly missing from every lane's selection — instead of running everywhere and permanently skipping

#### Scenario: Longnames do not depend on the selected profile
- **WHEN** the same test is discovered with no profile, with `real-wayland`, and with `mock`
- **THEN** it carries the identical longname in all three, so a longname taken from one selection selects that test under any other
