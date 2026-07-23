## ADDED Requirements

### Requirement: Self-bootstrapping build via Gradle wrapper
The SWT test app SHALL build via the checked-in Gradle wrapper (current Gradle) driven by a `just` recipe (`just build-test-app-swt`), requiring only a `java` (8+) on PATH: the wrapper client SHALL run on that JVM, the Gradle daemon JVM SHALL be auto-provisioned via committed daemon JVM criteria (`gradle/gradle-daemon-jvm.properties`), the build SHALL compile with an auto-provisioned JDK 21 toolchain (Foojay resolver) targeting Java 8 bytecode (`--release 8`), and SWT SHALL resolve from Maven Central pinned to the newest release whose class files still target Java 8. The sources SHALL remain outside the Cargo workspace (root `Cargo.toml` `exclude`).

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

### Requirement: Blueprint CLI contract
The app SHALL implement the `test-app-blueprint` CLI contract: `--title <text>` (shell title, default "PlatynUI SWT TestApp"), `--auto-close <seconds>` (self-terminate for CI), and `--open-modal` (additionally open the modal dialog at startup). Unknown arguments SHALL fail with a usage message and a non-zero exit code.

#### Scenario: Custom title and auto-close
- **WHEN** the app is started with `--title "My SWT Window" --auto-close 5`
- **THEN** the top-level shell's title is "My SWT Window" and the process exits with code 0 no later than a few seconds after the 5-second deadline without user interaction

#### Scenario: Modal dialog reachable without interaction
- **WHEN** the app is started with `--open-modal`
- **THEN** `dialog-modal` is present in the accessibility tree with modal state exposed, without any prior pointer or keyboard input

#### Scenario: Unknown argument
- **WHEN** the app is started with `--bogus`
- **THEN** it prints a usage message naming the unknown argument and exits with a non-zero code

### Requirement: Blueprint core-tier catalog
The app SHALL implement the `test-app-blueprint` core-tier control catalog under the blueprint's canonical names, wired through SWT accessible names (SWT exposes no AutomationId equivalent — the accessible name is the locator contract; name overrides via `getAccessible().addAccessibleListener` where control text is not the name): `main-window` (shell), `button-basic` with `status-label` (`clicks-<n>` counter), `checkbox-basic`, `groupbox-basic` (SWT `Group`) grouping `radio-first`/`radio-second`, `textfield-basic`, `textarea-basic` (multi-line, `SWT.MULTI`), `label-basic`, `text-basic` (plain text label), `image-basic` (an image label), `combobox-basic` with items, `list-basic` with `list-item-1..5`, `tree-basic` with three levels of `tree-node-*`, menu bar `main-menubar` with menus `menu-file`, `menu-edit` (incl. submenu `menu-edit-more` with `menu-edit-sub-*` items), and `menu-help` (items rename to `<ident>-activated` on activation), `context-menu` with items and submenu `ctx-more`, and dialogs `dialog-modeless`/`dialog-modal` with the `-clicked` rename observable on their buttons. Additions later SHALL NOT change existing names; surfaced UIA names SHALL be verified against the running fixture before the acceptance suites encode them.

#### Scenario: Core tier enumerable through UIA
- **WHEN** the Windows UIA provider walks the tree under the fixture's shell (real-provider verification)
- **THEN** every core-tier control resolves under its canonical name, and no interactive control reports an empty or duplicate name

#### Scenario: Click updates the observable
- **WHEN** `button-basic` is activated twice
- **THEN** `status-label`'s text and accessible name end with `clicks-2`

#### Scenario: Menu activation observable
- **WHEN** the menu item `menu-file-new` is activated via the real UI
- **THEN** an element named `menu-file-new-activated` is resolvable in the tree

### Requirement: Catalog-suite onboarding
The fixture SHALL be onboarded to the shared catalog acceptance suite via a thin `tests/acceptance/swt/catalog.robot` that supplies only launch configuration (installDist launcher from `PLATYNUI_TEST_APP_SWT_BIN`, Java 8 runtime by default) and declares the catalog tests; SWT/UIA limitations SHALL surface as documented skips naming the limitation. The shared catalog keywords themselves are delivered by `add-qml-test-app` and SHALL NOT be modified with SWT-specific logic.

#### Scenario: Catalog suite runs from launch configuration alone
- **WHEN** `catalog.robot` runs in the Windows acceptance lane with the fixture built
- **THEN** the shared catalog tests execute against the SWT fixture, and each non-passing catalog test is a documented skip naming its limitation

### Requirement: Classifies as a JVM+SWT application served by UIA
The running fixture SHALL be the real-app acceptance surface for `java-app-classification` on SWT: its top-level window carries an `SWT_Window*` window class, classifies as JVM-backed with toolkit SWT, is served by the UIA provider (no JAB claim), and triggers no "absent from native accessibility" diagnostic.

#### Scenario: Classification facts on the UIA window node
- **WHEN** the fixture is running on Windows and its top-level window node is inspected through the UIA provider with the platform classifier injected
- **THEN** the node reports `native:IsJvm = true` and `native:JvmToolkit = "SWT"`, and no JVM enablement diagnostic has been emitted for the window
