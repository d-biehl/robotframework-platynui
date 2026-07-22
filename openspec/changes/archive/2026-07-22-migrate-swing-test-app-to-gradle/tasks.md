## 1. Gradle scaffold (after add-swt-test-app has landed)

- [x] 1.1 `apps/test-app-swing`: add wrapper (Gradle 8.x pin) + `build.gradle.kts` (`java` plugin, JDK 21 toolchain via Foojay resolver, `options.release = 8`, no dependencies) next to the untouched sources; reuse the toolchain-home resolution task pattern from `add-swt-test-app` for the provisioned Java 8 launch runtime
- [x] 1.2 `justfile`: replace the `javac` bodies of `build-test-app-swing` (unix + windows) with the wrapper invocation; `run-test-app-swing` keeps the explicit `java -D… -cp` launch but resolves the JVM from the runtime selection (provisioned 8 default, PATH fallback)
- [x] 1.3 `apps/test-app-swing/README.md`: update build/run documentation (self-bootstrapping toolchain, network-on-first-build note); adjust the root `Cargo.toml` exclude comment if it mentions "no build system"

## 2. Consumer updates (launch contract preserved)

- [x] 2.1 `justfile` acceptance recipe: point `PLATYNUI_TEST_APP_SWING_CLASSES` at Gradle's classes output; soft-skip message now names the build (network) cause instead of missing `javac`
- [x] 2.2 `crates/provider-jab/tests/live_fixture.rs`: update the `swing_classes_dir()` fallback path; launch `java` from the explicit runtime selection env var (PATH fallback) instead of bare `Command::new("java")`
- [x] 2.3 `tests/acceptance/swing/resources`: launch keywords resolve the JVM the same way; no locator or assertion changes

## 3. Verification

- [x] 3.1 `just build-test-app-swing` from a clean checkout succeeds with only a `java` 8+ on PATH; dual-runtime smoke: the same classes start and honor `--auto-close` on the provisioned Java 8 and on the JDK 21 toolchain
- [x] 3.2 JAB live tests (including the bridge-off classification scenario) and the full Windows acceptance lane green with the fixture running on the provisioned Java 8 runtime; `just check` / `just test` unaffected
