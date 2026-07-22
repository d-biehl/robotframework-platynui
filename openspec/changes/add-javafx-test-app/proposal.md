## Why

Like SWT, the JavaFX half of the `java-app-classifier` verification ran only against a bare native window carrying a `Glass*` class — the repo has no real JavaFX application. A genuine JavaFX fixture matters twice over: on Windows it exercises the classifier and the UIA provider against JavaFX's native UIA implementation, and it is the designated target for the planned `provider-java-agent` — **JavaFX has no native accessibility on Linux at all** (see [`dev-docs/java-toolkits.md`](../../../dev-docs/java-toolkits.md)), so this fixture is the app the agent lane will eventually prove itself against.

## What Changes

- A new Java fixture **`apps/test-app-javafx`**: a self-contained Gradle project mirroring the fixture CLI contract (`--title`, `--auto-close`, usage on unknown args) with a small fixed control set, stable accessible names, and a click-observable state change.
- **Same self-bootstrapping toolchain as the other Java fixtures** (pattern established by the archived `migrate-swing-test-app-to-gradle` change): checked-in current-Gradle wrapper whose client runs on the PATH `java` (8+), daemon JVM auto-provisioned via committed daemon JVM criteria, Java 21 toolchain auto-provisioned via the Foojay resolver — nothing to install manually. JavaFX resolves from Maven Central via the official `org.openjfx.javafxplugin` Gradle plugin (handles platform classifiers, natives, and the module path). Unlike the SWT fixture there is **no Java 8 target/runtime lane**: OpenJFX exists on Maven Central only from version 11, and JavaFX-on-8 lives solely in FX-bundled JDK 8 distributions (Zulu FX / Liberica Full) — documented as out of scope in the design.
- `just build-test-app-javafx` / `just run-test-app-javafx` recipes; `apps/test-app-javafx` added to the Cargo workspace `exclude`.
- **Windows acceptance coverage**: the fixture window classifies as JVM + JavaFX (`native:IsJvm`, `native:JvmToolkit = "JavaFX"`, `Glass*` window class) while being served by the UIA provider (no JAB claim, no enablement diagnostic), plus basic UIA enumeration of its controls.

## Capabilities

### New Capabilities

- `javafx-test-app`: the JavaFX fixture application — self-bootstrapping Gradle build, fixture CLI conventions, named control set, and the JVM+JavaFX classification acceptance surface (and, later, the agent-lane target).

### Modified Capabilities

<!-- none — java-app-classification's requirements are unchanged; this change adds the real-app coverage its spec already describes. -->

## Impact

- **New**: `apps/test-app-javafx/**` (Gradle project + Java sources; wrapper JAR checked in), acceptance suite under `tests/acceptance`.
- **Modified**: `justfile` (build/run recipes, acceptance-lane wiring with soft-skip when the fixture is not built), root `Cargo.toml` (`exclude` entry), docs pointers where fixture apps are listed.
- **No Rust or Python code changes, no native rebuild** — pure fixture + test surface. No BREAKING changes.
- **Environment**: first build needs network access (Gradle distribution, JDK 21 auto-provision, OpenJFX from Maven Central — ~40 MB of JavaFX artifacts incl. Windows natives); machines only need the already-required `java` on PATH. Platform scope: Windows now; the fixture's sources are platform-neutral, and the Linux run (where JavaFX is invisible to AT-SPI) becomes the agent lane's test bed.
- **Depends on**: nothing hard — the toolchain pattern is settled by the archived `migrate-swing-test-app-to-gradle` change (copy the scaffold from `apps/test-app-swing`); `add-swt-test-app` additionally establishes the `installDist` launch wiring this fixture mirrors. **Unblocks**: real-app JavaFX coverage for `java-app-classification`; the primary target for `provider-java-agent`.
