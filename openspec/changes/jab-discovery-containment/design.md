# Design

## Context

See proposal.md for the motivation and the specs for the required behavior.

Line numbers are those of commit `1cf0020`, where `snapshot-validity` has landed. *Verified* marks what was read in the code. *Inferred* marks conclusions from the code. *External* marks facts about the JDK's bridge that the repository cannot show.

**The pump and its jobs (verified).**

- **The job type.** `Job` holds only `run: Box<dyn FnOnce(&Bridge) + Send>` (`crates/provider-java-jab/src/pump.rs:34-37`). It carries no call name, no target and no state.
- **The loop.** Each pass (`pump.rs:161-170`):
  1. pumps Win32 messages and dispatches them into the bridge DLL's window procedure (`pump_pending_messages`, `:183-195`);
  2. drains handle releases, each a blocking call into a JVM (`:197-225`);
  3. runs at most one job.

  While any of the three is blocked inside the bridge, nothing else runs.
- **Nobody knows what runs.** No record says which job or message is being handled, or since when.
- **Given-up jobs still run.** A job whose caller has already given up runs anyway: the closure ignores the dropped reply channel (`client.rs:182-184`).
- **A panic kills the pump.** A panicking job has no unwind guard and takes the pump down (`pump.rs:166`). This is an open item of the logging-concept review.
- **No test seam.** `Bridge` owns a `libloading::Library` (`dll.rs:103-132`), so a job cannot run in a test.

**The client (verified).**

- **Where the deadline starts.** `call_unchecked` sends the job, then waits `recv_timeout(call_timeout)` (`client.rs:176-192`). The deadline therefore starts at enqueue. A running job has always run for less than the deadline when its caller times out.
- **Who gets blamed.** `call` counts every `Timeout` of a call aimed at one JVM (a vm-targeted call) against that vm (`client.rs:161-171`).
- **Calls without a JVM.** `isJavaWindow` and `getAccessibleContextFromHWND` are sent without a vm (`client.rs:219-238`). The degraded tracker never protects them, and a repeated failure never degrades anything.
- **The health probe.** It runs through `call_unchecked` before the call it guards (`client.rs:197-216`). A caller can therefore wait two deadlines.
  - `probe_due` stamps the attempt before the call (`pump.rs:85-98`), so a probe that only waited in the queue keeps a healthy vm degraded for another second.
- **The error mapping.** `JabError` maps exhaustively to `ProviderError` (`error.rs:45-60`). `Timeout`, `VmDegraded` and `PumpUnavailable` become `CommunicationFailure`.

**The degraded tracker (verified).**

- `DegradedTracker` (`pump.rs:39-99`) marks a vm degraded after 3 consecutive timeouts and probes it at most once a second.
- It warns from the returning layer, naming only the vm (`pump.rs:68`), and records recovery at info (`pump.rs:75`). Both are open findings of the logging-concept review (`openspec/changes/archive/2026-09-27-logging-concept/review-findings.md:503-507`).
- It is `pub(crate)`, so tests outside the JAB crate cannot observe it.

**Discovery (verified).**

- **The loop.** `discover_java_windows` (`provider.rs:559-608`) skips the own process, a pid filter and excluded windows. It then asks `isJavaWindow` about every remaining visible top-level window, whatever its class, and `break`s on the first error (`:582-587`).
- **Windows the bridge says no to.** A window the bridge denies becomes an enablement suspect only if its class starts with `SunAwt` (`:589-597`).
- **A failed context lookup.** The loop goes on and the window is silently dropped (`:600-604`).
- **Application nodes.** `enumerate` builds them only for the pids of served windows (`:349-357`). `java_processes` is the served pids plus the suspects' pids (`:358-363`).
- **The start-up wait.** On the first empty pass after connecting, discovery is repeated for up to 1.5 s (`JabProvider::discover_with_rendezvous_grace`, `:247-259`). This happens whether or not any window looks like Java, although its own doc says it is paid "only when a Java-looking desktop yields nothing yet" (`:43-47`).
- **The lazy path.** The windows under an `app:Application` are listed through the free function `java_windows(&self.client, Some(self.pid), …)` (`node.rs:1416-1419`). It reaches only the client and the exclusions. A unit test pins that call's source text (`provider.rs:854-860`).
- **The no-DLL pass.** It already applies the class rule: `awt_windows_without_bridge` keeps only `SunAwt*` windows (`provider.rs:484-510`). It was split out to be testable without a desktop.

**Hit-testing (verified).**

- **The gate.** `element_at_point` resolves `GetAncestor(GA_ROOT)` of the window under the point (`provider.rs:527`). It answers `UnsupportedOperation` for the own process (`:408-410`). Otherwise it asks `isJavaWindow`, whatever the class (`:420`). It knows the root window's pid and, after the context lookup, its vm (`:403-433`).
- **How a timeout travels.** A timeout becomes `CommunicationFailure`. The router returns any answer that is not `UnsupportedOperation` (`crates/provider-java/src/provider.rs:303-307`). The runtime returns the first such error without asking the next provider (`crates/runtime/src/runtime/input.rs:90-94`).
- **The provider order.** Providers are ordered by technology string (`crates/runtime/src/provider/registry.rs:39-45`), and "Java" sorts before "UIAutomation".
- *Inferred:* while the pump is stuck, a pick over any window, Java or not, fails instead of reaching UI Automation.

**The router (verified).**

- **What the router sees.** It knows a backend's pass only as `crate::backend::Enumeration { served_windows, nodes, unserved, java_processes }` (`crates/provider-java/src/backend.rs:51-77`). `jab.rs:45-65` copies the JAB pass into it field by field. The router's stub backend builds it directly (`provider.rs:506-534`).
- **Claims.** `sync_window_claims` releases the claim of every window missing from the pass (`provider.rs:171-180`, called at `:280`), and the UI Automation provider honors claims.
  - *Inferred:* a Swing window hidden by an aborted pass is listed as its UI Automation shell.
- **Attaching the agent.** `consider_attaching` takes its candidates only from `java_processes` on Windows. It attaches synchronously inside `get_nodes` and then sweeps again (`:255-278`). One attempt costs up to 10 s (`agent/backend.rs:56`, `crates/java-agent/src/attach/mod.rs:43`), at most 3 attempts per process (`agent/backend.rs:65`). The 3 s handshake wait follows only an attach that succeeded (`agent/backend.rs:288-296`, `:306-337`).
  - After a successful attach, the agent backend serves that JVM under its own 5 s deadline (`agent/backend.rs:46`) until its session degrades.
- **Enablement diagnostics.** They are once per window for the life of the process and never re-armed (`crates/core/src/platform/java.rs:181-202`, router `:335-367`). A wrong "enable the bridge" hint would stick.
- **The classifier's claim invariant.** It documents that the JAB provider claims exactly the windows `isJavaWindow` acknowledged. It derives `native:JvmAccessibilityReachable` from the claim (`crates/core/src/platform/java.rs:151-168`).
- **Windows only.** The router crate is Windows-gated in full (`crates/provider-java/src/lib.rs:28-35`), and so is the JAB crate.

**Evidence of the symptom (verified).**

- **The lane runs.** `openspec/changes/snapshot-validity/tasks.md` 9.2 records that a JAB test overlapping the start-up of the java-agent live tests' JVMs never saw its own fixture (3 of 3 for `live_fixture_contract_and_interaction`), together with the `.config/nextest.toml` comment written for them.
- **No record of the cause.** The live tests install no tracing subscriber (`crates/provider-java/tests/live_fixture.rs:58-60`), so no record of those runs shows why.
- *Inferred:* the cause is a bridge call stuck on a starting or wedged JVM, a window call or a rendezvous dispatch, combined with the `break`.
- **A second possible cause.** The overlapping binary also wedges a bridge-enabled JVM for 40 s (`crates/java-agent/tests/live_fixture.rs:715-719`), longer than the JAB test's 20 s discovery deadline. This change does not address that (non-goal). The spike (decision 1) tells the two apart.

**The bridge itself (external, unverified in the repository).**

- **The event thread.** The Java side of the bridge answers window calls about its own windows on the event-dispatch thread (EDT), and waits without a timeout.
- **Fan-out.** The client DLL may send window calls to every connected JVM in turn. If it does, one stuck JVM also blocks calls about other JVMs' windows.
- **Stuck calls.** The client DLL may send with a timeout and return FALSE when it expires. That decides whether a stuck call returns before the stall ends, and what it returns.

## Goals / Non-Goals

**Goals:**

- A stall costs the Access Bridge's part of a discovery pass at most one deadline, and it is reported once.
- Healthy JVMs are never blamed or named for another JVM's stall.
- A window that does not answer never hides the windows of other JVMs once the bridge answers again.
- The bridge is asked only about windows it can serve.

**Non-Goals:**

- **Serving healthy JVMs while one call is stuck inside the bridge client.** That needs a separate, restartable bridge process.
- **Stalls longer than a test's own discovery deadline**, such as the 40 s wedge of the java-agent test above.
- **Changing the in-JVM agent backend.** It has its own deadline and containment.
- **Relaxing the test isolation in `.config/nextest.toml`.** A stall is process-wide by nature, and the new live test creates its own stall inside one test.
- **A configuration switch to probe every window.** See decision 7.

## Decisions

### 1. Measure the bridge first; the outcomes pick constants and one attribution rule

A spike runs before the live assertions are written. It builds on the fixture plumbing (task 1.1) and adds temporary timing records around each pump phase. Setup: two fixture JVMs with the agent off, A wedged with `--wedge-for`, B healthy.

It answers four questions:

1. **Which calls reach A's EDT while it is wedged,** compared with a whole-process freeze: `isJavaWindow`, `getAccessibleContextFromHWND`, `isSameObject`, `getVersionInfo`, `getAccessibleContextInfo`.
   - *Decides:* the call that re-probes a set-aside window and serves as the health probe. It must reach the EDT; otherwise a probe could clear a JVM whose EDT is still wedged. Episodes also end on it (decision 8).
2. **Whether a stuck call returns before the wedge ends, after how long, and with what answer.**
   - *Decides:* the 250 ms bound for a slow "not a Java window" answer (it must sit well above a normal answer and below the client DLL's own timeout), the re-probe cap, and whether the held-bridge hit-test can be shown live or only at unit level.
3. **Whether a window call about B's window, asked first after A's wedge begins, blocks** (fan-out).
   - *Decides:* whether a stuck window call names its window's process as the JVM that does not answer (no fan-out), or only as the window being asked about (fan-out). The spec states both cases.
4. **Which pump phase blocks while another bridge-enabled JVM starts up,** and whether the 9.2 overlap still fails once the 40 s wedge test is excluded.
   - *Decides:* whether the lane symptom is this defect.

The spec is written for every outcome: it states attribution in terms of whether the bridge client sends window calls to the window's own JVM alone. The outcomes fix that property, the constants and the probe call, which task 1.3 records under *Spike results* before any live assertion is written. If the spike raises the 250 ms bound or the 16 s cap, task 1.3 updates the delta.

### 2. A call counts against its JVM only for time it ran itself

Each job gets a state shared between its caller and the pump: *queued*, *running* or *given up*.

- **At the caller's deadline.** Measured from enqueue as today, the caller moves the job from *queued* to *given up* with one atomic compare-and-swap.
  - If that succeeds, the job never runs, and the caller gets a new error, `JabError::PumpBusy`, naming what holds the bridge. Nothing is counted.
  - If the job was already *running*, the caller gets `Timeout`, carrying the job's start time.
- **Running.** The pump moves a job from *queued* to *running* before calling into the bridge, and skips a job it finds *given up*.
- **Blame.** A running job is blamed only when its own run time reaches the deadline. Whoever sees that first records it, once per job: the busy check of decision 3, or the pump when the job ends.
  - Blame counts against the job's vm, if it has one.
  - It sets the job's window aside, if it has one (decision 5).
  - A late-started job that ends within its own deadline is not blamed.
- **The pump records state only.** When the pump records blame, including blame that crosses the degraded threshold, it only changes state. A caller thread picks the change up and reports it (decision 8).
- **The health probe.** It follows the same rule, and a probe that never started does not use up its once-a-second slot.

*Alternatives rejected:*

- **Starting the deadline at dequeue.** A caller could then wait without bound in the queue.
- **Counting only when the queue was empty at enqueue.** That is racy, and still blames a job that started late.
- **Cancelling the stuck call.** An OS call inside the bridge client cannot be cancelled.

### 3. A held bridge fails the next call at once

The pump publishes an in-flight record around every blocking entry into the bridge:

- **jobs;**
- **handle releases,** since a release to a wedged JVM that is not yet degraded holds the bridge just the same;
- **message dispatch.** A dispatched message carries the sending Java window where the message has one (the rendezvous and `WM_COPYDATA` carry it in `wParam`), and with it that window's pid. Otherwise the record says that the bridge's own message handling holds it.

The record holds the call, the target vm, the window and pid where known, the start time, and a flag the caller sets when it gives up on the running job. A guard clears it, also on unwind. Each job runs under `catch_unwind`, so a panic fails that call and not the pump.

**The busy check.** Before enqueueing, the client checks the record. It returns `PumpBusy` at once, without enqueueing, in either case:

- the holder has run for at least the deadline;
- the holder's caller has given up on it.

The second condition closes the gap in the pass where a stall begins: the pass's next call fails at once instead of waiting a second deadline. A call another thread had already queued waits at most its own deadline. A failure on the second condition alone is no evidence that anything does not answer: the holder may have started late and be about to finish. It adds a debug record only, and its `PumpBusy` names the holder as the call holding the bridge, never as a JVM that does not answer (decision 8).

Together with decision 2 this also ends the backlog of abandoned calls that today still runs after a stall. `PumpBusy` maps to `CommunicationFailure`, like `Timeout`.

*Alternative rejected:* a priority queue for discovery calls. It changes the order of waiting, not the fact that nothing runs while the pump is stuck.

### 4. The shared state lives next to the degraded tracker

One `Arc` sits next to `DegradedTracker`, which the provider, the client and the pump already share (`provider.rs:191`, `client.rs:132`, `pump.rs:113`). It holds:

- the in-flight record;
- the set-aside windows with their re-probe schedule;
- the held-window memory: the windows the last desktop pass served or held, with their pids and whether the bridge ever served them;
- the vm→pid map;
- the episode latch with its process identities;
- the conditions recorded for the latch (decision 8).

Rules for this state:

- **The lazy path sees it too.** `java_windows` reaches it through the client it already takes. Its call at `node.rs:1419` and the source-text test at `provider.rs:854-860` therefore stay unchanged, and so do `JabAppNode`'s constructors, which `snapshot-validity` and `application-process-attributes` also touch.
- **Filling the vm→pid map.** Wherever a context is obtained from a window, which knows both its vm and its pid: the desktop pass, the lazy children and the hit-test. Entries are pruned when their process has ended.
- **Who changes held windows.** Only a desktop pass updates the memory of windows served or held in the last desktop pass, and only a desktop pass forgets a set-aside entry. A blamed call on any path, the lazy children and the hit-test included, may set its own window aside (decision 2). A pid-filtered pass never drops a held window.

*Alternative rejected:* fields on `JabProvider`. The lazy path through `java_windows` could not see them.

### 5. Discovery becomes a pure pass that sets windows aside

`discover_java_windows` splits into a pure pass over window candidates and a thin wrapper that keeps `EnumWindows` as the only Win32 part. This follows the precedent `awt_windows_without_bridge`. The bridge is reached through a small `WindowProbe` trait that `JabClient` implements. The pass takes the shared state and a clock, so tests drive it with scripted answers and a controlled clock. The start-up wait moves into the same layer: it decides from the candidates' classes, before any bridge call, whether to wait at all.

**Per candidate, in order:**

1. **Own process, pid filter, exclusions:** as today.
2. **Class gate:** `JavaToolkit::from_window_class(class) == Some(JavaToolkit::SwingAwt)` (`crates/core/src/platform/java.rs:52-62`). Other windows are skipped with no bridge call. The suspect check and the no-DLL pass switch to the same helper, so one definition decides what is an AWT window.
3. **Set aside and not due yet:** held, without a call. That includes a set-aside root window under a hit-test (decision 9).
4. **The calls:**
   - `isJavaWindow` answers true and the context lookup succeeds → served. The vm→pid map learns the vm.
   - `isJavaWindow` answers true and the context lookup answers no context: within 250 ms → dropped, as today; after more than 250 ms → set aside.
   - `isJavaWindow` answers false within 250 ms → suspect, as today.
   - `isJavaWindow` answers false after more than 250 ms → set aside by the pass, never a suspect. The reply carries the job's run time for this.
   - Any `Timeout` or `PumpBusy` the pass receives → *not asked*. A caller never sees blame in its own result, because a running job has always run for less than the deadline when its caller gives up.

   Blame sets a window aside on its own (decision 2): the dispatcher records it in the shared state when a window job runs through its own deadline, whoever observes that. The pass reads the set-aside store, not the call's result. So a window whose call is stuck on its own JVM is set aside by the time the next pass runs, and a window whose call only started late is not.

   A not-asked window is held if the held-window memory has it. A never-served window that was not asked is left alone this pass: unclaimed and not reported.

**Re-probing.**

- **Attribution.** A failure is attributed to the window's own JVM when the call was aimed at that JVM, or when it was a window call and the bridge client sends window calls to the window's own JVM alone (spike question 3). Without fan-out every failure is attributed; with fan-out no window-call failure is.
- **The schedule.** It starts 1 s after a window is set aside. A re-probe fails when it does not answer within the deadline or answers only after more than 250 ms; a re-probe that meets `PumpBusy` was not asked and does not fail. After a failed re-probe whose failure is attributed, the window's own interval doubles, up to 16 s; it resets once the window answers. So a still-wedged JVM is not asked on every pass.
- **The shared interval.** The set-aside windows whose failures are not attributed share one interval, which any of their failed re-probes doubles, up to 16 s. With fan-out, a stuck re-probe cannot tell the healthy window from the wedged JVM's, so both are backed off together: the wedged JVM is not re-probed every second, and the healthy window is not pushed out on its own.
- **Due early.** When a window call answers within 250 ms, the shared interval resets, and every set-aside window whose failures are not attributed becomes due from the next pass on. A slow answer does not count, since it may be a stuck call the bridge client gave up on. With fan-out a prompt window call completes only when no connected JVM is stuck, so this cannot re-probe a still-wedged JVM on every pass, and a healthy window is asked in the first pass after the bridge answers.
- A long wedge therefore costs discovery at most one stuck call per re-probe, and each stuck call holds the bridge for as long as the bridge client blocks (spike question 2). The 16 s cap is raised if that hold is long enough to keep the bridge held more than about a quarter of the time.
- Calls on nodes that callers still hold, and the health probe of a degraded JVM, can still call into a wedged JVM. Hit-tests over a set-aside window cannot (decision 9).

**Forgetting.** A set-aside entry is forgotten when its window is no longer a candidate in a desktop pass, or when its process has ended.

*Alternative rejected:* keeping `break` and adding only the class gate. That still hides every window below a busy AWT window, and the start-up case is itself an AWT window.

### 6. Held windows keep their claims if the bridge served them

Both passes report held windows as a list with their pids and a flag for "the bridge served it before":

- the JAB pass (`JabEnumeration`);
- the router's pass (`crate::backend::Enumeration`, `backend.rs:51-77`), which `jab.rs` maps.

For every held window, the router:

- claims it, if the bridge served it before, and records it in the ownership map as the Access Bridge's own, so the agent can still take it over at its stronger rank;
- builds no node for it;
- leaves it out of the enablement diagnostics;
- adds its pid to `java_processes`.

A set-aside window the bridge never served stays unclaimed. Examples are a JVM whose very first call stalled at start-up, or a window without the bridge whose call was stuck on another JVM. Its UI Automation shell stays as it is today, and no provider claims a window that no backend has shown it can serve.

This extends `java-provider`'s rule that a window is claimed exactly when a backend can serve it, by the held window of a backend that served it. So `java-provider` gets a delta, and so do the two single-appearance requirements, since a held window is claimed but shown by no provider. The classifier's doc (`crates/core/src/platform/java.rs:151-153`) gains the held case. `native:JvmAccessibilityReachable` stays true for a held window, because the bridge did reach it.

*Alternatives rejected:*

- **Releasing held windows to UI Automation.** Users would see, and could act on, empty UI Automation shells under the Swing title. That is worse than a window that is briefly missing, and the report says why.
- **Claiming every set-aside window.** It would hide the UI Automation shell of a window no backend ever served.
- **Taking held pids out of the attach candidates.** A JVM busy at start-up is exactly one that should get the agent, and `java-provider` requires attaching automatically. The cost is under Risks.

### 7. The class gate has no escape hatch

Only AWT windows are asked, in discovery, in the lazy children, in the start-up wait and at the hit-test gate.

Why this is safe:

- The top-level windows of AWT are `SunAwtFrame`, `SunAwtDialog` and `SunAwtWindow` in every JDK the project knows (external). The bridge answers `isJavaWindow` only for AWT windows (external).
- The repository already treats `SunAwt*` as the complete set: the classifier, the enablement diagnostic and the no-DLL pass all do.
- Windows that look like Java but are not AWT are not served by the bridge today either:
  - `FileDialog` and print dialogs are native common dialogs, and `GA_ROOT` resolves them as their own root;
  - splash screens are native;
  - JavaFX and SWT windows are served by UI Automation;
  - embedded frames are child windows of their host.

A new live test pins the class on a Java 21 runtime. So far the lane has proven it only on Java 8.

*Alternative rejected:* a `providers.java.jab.probe_all_windows` switch. It has no known use, adds surface to document and test, and a JDK that renamed its classes would break the shared classifier first.

The remark at `node.rs:911-912` that a JavaFX window can be served through the bridge is corrected.

### 8. Conditions are recorded where they happen and reported where failures are swallowed

**Recording.** The busy check, blame (on a caller or on the pump) and the degraded threshold only record a condition in the shared state. A record holds:

- the subject;
- the call;
- the time it started;
- how the subject was found.

Only real evidence records a condition: a holder that has itself run for the deadline, a message dispatch that held the bridge that long, a blamed job, a second slow answer from a window already set aside for one, or a degraded mark. A single slow answer records nothing, since a busy machine can cause it (Risks). A busy failure caused only by the given-up flag records nothing (decision 3). These places write nothing at warning level. The pump writes no record about a job, a release, a message dispatch or blame; its lifecycle and UIPI records stay.

**Stale conditions.** A recorded condition whose episode ended before a swallowing layer reported it is dropped: its end is recorded at debug, and no warning follows. A stall seen only by a hit-test, followed much later by a desktop pass, therefore adds no warning in a session that is healthy by then.

**Reporting.** A latch built on `platynui_core::diagnostics::Transitions` turns the conditions into one warning per episode. The layers that swallow the failure call it, on their own thread:

- the desktop pass;
- the lazy children of an `app:Application`;
- the reads that turn a failure into a missing value: `info_opt` and `is_valid` in `node.rs`, and the interface reads in `interfaces.rs`.

A call whose failure is returned (a hit-test, a pattern action, a focus request) adds debug records only (`diagnostic-logging`, *A failure returned to the caller is not reported again*).

**The subject.** It is the JVM that does not answer, by pid, where the backend can tell it:

- For a vm-targeted call: through the vm→pid map.
- For a dispatched message: through the sender's window.
- For a window call: the window's process, **if** the spike shows no fan-out. With fan-out, the window and its process are named as the window being asked about, and the subject is the bridge, not that process.
- Without a known pid: the window, or the bridge's own message handling.

**Latch keys.** The latch is keyed by subject. A side map holds the `ProcessIdentity` captured when the episode started. Each desktop pass calls `retain`, and forgets a subject whose process has ended, with one debug record.

**Re-keying.** When a vm-targeted call or a re-probe identifies the JVM behind an open episode whose subject is the bridge (a window being asked about, or the message handling), the episode moves to that process with one debug record. No second warning is emitted for the same stall.

**When an episode ends.** For a JVM: the same call for the same JVM or window, made again, completes within its deadline; or a re-probe of one of its windows, or its health probe (both the EDT-reaching call chosen by the spike), answers; or its process has ended. For the bridge as subject (a window being asked about, or the message handling): the bridge is no longer held and a bridge call answers within its deadline. The stuck call itself returning after its deadline does not end it: when the bridge client gives up after its own timeout, the JVM may still be wedged, and the next re-probe would stall again. Other calls for the process do not end it either, so a prompt `isSameObject` between two stalls does not start a second warning.

**What the warning carries.**

- The message follows `dev-docs/logging.md` §10-§11: lower case, no interpolated values, and the consequence after a semicolon.
- **A JVM as subject.** Fields `pid`, `application` (the process name, where known), `call`, `timeout_ms`, `elapsed_ms`, and `window` and `vm` where known. Message: "JVM does not answer Access Bridge calls; its windows, and while it holds the bridge every window served through the bridge, are missing from query results".
- **The bridge as subject.** Fields `window`, `window_pid`, `call`, `timeout_ms`, `elapsed_ms`, and no `pid`, so a healthy process never appears as the one that does not answer. Message: "Access Bridge call about a window does not answer; every window served through the bridge is missing from query results while it is held". For the message handling, `call` names the message and `window` the sender, where known.

**Existing records brought in line.**

- `pump.rs:68`: the degraded warning goes; the latch covers it, naming the process instead of the vm.
- `pump.rs:75`: the recovery at info becomes the latch's debug end record.
- `provider.rs:585`, `:603`: the discovery debug records gain `error`, `window` and `pid`.
- Timeouts: a timeout gets one debug record with call, vm and `timeout_ms`, which is the open review item at `client.rs:189`.

*Alternatives rejected:*

- **Warning from the tracker, as today.** That layer returns the failure and knows no process.
- **Warning from the busy check.** It runs in calls whose failure is returned.

### 9. The hit-test gate uses the class first

`element_at_point` checks, in order, before any bridge call:

1. **The class** of the `GA_ROOT` window under the point, through `window_class_of` (`node.rs:953-965`). A non-AWT window gives `UnsupportedOperation`, so UI Automation resolves the point even during a stall.
2. **Whether that window is set aside and not due.** If the bridge served it before, the answer is a provider error without a bridge call. The error says that the window is set aside because its JVM did not answer, not that the bridge is busy. A set-aside window the bridge never served answers `UnsupportedOperation`, so UI Automation resolves its shell, as the tree shows it.

**While the bridge is held,** an AWT window fails with `PumpBusy`. That is the honest answer for a Java window that cannot be served now, and UI Automation abstains for claimed windows anyway.

**The vm→pid map** learns the vm and pid of every window the hit-test resolves.

The gate becomes a pure function over the class, the pid, the exclusions, the set-aside state and a `WindowProbe`, so it is unit-tested without a desktop.

*Alternative rejected:* answering `UnsupportedOperation` for AWT windows too during a stall. The empty shell would misrepresent the window, and UI Automation abstains for claimed windows anyway.

### 10. Where each property is proven

**Unit tests, no DLL:**

- **The dispatcher.** The pump loop, with its message, release and job phases each under the in-flight record, moves into a runner generic over the job argument. `pump_main` supplies only the Win32 message pump and `releaseJavaObject` as closures. The vm bookkeeping of `call` and the health probe move into the dispatcher, with the probe's job supplied by the client. A fake pump then runs jobs with `()`, and test-controlled gates stand in for a stuck call, release or message. `JabError::PumpBusy`, the job-state enum and the start time on `JabError::Timeout` (unset until the fix) exist from the seam task on, so the defect tests compile before the fix. Tests cover:
  - blame, including the late-start rule;
  - the busy check, including the given-up flag and message dispatch;
  - given-up jobs;
  - panics.
- **The pure discovery pass.** It takes the set-aside store and returns a held list from the start. Tests cover:
  - the class gate;
  - the start-up wait;
  - setting aside, including slow false and late start;
  - re-probing, its backoff and the due rules;
  - held windows over several busy passes;
  - the pid-filtered path;
  - forgetting;
  - the vm→pid map.
- **The latch and its reporting layers.**
- **The router,** with its stub backend returning held windows.
- **The hit-test gate,** as a pure function.

**Live tests** (`crates/provider-java/tests/live_fixture.rs`, `java-live` group):

- **The two-JVM wedge test.** Order matters:
  1. B is launched, served, and its window node, vm, bounds and `app:Application` captured.
  2. A is launched with `--wedge-after` long enough that A's window is served before the wedge.
  3. Before the wedge, B is activated again. The fixture places a new frame by the platform (`setLocationByPlatform`), so A opens over B. Afterwards B is above A in Z-order:
     - a hit-test at B's centre lands on B, which the test asserts through the root window under that point;
     - discovery asks about B's window first, which exercises the fan-out case.

  During the wedge the test:
  - enumerates;
  - reads B's window name at least three times, vm-targeted, which today degrades B;
  - enumerates through a UI Automation provider in the same process, to see shells;
  - hit-tests B, if the spike shows the bridge stays held past the deadline.

  Timings are anchored to the fixture's "wedging" and "released" lines. A pass that asks a window again that the bridge has not answered about since the wedge began is exempt from the half-deadline bound. Each such ask emits one debug record, "held window asked again", with `window` and `pid` (task 6.2), and the test finds it in its capture. The agent is off and `call_timeout_ms = 750`.

  Six scenarios share this one test, because each needs two JVMs and a 15 s wedge. That deviates from `dev-docs/testing-strategy.md` §7's "one behavior per test". The test's summary message names the scenario of every failed assertion.
- **The fixture on the Java 21 launcher.**

**The lane.** Its Robot suites must stay free of JAB warnings.

## Risks / Trade-offs

- **[A long stall still hides every Java window served through the bridge]** → It is bounded to one deadline per pass, reported once, and recovered once the bridge answers. A restartable bridge process is the only way past it.
- **[With fan-out the stuck JVM cannot be named from a window call]** → The report then names the window being asked about as such, never a healthy JVM as the one that does not answer. A vm-targeted call or a re-probe names the stuck JVM once it identifies it, moving the open episode without a second warning (decision 8).
- **[A healthy lane run could meet a JVM busy past the deadline at start-up and now warn]** → The warning then says something true: that JVM's windows were missing for that time. Task 9.2 checks the Robot suites for JAB warnings. If a healthy fixture triggers it, the per-call deadline has to be revisited, not the report.
- **[The 250 ms bound for a slow "not a Java window"]** → A busy machine could make a healthy "no" slower. The window is then set aside instead of reported, and its re-probe reports it once it answers promptly. The spike checks the bound against real answer times.
- **[The class prefix becomes load-bearing]** → The Java 21 live check. The classifier is shared, so a rename would show everywhere at once. No known exception exists (decision 7).
- **[A held window is served by no provider for the length of a stall]** → That is the honest state, and the report names the cause. Releasing it to UI Automation would show an empty shell (decision 6).
- **[A held JVM stays an attach candidate]** →
  - **A whole-process freeze.** The attach times out: up to 10 s per attempt, at most 3 per process and session, each blocking the enumeration that makes it.
  - **A wedged EDT.** It does not block the attach listener (inferred), so the attach succeeds quickly, and the agent then serves that JVM under its own 5 s deadline until its session degrades, with its own warning.
  - **So:** with the agent on, a pass during a wedge is bounded by the agent's deadline, not by the JAB one. The live test runs with the agent off. `dev-docs/platform-windows.md` states this (task 8.1).
- **[A shared file with other changes]** →
  - `fix-jab-hit-test-virtual-children` touches `live_fixture.rs` and `dev-docs/platform-windows.md:143`.
  - `application-process-attributes` touches `enumerate_visible_top_level_windows` (`WindowCandidate`'s pid), `awt_windows_without_bridge`, and the hit-test (`top_level_window_at`, the orphan application node). This change touches all of them too. That change should list this one in its Coordination, and whichever lands second rebases.
  - `gate-uia-window-patterns` (task 3.5) touches the justfile's nextest line.

## Migration Plan

- **Behavioral:**
  - during a stall, calls fail sooner;
  - served windows keep their claims;
  - a new warning replaces the degraded warning;
  - non-AWT windows are no longer offered to the bridge;
  - a hit-test over a non-AWT window no longer waits for the bridge.
- **Additive:**
  - `JabError::PumpBusy`;
  - the held-window lists of `JabEnumeration` and `Enumeration`;
  - the Java 21 launcher in the lane.
- **Native rebuild:** yes. The JAB crate is linked into the native extension on Windows.
- **Platforms:** Windows only. Both crates are Windows-gated, so no cross-target check applies.
- **Sequence:** the fixture plumbing and the spike first, then the failing live tests, the dispatcher, discovery and the router, the report, and the docs.
- **Rollback:** revert per commit. The dispatcher change stands alone, and discovery depends on it only for `PumpBusy`.

## Spike results

To be filled in by task 1.3, for each of the four questions of decision 1:

- the calls that reach the EDT, and the chosen re-probe and health-probe call;
- whether and when a stuck call returns, and with what;
- whether window calls fan out;
- which pump phase blocks while a JVM starts up, and whether the 9.2 overlap reproduces without the 40 s wedge test.

Also the confirmed or raised 250 ms bound, the confirmed re-probe cap, and which held-bridge observations the live test can make.
