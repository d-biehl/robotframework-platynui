## 1. Build scaffolding

- [ ] 1.1 `apps/test-app-javafx`: Gradle project copying the scaffold from `apps/test-app-swing` (current-Gradle wrapper with checksum-verified jar, committed daemon JVM criteria `gradle-daemon-jvm.properties`, `.gitattributes`, foojay-resolver-convention 1.0.0), plus `application` plugin, Java 21 toolchain, and the `org.openjfx.javafxplugin` plugin with a pinned JavaFX version and `javafx.controls` module
- [ ] 1.2 Root `Cargo.toml`: add `apps/test-app-javafx` to the workspace `exclude` (with the existing "not a Cargo crate" comment pattern)
- [ ] 1.3 `justfile`: `build-test-app-javafx` (wrapper → `installDist`, fail-fast message when `java` is missing) and `run-test-app-javafx *ARGS` recipes
- [ ] 1.4 `apps/test-app-javafx/README.md`: what self-bootstraps (Gradle, JDK 21 via Foojay, OpenJFX from Maven Central), what a machine needs (any `java` 8+ on PATH, network on first build), proxy notes, and the Linux note (JavaFX has no native accessibility there — the fixture is the future agent-lane target)

## 2. Fixture app

- [ ] 2.1 `Main` (JavaFX `Application`) with the blueprint CLI contract: `--title` (default "PlatynUI JavaFX TestApp"), `--auto-close <seconds>`, `--open-modal`, usage + non-zero exit on unknown arguments
- [ ] 2.2 Blueprint core-tier catalog under canonical names via `setAccessibleText`: `main-window`, `button-basic`+`status-label`, `checkbox-basic`, `groupbox-basic`+`radio-first`/`radio-second`, `textfield-basic`, `textarea-basic` (multi-line `TextArea`), `label-basic`, `text-basic`, `image-basic`, `combobox-basic`+items, `list-basic`+`list-item-1..5`, `tree-basic` (three levels), `main-menubar` with `menu-file`/`menu-edit` (+submenu `menu-edit-more`)/`menu-help`, `context-menu`+submenu `ctx-more`, dialogs `dialog-modeless`/`dialog-modal`
- [ ] 2.3 Blueprint observables: `status-label` click counter (`clicks-<n>`), menu-item rename to `<ident>-activated`, dialog-button rename to `<ident>-clicked`; verify surfaced UIA names against the running fixture before encoding them in suites

## 3. Acceptance coverage

- [ ] 3.1 Wire the fixture into `just test-acceptance-windows`: build with soft-skip (no JDK/network → warn and skip, mirroring the Swing pattern), hand the installDist launcher over as `PLATYNUI_TEST_APP_JAVAFX_BIN`
- [ ] 3.2 Windows acceptance suite: launch the fixture, assert the UIA window node carries `native:IsJvm = true` and `native:JvmToolkit = "JavaFX"`, no JVM enablement diagnostic fires, and the named controls enumerate (poll-based — JavaFX activates UIA on demand) with the click-counter observable working
- [ ] 3.2a Catalog onboarding: thin `tests/acceptance/javafx/catalog.robot` over the shared catalog resource (from `add-qml-test-app`) with launch config only; documented skips for verified JavaFX/UIA limitations

## 4. Verification

- [ ] 4.1 `just check` and `just test` stay green (workspace unaffected by the excluded app); `just build-test-app-javafx` from a clean checkout succeeds with only a JDK 8 on PATH
- [ ] 4.2 Windows acceptance run green including the new JavaFX suite; `dev-docs/java-toolkits.md` gains a pointer to the fixture as the JavaFX acceptance surface (and future agent-lane target)
