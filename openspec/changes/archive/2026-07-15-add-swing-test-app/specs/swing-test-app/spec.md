## ADDED Requirements

### Requirement: Buildable with a plain JDK via just recipes
The Swing test app SHALL build from plain Java 8 sources using only `javac` driven by a `just` recipe (`just build-test-app-swing`), with no Maven/Gradle or other build-system dependency, and the sources SHALL remain outside the Cargo workspace (workspace `exclude`).

#### Scenario: Clean build with a JDK on PATH
- **WHEN** `just build-test-app-swing` runs on a machine with a JDK 8+ (`javac`) on `PATH`
- **THEN** all sources under `apps/test-app-swing/src` compile with `-encoding UTF-8 -source 8 -target 8` into `apps/test-app-swing/build/classes` and the recipe exits successfully

#### Scenario: Missing JDK fails fast
- **WHEN** `just build-test-app-swing` runs on a machine without `javac` on `PATH`
- **THEN** the recipe aborts with a message naming the missing tool and how to get it, and no partial build output is left behind

#### Scenario: Cargo workspace is unaffected
- **WHEN** `cargo metadata` (or `just check`) runs after the app directory is added
- **THEN** the Cargo workspace resolves without errors because `apps/test-app-swing` is listed in the root `Cargo.toml` `exclude`

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
