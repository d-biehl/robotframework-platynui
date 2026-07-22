## Context

JavaFX left the JDK with Java 11: no JDK distribution the project requires ships it (Temurin never has), so the fixture must pull OpenJFX itself — JARs with platform classifiers carrying the Glass/Prism natives, launched over the module path. That makes a real build tool non-negotiable, and the toolchain decision is already settled by the archived `migrate-swing-test-app-to-gradle` change (reused by the sibling `add-swt-test-app`): current-Gradle wrapper whose client runs on the PATH `java` 8+, daemon JVM auto-provisioned via committed daemon JVM criteria, Java 21 toolchain via the Foojay resolver (convention plugin 1.0.0). This design covers only what is JavaFX-specific.

The strategic weight of this fixture is on the *agent* side: on Windows JavaFX implements UIA natively (activated on demand when an accessibility client asks), but on Linux JavaFX has no accessibility at all — this app is the future `provider-java-agent` proving ground.

## Goals / Non-Goals

**Goals:**

- A real JavaFX window on the Windows desktop with the standard fixture ergonomics (unique title, auto-close, stable accessible names, click observable).
- Zero manual installs, same as the SWT fixture: PATH `java` 8+ suffices, everything else self-bootstraps pinned.
- Acceptance coverage for the `java-app-classification` facts on a real JavaFX app (JVM + JavaFX via the `Glass*` window class, served by UIA, no diagnostic).

**Non-Goals:**

- No Linux/macOS lane yet (on Linux the fixture is *deliberately* invisible to AT-SPI — that gap is the agent change's problem, not this one's).
- No deep JavaFX control-pattern coverage in the UIA provider; classification + basic enumeration only.
- No embedded-content scenarios (JFXPanel/FXCanvas) — the classifier documents host-toolkit-only semantics; embedding fixtures come with the agent work if needed.

## Decisions

1. **OpenJFX via the official Gradle plugin** (`org.openjfx.javafxplugin`): declares `javafx { version = <pin>; modules = ["javafx.controls"] }` and the plugin resolves the platform-classified artifacts from Maven Central and assembles the module path for compile, `run`, and `installDist`. *Alternative:* hand-written dependencies with classifiers + manual `--module-path` JVM args — rejected, the plugin exists precisely for this and is the reason Gradle was chosen over Maven for the Java fixtures.

2. **JavaFX version pinned to the newest release compatible with the JDK 21 toolchain** (JavaFX 21 LTS line or newer; exact pin at implementation). The toolchain version and the JavaFX pin move together — recorded next to each other in the build file.

3. **No Java 8 runtime lane for this fixture** — deliberately diverging from the SWT fixture's build-on-21/target-8/test-on-8 policy. OpenJFX exists on Maven Central only from version 11 (class files Java 11+), so `--release 8` cannot compile against it; JavaFX on Java 8 exists solely inside FX-bundled JDK 8 distributions (Azul Zulu FX 8, Liberica Full 8) with their own compile basis (`jfxrt.jar`) that the Gradle JavaFX plugin does not model. If a legacy FX-8 target ever becomes a real customer scenario, it would be a separate optional lane built on a Zulu FX 8 download — recorded as an open question, not scoped here.

4. **Launch through the `installDist` output** (`bin/test-app-javafx(.bat)` carries the module-path setup), handed to the acceptance lane as `PLATYNUI_TEST_APP_JAVAFX_BIN` — same pattern as the SWT fixture, no Gradle invocation at test time.

5. **Accessible-name discipline via JavaFX's accessibility API**: `Node.setAccessibleText(...)` (JavaFX's accessible name; surfaces as the UIA `Name`) on every interactive control, unique app-wide. JavaFX exposes no AutomationId equivalent, so the accessible name is the locator contract — same rule as Swing/JAB and SWT.

6. **UIA activation is demand-driven in JavaFX** (the Glass a11y layer spins up when Windows asks): the acceptance suite must tolerate the first UIA query being the activation trigger — poll-based assertions as in the existing suites, no fixed sleeps.

## Risks / Trade-offs

- [First build downloads Gradle + JDK 21 + OpenJFX (~400 MB total, one-time per machine)] → accepted; pinned, cached user-level, proxy-configurable.
- [`Glass*` window-class literal has drifted across JavaFX versions (documented in `java-toolkits.md`)] → the classifier matches on the `Glass` prefix; the acceptance test pins today's behavior and will catch a drift on version bumps — which is a feature of having the fixture.
- [JavaFX's on-demand UIA activation could make the first enumeration racy] → poll-based acceptance assertions (existing suite convention) absorb it.
- [Three fixtures (Swing/SWT/JavaFX) duplicate Gradle wrapper + toolchain boilerplate] → accepted for per-app independence (decision in `add-swt-test-app`); consolidation is a possible later cleanup.

## Migration Plan

Purely additive: new app directory, recipes, exclude entry, acceptance suite. Rollback = delete the directory and recipes. No existing behavior changes.

## Open Questions

- Exact JavaFX version pin (newest stable working with the JDK 21 toolchain at implementation time).
- Whether a legacy JavaFX-on-Java-8 lane (Zulu FX 8 / Liberica Full 8 runtime) is ever needed for a real customer scenario — out of scope here, would be its own change.
- Whether the click observable is a plain `Label` text change (UIA `Name`) or additionally mirrored via `accessibleText` — decide against what the UIA provider actually surfaces during implementation.
