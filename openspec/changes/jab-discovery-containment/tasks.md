# Tasks

Tests come first in each group. The mock provider has no Access Bridge, so every property is proven either:

- by unit tests against the new seams, which run without the DLL (Windows `just test`, since the JAB crate is Windows-only);
- or by live tests against the Swing fixture in the Windows acceptance lane.

No Robot Framework suite is added: the property lives inside the provider, and the Rust live test shows it on the same fixture and runtime.

Before starting, `snapshot-validity` is committed. This change builds on its `.config/nextest.toml` serialization, its `fixture_application` test helper and `JabAppNode`'s process identity.

## 1. Measure the bridge

- [ ] 1.1 Write a throwaway ignored test (design decision 1). Setup:
  - two bridge-enabled fixture JVMs, with the agent off and `call_timeout_ms = 750`;
  - A wedged with `--wedge-after 3 --wedge-for 20`, B healthy;
  - timed debug records around every bridge call.

  Measure:
  - which calls against A block while its event thread is wedged, compared with a whole-process freeze (`set_process_frozen`);
  - whether a stuck call returns before the wedge ends;
  - whether `isJavaWindow` about B's window, asked first after A's wedge begins, blocks.

  Record the numbers and the chosen re-probe call under *Spike results* in `design.md`, then delete the test. Verify by reading `design.md`: the section is filled, and no spike code remains in the tree.

## 2. Failing live tests first

- [ ] 2.1 Fixture plumbing in `crates/provider-java/tests/live_fixture.rs`:
  - `FixtureApp` takes app arguments, modelled on `FixtureModes` in `crates/java-agent/tests/live_fixture.rs:53-78`;
  - it can capture stderr through a reader thread that timestamps lines, so a test can sync on "[TestApp] wedging this event queue" and "[TestApp] event queue released";
  - a process-wide tracing subscriber with an in-memory writer replaces the `tracing` placeholders at `:58-60`;
  - the file header (`:1-18`) lists the new tests and says that both crates run in the lane.

  Verify that every existing live test still passes with `cargo nextest run -p platynui-provider-java --run-ignored ignored-only`, using the lane's environment variables.
- [ ] 2.2 `live_a_wedged_jvm_does_not_black_out_the_other_jvms`, for the scenarios of `jab-provider`:
  - *A discovery pass during a stall is bounded*;
  - *A Swing window keeps its place while the bridge does not answer*;
  - *The other JVM's windows return once the bridge answers*;
  - *A wedged JVM is named once*;
  - *A healthy JVM is never named*;

  and of `jab-hit-test`: *A held bridge fails a hit-test at once*.

  Setup: the two JVMs of 1.1, with the wedge at `--wedge-after 3 --wedge-for 15`. Collect everything during the wedge and assert at the end with a summary message:
  - every pass bounded;
  - exactly one warning naming the wedged pid;
  - no warning and no degradation for the healthy pid;
  - no UI Automation shell for either Swing title;
  - the healthy `app:Application` valid;
  - a prompt hit-test error over the healthy window;
  - both windows back after the release.

  Use the timing expectations of 1.1. Verify on Windows that it fails today, on the bounded passes, the warning, the healthy JVM's degradation and the UI Automation shells.
- [ ] 2.3 `live_a_java_21_window_is_discovered_through_its_awt_class`, for *A modern JDK's window is discovered through its AWT class*. The lane recipe (`justfile:386`) hands over the `java21` launcher from `java-launchers.properties` as `PLATYNUI_TEST_APP_SWING_JAVA21`, and a missing launcher fails the test rather than skipping it. Verify that it passes today and pins the class for 5.1.

## 3. Failing tests first — the dispatcher

- [ ] 3.1 Split the dispatcher so that it runs without the DLL (design decision 10):
  - the job and its runner are generic over the job argument;
  - `pump_main` keeps only the DLL and Win32 parts and runs jobs through the shared runner;
  - `JabClient` wraps the dispatcher.

  This is a refactor with no behavior change. Verify that `just test-crate platynui-provider-java-jab` still passes.
- [ ] 3.2 Unit tests with a fake pump thread and a test-controlled gate standing in for a stuck call, for the scenarios of *Robustness against unresponsive JVMs*:
  - a call that never started fails as busy, never runs and is not counted (defect today);
  - a call that ran through the deadline is counted, and the threshold degrades its JVM;
  - a call that started late and ends within the deadline is not counted;
  - calls given up during a stall do not run afterwards;
  - after a stall of at least one deadline, the next call fails in under a quarter deadline without being enqueued;
  - a call behind a busy job that finishes within the deadline succeeds;
  - a health probe that never started does not use up its slot;
  - "given up" and "started" are decided exactly once per call, checked over 1000 calls with a near-zero deadline.

  Verify that the defect cases fail today with `just test-crate platynui-provider-java-jab`.

## 4. The dispatcher

- [ ] 4.1 Job state and blame (design decision 2):
  - *queued*, *running* and *given up*, with one compare-and-swap between caller and pump;
  - blame by the job's own run time, recorded once;
  - `JabError::PumpBusy` and its mapping to `CommunicationFailure`, with a test in `error.rs`.

  Verify that the blame cases of 3.2 pass.
- [ ] 4.2 The in-flight record and the busy check (design decision 3), covering jobs and handle releases. Verify that the busy and given-up cases of 3.2 pass.

## 5. Failing tests first — discovery, router, hit-test

- [ ] 5.1 Split `discover_java_windows` into a pure pass over window candidates behind a `WindowProbe` trait, and add a scripted probe with a call log and a controlled clock (design decision 5). Tests:
  - non-AWT candidates are never probed;
  - a window that times out is set aside and the pass goes on;
  - `PumpBusy` means not asked, not set aside;
  - a set-aside window is not asked before its re-probe is due, then is asked, with doubling up to 16 s and a reset once it answers;
  - held windows are reported: set aside, plus previously served but not asked;
  - a context lookup that times out sets the window aside;
  - pid-filtered discovery skips another pid's set-aside window;
  - a set-aside entry is forgotten when its window or process is gone;
  - an excluded window is neither probed nor set aside;
  - no start-up wait without an AWT candidate.

  Verify that the behavioral cases fail today with `just test-crate platynui-provider-java-jab`.
- [ ] 5.2 Router tests in `crates/provider-java/src/provider.rs`, extending its stub backend with held windows. A held window:
  - is claimed;
  - yields no node;
  - gets no enablement diagnostic;
  - keeps its pid among the attach candidates;
  - is recorded as the Access Bridge's own in the ownership map.

  Verify that they fail today with `just test-crate platynui-provider-java`.
- [ ] 5.3 A pure test of the hit-test gate: a root window whose class is not AWT is unsupported without a bridge call. Verify that it fails today.

## 6. Discovery, router, hit-test

- [ ] 6.1 The class gate through `JavaToolkit::from_window_class` in:
  - discovery;
  - the suspect check;
  - the no-DLL pass;
  - the hit-test gate (`window_class_of`).

  Correct the JavaFX remark at `crates/provider-java-jab/src/node.rs:911-912`. Verify 5.3, the class-gate case of 5.1, and the existing no-DLL test.
- [ ] 6.2 The shared state next to the degraded tracker: the in-flight record, the set-aside windows, the vm→pid map and room for the latch (design decision 4).
  - Setting windows aside, re-probing and held windows in the pure pass.
  - `java_windows` passes the state on. Update the source-text test at `provider.rs:854-860` with it.

  Verify the remaining cases of 5.1.
- [ ] 6.3 Skip the start-up wait without an AWT candidate, and make the doc at `provider.rs:43-47` match. Verify its case in 5.1.
- [ ] 6.4 `JabEnumeration` carries held windows, and the router acts on them (design decision 6):
  - it claims them;
  - it records them in the ownership map;
  - it builds no node for them;
  - it leaves them out of the enablement diagnostics;
  - it keeps their pids among the attach candidates.

  Verify 5.2.

## 7. The report

- [ ] 7.1 Unit tests for the scenarios of *A JVM that does not answer is reported once per episode, naming its process*, using the log capture at `provider.rs:688-717`:
  - one warning per episode, with `pid`, `call`, `timeout_ms`, `elapsed_ms` and the consequence;
  - debug records while it lasts;
  - one debug record at its end, and nothing at info;
  - a new warning after recovery;
  - the end when the process has ended;
  - the process of a vm-targeted call found through the vm→pid map;
  - a healthy JVM never named;
  - every record emitted on the calling thread.

  Verify that they fail today.
- [ ] 7.2 Implement the latch on `platynui_core::diagnostics::Transitions` (design decision 8), and bring the touched records in line:
  - drop the degraded warning at `pump.rs:68`;
  - `pump.rs:75` becomes the latch's debug end;
  - the discovery records at `provider.rs:585` and `:603` gain `error`, `window` and `pid`;
  - each timeout gets one debug record with call, vm and `timeout_ms`.

  Verify 7.1, and that `just check` is clean.

## 8. Documentation

- [ ] 8.1 Correct the containment claims:
  - the module docs of `crates/provider-java-jab/src/pump.rs:1-15` and `client.rs:1-8`;
  - `dev-docs/platform-windows.md` at the threading model (`:114`), hit-testing (`:143`) and the enablement diagnostics (`:145`).

  They must say what a stall costs: one deadline per pass, a warning naming the process, held windows keeping their claims, no service until the bridge answers. Also:
  - that only AWT windows are asked;
  - the Java 21 check in the lane's description.

  `fix-jab-hit-test-virtual-children` rewrites the same `:143` paragraph; whichever lands second merges. Verify by reading.

## 9. Verification

- [ ] 9.1 Run `just check` and `just test` on Windows, then `just build-native`. The router change is portable code, so also run clippy for `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`, as for `snapshot-validity`. Verify that everything is green.
- [ ] 9.2 With the maintainer's go-ahead, run `just install-provider-java`, then `just test-acceptance-windows`. Verify that:
  - the live tests are green, 2.2 and 2.3 included;
  - the Robot lane is green;
  - `output.xml` holds no WARN or ERROR message, in particular none from the JAB backend in a healthy run.

  Record the outcome in this task.

## 10. Commit (only when the maintainer asks)

- [ ] 10.1 Commit in reviewable steps, each lint-clean on its own:
  - the dispatcher;
  - discovery, router and hit-test;
  - the report;
  - the live tests and the docs.

  Subjects are at most 72 characters. The bodies list the behavior changes for the release notes (no breaking framing, PlatynUI is 0.x): calls fail sooner during a stall, windows keep their claims, the new warning replaces the degraded warning, and non-AWT windows are no longer offered to the bridge.
