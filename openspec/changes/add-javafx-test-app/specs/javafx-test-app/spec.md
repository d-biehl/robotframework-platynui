## ADDED Requirements

### Requirement: Self-bootstrapping build via Gradle wrapper with OpenJFX from Maven Central
The JavaFX test app SHALL build via the checked-in Gradle wrapper driven by a `just` recipe (`just build-test-app-javafx`), requiring only a `java` (8+) on PATH: the wrapper SHALL run on that JVM (Gradle 8.x pin), the build SHALL declare a Java 21 toolchain that Gradle auto-provisions via the Foojay resolver, and JavaFX SHALL resolve from Maven Central through the official `org.openjfx.javafxplugin` plugin (pinned version, platform-classified artifacts, module path handled by the plugin). The sources SHALL remain outside the Cargo workspace (root `Cargo.toml` `exclude`).

#### Scenario: Clean build with only the PATH JDK
- **WHEN** `just build-test-app-javafx` runs on a machine with any JDK 8+ on PATH (and network access on first run)
- **THEN** the build compiles against the auto-provisioned JDK 21 toolchain, resolves the pinned OpenJFX artifacts with Windows natives from Maven Central, and produces a launchable distribution under `apps/test-app-javafx/build/install/` whose start script carries the required module-path configuration

#### Scenario: Cargo workspace is unaffected
- **WHEN** `cargo metadata` (or `just check`) runs after the app directory is added
- **THEN** the workspace resolves without errors because `apps/test-app-javafx` is listed in the root `Cargo.toml` `exclude`

#### Scenario: Unavailable dependencies fail fast, acceptance skips softly
- **WHEN** the fixture cannot be built (e.g. no network on first run) and the Windows acceptance lane runs
- **THEN** the build step reports the actionable cause, and the JavaFX acceptance suites skip with a clear message instead of failing the lane

### Requirement: Test-app CLI conventions
The app SHALL support the CLI surface of the existing fixture apps: `--title <text>` (stage title, default "PlatynUI JavaFX TestApp") and `--auto-close <seconds>` (self-terminate for CI). Unknown arguments SHALL fail with a usage message and a non-zero exit code.

#### Scenario: Custom title and auto-close
- **WHEN** the app is started with `--title "My JavaFX Window" --auto-close 5`
- **THEN** the top-level stage's title is "My JavaFX Window" and the process exits with code 0 no later than a few seconds after the 5-second deadline without user interaction

#### Scenario: Unknown argument
- **WHEN** the app is started with `--bogus`
- **THEN** it prints a usage message naming the unknown argument and exits with a non-zero code

### Requirement: Named control set with an observable state change
The app SHALL contain, with fixed unique accessible names set via JavaFX's accessibility API (`setAccessibleText`; JavaFX exposes no AutomationId equivalent — the accessible name is the locator contract): a top-level stage, a push button, a single-line text field, a checkbox, a combo box, and a status label. Clicking the button SHALL change the status label's text (and accessible name) to include a click counter (`clicks-1`, `clicks-2`, …). Additions later SHALL NOT change existing names.

#### Scenario: Controls enumerable through UIA
- **WHEN** the Windows UIA provider walks the tree under the fixture's window (real-provider verification; JavaFX activates its UIA layer on demand, so assertions poll rather than assume immediate availability)
- **THEN** it finds the button, text field, checkbox, combo box, and label with their designated names, and no interactive control reports an empty or duplicate name

#### Scenario: Click updates the observable
- **WHEN** the button is activated twice
- **THEN** the status label's text and accessible name end with `clicks-2`

### Requirement: Classifies as a JVM+JavaFX application served by UIA
The running fixture SHALL be the real-app acceptance surface for `java-app-classification` on JavaFX: its top-level window carries a `Glass*` window class, classifies as JVM-backed with toolkit JavaFX, is served by the UIA provider (no JAB claim), and triggers no "absent from native accessibility" diagnostic.

#### Scenario: Classification facts on the UIA window node
- **WHEN** the fixture is running on Windows and its top-level window node is inspected through the UIA provider with the platform classifier injected
- **THEN** the node reports `native:IsJvm = true` and `native:JvmToolkit = "JavaFX"`, and no JVM enablement diagnostic has been emitted for the window
