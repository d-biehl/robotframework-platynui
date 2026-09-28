# Tasks

Tests come first in each group. The mock provider has no Access Bridge, so every property is proven either:

- by unit tests against the new seams, which run without the DLL (Windows `just test`, since both crates are Windows-only);
- or by live tests against the Swing fixture in the Windows acceptance lane.

No Robot Framework suite is added: the property lives inside the provider, and the Rust live test shows it on the same fixture and runtime.

`snapshot-validity` has landed (`8f6fc02`…`1f09f50`). This change builds on its `.config/nextest.toml` serialization, its `fixture_application` test helper and `JabAppNode`'s process identity.

## 1. Fixture plumbing and the measurement

- [ ] 1.1 Fixture plumbing in `crates/provider-java/tests/live_fixture.rs`:
  - `FixtureApp` takes app arguments, modelled on `FixtureModes` in `crates/java-agent/tests/live_fixture.rs:53-78`, and an optional launcher, defaulting to `swing_java_launcher()`;
  - it can capture stderr through a reader thread that timestamps each line, so a test can sync on "[TestApp] wedging this event queue" and "[TestApp] event queue released";
  - a process-wide tracing subscriber with an in-memory writer replaces the `tracing` placeholders at `:58-60`.

  Verify that every existing live test still passes with `cargo nextest run -p platynui-provider-java --run-ignored ignored-only`, using the lane's environment variables.
- [ ] 1.2 The spike of design decision 1, on the plumbing of 1.1. Add temporary timing records around each pump phase (job, release, message dispatch) and each bridge call. Write a throwaway ignored test with two fixture JVMs, agent off and `call_timeout_ms = 750`. B is healthy and served before any wedge.

  The first call that reaches a wedged EDT holds the pump until the wedge ends or the bridge client gives up. So run one wedge per probed call: relaunch A each time, or use one fixture with repeated wedges. Issue exactly that call first after the "wedging" line, and record whether and when it returns before "released". Do the same with a whole-process freeze through `set_process_frozen`.

  Answer the four questions of decision 1:
  1. which of `isJavaWindow`, `getAccessibleContextFromHWND`, `isSameObject`, `getVersionInfo` and `getAccessibleContextInfo` reach A's EDT;
  2. whether and when a stuck call returns, and with what;
  3. whether a window call about B's window blocks on A: B's call issued first in its own wedge;
  4. which pump phase blocks while another bridge-enabled JVM starts up. For this, also rerun the `snapshot-validity` 9.2 overlap: the java-agent live binary temporarily outside the `java-live` group, and `one_wedged_toolkit_world_does_not_disable_the_others` excluded. Does the JAB test then see its fixture?

  Delete the test and every temporary record afterwards. Verify with `git status` that no spike code remains.
- [ ] 1.3 Record the results under *Spike results* in `design.md`, and apply them:
  - the re-probe and health-probe call, which reaches the EDT;
  - the call 2.1 uses to hold the bridge: a vm-targeted call aimed at A that reaches its EDT;
  - the 250 ms bound, raised in the `jab-provider` delta if a normal answer takes longer;
  - the re-probe cap, raised in the `jab-provider` delta if needed, in:
    - *Asking again*;
    - *A set-aside window is asked again on a doubling interval*;
    - *The other JVM's windows return once the bridge answers*;
  - whether the bridge client sends window calls to the window's own JVM alone, which decides the attribution of window-call failures;
  - which held-bridge observations the live test of 2.1 makes.

  Verify by reading, and with `openspec validate jab-discovery-containment --strict`.

## 2. Failing live tests first

- [ ] 2.1 `live_a_wedged_jvm_does_not_black_out_the_other_jvms`, following the sequence of design decision 10. It covers these scenarios of `jab-provider`:
  - *A discovery pass during a stall is bounded*;
  - *A Swing window keeps its place while the bridge does not answer*;
  - *The other JVM's windows return once the bridge answers*;
  - *A wedged JVM is reported once*;
  - *A healthy JVM is never named as not answering*;

  and, if 1.3 allows it live, *A held bridge fails a hit-test at once* of `jab-hit-test`.

  Setup:
  1. Launch B, wait until its window and `app:Application` are served, and capture its window node, vm (from its RuntimeId), bounds and application node.
  2. Launch A with `--wedge-after` long enough that A is served before the wedge, and `--wedge-for 15`.
  3. Once A is served and before the fixture's wedging line, activate B again, and assert that the root window under B's centre is B's.
  4. After the wedging line, make the hold call of 1.3 on A's window node once, so the bridge is held whatever discovery asks.

  During the wedge, collect:
  - timed passes, each marked when the 1.1 capture holds a "held window asked again" record (6.2) between its start and end;
  - reads of B's window name, at least three;
  - enumerations by a UI Automation provider in the same process: the shells they show, and that each returns (within 20 s, as in `live_frozen_jvm_stays_contained`);
  - B's `app:Application` validity;
  - the hit-test over B, if allowed.

  After the release, read B again, starting at least one deadline after the fixture's released line, and check that both windows return.

  Anchor all timings to the fixture's stderr lines. Exempt a marked pass from the half-deadline bound. Assert at the end with a summary message that names the scenario of every failed assertion.

  Verify on Windows that it fails today on:
  - the bounded passes;
  - the report, where today's only warning, "JVM marked degraded", names B's vm;
  - the UI Automation shells.
- [ ] 2.2 `live_a_java_21_window_is_discovered_through_its_awt_class`, for *A modern JDK's window is discovered through its AWT class*.
  - In the lane recipe (`justfile:386`), inside the existing `if (Test-Path …)` block, add `$env:PLATYNUI_TEST_APP_SWING_JAVA21 = ((Get-Content -Raw "{{ swing_app_launchers }}") | ConvertFrom-StringData).java21`.
  - The test launches the fixture through that launcher (1.1), and a missing launcher fails the test rather than skipping it.

  Verify that it passes today and pins the class for 6.1.

## 3. Failing tests first — the dispatcher

- [ ] 3.1 Split the dispatcher so it runs without the DLL (design decision 10):
  - the pump loop, with its message, release and job phases, moves into a runner generic over the job argument;
  - `pump_main` supplies only the Win32 message pump and `releaseJavaObject` as closures;
  - the vm bookkeeping of `JabClient::call` and the health probe move into the dispatcher, with the probe's job supplied by the client;
  - `JabClient` wraps the dispatcher.

  Also add, with today's semantics and not yet published or used, so the tests of 3.2 compile:
  - `JabError::PumpBusy`, with its holder, mapped to `CommunicationFailure`;
  - the job-state enum;
  - the start time on `JabError::Timeout`, an `Option` left unset;
  - jobs that carry their call, vm, window and pid;
  - the shared state of decision 4 with all its parts, empty and unused: the in-flight record type with its holder kinds (job, release, and message handling with an optional sending window and pid), the set-aside store, the held-window memory and the vm→pid map.

  Verify that `just test-crate platynui-provider-java-jab` still passes.
- [ ] 3.2 Unit tests with a fake pump thread whose phases run under test-controlled gates standing in for a stuck call, release or message. They cover the scenarios of *Robustness against unresponsive JVMs*. Each case is marked with what it does today:
  - a call that never started fails as busy, never runs and is not counted (fails today);
  - B's next call succeeds as soon as A's stuck call has returned (passes today);
  - a call aimed at a JVM that ran through its own deadline is blamed, and the threshold degrades its JVM (passes today);
  - a late-started call that ends within its own deadline is not blamed, and its `Timeout` carries its start time (fails today);
  - a window call that runs through its own deadline sets its window aside, once, whichever of the busy check or the pump records the blame; a late-started one that ends within its own deadline does not (fails today; S2);
  - calls given up during a stall do not run afterwards (fails today);
  - right after one caller has given up on a running job, the next call fails within a quarter deadline without being enqueued (fails today);
  - when that job started late and then ends within its own deadline, the busy failure records no condition, and a later desktop pass adds no warning (fails today);
  - after a stall of at least one deadline, the same (fails today);
  - a call behind a busy job that finishes within the deadline succeeds (passes today);
  - a message dispatch held past the deadline makes the next call fail at once, recorded as the bridge's message handling (fails today);
  - a release to a JVM that is not degraded, held past the deadline, makes the next call fail at once (fails today);
  - a panicking job fails that call only, leaves no in-flight record, and the next call succeeds (fails today);
  - a health probe that never started, or met `PumpBusy`, does not use up its slot (fails today);
  - "given up" and "started" are decided exactly once per call, checked over 1000 calls with a near-zero deadline (fails today).

  Verify with `just test-crate platynui-provider-java-jab` that the cases marked as failing today fail, and the others pass.

## 4. The dispatcher

- [ ] 4.1 Job state and blame (design decision 2):
  - *queued*, *running* and *given up*, with one compare-and-swap between caller and pump;
  - blame by the job's own run time, recorded once, by the pump as state only;
  - blame of a window job sets its window aside in the shared state (decision 5);
  - `PumpBusy` returned for a job that never started;
  - `Timeout` carrying the start time;
  - a health probe that never started, or met `PumpBusy`, gives its slot back.

  Verify the blame, set-aside, given-up and health-probe cases of 3.2.
- [ ] 4.2 The in-flight record around jobs, releases and message dispatch, cleared by a guard, with each job under `catch_unwind`. The busy check, including the given-up flag (design decision 3). Verify the busy, dispatch, release and panic cases of 3.2.

## 5. Failing tests first — discovery, router, hit-test

- [ ] 5.1 Split `discover_java_windows` and the start-up wait into a pure layer over a candidate source, a `WindowProbe` trait, the shared state and a clock. Add a scripted probe with a call log. From the start, the pure pass takes the set-aside store and the vm→pid map, and returns a held list, all empty and unused today. Tests:
  - non-AWT candidates are never probed;
  - no start-up wait and no call without an AWT candidate this backend could serve; a wait up to its deadline with one;
  - a window the store holds as set aside by a blamed call, seeded as 4.1 records it, is held without a call in the next pass, while the other windows are asked (S1);
  - a `Timeout` or `PumpBusy` the pass receives leaves the window not asked, and a not-asked window is asked normally in the next pass (S2);
  - in one pass a window is not asked because of `PumpBusy`; in the next pass, whose probe answers, it is served, and its pid is among the pass's application pids (S1);
  - a false answer after 0.8 of the deadline, and after 5 s with a 10 s deadline, sets the window aside and makes no suspect; a prompt false makes a suspect;
  - a context lookup that answers no context after more than 250 ms sets the window aside; a prompt one drops the window, as today;
  - with failures attributed, re-probes come 1 s, 2 s, 4 s, 8 s, then every 16 s apart, and the schedule resets once the window answers;
  - with window calls scripted to reach every JVM, the unattributed set-aside windows share one interval that each failed re-probe doubles, and a window call answering within 250 ms resets it and makes them due from the next pass on, while a slow answer does neither;
  - a re-probe that meets `PumpBusy` does not double the interval;
  - a window served before a stall stays held across three busy passes;
  - a pid-filtered pass neither drops a held window nor forgets a set-aside entry;
  - an entry is forgotten only by a desktop pass that no longer sees its window, or when its process has ended;
  - an excluded window is neither probed nor set aside;
  - a served window teaches the vm→pid map.

  Verify that the behavioral cases fail today.
- [ ] 5.2 Router tests in `crates/provider-java/src/provider.rs`. First add an empty `held_windows` list, with pids and a "served before" flag, to `JabEnumeration` and to the router's `Enumeration` (`backend.rs:51-77`). Then extend the stub backend (`provider.rs:460-542`) to return it. The tests cover:
  - *A held window stays claimed without a node* (`java-provider`);
  - *A held window is shown by no provider* and *A window that does not answer is not reported as bridge-not-enabled* (`jab-provider`);
  - *A point over a held window yields a provider error, not the UIA shell* (`jab-hit-test`).

  Check:
  - a held window served before is claimed, and recorded as the Access Bridge's own;
  - it yields no node;
  - the stub also lists it in `unserved` as `BridgeNotEnabled`, and no enablement diagnostic is emitted for it;
  - its pid is among the attach candidates;
  - a held window never served stays unclaimed, and its pid is among the attach candidates too;
  - a stub whose `element_at_point` answers `CommunicationFailure` makes the Java provider return that error, not `UnsupportedOperation`.

  Verify with `just test-crate platynui-provider-java` that the claim, diagnostic and attach-candidate cases fail today. The no-node, never-served-claim and error cases pass today and stay as guards.
- [ ] 5.3 Extract the hit-test gate into a pure function over the class, the pid, the own pid, the exclusions, the set-aside state, the vm→pid map and a `WindowProbe`. Keep today's behavior: the probe is asked for every class. Tests:
  - a non-AWT root window is unsupported without a probe call;
  - a set-aside root window that the bridge served before, and that is not due, fails with a provider error saying it is set aside, without a probe call;
  - a set-aside root window the bridge never served is unsupported, without a probe call;
  - an AWT root window whose probe answers `PumpBusy` fails as busy, never as unsupported (*A point over a held window yields a provider error, not the UIA shell*, and *A held bridge fails a hit-test at once* at unit level, with the busy cases of 3.2);
  - the own process is unsupported;
  - a resolved window teaches the vm→pid map.

  Verify that the class and set-aside cases fail today on the probe calls, the map case fails on the empty map, and the own-process and `PumpBusy` cases pass.

## 6. Discovery, router, hit-test

- [ ] 6.1 The class gate through `JavaToolkit::from_window_class` in:
  - discovery;
  - the suspect check;
  - the no-DLL pass;
  - the hit-test gate.

  Correct the JavaFX remark at `crates/provider-java-jab/src/node.rs:911-912`. Verify the class cases of 5.1 and 5.3, and the existing no-DLL test.
- [ ] 6.2 Setting windows aside, re-probing, the due rules, the held-window memory, forgetting, and the vm→pid map, all in the shared state (design decisions 4 and 5). Each ask of a window the bridge has not answered about since it became busy, whether a re-probe of a set-aside window or the first ask of a held one, emits one debug record, "held window asked again", with `window` and `pid`. Verify:
  - the remaining cases of 5.1;
  - that the source-text test at `provider.rs:854-860` passes unchanged.
- [ ] 6.3 The start-up wait in the pure layer, and the doc at `provider.rs:43-47` made to match. Verify its cases in 5.1.
- [ ] 6.4 The router acts on held windows (design decision 6):
  - `jab.rs` maps them;
  - `sweep` claims those served before and records them in the ownership map;
  - no node is built for them;
  - they are left out of the enablement diagnostics;
  - their pids join `java_processes`.

  Update the `Enumeration` doc (`backend.rs:54-58`) and the classifier's claim invariant (`crates/core/src/platform/java.rs:151-153`). Verify 5.2.
- [ ] 6.5 The hit-test gate in `element_at_point` (design decision 9). Verify 5.3.

## 7. The report

- [ ] 7.1 Unit tests for the scenarios of *A JVM that does not answer is reported once per episode, naming its process*. They use the log capture at `provider.rs:688-717`, and run the fake pump thread under a clone of the same `Dispatch` with thread names in each line. Cases:
  - one warning per episode, from the desktop pass, with `pid`, `call`, `timeout_ms`, `elapsed_ms` and the consequence, and debug records while it lasts;
  - the same from a stall met while listing an application's windows, and from a read that becomes a missing value;
  - the end, at debug and nothing at info, when:
    - the same call, made again, answers within its deadline;
    - a re-probe or the health probe answers;
    - the process has ended;
  - an episode reported by the message handling or a window asked about ends once the bridge is free and a call answers, and a second such stall warns again, with the bridge message and fields and no `pid`;
  - a stall recorded only by a hit-test, whose episode ends before the next desktop pass, adds no warning in that pass;
  - a window set aside for a slow answer is not reported, and reported once when its re-probe again answers slowly;
  - after the process has ended, a new process under the same pid that stops answering gets a new warning;
  - a prompt other call between two stalls adds no second warning;
  - the stuck call returning after its deadline, followed by a re-probe that stalls again, ends nothing and adds no second warning;
  - identifying the stuck JVM during a stall reported by a window moves the episode, with a debug record and no second warning;
  - a new warning after recovery;
  - a hit-test that fails during a stall adds debug records only, and the next desktop pass adds the warning;
  - a vm-targeted stall on a node from a hit-test, with no discovery pass before it, names the pid;
  - a stuck window call is named per the attribution recorded in 1.3;
  - a dispatch without a sending window names the bridge's message handling;
  - a degraded mark recorded on the pump is reported by the next swallowing layer, on its thread;
  - a healthy JVM is never named as not answering;
  - no captured line about a stall or blame carries the pump thread's name.

  Verify that they fail today.
- [ ] 7.2 Implement the recording and the latch on `platynui_core::diagnostics::Transitions` (design decision 8), with the process identities and `retain` per desktop pass. Bring the touched records in line:
  - drop the degraded warning at `pump.rs:68`;
  - `pump.rs:75` becomes the latch's debug end;
  - the discovery records at `provider.rs:585` and `:603` gain `error`, `window` and `pid`;
  - each timeout gets one debug record with call, vm and `timeout_ms`;
  - the reads that turn a failure into a missing value report through the latch: `info_opt` and `is_valid` in `node.rs`, and the interface reads in `interfaces.rs`.

  Verify 7.1, and that `just check` is clean.

## 8. Documentation and coordination

- [ ] 8.1 Correct the containment claims:
  - the module docs of `crates/provider-java-jab/src/pump.rs:1-15` and `client.rs:1-8`;
  - `dev-docs/platform-windows.md`:
    - the threading model (`:114`);
    - handle discipline (`:117`): release deferral protects only degraded JVMs;
    - hit-testing (`:143`);
    - the enablement diagnostics (`:145`).

  They must say what a stall costs:
  - one deadline per pass, a warning, held windows keeping their claims, and no service until the bridge answers;
  - with the agent on, the agent's own deadline;
  - that only AWT windows are asked.

  Also:
  - the Java 21 check in `justfile:376-379`, `apps/test-app-swing/build.gradle.kts:36-45` and `apps/test-app-swing/README.md:42-47`, which say the live tests read only `java8`;
  - the `live_fixture.rs` header (`:1-18`): its test list, and that both crates run in the lane;
  - the comments and assertion messages of `live_frozen_jvm_stays_contained` (`live_fixture.rs:598-607`, `:661-662`): follow-up calls now fail as busy while the call is stuck, and nothing gets degraded by them.

  `fix-jab-hit-test-virtual-children` rewrites the same `:143` paragraph; whichever lands second merges. Verify by reading.
- [ ] 8.2 Add this change to the Coordination section of `openspec/changes/application-process-attributes/proposal.md`. Name the functions both changes touch:
  - `enumerate_visible_top_level_windows`;
  - `awt_windows_without_bridge`;
  - the hit-test's `top_level_window_at` and the orphan application node.

  Verify with `openspec validate application-process-attributes --strict`.

## 9. Verification

- [ ] 9.1 Run `just check` and `just test` on Windows, then `just build-native`. Both crates are Windows-gated (`crates/provider-java/src/lib.rs:28-35`), so no cross-target check applies. Verify that everything is green.
- [ ] 9.2 With the maintainer's go-ahead, run `just install-provider-java`, then `just test-acceptance-windows`. Verify that:
  - the live tests are green, 2.1 and 2.2 included;
  - the Robot lane is green;
  - its `output.xml` holds no WARN or ERROR message, and in particular none from the JAB backend (*A healthy run of the Robot suites reports no JAB warning*).

  Record the outcome in this task.

## 10. Commit (only when the maintainer asks)

- [ ] 10.1 Commit in reviewable steps, each lint-clean on its own:
  - the fixture plumbing and the live tests;
  - the dispatcher;
  - discovery, router and hit-test;
  - the report;
  - the docs.

  Subjects are at most 72 characters. The bodies list the behavior changes for the release notes (no breaking framing, PlatynUI is 0.x):
  - calls fail sooner during a stall;
  - served windows keep their claims;
  - the new warning replaces the degraded warning;
  - non-AWT windows are no longer offered to the bridge;
  - a hit-test over a non-AWT window no longer waits for the bridge.
