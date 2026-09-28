# Proposal

## Why

One Java application whose event thread does not answer can take every other Java application out of the tree when both are served through the Java Access Bridge (JAB). A Swing application that hangs, or is merely busy while it starts, then makes a suite fail on a different, healthy application with `ElementNotFoundError`. The JAB specs and docs promise that a timeout affects "the affected node only"; the code does not keep that promise.

What the code does today, found by a read-only analysis on 2026-09-28 and confirmed file by file:

- **One pump thread.** All bridge calls run one at a time on one pump thread, and a call that has run past its deadline cannot be cancelled.
- **The first failure ends discovery.** Discovery asks the bridge about every visible top-level window, and abandons the whole pass at the first `isJavaWindow` call that fails, a timeout included. Every Java window below that one in Z-order is then missing from the pass:
  - it has no node;
  - it has no `app:Application`;
  - it loses its window claim, so its UI Automation shell appears;
  - its JVM gets no offer of the in-JVM agent.
- **Healthy JVMs get the blame.** A call's deadline starts when it is queued. A call that timed out only because it waited behind another JVM's stuck call is counted against its own JVM, so healthy JVMs get marked degraded.
- **Nothing useful is reported.** The aborted pass and the stuck process are recorded at debug only. The one warning, "JVM marked degraded", names only an opaque vmID and can name a healthy JVM.

**How it surfaced.** The Windows lane of `snapshot-validity` failed on it: a JAB test that overlapped the start-up of other bridge-enabled JVMs never saw its own fixture, and the existing `live_fixture_contract_and_interaction` failed the same way (3 of 3, `openspec/changes/snapshot-validity/tasks.md` 9.2). The runs are consistent with the defect, but they do not prove it on their own: the overlapping test binary also wedges a bridge-enabled JVM for 40 s, which no in-process design can serve through. The spike of this change tells the two apart.

The lane was made stable by serializing the live tests (`.config/nextest.toml`). That hides the defect in tests; users still meet it.

## What Changes

- **Waiting is not failing.** Only a call that has itself run for its full deadline counts against its JVM. A call that never started, because the bridge was held by another call, is not counted against its JVM and is never run later.
- **A held bridge fails fast.** While the bridge is held past its deadline, further calls fail at once instead of queueing for their own deadline. This holds whether a call or the bridge's own message handling holds it.
- **One window does not hide the others.** Discovery no longer abandons its pass at the first unanswered window. A window that does not answer is set aside, asked again on a doubling interval, and reported. Its process stays a candidate for the in-JVM agent.
- **Windows keep their claim through a stall.** A window the bridge served keeps its window claim while it is held, for as long as the stall lasts. It is not replaced by its UI Automation shell. A window the bridge never served stays unclaimed, as today.
- **Only AWT windows are asked.** The bridge is asked only about top-level windows whose class marks them as AWT windows. Other windows cost no bridge call in:
  - discovery;
  - the lazy children of `app:Application`;
  - the start-up wait;
  - hit-testing.

  A point over a non-AWT window is therefore handed to the other providers even while the bridge is held.
- **The stuck JVM is named once.** A JVM that does not answer is reported once per episode at warning level, by the part of the backend that swallows the failure. The report names the process, where the backend can tell which JVM it is, together with the call and the deadline, and states that its windows are missing from query results. A healthy JVM is never named as the one that does not answer. The end of the episode is recorded at debug level.
- **Honest documentation.** The code comments and `dev-docs/platform-windows.md` stop promising containment the code cannot give.

Behavior changes for release notes (PlatynUI is 0.x, so none of this is framed as breaking):

- during a JAB stall, calls against the bridge fail sooner;
- Swing windows the bridge served no longer turn into their UI Automation shells while the bridge does not answer;
- the new warning replaces the "JVM marked degraded" warning;
- a top-level window whose class does not start with `SunAwt` is no longer offered to the bridge. No JDK window that the bridge can serve is known to have such a class (see design);
- a hit-test over a non-AWT window no longer waits for the bridge.

Out of scope:

- **Serving healthy JVMs during a long stall.** A call stuck inside the bridge client cannot be cancelled, so no in-process design serves healthy JVMs through the bridge while one call is stuck. That needs a separate, restartable bridge process and is left for a later change.
- **A de-calibration effect.** A healthy window whose first calibration times out behind a stuck call keeps an identity transform. That is the observable of `verify-display-scaling`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `jab-provider`:
  - discovery asks only AWT windows, and continues past a window that does not answer;
  - robustness distinguishes waiting from failing, and fails fast while the bridge is held;
  - single appearance allows a held window to be shown by no provider;
  - the enablement diagnostic applies only to a prompt "not a Java window" answer;
  - new requirements:
    - a Java window that does not answer does not hide the others;
    - a JVM that does not answer is reported once per episode, naming its process.
- `jab-hit-test`:
  - the Java-window gate asks the bridge only about AWT windows;
  - a point over a held window yields a provider error, never the UIA shell;
  - a held bridge makes hit-testing fail fast.
- `java-provider`: a window a backend served and now holds, because it cannot ask about it, stays claimed, yields no node and is not reported as unreachable.

## Impact

- **Rust, `crates/provider-java-jab`:**
  - `pump.rs`: job state, the in-flight record around jobs, releases and message dispatch, an unwind guard, and the degraded bookkeeping.
  - `client.rs`: the busy check, deadline attribution, and a new not-started error.
  - `error.rs`: the new variant and its mapping to `ProviderError`.
  - `provider.rs`: a pure discovery layer with the start-up wait, the class gate, set-aside and held windows, the hit-test gate, and the episode report.
  - `node.rs` and `interfaces.rs`: the reads that turn a failure into a missing value report through the latch, and a remark about JavaFX is corrected. `JabAppNode`'s constructors and the `java_windows` call stay as they are.
- **Rust, `crates/provider-java`:**
  - `backend.rs`: `Enumeration` gains the held windows.
  - `jab.rs`: maps them.
  - `provider.rs`: claims, ownership, attach candidates and diagnostics for held windows.
  - `tests/live_fixture.rs`:
    - fixture plumbing for app arguments, a launcher and stderr capture;
    - a tracing subscriber;
    - a two-JVM live test with one wedged JVM;
    - a JAB check on a Java 21 runtime.
- **Rust, `crates/core`:** only the documented claim invariant of the Java classifier (`platform/java.rs`) gains the held case. No behavior change.
- **Test app and lane:**
  - `apps/test-app-swing` has no code change: it already has `--wedge-after` and `--wedge-for`. Its launcher comments in `build.gradle.kts` and `README.md` gain the Java 21 live check.
  - The justfile's Windows lane hands over the Java 21 launcher.
- **Documentation:** `dev-docs/platform-windows.md` (the JAB threading model, handle discipline, hit-testing and the enablement diagnostics), and the module docs of `pump.rs` and `client.rs`.
- **Python and Robot Framework:** no code change. The behavior reaches them through the native extension, so a native rebuild is needed.
- **Platforms:** Windows only. JAB exists only there, and both Java crates are built only for Windows. No other provider changes.
- **Sequencing:**
  - **`snapshot-validity` has landed.** This change builds on its `.config/nextest.toml` serialization, its `fixture_application` test helper and `JabAppNode`'s process identity.
  - **`fix-jab-hit-test-virtual-children`** rewrites the same paragraph of `platform-windows.md` and touches `live_fixture.rs`. Whichever lands second rebases.
  - **`application-process-attributes`** also edits:
    - `enumerate_visible_top_level_windows` (`WindowCandidate`'s pid);
    - `awt_windows_without_bridge`;
    - the hit-test's `top_level_window_at` and orphan application node.

    Whichever lands second rebases, and its Coordination section gains this change (task 8.2).
