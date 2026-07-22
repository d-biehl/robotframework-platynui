## Why

With the SWT and JavaFX fixtures (changes `add-swt-test-app`, `add-javafx-test-app`) the repo standardizes on the self-bootstrapping Gradle toolchain — wrapper on the PATH JVM, JDKs auto-provisioned via the Foojay resolver. The Swing fixture is the odd one out: a hand-rolled `javac` recipe that hard-requires a JDK (`javac`) on PATH, silently couples the test runtime to whatever `java` happens to be on PATH, and duplicates none of the toolchain benefits (explicit runtime selection, one build pattern to maintain). Migrating it aligns all three Java fixtures on one build story and applies the agreed policy — build on JDK 21, target Java 8 bytecode, test on a genuine Java 8 runtime — to the fixture where Java 8 matters most (legacy Swing/JAB apps).

## What Changes

- `apps/test-app-swing` becomes a **self-contained Gradle project** like the SWT fixture: checked-in wrapper (current Gradle; the wrapper client runs on the PATH `java` 8+, the daemon JVM is auto-provisioned via committed daemon JVM criteria), JDK 21 compile toolchain with `--release 8` (sources stay Java-8 and platform-neutral — no source changes), auto-provisioned Temurin 8 as the default launch runtime.
- **The launch contract stays**: consumers keep building their own `java -D… -cp <classes> platynui.testapp.Main` command line (the JAB live tests must control the AccessBridge flag per launch), and `PLATYNUI_TEST_APP_SWING_CLASSES` keeps meaning "the compiled classes directory" — only the path behind it moves to Gradle's output. Launchers additionally honor an explicit runtime selection (provisioned Java 8 by default) instead of the ambient PATH `java`.
- `just build-test-app-swing` / `run-test-app-swing` recipes switch from `javac`/`java` to the wrapper; the acceptance lane's soft-skip condition changes from "no `javac` on PATH" to "fixture not built (e.g. no network on first run)".
- **BREAKING** (dev-workflow only, no shipped behavior): building the Swing fixture now requires network access on first run instead of a local JDK with `javac`; machines only need a `java` 8+.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `swing-test-app`: the "Buildable with a plain JDK via just recipes (no Maven/Gradle)" requirement is replaced by the self-bootstrapping Gradle-wrapper build, and an explicit dual-runtime requirement (runs on provisioned Java 8 and on the JDK 21 toolchain) is added. CLI conventions, accessibility enablement, naming discipline, control coverage, and platform-neutral sources are unchanged.

## Impact

- **Modified**: `apps/test-app-swing/**` (Gradle build files + wrapper added; Java sources untouched), `justfile` (build/run/acceptance recipes), `crates/provider-jab/tests/live_fixture.rs` (classes-dir fallback path + explicit runtime selection), `tests/acceptance/swing/resources` (launch keyword runtime selection), `apps/test-app-swing/README.md`, root `Cargo.toml` comment (exclude entry itself stays).
- **No Rust/Python production code changes, no native rebuild.** The `swing-test-app` spec changes (delta included).
- **Depends on**: `add-swt-test-app` (establishes the Gradle pattern and the runtime-selection machinery this change reuses). Best implemented directly after it.
