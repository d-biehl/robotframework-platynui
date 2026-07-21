## Context

The Swing fixture predates the Gradle decision: `just build-test-app-swing` shells out to `javac -source 8 -target 8` and every consumer launches `java -cp <classes>` with whatever `java` is on PATH. Three consumers exist: the `justfile` recipes, the JAB live tests (`crates/provider-jab/tests/live_fixture.rs`, which build their own command line because they toggle the AccessBridge `-D` flag per launch), and the Robot acceptance suites (`tests/acceptance/swing/resources`). The `swing-test-app` spec currently *requires* the no-build-system property — this change deliberately retires that requirement in favor of the fixture-wide Gradle pattern established by `add-swt-test-app`.

## Goals / Non-Goals

**Goals:**

- One build story for all three Java fixtures (wrapper, toolchains, Foojay auto-provisioning, same README shape).
- Apply the toolchain policy to Swing: compile on JDK 21 with `--release 8`, test on a genuine, explicitly selected Java 8 runtime.
- Zero changes to the Java sources and zero changes to what tests assert.

**Non-Goals:**

- No change to the fixture's CLI, control set, accessible names, or staging plan.
- No switch of consumers to `installDist` start scripts (see decision 2).
- No Linux lane changes.

## Decisions

1. **Same scaffold as the SWT fixture**: checked-in Gradle wrapper (8.x pin — runs on the PATH `java` 8+), `java` plugin, JDK 21 compile toolchain + Foojay resolver, `options.release = 8`, plus the provisioned Java 8 launch toolchain and the toolchain-home resolution task from `add-swt-test-app`. No external dependencies — the fixture stays JDK-API-only, so the build file is minimal.

2. **Keep the classes-dir launch contract, not `installDist`.** The JAB live tests must own the full `java` command line: they launch the fixture with the AccessBridge system property enabled, disabled (empty override), and with varying flags — an `installDist` start script would bury that behind `JAVA_OPTS` indirection and change three consumers for no gain. So `PLATYNUI_TEST_APP_SWING_CLASSES` keeps its meaning; the path behind it moves from `build/classes` to Gradle's `build/classes/java/main` (the live tests' built-in fallback path is updated once). *Alternative:* `installDist` like SWT — right for fixtures with library dependencies on the classpath; wrong here, where the classpath is a single directory and callers need `-D` control.

3. **Explicit runtime selection instead of PATH `java`.** Consumers resolve the launch JVM from an env var (e.g. `PLATYNUI_TEST_APP_JAVA`, set by the acceptance recipe to the provisioned Temurin 8's `java`) and fall back to PATH `java` for ad-hoc local runs. This pins what the lane actually tests (legacy Swing = Java 8) even after the machine's PATH JDK eventually moves to 21, and lets a dual-runtime smoke launch the same classes on 8 and 21.

4. **Soft-skip condition changes with eyes open**: today the lane skips Swing suites when `javac` is missing; afterwards it skips when the Gradle build fails (typically: no network on first run). The recipe's warning names the actual cause. The old "works fully offline with just a JDK" property is consciously traded for toolchain consistency — recorded in the retired spec requirement.

## Risks / Trade-offs

- [First build needs network (Gradle + JDK downloads) where `javac` alone used to suffice] → accepted trade-off for one unified pattern; caches are user-level and shared with the SWT/JavaFX fixtures, so in practice it is one download for all three.
- [Consumers hardcode the classes path fallback (`live_fixture.rs`)] → single constant, updated in this change; the env var remains the primary mechanism.
- [Gradle's default `build/` layout differs from the current hand-made `build/classes`] → keep Gradle defaults (`build/classes/java/main`) rather than bending Gradle to the old path; one-time consumer update is cheaper than a nonstandard build file forever.
- [Spec history: a requirement is retired, not just reworded] → the delta spec makes the replacement explicit (REMOVED + ADDED) so the archived lineage stays honest.

## Migration Plan

1. Land `add-swt-test-app` first (pattern + runtime-selection machinery).
2. Add the Gradle scaffold next to the existing sources; switch the `just` recipes; update the two consumer path/runtime spots; delete the `javac` recipe bodies.
3. Verification: JAB live tests and the Swing acceptance suites green on the provisioned Java 8 runtime; dual-runtime smoke on 21.

Rollback: restore the `javac` recipe bodies and the old fallback path — the sources never changed.

## Open Questions

- Env-var name for the launch JVM (`PLATYNUI_TEST_APP_JAVA` vs. per-app variants) — align with what `add-swt-test-app` implements.
