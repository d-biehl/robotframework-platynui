# Proposal

## Why

One Java application whose event thread does not answer can take every other Java application out of the tree when both are served through the Java Access Bridge (JAB). A Swing application that hangs, or is merely busy while it starts, then makes a suite fail on a different, healthy application with `ElementNotFoundError`. The JAB specs and docs promise that a timeout affects "the affected node only"; the code does not keep that promise.

The defect was found on 2026-09-28, while the Windows lane of `snapshot-validity` failed. Its evidence:

- the lane experiments recorded in `openspec/changes/snapshot-validity/tasks.md` 9.2: a JAB test that overlapped the start-up of other bridge-enabled JVMs never saw its own fixture, and the existing `live_fixture_contract_and_interaction` failed the same way (3 of 3);
- a read-only analysis of the code, confirmed file by file.

The lane was made stable by serializing the live tests (`.config/nextest.toml`). That hides the defect in tests; users still meet it.

What the code does today:

- **The pump.** All bridge calls run one at a time on one pump thread, and a call that has run past its deadline cannot be cancelled.
- **Discovery.** Discovery asks the bridge about every visible top-level window and abandons the whole pass at the first call that times out. Every Java window below that one in Z-order is then missing from the pass: no node, no `app:Application`, no window claim, no offer of the in-JVM agent.
- **Degraded JVMs.** A call's deadline starts when it is queued, and a call that timed out only because it waited behind another JVM's stuck call is counted against its own JVM. Healthy JVMs get marked degraded.
- **The report.** None of this is reported above debug level.

## What Changes

- **Waiting is not failing.** Only a call that has itself run past its deadline counts against its JVM. A call that never started, because the pump was held by another call, is not counted against its JVM and is never run later.
- **A held bridge fails fast.** While the bridge is held by a call that has already run past its deadline, further calls fail at once instead of queueing for their own deadline.
- **One window does not hide the others.** Discovery no longer abandons its pass at the first unanswered window. A window that does not answer is set aside, asked again at a bounded rate, and reported. Its process stays a candidate for the in-JVM agent.
- **Windows keep their claim through a stall.** A set-aside window keeps its window claim. So do windows served before the stall that could not be asked. They are not replaced by their UI Automation shells.
- **Only AWT windows are asked.** The bridge is asked only about top-level windows whose class marks them as AWT windows. Other windows cost no bridge call in discovery, in the lazy children of `app:Application`, in the start-up wait, or in hit-testing.
  - A point over a non-AWT window is therefore handed to the other providers even while the bridge is held.
- **The stuck process is named once.** A JVM that does not answer is reported once per episode at warning level. The report names the process, the call and the timeout, and states that its windows are missing from query results. The end of the episode is recorded at debug level.
- **Honest documentation.** The code comments and `dev-docs/platform-windows.md` stop promising containment the code cannot give.

Behavior changes for release notes (PlatynUI is 0.x, so none of this is framed as breaking):

- during a JAB stall, calls against the bridge fail sooner;
- Swing windows no longer turn into their UI Automation shells while the bridge does not answer;
- the new warning replaces the "JVM marked degraded" warning, which named only an opaque vmID;
- a top-level window whose class does not start with `SunAwt` is no longer offered to the bridge. No JDK window that the bridge can serve is known to have such a class (see design).

Out of scope:

- **Serving healthy JVMs during a long stall.** A call stuck inside the bridge client cannot be cancelled, so no in-process design serves healthy JVMs through the bridge while one call is stuck. That needs a separate, restartable bridge process and is left for a later change.
- **A de-calibration effect.** A healthy window whose first calibration times out behind a stuck call keeps an identity transform. That is the observable of `verify-display-scaling`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `jab-provider`:
  - discovery asks only AWT windows and continues past a window that does not answer;
  - robustness distinguishes waiting from failing and fails fast while the bridge is held;
  - new requirements: a Java window that does not answer does not hide the others; an unanswering JVM is reported once per episode, naming the process.
- `jab-hit-test`:
  - the Java-window gate asks the bridge only about AWT windows, so a point over any other window is deferred without a bridge call;
  - a held bridge makes hit-testing fail fast rather than wait.

## Impact

- **Rust, `crates/provider-java-jab`:**
  - `pump.rs`: job state, the in-flight record and the degraded bookkeeping;
  - `client.rs`: admission, deadline attribution and a new not-started error;
  - `error.rs`: the new variant and its mapping to `ProviderError`;
  - `provider.rs`: discovery split into a pure, testable pass, the class gate, the set-aside windows, the start-up wait, the hit-test gate, and the episode report.
- **Rust, `crates/provider-java`:**
  - `jab.rs` and `provider.rs`: set-aside and held windows keep their claims and get no enablement diagnostic.
  - `tests/live_fixture.rs`: a two-JVM live test with one wedged JVM, a JAB check on a Java 21 runtime, fixture plumbing for app arguments, and a tracing subscriber.
- **No change to `apps/test-app-swing`:** the fixture already has `--wedge-after` and `--wedge-for`.
- **The justfile's Windows lane:** hands over the Java 21 launcher for the new check.
- **Documentation:** `dev-docs/platform-windows.md` (the JAB threading model, hit-testing and the configuration) and the module docs of `pump.rs` and `client.rs`.
- **Python and Robot Framework:** no code change. The behavior reaches them through the native extension, so a native rebuild is needed.
- **Platforms:** Windows only. JAB exists only there, and the JAB crate is built only for Windows. No other provider changes.
- **Sequencing:**
  - `snapshot-validity` lands first. It owns the `.config/nextest.toml` serialization, the `fixture_application` test helper and `JabAppNode`'s process identity.
  - `fix-jab-hit-test-virtual-children` rewrites the same paragraph of `platform-windows.md` and touches `live_fixture.rs`. Whichever lands second rebases.
  - `application-process-attributes` edits `WindowCandidate` and `discover_java_windows`; it rebases onto this change.
