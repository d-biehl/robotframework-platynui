## Why

The `java-app-classifier` change landed with its SWT scenario verified only against a bare native window carrying the `SWT_Window0` class — the repo has no real SWT application (see the archived change's task 4.2 note). A genuine SWT fixture closes that gap: it exercises the classifier against a real JVM+SWT window, gives the UIA provider its first Java-toolkit acceptance surface (SWT renders through native Win32 controls and is served by UIA, per [`dev-docs/java-toolkits.md`](../../../dev-docs/java-toolkits.md)), and provides the SWT target the planned `provider-java-agent` routing work needs.

## What Changes

- A new Java fixture **`apps/test-app-swt`**: a self-contained Gradle project mirroring the Swing fixture's CLI contract (`--title`, `--auto-close`, usage on unknown args) and accessible-name discipline, with a small fixed control set and a click-observable state change.
- **Self-bootstrapping toolchain** (the dependency answer for all future Java fixtures): the checked-in Gradle wrapper runs on the existing PATH JDK (8+, Gradle 8.x pinned), and Gradle Toolchains + the Foojay resolver plugin auto-provision the JDKs into `~/.gradle/jdks` — nothing to install manually. SWT itself resolves from Maven Central (`org.eclipse.platform:org.eclipse.swt.win32.win32.x86_64`, natives bundled in the JAR).
- **Build on 21, target 8, test on 8 and 21**: the fixture compiles with the JDK 21 toolchain using `--release 8`, SWT is pinned to the newest release that still ships Java-8-compatible class files (real-world SWT apps are exactly that vintage), and a Temurin 8 JRE is auto-provisioned as a second toolchain so the acceptance lane can launch the fixture on a genuine Java 8 runtime as well as on 21.
- `just build-test-app-swt` / `just run-test-app-swt` recipes; `apps/test-app-swt` added to the Cargo workspace `exclude`.
- **Windows acceptance coverage**: the fixture window classifies as JVM + SWT (`native:IsJvm`, `native:JvmToolkit = "SWT"`) while being served by the UIA provider (no JAB claim, no enablement diagnostic), plus basic UIA enumeration of its controls.

## Capabilities

### New Capabilities

- `swt-test-app`: the SWT fixture application — self-bootstrapping Gradle build, fixture CLI conventions, named control set, and the JVM+SWT classification acceptance surface.

### Modified Capabilities

<!-- none — java-app-classification's requirements are unchanged; this change adds the real-app coverage its spec already describes. -->

## Impact

- **New**: `apps/test-app-swt/**` (Gradle project + Java sources; wrapper JAR checked in), acceptance suite under `tests/acceptance`.
- **Modified**: `justfile` (build/run recipes, acceptance-lane wiring with soft-skip when the fixture is not built), root `Cargo.toml` (`exclude` entry), `AGENTS.md`/docs pointers where fixture apps are listed.
- **No Rust or Python code changes, no native rebuild** — pure fixture + test surface. No BREAKING changes.
- **Environment**: first build needs network access (Gradle distribution, JDK 21 auto-provision, Maven Central); machines only need the already-required `java` on PATH. Platform scope: Windows (the SWT artifact is per-platform; a Linux/GTK variant is a follow-up for the AT-SPI lane).
- **Depends on**: nothing. **Unblocks**: real-app SWT coverage for `java-app-classification`; an SWT target for `provider-java-agent`.
