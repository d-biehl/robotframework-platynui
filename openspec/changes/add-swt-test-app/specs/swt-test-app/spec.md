## ADDED Requirements

### Requirement: Self-bootstrapping build via Gradle wrapper
The SWT test app SHALL build via the checked-in Gradle wrapper driven by a `just` recipe (`just build-test-app-swt`), requiring only a `java` (8+) on PATH: the wrapper SHALL run on that JVM (Gradle 8.x pin), the build SHALL compile with an auto-provisioned JDK 21 toolchain (Foojay resolver) targeting Java 8 bytecode (`--release 8`), and SWT SHALL resolve from Maven Central pinned to the newest release whose class files still target Java 8. The sources SHALL remain outside the Cargo workspace (root `Cargo.toml` `exclude`).

#### Scenario: Clean build with only the PATH JDK
- **WHEN** `just build-test-app-swt` runs on a machine with any JDK 8+ on PATH (and network access on first run)
- **THEN** the build compiles against the auto-provisioned JDK 21 toolchain with `--release 8`, resolves SWT from Maven Central, and produces a launchable distribution under `apps/test-app-swt/build/install/` without any manually installed JDK or library

#### Scenario: Cargo workspace is unaffected
- **WHEN** `cargo metadata` (or `just check`) runs after the app directory is added
- **THEN** the workspace resolves without errors because `apps/test-app-swt` is listed in the root `Cargo.toml` `exclude`

#### Scenario: Unavailable dependencies fail fast, acceptance skips softly
- **WHEN** the fixture cannot be built (e.g. no network on first run) and the Windows acceptance lane runs
- **THEN** the build step reports the actionable cause, and the SWT acceptance suites skip with a clear message instead of failing the lane

### Requirement: Runs on Java 8 and Java 21 runtimes
The built fixture SHALL run unmodified on a genuine Java 8 runtime and on the JDK 21 toolchain. A Java 8 toolchain (Temurin via the Foojay resolver — auto-provisioned, nothing installed manually) SHALL be available for launching, the launch path SHALL select the runtime explicitly (never the ambient PATH `java`), and the acceptance lane SHALL launch the fixture on the Java 8 runtime by default — matching the legacy SWT applications the fixture represents.

#### Scenario: Same distribution runs on both runtimes
- **WHEN** the `installDist` output is launched once with the provisioned Java 8 runtime and once with the JDK 21 toolchain
- **THEN** the fixture starts, renders its control set, and honors `--auto-close` identically on both

#### Scenario: Acceptance tests a real Java 8 process
- **WHEN** the Windows acceptance suite launches the fixture
- **THEN** the fixture process runs on the provisioned Java 8 runtime, and the classification facts (JVM + SWT) hold for that process

### Requirement: Test-app CLI conventions
The app SHALL support the CLI surface of the existing fixture apps: `--title <text>` (shell title, default "PlatynUI SWT TestApp") and `--auto-close <seconds>` (self-terminate for CI). Unknown arguments SHALL fail with a usage message and a non-zero exit code.

#### Scenario: Custom title and auto-close
- **WHEN** the app is started with `--title "My SWT Window" --auto-close 5`
- **THEN** the top-level shell's title is "My SWT Window" and the process exits with code 0 no later than a few seconds after the 5-second deadline without user interaction

#### Scenario: Unknown argument
- **WHEN** the app is started with `--bogus`
- **THEN** it prints a usage message naming the unknown argument and exits with a non-zero code

### Requirement: Named control set with an observable state change
The app SHALL contain, with fixed unique accessible names (SWT exposes no AutomationId equivalent — the accessible name is the locator contract): a top-level shell, a push button, a single-line text field, a checkbox, a combo, and a status label. Clicking the button SHALL change the status label's text (and accessible name) to include a click counter (`clicks-1`, `clicks-2`, …) so interaction tests can assert a click landed without screenshots. Additions later SHALL NOT change existing names.

#### Scenario: Controls enumerable through UIA
- **WHEN** the Windows UIA provider walks the tree under the fixture's shell (real-provider verification)
- **THEN** it finds the button, text field, checkbox, combo, and label with their designated names, and no interactive control reports an empty or duplicate name

#### Scenario: Click updates the observable
- **WHEN** the button is activated twice
- **THEN** the status label's text and accessible name end with `clicks-2`

### Requirement: Classifies as a JVM+SWT application served by UIA
The running fixture SHALL be the real-app acceptance surface for `java-app-classification` on SWT: its top-level window carries an `SWT_Window*` window class, classifies as JVM-backed with toolkit SWT, is served by the UIA provider (no JAB claim), and triggers no "absent from native accessibility" diagnostic.

#### Scenario: Classification facts on the UIA window node
- **WHEN** the fixture is running on Windows and its top-level window node is inspected through the UIA provider with the platform classifier injected
- **THEN** the node reports `native:IsJvm = true` and `native:JvmToolkit = "SWT"`, and no JVM enablement diagnostic has been emitted for the window
