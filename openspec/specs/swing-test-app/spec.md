# swing-test-app Specification

## Purpose

Swing implements no UIA provider on Windows, so a Swing window is opaque to the existing Windows provider; the Java Access Bridge (JAB) is the only sanctioned out-of-process channel. The Swing test app (`apps/test-app-swing`, plain Java 8 sources, self-contained Gradle project) is the controlled fixture the JAB provider work develops and tests against — and, because its sources are platform-neutral, the same app later serves a Linux acceptance lane through `java-atk-wrapper` and the existing AT-SPI2 provider. Every interactive control carries an explicit unique accessible name because JAB exposes no AutomationId equivalent: the accessible name is the locator contract, and it must stay stable as the app grows stage by stage.

## Requirements

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

### Requirement: Runs on Java 8 and Java 21 runtimes
The built fixture SHALL run unmodified on a genuine Java 8 runtime and on the JDK 21 toolchain. The launch path used by tests SHALL select the runtime explicitly (provisioned Temurin 8 by default, PATH `java` only as ad-hoc fallback), so what the acceptance lane tests does not drift with the machine's PATH JDK.

#### Scenario: Same classes run on both runtimes
- **WHEN** the compiled classes are launched once with the provisioned Java 8 runtime and once with the JDK 21 toolchain
- **THEN** the fixture starts, renders its control set, and honors `--auto-close` identically on both

#### Scenario: Acceptance tests a real Java 8 process
- **WHEN** the Windows acceptance suites and the JAB live tests launch the fixture
- **THEN** the fixture process runs on the explicitly selected Java 8 runtime, and all existing suite assertions (JAB discovery, classification facts, interaction) hold unchanged

### Requirement: Per-process accessibility enablement at launch
The run recipe (`just run-test-app-swing`) SHALL launch the app with `-Djavax.accessibility.assistive_technologies=com.sun.java.accessibility.AccessBridge` on Windows, and enabling accessibility SHALL NOT write any persistent machine or user configuration (no `jabswitch`, no `.accessibility.properties`).

#### Scenario: JAB active for the launched process only
- **WHEN** the app is started via `just run-test-app-swing` on Windows
- **THEN** a JAB client (the spike) sees the app's top-level frame via `isJavaWindow` (verifiable only against the real JAB client, not the mock lane)
- **AND** `%USERPROFILE%\.accessibility.properties` is not created or modified by the recipe

#### Scenario: App runs without the flag
- **WHEN** the app is started with plain `java platynui.testapp.Main` and no accessibility flag
- **THEN** it starts and behaves identically apart from accessibility exposure (the app itself has no dependency on the bridge)

### Requirement: Test-app CLI conventions
The app SHALL support the CLI surface of the existing fixture apps: `--title <text>` (window title, default "PlatynUI Swing TestApp"), `--auto-close <seconds>` (self-terminate for CI), `--dialogs <n>` and `--open-modal` (reserved stage-4 flags that are accepted and, until dialogs exist, act as no-ops so launcher scripts stay stable). Unknown arguments SHALL fail with a usage message.

#### Scenario: Custom title
- **WHEN** the app is started with `--title "My Swing Window"`
- **THEN** the top-level frame's title (and its accessible name) is "My Swing Window"

#### Scenario: Auto-close for CI
- **WHEN** the app is started with `--auto-close 5`
- **THEN** the process exits with code 0 no later than a few seconds after the 5-second deadline without user interaction

#### Scenario: Unknown argument
- **WHEN** the app is started with `--bogus`
- **THEN** it prints a usage message naming the unknown argument and exits with a non-zero code

### Requirement: Accessible-name discipline
Every interactive control the app creates SHALL carry an explicit, non-empty, unique `accessibleName` (via `getAccessibleContext().setAccessibleName(...)`), because JAB exposes no AutomationId equivalent and the accessible name is the locator anchor for all downstream tests.

#### Scenario: Names visible through the accessibility API
- **WHEN** the accessibility tree of the running app is read by a JAB client (spike; real-provider-only verification)
- **THEN** every stage-1/2 interactive control reports its designated accessible name and no interactive control reports an empty name

#### Scenario: Names are unique
- **WHEN** all accessible names of interactive controls are collected
- **THEN** no name occurs twice within the app

### Requirement: Stage 1 and stage 2 control coverage
The app SHALL contain, with fixed accessible names: stage 1 — a top-level `JFrame`, a menu bar (File menu with an Exit item, Help menu with an About item), a push button, a single-line text field, and a status label; stage 2 — a checkbox, a radio-button group (≥ 2 options), an editable combo box or non-editable combo box, a slider, a spinner, and a progress bar, grouped in a titled panel. Later stages SHALL be additive: introducing them MUST NOT change any existing accessible name.

#### Scenario: Stage-1 controls enumerable
- **WHEN** a JAB client walks the tree under the top-level frame (real-provider-only verification)
- **THEN** it finds menu bar, button, text field, and label with their designated accessible names and Swing roles (`menu bar`, `push button`, `text`, `label`)

#### Scenario: Stage-2 controls enumerable
- **WHEN** a JAB client walks the stage-2 panel
- **THEN** it finds checkbox, radio buttons, combo box, slider, spinner, and progress bar with their designated accessible names and the roles Swing reports for them (recorded verbatim by the spike)

### Requirement: Click-observable state change
Clicking the stage-1 button SHALL produce a deterministic, accessibility-visible state change: the status label's text and accessible name change to include a click counter (e.g. `clicks-1`, `clicks-2`, …), so interaction tests can assert a click landed without screenshots.

#### Scenario: Click updates the observable
- **WHEN** the stage-1 button is activated twice (by pointer or by accessible action)
- **THEN** the status label's text and accessible name end with `clicks-2`

#### Scenario: Text field edits are observable
- **WHEN** text is typed into the stage-1 text field
- **THEN** the typed text is readable back through the accessibility text API of the field (real-provider-only verification)

### Requirement: Platform-neutral sources
The Java sources SHALL contain no Windows-specific code or dependencies; only the launch recipe differs per OS (Windows: AccessBridge flag; Linux, future lane: `java-atk-wrapper` enablement).

#### Scenario: Runs on Linux unmodified
- **WHEN** the compiled app is started on Linux with a JDK 8+
- **THEN** it starts and renders the same control set (accessibility exposure via ATK is out of scope until the Linux lane exists)
