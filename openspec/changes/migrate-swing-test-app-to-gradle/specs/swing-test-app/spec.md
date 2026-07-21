## REMOVED Requirements

### Requirement: Buildable with a plain JDK via just recipes

*Reason: retired in favor of the fixture-wide self-bootstrapping Gradle pattern (established by `add-swt-test-app`). The "no build system, javac only" property is consciously traded for one unified toolchain across all Java fixtures; the machine requirement drops from "JDK with javac" to "any `java` 8+" (plus network on first build).*

## ADDED Requirements

### Requirement: Self-bootstrapping build via Gradle wrapper
The Swing test app SHALL build via the checked-in Gradle wrapper driven by a `just` recipe (`just build-test-app-swing`), requiring only a `java` (8+) on PATH: the wrapper SHALL run on that JVM (Gradle 8.x pin), the build SHALL compile with an auto-provisioned JDK 21 toolchain (Foojay resolver) targeting Java 8 bytecode (`--release 8`), and the app SHALL keep depending on nothing beyond the JDK APIs (no external libraries). The sources SHALL remain outside the Cargo workspace (root `Cargo.toml` `exclude`) and SHALL NOT change for this migration.

#### Scenario: Clean build with only a PATH java
- **WHEN** `just build-test-app-swing` runs on a machine with any `java` 8+ on PATH (and network access on first run)
- **THEN** the build compiles all sources with the JDK 21 toolchain at `--release 8` into Gradle's classes output and the recipe exits successfully, with no manually installed JDK or library

#### Scenario: Launch contract is preserved
- **WHEN** a consumer (JAB live test, Robot suite, `just run-test-app-swing`) launches the fixture
- **THEN** it still builds its own `java` command line against a classes directory (`PLATYNUI_TEST_APP_SWING_CLASSES`, now Gradle's output) with full control over JVM flags — in particular the per-launch AccessBridge enable/disable system property

#### Scenario: Unavailable build fails fast, acceptance skips softly
- **WHEN** the fixture cannot be built (e.g. no network on first run) and the Windows acceptance lane runs
- **THEN** the build step reports the actionable cause, and the Swing suites (plus the JAB live tests) skip with a clear message instead of failing the lane

### Requirement: Runs on Java 8 and Java 21 runtimes
The built fixture SHALL run unmodified on a genuine Java 8 runtime and on the JDK 21 toolchain. The launch path used by tests SHALL select the runtime explicitly (provisioned Temurin 8 by default, PATH `java` only as ad-hoc fallback), so what the acceptance lane tests does not drift with the machine's PATH JDK.

#### Scenario: Same classes run on both runtimes
- **WHEN** the compiled classes are launched once with the provisioned Java 8 runtime and once with the JDK 21 toolchain
- **THEN** the fixture starts, renders its control set, and honors `--auto-close` identically on both

#### Scenario: Acceptance tests a real Java 8 process
- **WHEN** the Windows acceptance suites and the JAB live tests launch the fixture
- **THEN** the fixture process runs on the explicitly selected Java 8 runtime, and all existing suite assertions (JAB discovery, classification facts, interaction) hold unchanged
