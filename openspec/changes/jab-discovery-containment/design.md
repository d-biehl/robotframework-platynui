# Design

## Context

See proposal.md for the motivation and the specs for the required behavior.

Line numbers are those of the working tree on 2026-09-28: HEAD `65e4a4f` plus the uncommitted `snapshot-validity` work. *Verified* marks what was read in the code. *Inferred* marks conclusions from the code. *External* marks facts about the JDK's bridge that the repository cannot show.

**The pump and its jobs (verified).**

- **The job type.** `Job` holds only `run: Box<dyn FnOnce(&Bridge) + Send>` (`crates/provider-java-jab/src/pump.rs:34-37`). It carries no call name, no target and no state.
- **The loop.** It pumps Win32 messages, drains handle releases, then runs at most one job (`pump.rs:161-170`). While a job, a release or a dispatched message is blocked inside the bridge, nothing else runs.
- **Nobody knows what runs.** No record says which job is running or since when.
- **Given-up jobs still run.** A job whose caller has already given up runs anyway: the closure ignores the dropped reply channel (`client.rs:182-184`).
- **No test seam.** `Bridge` owns a `libloading::Library` (`dll.rs:103-132`), so a job cannot run in a test.

**The client (verified).**

- **Where the deadline starts.** `call_unchecked` sends the job and then waits `recv_timeout(call_timeout)` (`client.rs:176-192`), so the deadline starts at enqueue.
- **Who gets blamed.** `call` counts every `Timeout` of a call aimed at one JVM (a vm-targeted call) against that vm (`client.rs:161-171`, `:168`).
- **Calls without a JVM.** `isJavaWindow` and `getAccessibleContextFromHWND` are sent without a vm (`client.rs:219-238`). The degraded tracker never protects them.
- **The health probe** goes through `call_unchecked` (`client.rs:204-208`). `probe_due` stamps the attempt before the call (`pump.rs:85-98`), so a probe that only waited in the queue keeps a healthy vm degraded for another second.
- **The error mapping.** `JabError` maps exhaustively to `ProviderError` (`error.rs:45-60`). `Timeout`, `VmDegraded` and `PumpUnavailable` become `CommunicationFailure`.

**The degraded tracker (verified).**

- `DegradedTracker` (`pump.rs:39-99`) marks a vm degraded after 3 consecutive timeouts and probes it at most once a second.
- It warns from the returning layer, naming only the vm (`pump.rs:68`), and records recovery at info (`pump.rs:75`). Both are open findings of the logging-concept review (`openspec/changes/archive/2026-09-27-logging-concept/review-findings.md:503-507`).

**Discovery (verified).**

- **The loop.** `discover_java_windows` (`provider.rs:559-608`) skips the own process, a pid filter and excluded windows. It then asks `isJavaWindow` about every remaining visible top-level window and `break`s on the first error (`:582-587`).
- **Windows the bridge says no to.** A window the bridge denies becomes an enablement suspect only if its class starts with `SunAwt` (`:589-597`).
- **A failed context lookup.** When `getAccessibleContextFromHWND` fails, the loop goes on and the window is silently dropped (`:600-604`).
- **Application nodes.** `enumerate` builds `app:Application` nodes only for the pids of served windows (`:349-357`). `java_processes` is the served pids plus the suspects' pids (`:358-363`).
- **The start-up wait.** On the first empty pass after connecting, discovery is repeated for up to 1.5 s (`:247-259`). This happens whether or not any window looks like Java, although its own doc says it is paid "only when a Java-looking desktop yields nothing yet" (`:43-47`).
- **The lazy path.** The windows under an `app:Application` are listed lazily through the free function `java_windows(&self.client, Some(self.pid), …)` (`node.rs:1416-1419`). It reaches only the client and the exclusions. A unit test pins that call's source text (`provider.rs:854-860`).
- **The no-DLL pass.** It already applies the class rule: `awt_windows_without_bridge` keeps only `SunAwt*` windows (`provider.rs:484-510`). It was split out to be testable without a desktop.

**Hit-testing (verified).**

- **The gate.** `element_at_point` asks `isJavaWindow` about the root window under the point, whatever its class (`provider.rs:420`).
- **How a timeout travels.** A timeout becomes `CommunicationFailure`. The router returns any answer that is not `UnsupportedOperation` (`crates/provider-java/src/provider.rs:303-307`). The runtime returns the first such error without asking the next provider (`crates/runtime/src/runtime/input.rs:90-94`).
- **The provider order.** Providers are ordered by technology string (`crates/runtime/src/provider/registry.rs:39-45`), and "Java" sorts before "UIAutomation".
- *Inferred:* while the pump is stuck, a pick over any window, Java or not, fails instead of reaching UI Automation.

**The router (verified).**

- **Claims.** `sync_window_claims` releases the claim of every window missing from the pass (`crates/provider-java/src/provider.rs:171-180`, called at `:280`). The UI Automation provider honors claims.
  - *Inferred:* a Swing window hidden by an aborted pass is listed as its UI Automation shell.
- **Attach candidates.** `consider_attaching` takes its candidates only from `java_processes` on Windows (`:255-262`). It attaches synchronously: 10 s per attempt, plus up to 3 s for the handshake, at most 3 attempts per process (`agent/backend.rs:56`, `:65`, `:78`, `:257-331`).
- **Enablement diagnostics.** They are once per window for the life of the process and never re-armed (`crates/core/src/platform/java.rs:181-202`, router `:335-367`). A wrong "enable the bridge" hint would stick.

**Evidence of the defect (verified).**

- The lane experiments in `openspec/changes/snapshot-validity/tasks.md` 9.2.
- The `.config/nextest.toml` comment written for them.
- A read-only analysis on 2026-09-28 that checked each claim against the code.

**The bridge itself (external, unverified in the repository).**

- **The event thread.** The Java side of the bridge answers `isJavaWindow` and `getAccessibleContextFromHWND` for its own windows on the event-dispatch thread (EDT), and waits without a timeout. So a busy or wedged EDT blocks the call.
- **Other JVMs' windows.** The client DLL may send those calls to every connected JVM in turn. If it does, one stuck JVM also blocks calls about other JVMs' windows.
- **Whether a stuck call ever returns.** The client DLL may send with `SendMessage` (never returns while the JVM does not answer) or with a timeout. That decides whether a stuck call returns before the stall ends.

## Goals / Non-Goals

**Goals:**

- A stall costs a discovery pass at most one deadline, and it is reported once, naming the process.
- Healthy JVMs are never blamed for another JVM's stall, and their windows return in the first pass after the bridge answers again.
- A window that does not answer never hides the windows of other JVMs once the bridge answers.
- The bridge is asked only about windows it can serve.

**Non-Goals:**

- **Serving healthy JVMs while one call is stuck inside the bridge client.** That needs a separate, restartable bridge process.
- **Changing the in-JVM agent backend.** It has its own containment.
- **Relaxing the test isolation in `.config/nextest.toml`.** It stays: a stall is process-wide by nature, and the new live test creates its own stall inside one test.
- **A configuration switch to probe every window.** See decision 7.

## Decisions

### 1. Measure the bridge first, but do not let the design depend on it

A short spike runs before the live assertions are fixed. It is a throwaway ignored test with timed debug records around every bridge call, against two fixture JVMs: A with a wedged EDT (`--wedge-for`), and B healthy. It measures:

- which calls against A block while its EDT is wedged, compared with a whole-process freeze (`set_process_frozen`, `live_fixture.rs:513-528`);
- whether a stuck call returns before the wedge ends;
- whether `isJavaWindow` about B's window, asked first after A's wedge begins, blocks.

The results are recorded under *Spike results* below.

The design holds either way:

- **If a stuck call returns early,** the set-aside rule (decision 5) lets B be served during A's wedge without any further change.
- **If it never returns early,** the bridge stays held, and decisions 2-3 bound and report the stall.

What the spike changes:

- which call re-probes a set-aside window;
- the wording of the docs;
- the timing expectations in the live test.

It changes no requirement.

### 2. A call counts against its JVM only for time it ran itself

Each job gets a state shared between its caller and the pump: *queued*, *running* or *given up*.

- **At the caller's deadline.** Measured from enqueue as today, the caller moves the job from *queued* to *given up* with one atomic compare-and-swap.
  - If that succeeds, the job never runs. The caller gets a new error, `JabError::PumpBusy`, naming the call that holds the bridge. Nothing is counted.
  - If the job was already *running*, the caller gets `Timeout`.
- **Running.** The pump moves a job from *queued* to *running* before calling into the bridge, and skips a job it finds *given up*.
- **Blame.** A running job counts against its vm only if its own run time reaches the deadline. Whoever sees that first records it, once per job:
  - the pump, when the job ends;
  - or the busy check of decision 3, while the job is still running.
  A late-started job that ends within the deadline is not blamed.
- **The health probe.** It follows the same rule, and a probe that never started does not use up its once-a-second slot.

*Alternatives rejected:*

- **Starting the deadline at dequeue.** A caller could then wait without bound in the queue.
- **Counting only when the queue was empty at enqueue.** That is racy, and still blames a job that started late.
- **Cancelling the stuck call.** An OS call inside the bridge client cannot be cancelled.

### 3. A bridge held past its deadline fails the next call at once

The pump publishes an in-flight record around every blocking entry into the bridge: jobs, and also handle releases (`drain_releases`, `pump.rs:197-225`). A release to a wedged JVM that is not yet degraded holds the bridge just the same. The record holds the call, the target vm, the pid and window where known, and the start time.

Before enqueueing, the client checks the record. If a call has held the bridge for at least the deadline, it returns `PumpBusy` at once, without enqueueing. That bounds a discovery pass to one deadline instead of one deadline per window. Together with decision 2, it also ends the backlog of abandoned calls that today still runs after a stall.

`PumpBusy` maps to `CommunicationFailure`, like `Timeout`.

*Alternative rejected:* a priority queue for discovery calls. It changes the order of waiting, not the fact that nothing runs while the pump is stuck.

### 4. The shared state lives next to the degraded tracker

The in-flight record, the set-aside windows, a vm→pid map and the episode latch (decision 8) share one `Arc`. It sits next to `DegradedTracker`, which the provider, the client and the pump already share (`provider.rs:191`, `client.rs:132`, `pump.rs:113`).

- **The lazy path sees it too.** `java_windows` reaches the client, so the windows under an `app:Application` see the same set-aside windows.
- **No constructor changes.** `JabAppNode`'s constructors stay as they are. `snapshot-validity` and `application-process-attributes` both touch them.
- **The vm→pid map.** Discovery fills it from each served window, which knows both its vm and its pid. It names the process behind a stuck vm-targeted call.

*Alternative rejected:* fields on `JabProvider`. The lazy path through `java_windows` could not see them.

### 5. Discovery becomes a pure pass that sets windows aside

`discover_java_windows` splits into a pure pass over window candidates and a thin wrapper that keeps `EnumWindows` as the only Win32 part. This follows the precedent `awt_windows_without_bridge`. The bridge is reached through a small `WindowProbe` trait that `JabClient` implements, so the pass runs in tests against scripted answers.

Per candidate, in order:

1. **Own process, pid filter, exclusions:** as today.
2. **Class gate:** `JavaToolkit::from_window_class(class) == Some(JavaToolkit::SwingAwt)` (`crates/core/src/platform/java.rs:52-62`). Other windows are skipped with no bridge call. The suspect check and the no-DLL pass switch to the same helper, so one definition decides what is an AWT window.
3. **Not due yet:** a set-aside window whose re-probe is not yet due is *held*, without a call.
4. **`isJavaWindow`, then the context:**
   - `Ok(true)` with a context → served.
   - `Ok(false)` → suspect, as today.
   - `Timeout` on either call → set aside and held.
   - `PumpBusy` → *not asked*. The pass goes on, because every further call fails at once.

**Held windows.** The pass reports the set-aside windows and the windows it could not ask that the previous pass served. It remembers last pass's served windows with their pids, and returns both groups as held.

**Re-probing.**

- It starts 1 s after a window is set aside and doubles after each failed re-probe, up to 16 s. It resets once the window answers.
- A short start-up stall is therefore probed again soon, and a long wedge costs the pump at most one deadline every 16 s.
- The re-probe uses `isJavaWindow` unless the spike shows it does not reach the EDT.
- A set-aside entry is forgotten when its window is no longer a candidate or its process has ended.

**Other paths.**

- The start-up wait is skipped when no AWT candidate is visible, which makes its doc true.
- `java_windows` gets the shared state through the client, so the source-text test at `provider.rs:854-860` changes with its call.

*Alternative rejected:* keeping `break` and adding only the class gate. That still hides every window below a busy AWT window, and the start-up case is itself an AWT window.

### 6. Held windows keep their claims and stay attach candidates

`JabEnumeration` gains a list of held windows with their pids. The router:

- claims served ∪ held windows;
- records held windows as the Access Bridge's own in the ownership map, so the agent can still take them over at its stronger rank;
- builds no node for them;
- leaves them out of the enablement diagnostics;
- adds their pids to `java_processes`.

This fits `java-provider`'s rule that a window is claimed exactly when a backend can serve it: the Access Bridge serves these windows, and they are only not answering for now. No `java-provider` delta is needed.

*Alternatives rejected:*

- **Releasing held windows to UI Automation.** Users would see, and could act on, empty UI Automation shells under the Swing title. That is worse than a window that is briefly missing, and the report says why.
- **Taking held pids out of the attach candidates.** A JVM busy at start-up is exactly one that should get the agent, and `java-provider` requires attaching automatically. The cost is recorded under Risks.

### 7. The class gate has no escape hatch

Only AWT windows are asked, in discovery, in the lazy children and at the hit-test gate.

Why this is safe:

- The top-level windows of AWT are `SunAwtFrame`, `SunAwtDialog` and `SunAwtWindow` in every JDK the project knows (external). The bridge answers `isJavaWindow` only for AWT windows (external).
- The repository already treats `SunAwt*` as the complete set: the classifier, the enablement diagnostic and the no-DLL pass all do.
- Windows that look like Java but are not AWT are not served by the bridge today either:
  - `FileDialog` and print dialogs are native common dialogs;
  - splash screens are native;
  - JavaFX and SWT windows are served by UI Automation;
  - embedded frames are child windows of their host.

A new live test pins the class on a Java 21 runtime. So far the lane has proven it only on Java 8.

*Alternative rejected:* a `providers.java.jab.probe_all_windows` switch. It has no known use, adds surface to document and test, and a JDK that renamed its classes would break the shared classifier first.

The remark at `node.rs:911-912` that a JavaFX window can be served through the bridge is corrected.

### 8. One episode per process, reported on the caller's thread

A latch built on `platynui_core::diagnostics::Transitions`, keyed by pid, turns the conditions into one warning per episode:

- **What feeds it.** The busy check sees a call for a process hold the bridge past its deadline. A process's JVM is marked degraded.
- **Which process.** The pid comes from the window call itself, or from the vm→pid map for a vm-targeted call.
- **When an episode ends.** A call for that process completes within its deadline, or a re-probe answers. An episode also ends when the process has ended (`ProcessIdentity`, as `JabAppNode::is_valid` already uses). Each end is one debug record.
- **Which thread logs.** Records are emitted on the threads that make calls, never on the pump thread. There, the thread-local capture of the unit tests sees them (`provider.rs:688-717`).
- **What the warning carries.**
  - Fields: `pid`, `application` (the process name, where known), `call`, `timeout_ms`, `elapsed_ms`, and `window` and `vm` where known.
  - The message follows `dev-docs/logging.md` §10-§11. It is lower case, holds no interpolated values, and states the consequence after a semicolon. For example: "JVM does not answer Access Bridge calls; its windows, and while it holds the bridge every window served through the bridge, are missing from query results".
- **Who reports.** The returned errors are not reported at warning level anywhere (`diagnostic-logging`, *A failure returned to the caller is not reported again*). The warning is the backend's record of a process's state, owned by the latch.

Existing records this change touches are brought in line with the same rules:

- **`pump.rs:68`:** the degraded warning goes. The latch covers it, naming the process instead of the vm.
- **`pump.rs:75`:** the recovery at info becomes the latch's debug end record.
- **`provider.rs:585`, `:603`:** the discovery debug records gain `error`, `window` and `pid`.
- **Timeouts:** a timeout gets one debug record with call, vm and `timeout_ms`. This is the open logging-review item at `client.rs:189`.

*Alternative rejected:* warning from the tracker, as today. That layer returns the failure and knows no process.

### 9. The hit-test gate uses the class first

`element_at_point` checks the class of the root window under the point before any bridge call, using `window_class_of` (`node.rs:953-965`).

- **A non-AWT window.** The provider answers `UnsupportedOperation` without a bridge call, so UI Automation resolves the point even during a stall.
- **An AWT window while the bridge is held.** It fails with `PumpBusy`, mapped to `CommunicationFailure`. That is the honest answer for a Java window that cannot be served.

*Alternative rejected:* answering `UnsupportedOperation` for AWT windows too during a stall. UI Automation abstains for claimed windows anyway, and the empty shell would misrepresent the window.

### 10. Where each property is proven

**Unit tests, no DLL:**

- **The dispatcher.** It is made generic over the job argument, so a fake pump runs jobs with `()` and a test-controlled gate stands in for a stuck call. Tests cover:
  - blame (decision 2), including the late-start rule;
  - the busy check and the given-up jobs (decision 3);
  - the latch (decision 8).
- **The pure discovery pass**, with a scripted `WindowProbe` and a controlled clock. Tests cover:
  - the class gate;
  - setting aside;
  - re-probing and its backoff;
  - held windows;
  - the pid-filtered path;
  - forgetting a set-aside entry.
- **The router**, with its stub backend (`crates/provider-java/src/provider.rs:460-542`). Tests cover claims, diagnostics and attach candidates for held windows.
- **The hit-test gate**, as a pure test.

**Live tests** (`crates/provider-java/tests/live_fixture.rs`, `java-live` group):

- **Two fixture JVMs**, one of them wedged with `--wedge-after 3 --wedge-for 15`. Stderr is captured to sync on the fixture's "wedging" and "released" lines, `call_timeout_ms = 750`, and the agent is off. The test asserts:
  - bounded passes;
  - exactly one warning naming the wedged pid;
  - no warning and no degradation for the healthy pid;
  - no UI Automation shell for either Swing title;
  - the healthy `app:Application` staying valid;
  - a prompt hit-test error over the healthy window;
  - recovery after the release.
- **The fixture on the Java 21 launcher.**

A process-wide tracing subscriber replaces the `tracing` placeholders of that file. **The lane:** the Windows lane must stay free of JAB warnings.

## Risks / Trade-offs

- **[A long stall still hides every Java window served through the bridge]** → It is bounded to one deadline per pass, reported once with the process named, and recovered in the next pass. A restartable bridge process is the only way past it and is left for a later change.
- **[The bridge client may send window calls to every JVM (external)]** → The design does not rely on either answer (decision 1). The class gate removes the calls for non-AWT windows either way.
- **[A healthy lane run could meet a JVM busy past the deadline at start-up and now warn]** → The warning then says something true: that JVM's windows were missing for that time. The Windows lane checks for JAB warnings (task 9.2). If a healthy fixture triggers it, the per-call deadline has to be revisited, not the report.
- **[The class prefix becomes load-bearing]** → The Java 21 live check. The classifier is shared, so a rename would show everywhere at once. No known exception exists (decision 7).
- **[A held window is served by no provider for the length of a stall]** → That is the honest state, and the report names the cause. Releasing it to UI Automation would show an empty shell (decision 6).
- **[A held JVM stays an attach candidate]** → A whole-process freeze makes attaching time out: at most 3 attempts of about 13 s per process and session (`agent/backend.rs:56-78`). A wedged EDT does not block the attach listener (inferred), so the usual cost is one quick attach. Today such a JVM is dropped from the candidates by the aborted pass only by accident.
- **[A shared file with other changes]** →
  - `live_fixture.rs` and `dev-docs/platform-windows.md:143` are also touched by `fix-jab-hit-test-virtual-children`.
  - `WindowCandidate` and `discover_java_windows` are also touched by `application-process-attributes`, which rebases onto this change.
  - The justfile's nextest line is also touched by `gate-uia-window-patterns` (task 3.5).
  - Whichever lands second rebases.

## Migration Plan

- **Behavioral:** during a stall, calls fail sooner, windows keep their claims, and a new warning replaces the degraded warning. Non-AWT windows are no longer offered to the bridge.
- **Additive:** `JabError::PumpBusy`, the held-window list of `JabEnumeration`, and the Java 21 launcher in the lane.
- **Native rebuild:** yes. The JAB crate is linked into the native extension on Windows.
- **Sequence:** `snapshot-validity` is committed first. Then the spike, the dispatcher, discovery and the router, the report, the live tests, and the docs.
- **Rollback:** revert per commit. The dispatcher change stands alone, and discovery depends on it only for `PumpBusy`.

## Spike results

To be filled in by task 1.1: which calls block during an EDT wedge and a process freeze, whether a stuck call returns before the stall ends, whether a call about another JVM's window blocks, and the chosen re-probe call.
