## ADDED Requirements

### Requirement: Self-bootstrapping build via Gradle wrapper with OpenJFX from Maven Central
The JavaFX test app SHALL build via the checked-in Gradle wrapper (current Gradle) driven by a `just` recipe (`just build-test-app-javafx`), requiring only a `java` (8+) on PATH: the wrapper client SHALL run on that JVM, the Gradle daemon JVM SHALL be auto-provisioned via committed daemon JVM criteria (`gradle/gradle-daemon-jvm.properties`), the build SHALL declare a Java 21 toolchain that Gradle auto-provisions via the Foojay resolver, and JavaFX SHALL resolve from Maven Central through the official `org.openjfx.javafxplugin` plugin (pinned version, platform-classified artifacts, module path handled by the plugin). The sources SHALL remain outside the Cargo workspace (root `Cargo.toml` `exclude`).

#### Scenario: Clean build with only the PATH JDK
- **WHEN** `just build-test-app-javafx` runs on a machine with any JDK 8+ on PATH (and network access on first run)
- **THEN** the build compiles against the auto-provisioned JDK 21 toolchain, resolves the pinned OpenJFX artifacts with Windows natives from Maven Central, and produces a launchable distribution under `apps/test-app-javafx/build/install/` whose start script carries the required module-path configuration

#### Scenario: Cargo workspace is unaffected
- **WHEN** `cargo metadata` (or `just check`) runs after the app directory is added
- **THEN** the workspace resolves without errors because `apps/test-app-javafx` is listed in the root `Cargo.toml` `exclude`

#### Scenario: Unavailable dependencies fail fast, acceptance skips softly
- **WHEN** the fixture cannot be built (e.g. no network on first run) and the Windows acceptance lane runs
- **THEN** the build step reports the actionable cause, and the JavaFX acceptance suites skip with a clear message instead of failing the lane

### Requirement: Blueprint CLI contract
The app SHALL implement the `test-app-blueprint` CLI contract: `--title <text>` (stage title, default "PlatynUI JavaFX TestApp"), `--auto-close <seconds>` (self-terminate for CI), and `--open-modal` (additionally open the modal dialog at startup). Unknown arguments SHALL fail with a usage message and a non-zero exit code.

#### Scenario: Custom title and auto-close
- **WHEN** the app is started with `--title "My JavaFX Window" --auto-close 5`
- **THEN** the top-level stage's title is "My JavaFX Window" and the process exits with code 0 no later than a few seconds after the 5-second deadline without user interaction

#### Scenario: Modal dialog reachable without interaction
- **WHEN** the app is started with `--open-modal`
- **THEN** `dialog-modal` is present in the accessibility tree with modal state exposed, without any prior pointer or keyboard input

#### Scenario: Unknown argument
- **WHEN** the app is started with `--bogus`
- **THEN** it prints a usage message naming the unknown argument and exits with a non-zero code

### Requirement: Blueprint core-tier catalog
The app SHALL implement the `test-app-blueprint` core-tier control catalog under the blueprint's canonical names, wired via JavaFX's accessibility API (`setAccessibleText`; JavaFX exposes no AutomationId equivalent — the accessible name is the locator contract): `main-window` (stage), `button-basic` with `status-label` (`clicks-<n>` counter), `checkbox-basic`, `groupbox-basic` (a `TitledPane`/titled group) grouping `radio-first`/`radio-second`, `textfield-basic`, `textarea-basic` (multi-line `TextArea`), `label-basic`, `text-basic` (plain `Text` node), `image-basic` (`ImageView`), `combobox-basic` with items, `list-basic` with `list-item-1..5`, `tree-basic` with three levels of `tree-node-*`, menu bar `main-menubar` with menus `menu-file`, `menu-edit` (incl. submenu `menu-edit-more` with `menu-edit-sub-*` items), and `menu-help` (items rename to `<ident>-activated` on activation), `context-menu` with items and submenu `ctx-more`, and dialogs `dialog-modeless`/`dialog-modal` with the `-clicked` rename observable on their buttons. Additions later SHALL NOT change existing names; surfaced UIA names SHALL be verified against the running fixture before the acceptance suites encode them.

#### Scenario: Core tier enumerable through UIA
- **WHEN** the Windows UIA provider walks the tree under the fixture's window (real-provider verification; JavaFX activates its UIA layer on demand, so assertions poll rather than assume immediate availability)
- **THEN** every core-tier control resolves under its canonical name, and no interactive control reports an empty or duplicate name

#### Scenario: Click updates the observable
- **WHEN** `button-basic` is activated twice
- **THEN** `status-label`'s text and accessible name end with `clicks-2`

#### Scenario: Menu activation observable
- **WHEN** the menu item `menu-file-new` is activated via the real UI
- **THEN** an element named `menu-file-new-activated` is resolvable in the tree

### Requirement: Catalog-suite onboarding
The fixture SHALL be onboarded to the shared catalog acceptance suite via a thin `tests/acceptance/javafx/catalog.robot` that supplies only launch configuration (installDist launcher from `PLATYNUI_TEST_APP_JAVAFX_BIN`) and declares the catalog tests; JavaFX/UIA limitations SHALL surface as documented skips naming the limitation. The shared catalog keywords themselves are delivered by `add-qml-test-app` and SHALL NOT be modified with JavaFX-specific logic.

#### Scenario: Catalog suite runs from launch configuration alone
- **WHEN** `catalog.robot` runs in the Windows acceptance lane with the fixture built
- **THEN** the shared catalog tests execute against the JavaFX fixture, and each non-passing catalog test is a documented skip naming its limitation

### Requirement: Classifies as a JVM+JavaFX application served by UIA
The running fixture SHALL be the real-app acceptance surface for `java-app-classification` on JavaFX: its top-level window carries a `Glass*` window class, classifies as JVM-backed with toolkit JavaFX, is served by the UIA provider (no JAB claim), and triggers no "absent from native accessibility" diagnostic.

#### Scenario: Classification facts on the UIA window node
- **WHEN** the fixture is running on Windows and its top-level window node is inspected through the UIA provider with the platform classifier injected
- **THEN** the node reports `native:IsJvm = true` and `native:JvmToolkit = "JavaFX"`, and no JVM enablement diagnostic has been emitted for the window
