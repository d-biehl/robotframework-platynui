## 1. Build scaffolding

- [ ] 1.1 `apps/test-app-javafx`: Gradle project with checked-in wrapper (Gradle 8.x pinned so the wrapper runs on the PATH JDK 8), `application` plugin, Java 21 toolchain + `org.gradle.toolchains.foojay-resolver-convention`, and the `org.openjfx.javafxplugin` plugin with a pinned JavaFX version and `javafx.controls` module
- [ ] 1.2 Root `Cargo.toml`: add `apps/test-app-javafx` to the workspace `exclude` (with the existing "not a Cargo crate" comment pattern)
- [ ] 1.3 `justfile`: `build-test-app-javafx` (wrapper → `installDist`, fail-fast message when `java` is missing) and `run-test-app-javafx *ARGS` recipes
- [ ] 1.4 `apps/test-app-javafx/README.md`: what self-bootstraps (Gradle, JDK 21 via Foojay, OpenJFX from Maven Central), what a machine needs (any `java` 8+ on PATH, network on first build), proxy notes, and the Linux note (JavaFX has no native accessibility there — the fixture is the future agent-lane target)

## 2. Fixture app

- [ ] 2.1 `Main` (JavaFX `Application`) with the fixture CLI contract: `--title` (default "PlatynUI JavaFX TestApp"), `--auto-close <seconds>`, usage + non-zero exit on unknown arguments
- [ ] 2.2 Control set with fixed unique accessible names via `setAccessibleText`: stage, push button, single-line text field, checkbox, combo box, status label
- [ ] 2.3 Click-observable state change: button clicks update the status label text/accessible name to `clicks-<n>`

## 3. Acceptance coverage

- [ ] 3.1 Wire the fixture into `just test-acceptance-windows`: build with soft-skip (no JDK/network → warn and skip, mirroring the Swing pattern), hand the installDist launcher over as `PLATYNUI_TEST_APP_JAVAFX_BIN`
- [ ] 3.2 Windows acceptance suite: launch the fixture, assert the UIA window node carries `native:IsJvm = true` and `native:JvmToolkit = "JavaFX"`, no JVM enablement diagnostic fires, and the named controls enumerate (poll-based — JavaFX activates UIA on demand) with the click-counter observable working

## 4. Verification

- [ ] 4.1 `just check` and `just test` stay green (workspace unaffected by the excluded app); `just build-test-app-javafx` from a clean checkout succeeds with only a JDK 8 on PATH
- [ ] 4.2 Windows acceptance run green including the new JavaFX suite; `dev-docs/java-toolkits.md` gains a pointer to the fixture as the JavaFX acceptance surface (and future agent-lane target)
