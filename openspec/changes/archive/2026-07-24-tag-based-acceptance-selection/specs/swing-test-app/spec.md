# swing-test-app Delta

## MODIFIED Requirements

### Requirement: Self-bootstrapping build via Gradle wrapper
The Swing test app SHALL build via the checked-in Gradle wrapper (current Gradle) driven by a `just` recipe (`just build-test-app-swing`), requiring only a `java` (8+) on PATH: the wrapper client SHALL run on that JVM, the Gradle daemon JVM SHALL be auto-provisioned via committed daemon JVM criteria (`gradle/gradle-daemon-jvm.properties`), the build SHALL compile with an auto-provisioned JDK 21 toolchain (Foojay resolver) targeting Java 8 bytecode (`--release 8`), and the app SHALL keep depending on nothing beyond the JDK APIs (no external libraries). The sources SHALL remain outside the Cargo workspace (root `Cargo.toml` `exclude`) and SHALL NOT change for this migration.

Because the build self-provisions every JVM it needs, the Windows acceptance lane SHALL treat it as a hard prerequisite: a failed fixture build fails the lane. The Swing suites themselves are selected by platform tag (`platform:windows`); their remaining runtime prerequisite checks (usable launcher, built classes) SHALL fail with an actionable message naming `just build-test-app-swing` — they SHALL NOT skip.

#### Scenario: Clean build with only a PATH java
- **WHEN** `just build-test-app-swing` runs on a machine with any `java` 8+ on PATH (and network access on first run)
- **THEN** the build compiles all sources with the JDK 21 toolchain at `--release 8` into Gradle's classes output and the recipe exits successfully, with no manually installed JDK or library

#### Scenario: Launch contract is preserved
- **WHEN** a consumer (JAB live test, Robot suite, `just run-test-app-swing`) launches the fixture
- **THEN** it still builds its own `java` command line against a classes directory (`PLATYNUI_TEST_APP_SWING_CLASSES`, now Gradle's output) with full control over JVM flags — in particular the per-launch AccessBridge enable/disable system property

#### Scenario: Unavailable build fails the lane loudly
- **WHEN** the fixture cannot be built (e.g. no network on first run) and `just test-acceptance-windows` runs
- **THEN** the lane stops at the build step with the actionable cause; the Swing suites and JAB live tests do not run and are not reported as skipped

#### Scenario: Missing fixture at suite level is a failure, not a skip
- **GIVEN** the `real-windows` profile selected the Swing suites but the fixture classes or launcher are absent (e.g. `robotcode` invoked outside the recipe)
- **WHEN** the suite prerequisite check runs
- **THEN** the suite fails with a message naming `just build-test-app-swing`, instead of skipping (verifiable only on a real Windows lane, not the mock lane)

#### Scenario: Platform scoping via tag, not OS probe
- **WHEN** a Linux acceptance lane (`real-x11` / `real-wayland`) runs
- **THEN** the Swing suites are excluded by their `platform:windows` tag — no `os.name` check executes and no skip appears in the report
