## Why

The `java-app-classifier` change landed with its SWT scenario verified only against a bare native window carrying the `SWT_Window0` class — the repo has no real SWT application (see the archived change's task 4.2 note). A genuine SWT fixture closes that gap: it exercises the classifier against a real JVM+SWT window, gives the UIA provider its first Java-toolkit acceptance surface (SWT renders through native Win32 controls and is served by UIA, per [`dev-docs/java-toolkits.md`](../../../dev-docs/java-toolkits.md)), and provides the SWT target the planned `provider-java-agent` routing work needs.

## What Changes

- A new Java fixture **`apps/test-app-swt`**: a self-contained Gradle project that is a **`test-app-blueprint`-conforming fixture** — it implements the blueprint's core-tier control catalog (button + click counter, checkbox, radio group, text field, label, combo, list, tree, menu bar, context menu with submenu, modeless + modal dialog) under the canonical names, the blueprint CLI contract (`--title`, `--auto-close`, `--open-modal`, usage on unknown args), and the name-based action observables (`clicks-<n>` on `status-label`, rename-on-activate for menu items and dialog buttons).
- **Catalog-suite onboarding**: a thin `tests/acceptance/swt/catalog.robot` supplies only the SWT launch configuration and runs the shared catalog keywords (reference implementation lands with `add-qml-test-app`); SWT-specific limitations surface as documented skips per the blueprint.
- **Self-bootstrapping toolchain** (the pattern established by the archived `migrate-swing-test-app-to-gradle` change): checked-in current-Gradle wrapper whose client runs on the existing PATH `java` (8+), committed daemon JVM criteria auto-provision the Gradle daemon JVM, and Gradle Toolchains + the Foojay resolver plugin auto-provision the toolchain JDKs into `~/.gradle/jdks` — nothing to install manually. SWT itself resolves from Maven Central (`org.eclipse.platform:org.eclipse.swt.win32.win32.x86_64`, natives bundled in the JAR).
- **Build on 21, target 8, test on 8 and 21**: the fixture compiles with the JDK 21 toolchain using `--release 8`, SWT is pinned to the newest release that still ships Java-8-compatible class files (real-world SWT apps are exactly that vintage), and a Temurin 8 JRE is auto-provisioned as a second toolchain so the acceptance lane can launch the fixture on a genuine Java 8 runtime as well as on 21.
- `just build-test-app-swt` / `just run-test-app-swt` recipes; `apps/test-app-swt` added to the Cargo workspace `exclude`.
- **Windows acceptance coverage**: the fixture window classifies as JVM + SWT (`native:IsJvm`, `native:JvmToolkit = "SWT"`) while being served by the UIA provider (no JAB claim, no enablement diagnostic), plus basic UIA enumeration of its controls.

## Capabilities

### New Capabilities

- `swt-test-app`: the SWT fixture application — self-bootstrapping Gradle build, blueprint-conforming core-tier catalog and CLI, catalog-suite onboarding, and the JVM+SWT classification acceptance surface.

### Modified Capabilities

<!-- none — java-app-classification's requirements are unchanged; this change adds the real-app coverage its spec already describes. -->

## Impact

- **New**: `apps/test-app-swt/**` (Gradle project + Java sources; wrapper JAR checked in), acceptance suite under `tests/acceptance`.
- **Modified**: `justfile` (build/run recipes; acceptance-lane wiring with the fixture build as a hard prerequisite, per `tag-based-acceptance-selection`), root `Cargo.toml` (`exclude` entry), `AGENTS.md`/docs pointers where fixture apps are listed.
- **No Rust or Python code changes, no native rebuild** — pure fixture + test surface. No BREAKING changes.
- **Environment**: first build needs network access (Gradle distribution, JDK 21 auto-provision, Maven Central); machines only need the already-required `java` on PATH. Platform scope: Windows (the SWT artifact is per-platform; a Linux/GTK variant is a follow-up for the AT-SPI lane).
- **Depends on**: `test-app-blueprint` (catalog, canonical names, CLI, suite conventions) and `add-qml-test-app` (the shared catalog resource this fixture's `catalog.robot` consumes). The Gradle scaffold, runtime selection (`writeJavaLaunchers` → `build/java-launchers.properties`, per-app `PLATYNUI_TEST_APP_*` env vars), and daemon JVM criteria are established by the archived `migrate-swing-test-app-to-gradle` change and reused here. **Unblocks**: real-app SWT coverage for `java-app-classification`; an SWT target for `provider-java-agent`; the SWT row of the fixture technology matrix.
