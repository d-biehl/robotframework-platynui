## Context

The Swing fixture (`apps/test-app-swing`) is deliberately plain-Java-8 + `javac` — no build system, because Swing needs nothing beyond the JDK. SWT breaks that property: it is an external, platform-specific library, and current SWT releases require Java 17+ while the fixture machines have JDK 8 on PATH. So this change also settles *how Java fixtures with dependencies acquire their toolchain and libraries* — the same mechanism the JavaFX fixture (separate change) reuses.

## Goals / Non-Goals

**Goals:**

- A real SWT window on the Windows desktop with the same fixture ergonomics as the Swing app (unique title, auto-close, stable names).
- Zero manual installs: existing `java` on PATH is enough; everything else self-bootstraps with pinned versions.
- Acceptance coverage for the `java-app-classification` facts on a real SWT app (JVM + SWT, served by UIA, no diagnostic).

**Non-Goals:**

- No Linux/macOS fixture lane yet (the SWT artifact is per-platform; GTK variant is a follow-up).
- No migration of the Swing fixture to Gradle (its zero-dependency `javac` build is a feature; revisit only if maintenance demands it).
- No deep SWT control-pattern coverage in the UIA provider — the fixture enables such tests later, this change only adds classification + basic enumeration.

## Decisions

1. **Gradle wrapper + Toolchains + Foojay resolver, Gradle 8.x pinned.** The checked-in `gradlew`/`gradlew.bat` runs on the PATH JDK (Gradle 8.x supports running on Java 8); the build declares `languageVersion = 21` and the `org.gradle.toolchains.foojay-resolver-convention` plugin auto-downloads a Temurin 21 into `~/.gradle/jdks` — the "rustup for JDKs" answer. Gradle 9 is out because it requires JVM 17+ *to run*; revisit the pin when the PATH JDK moves. *Alternatives considered:* plain `javac` + curl'd JARs (rejected — hand-rolled dependency and JDK management, exactly what build tools solve); Maven + `maven-toolchains-plugin` auto-provisioning (workable, but younger than Gradle's toolchain support and without an official JavaFX plugin, which the sibling JavaFX fixture needs — one build tool for both).

2. **Self-contained Gradle project per fixture** (`apps/test-app-swt` with its own wrapper), not a shared multi-project root: matches the repo's one-app-per-directory convention and keeps the two fixture changes independent. The duplicated wrapper (~60 KB) is accepted. *Alternative:* a shared `apps/java-fixtures/` root — rejected as a new nesting convention for little gain.

3. **SWT from Maven Central, pinned to the last Java-8-compatible release**: `org.eclipse.platform:org.eclipse.swt.win32.win32.x86_64` (natives ship inside the JAR and self-extract). Current SWT requires Java 17+, so runtime-8 compatibility dictates an older pin — the newest version whose class files still target Java 8 (verify on Maven Central at implementation). That vintage is a feature, not a compromise: the SWT apps PlatynUI meets in the wild are Eclipse-RCP-era Java 8 applications, and the classification/UIA surface this fixture exists for (window class `SWT_Window*`, native Win32 controls) is unchanged across those versions. Known quirk: the artifact's POM references a `org.eclipse.swt.${osgi.platform}` placeholder dependency — the build must exclude that transitive.

4. **Build on 21, target 8, run on 8 and 21.** The fixture compiles with the JDK 21 toolchain and `options.release = 8`, so one build output runs on any Java 8+ runtime. A second Gradle toolchain (`languageVersion = 8`, Temurin via Foojay — auto-provisioned like the 21 one, nothing to install) provides a genuine Java 8 runtime for testing: the `installDist` start scripts honor `JAVA_HOME`, and a small Gradle task resolves the provisioned toolchain homes so the `just` run recipe and the acceptance lane can launch the fixture on 8 (the default — it matches the legacy apps this fixture represents) or on 21. *Alternative:* rely on whatever `java` is on PATH — rejected; PATH drift would silently change what the lane tests.

5. **Launchable output via `installDist`** (Gradle `application` plugin): `just build-test-app-swt` produces `apps/test-app-swt/build/install/test-app-swt/bin/test-app-swt(.bat)` with the correct classpath; the acceptance lane hands the path over as `PLATYNUI_TEST_APP_SWT_BIN`, mirroring the existing `PLATYNUI_TEST_APP_*` pattern. Robot suites (and any Rust live test) launch the script directly — no Gradle invocation at test time.

6. **Accessible-name discipline carries over**: every interactive control gets an explicit, unique accessible name (SWT: control text where natural, `getAccessible().addAccessibleListener` name override where not). SWT maps to UIA natively, so these surface as UIA `Name`; SWT exposes no AutomationId equivalent, so — as with JAB — the name is the locator contract.

7. **Acceptance soft-skips like the Swing fixture**: if the fixture is not built (no network on first run, Gradle failure), the SWT suites skip with a clear message instead of failing the lane.

## Risks / Trade-offs

- [First build downloads Gradle + JDK 21 + SWT (~350 MB total, one-time per machine)] → accepted; all pinned, cached in user-level directories, and proxy-configurable via standard Gradle properties.
- [A binary `gradle-wrapper.jar` enters the repo] → standard, auditable practice; version-pinned via `gradle-wrapper.properties`.
- [Gradle 8.x pin ages] → documented at the pin site; trivially bumped once fixture machines run a 17+ PATH JDK.
- [SWT window class contract (`SWT_Window*`) could drift across SWT versions] → the classification acceptance test would catch it immediately — that is partly the point of the fixture.
- [An old (Java-8-era) SWT pin means old library behavior on current Windows] → acceptable and intended: it mirrors the deployed base; if a modern-SWT surface is ever needed, a second dependency configuration can be added without touching the fixture sources.

## Migration Plan

Purely additive: new app directory, recipes, exclude entry, acceptance suite. Rollback = delete the directory and recipes. No existing behavior changes.

## Open Questions

- Exact SWT version pin: the newest release on Maven Central whose class files still target Java 8 (determine at implementation).
- Whether the acceptance robot suite reuses the generic app-launch keywords or needs an SWT-specific resource file — decide against the existing Swing suite structure during implementation.
