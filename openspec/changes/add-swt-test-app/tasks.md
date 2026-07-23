## 1. Build scaffolding

- [ ] 1.1 `apps/test-app-swt`: Gradle project copying the scaffold from `apps/test-app-swing` (current-Gradle wrapper with checksum-verified jar, committed daemon JVM criteria `gradle-daemon-jvm.properties`, `.gitattributes`, foojay-resolver-convention 1.0.0), plus `application` plugin, Java 21 compile toolchain with `options.release = 8` (+ `-Xlint:-options`), and `org.eclipse.platform:org.eclipse.swt.win32.win32.x86_64` pinned to the newest Java-8-compatible release (excluding the `${osgi.platform}` placeholder transitive)
- [ ] 1.1a Java 8 launch runtime: second toolchain (`languageVersion = 8`, auto-provisioned) plus the `writeJavaLaunchers` task pattern from the Swing fixture (`build/java-launchers.properties`), so run recipes/acceptance can set `JAVA_HOME` for the `installDist` start script explicitly (8 by default, 21 selectable)
- [ ] 1.2 Root `Cargo.toml`: add `apps/test-app-swt` to the workspace `exclude` (with the existing "not a Cargo crate" comment pattern)
- [ ] 1.3 `justfile`: `build-test-app-swt` (wrapper → `installDist`, fail-fast message when `java` is missing) and `run-test-app-swt *ARGS` recipes
- [ ] 1.4 `apps/test-app-swt/README.md`: what self-bootstraps (Gradle, JDK 21 via Foojay, SWT from Maven Central), what a machine needs (any `java` 8+ on PATH, network on first build), proxy notes

## 2. Fixture app

- [ ] 2.1 `Main` with the blueprint CLI contract: `--title` (default "PlatynUI SWT TestApp"), `--auto-close <seconds>`, `--open-modal`, usage + non-zero exit on unknown arguments
- [ ] 2.2 Blueprint core-tier catalog under canonical names (name overrides via `getAccessible().addAccessibleListener` where control text is not the name): `main-window`, `button-basic`+`status-label`, `checkbox-basic`, `groupbox-basic`+`radio-first`/`radio-second`, `textfield-basic`, `textarea-basic` (`SWT.MULTI`), `label-basic`, `text-basic`, `image-basic`, `combobox-basic`+items, `list-basic`+`list-item-1..5`, `tree-basic` (three levels), `main-menubar` with `menu-file`/`menu-edit` (+submenu `menu-edit-more`)/`menu-help`, `context-menu`+submenu `ctx-more`, dialogs `dialog-modeless`/`dialog-modal`
- [ ] 2.3 Blueprint observables: `status-label` click counter (`clicks-<n>`), menu-item rename to `<ident>-activated`, dialog-button rename to `<ident>-clicked`; verify surfaced UIA names against the running fixture before encoding them in suites

## 3. Acceptance coverage

- [ ] 3.1 Wire the fixture into `just test-acceptance-windows`: build with soft-skip (no JDK/network → warn and skip, mirroring the Swing pattern), hand the installDist launcher over as `PLATYNUI_TEST_APP_SWT_BIN` with the provisioned Java 8 runtime as launch `JAVA_HOME`
- [ ] 3.2 Windows acceptance suite: launch the fixture (on the Java 8 runtime), assert the UIA window node carries `native:IsJvm = true` and `native:JvmToolkit = "SWT"`, no JVM enablement diagnostic fires, and the named controls enumerate with the click-counter observable working
- [ ] 3.2a Catalog onboarding: thin `tests/acceptance/swt/catalog.robot` over the shared catalog resource (from `add-qml-test-app`) with launch config only; documented skips for verified SWT/UIA limitations
- [ ] 3.3 Dual-runtime smoke: the same installDist output starts and honors `--auto-close` on both the provisioned Java 8 runtime and the JDK 21 toolchain

## 4. Verification

- [ ] 4.1 `just check` and `just test` stay green (workspace unaffected by the excluded app); `just build-test-app-swt` from a clean checkout succeeds with only a JDK 8 on PATH
- [ ] 4.2 Windows acceptance run green including the new SWT suite; `dev-docs/java-toolkits.md` gains a pointer to the fixture as the SWT acceptance surface
