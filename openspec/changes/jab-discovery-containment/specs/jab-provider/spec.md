# Spec Delta

## ADDED Requirements

### Requirement: A Java window that does not answer does not hide the other Java windows

Discovery SHALL NOT abandon its pass because the bridge did not answer about one top-level window.

- **Setting a window aside.** Discovery SHALL set a window aside, and go on with the windows it can still ask, when either:
  - a bridge call about that window has itself run for the full per-call deadline; or
  - the bridge answered that the window is not a Java window only after holding the call for more than 250 ms, which a bridge that answers does not need; or
  - the bridge answered the window's context lookup with no context only after holding the call for more than 250 ms.

  A call that timed out only because it waited or started late SHALL leave the window *not asked*, not set aside.
- **Attributing a failure.** A failure is attributed to the window's own JVM when either:
  - the call was aimed at that JVM; or
  - it was a window call, and the bridge client sends window calls to the window's own JVM alone.
- **Asking again.** A set-aside window SHALL NOT be asked again sooner than 1 s after it was set aside, unless it has become due earlier (below).
  - A re-probe fails when it does not answer within the deadline, or answers only after more than 250 ms. A re-probe that meets a busy bridge was not asked and does not fail.
  - After a failed re-probe whose failure is attributed to the window's own JVM, that window's interval SHALL double, up to 16 s. It SHALL reset once the window answers.
  - The set-aside windows whose failures are not attributed to their own JVM share one interval. A failed re-probe of any of them SHALL double it, up to 16 s.
  - When a window call answers within 250 ms, that shared interval SHALL reset, and those windows SHALL become due from the next pass on.
- **Held windows.** A window is held while it is set aside, or while the bridge was too busy to ask it and it was served or held in the last desktop pass.
  - It stays held until the bridge answers for it, it is no longer a visible top-level window, or its process has ended.
  - A blamed call on any path MAY set its own window aside.
  - Only a desktop pass updates which windows count as served or held in the last desktop pass, and only a desktop pass forgets a set-aside window. Listing the windows of one `app:Application` never drops a held window.
- **What a held window does.**
  - It SHALL keep its window claim if the bridge has served it before. A set-aside window that the bridge never served SHALL stay unclaimed.
  - It SHALL yield no node.
  - It SHALL NOT be reported as a window whose bridge is not enabled.
  - Its process SHALL stay a candidate for the in-JVM agent.
- **Other JVMs.** A window that was held only because the bridge was busy SHALL be asked in the next pass once the bridge answers.

#### Scenario: A JVM that is starting up does not hide the JVMs already running

- **GIVEN** a served JVM B, and a JVM A whose window does not answer its first bridge call within the deadline
- **WHEN** the desktop is enumerated again after A's call has returned
- **THEN** B's window is served and B's `app:Application` is present, while A's window is set aside and not asked in that pass
- **NOTE:** Verifiable at unit level with scripted windows, scripted bridge answers and a controlled clock.

#### Scenario: A window that only waited behind a stuck call is not set aside

- **GIVEN** a call about B's window that started late, behind another call that held the bridge, and was still running when its caller's deadline passed
- **WHEN** that call then completes within a deadline of its own start
- **THEN** B's window is not set aside, and it is asked normally in the next pass
- **NOTE:** Verifiable at unit level with a controllable pump.

#### Scenario: A slow "not a Java window" answer sets the window aside

- **GIVEN** an AWT window about which the bridge answers "not a Java window" only after 0.8 of the deadline
- **WHEN** discovery asks about it
- **THEN** the window is set aside, and it is not reported as a window whose bridge is not enabled
- **NOTE:** Verifiable at unit level with scripted bridge answers.

#### Scenario: A set-aside window is asked again on a doubling interval

- **GIVEN** a window set aside after a failure attributed to its own JVM
- **WHEN** the desktop is enumerated repeatedly within 1 s of that
- **THEN** the bridge is not asked about that window in those passes
- **AND WHEN** its re-probes keep failing
- **THEN** they come 1 s, 2 s, 4 s, 8 s and then every 16 s apart
- **AND WHEN** a re-probe answers
- **THEN** the window is served again in that pass, and the interval starts again at 1 s
- **NOTE:** Verifiable at unit level with scripted bridge answers and a controlled clock.

#### Scenario: A window whose failure is not attributed is asked as soon as the bridge answers

- **GIVEN** a bridge client that sends window calls to every connected JVM, and a window set aside after its window call ran past the deadline
- **WHEN** its re-probes keep stalling, and then another window call completes within its deadline
- **THEN** its interval has not grown while its re-probes stalled, and it is asked in the next pass, whether or not 1 s has passed
- **NOTE:** Verifiable at unit level with scripted bridge answers, a scripted client behavior and a controlled clock.

#### Scenario: A served window stays held through a long stall

- **GIVEN** a window served before the bridge became busy
- **WHEN** three desktop passes in a row find the bridge busy, and the windows of one `app:Application` are listed in between
- **THEN** the window is held in all three passes and keeps its claim, and listing the application's windows changes nothing about which windows are held
- **NOTE:** Verifiable at unit level with scripted bridge answers.

#### Scenario: A window that does not answer is not reported as bridge-not-enabled

- **GIVEN** a held window whose class starts with `SunAwt`
- **WHEN** the Java provider emits its enablement diagnostics for the pass
- **THEN** no enablement diagnostic is logged for that window, and it stays claimed if the bridge served it before
- **NOTE:** Verifiable at unit level with a stub backend in the Java provider.

#### Scenario: A Swing window keeps its place while the bridge does not answer

- **GIVEN** two bridge-enabled fixture JVMs, both served, then the event thread of one of them wedged, with window claims honored
- **WHEN** the Java and UI Automation providers enumerate the desktop throughout the wedge
- **THEN** no UI Automation shell appears for either Swing window

#### Scenario: The other JVM's windows return once the bridge answers

- **GIVEN** two bridge-enabled fixture JVMs, the event thread of one of them wedged
- **WHEN** the fixture reports that its event queue is released
- **THEN** the healthy JVM's window and its `app:Application` are present in every pass that starts at least one deadline plus 1 s later
- **AND** the wedged JVM's window is served again within 16 s plus one deadline of the release

### Requirement: A JVM that does not answer is reported once per episode, naming its process

The JAB backend SHALL report once at warning level when the bridge does not answer. That is the case when a call has itself run past its deadline, when the bridge's own message handling has held it that long, when a re-probe of a window set aside for a slow answer again answers only after more than 250 ms, or when a JVM has been marked degraded. A single slow answer is no such case. A call that fails only because the caller of the call holding the bridge has stopped waiting is no such case: it SHALL add debug records only.

- **Content.** The report SHALL name:
  - the JVM that does not answer, by its process (its pid and, where known, its name), when the backend can tell which JVM it is;
  - otherwise the window the bridge was being asked about, with that window's process, stated as the window being asked about and not as the JVM that does not answer;
  - otherwise, for a stall of the bridge's own message handling with no sender window, that the bridge's message handling holds it;
  - the call and the deadline.
- **Consequence.** It SHALL state that the JVM's windows are missing from query results until it answers, and, while the bridge is held, the windows of every JVM served through the bridge.
- **Who reports.** The report SHALL come from the part of the backend that swallows the failure: the desktop pass, the listing of an application's windows, and reads that turn the failure into a missing value. A call whose failure is returned to its caller, such as a hit-test or a pattern action, SHALL add debug records only.
- **While it persists.** The condition SHALL be recorded at debug level only.
- **One episode per stall.** When the backend identifies the JVM behind an episode it reported by a window or by the message handling only, the episode SHALL continue under that JVM, with a debug record and without a second warning.
- **End of an episode.**
  - The episode of a JVM ends when the same call for that JVM or window, made again, answers within its deadline; when a re-probe of one of its windows, or a health probe of the JVM, answers; or when its process ends.
  - An episode reported by a window being asked about, or by the bridge's message handling, ends when the bridge is no longer held and a bridge call answers within its deadline.
  - The stuck call itself returning late, and other calls for the process, SHALL NOT end an episode.
  - The end SHALL be recorded once, at debug level. A later failure SHALL start a new episode.
- **Late reports.** A condition whose episode ended before it was reported SHALL NOT be reported at warning level; its end is recorded at debug level.
- **No repeat.** The failed calls themselves SHALL NOT be reported again at warning level.

#### Scenario: A wedged JVM is reported once

- **GIVEN** a bridge-enabled fixture JVM whose event thread is wedged, and a healthy one
- **WHEN** the desktop is enumerated many times during the wedge
- **THEN** exactly one warning reports the stall, stating the call and the deadline and what is missing from query results
- **AND** the later passes of the same wedge add debug records only
- **NOTE:** Verifiable at unit level with a controllable pump, and live with the wedged fixture.

#### Scenario: A stall of a call aimed at one JVM names that JVM's process

- **GIVEN** a JVM whose process is known to the backend, from discovery or from a hit-test
- **WHEN** a call aimed at that JVM holds the bridge past its deadline, and the desktop is enumerated
- **THEN** the warning names that process's pid as the JVM that does not answer
- **NOTE:** Verifiable at unit level with a controllable pump.

#### Scenario: An episode ends at debug level, and a new stall warns again

- **GIVEN** a JVM that was reported as not answering
- **WHEN** the same call for that JVM, made again, answers within its deadline
- **THEN** one debug record marks the end of the episode, and no record at info or warning level is added
- **AND WHEN** it stops answering again later
- **THEN** a new warning reports it
- **NOTE:** Verifiable at unit level.

#### Scenario: A prompt call between two stalls does not start a new episode

- **GIVEN** a JVM that was reported as not answering
- **WHEN** some other call for its process completes within its deadline, and the stall goes on
- **THEN** no second warning is added
- **NOTE:** Verifiable at unit level.

#### Scenario: A stuck call that returns late does not end the episode

- **GIVEN** a JVM that was reported as not answering
- **WHEN** the call that stalled returns after its deadline, and the next re-probe stalls again
- **THEN** no debug end record and no second warning are added
- **NOTE:** Verifiable at unit level with a controllable pump.

#### Scenario: Identifying the stuck JVM during a stall adds no second warning

- **GIVEN** a stall reported by the window the bridge was being asked about
- **WHEN** a call aimed at one JVM identifies that JVM as the one that does not answer, while the stall goes on
- **THEN** the episode continues under that JVM with a debug record, and no second warning is added
- **NOTE:** Verifiable at unit level with a controllable pump.

#### Scenario: A JVM whose process ended ends its episode

- **GIVEN** a JVM that was reported as not answering
- **WHEN** its process ends
- **THEN** one debug record marks the end of the episode, and a new process that stops answering later is reported again
- **NOTE:** Verifiable at unit level.

#### Scenario: A hit-test during a stall adds no warning

- **GIVEN** a bridge held past its deadline
- **WHEN** a hit-test fails because of it, and the desktop is enumerated afterwards
- **THEN** the hit-test adds debug records only, and the enumeration adds the one warning
- **NOTE:** Verifiable at unit level.

#### Scenario: A healthy JVM is never named as not answering

- **GIVEN** two bridge-enabled fixture JVMs, both served, then the event thread of one of them wedged
- **WHEN** the desktop is enumerated, and the healthy JVM's window is read, during and after the wedge
- **THEN** no warning names the healthy JVM as the one that does not answer
- **AND** a read of the healthy window that starts at least one deadline after the fixture reports its event queue released returns its name, not a failure for a degraded JVM

#### Scenario: A healthy run of the Robot suites reports no JAB warning

- **GIVEN** the Robot Framework suites of the Windows acceptance lane, none of which wedges or freezes a fixture
- **WHEN** they run to completion
- **THEN** their output holds no warning from the JAB backend

## MODIFIED Requirements

### Requirement: Java top-level window discovery

The provider SHALL discover Java top-level windows via `EnumWindows` and `isJavaWindow`, and expose each as a `control:Window` (dialogs as `Dialog`) under the desktop, with `Technology` reported as "JAB", plus `app:Application` grouping by process id.

- **Which windows are asked.** The provider SHALL ask `isJavaWindow` only about visible top-level windows whose window class marks them as AWT windows, that is whose class name starts with `SunAwt`. A window of any other class SHALL cost no bridge call:
  - in the desktop pass;
  - when the windows of an `app:Application` are listed;
  - in the wait for the bridge's first rendezvous after connecting. That wait SHALL be skipped when no AWT window that this backend could serve is visible. The host's own windows and windows another Java backend serves do not count.
- **Which scenarios need a real provider.** Every scenario in this spec is real-provider-only unless it says otherwise. Such a scenario requires a live JVM with the bridge enabled, and runs in the Windows acceptance lane against the Swing fixture app with the in-JVM agent disabled (`providers.java.agent.enabled = false`).

#### Scenario: Fixture window appears
- **WHEN** the Swing fixture app runs with the bridge enabled and the desktop children are enumerated
- **THEN** exactly one `control:Window` with the fixture's title exists with `@Technology = "JAB"`, and an `app:Application` node for the fixture's PID groups it

#### Scenario: Late-started JVM appears on a later poll
- **WHEN** the fixture app starts after the runtime already answered a query
- **THEN** a subsequent query finds the fixture window without recreating the runtime

#### Scenario: A window that is not an AWT window costs no bridge call

- **GIVEN** a desktop whose visible top-level windows include AWT windows and windows of other classes
- **WHEN** the provider discovers Java windows
- **THEN** the bridge is asked only about the AWT windows, and the other windows are neither served, set aside, nor reported as unserved
- **NOTE:** Verifiable at unit level with scripted windows and scripted bridge answers.

#### Scenario: No rendezvous wait without an AWT window to serve

- **GIVEN** a connected bridge, and a desktop whose only visible AWT windows, if any, belong to the host process or are served by another Java backend
- **WHEN** the first enumeration after connecting runs
- **THEN** it returns without waiting for the bridge's rendezvous, and without a bridge call
- **NOTE:** Verifiable at unit level with a scripted candidate source and a controlled clock.

#### Scenario: A modern JDK's window is discovered through its AWT class

- **GIVEN** the Swing fixture running on a Java 21 runtime with the bridge enabled
- **WHEN** the desktop children are enumerated
- **THEN** the fixture's window is present with `@Technology = "JAB"` and reports the Swing/AWT toolkit

### Requirement: Robustness against unresponsive JVMs

JAB calls SHALL run on the backend's dedicated pump thread with a per-call deadline (`providers.java.jab.call_timeout_ms`).

- **The deadline.** It bounds how long a caller waits for one bridge call, counted from the moment it asks. A health probe that precedes a call has a deadline of its own.
- **A call that never started.** A call that had not started when its caller stopped waiting SHALL NOT run afterwards, and SHALL NOT count against the JVM it targets. It SHALL fail with an error saying that the bridge was busy.
- **A call that ran through its deadline.** A call aimed at one JVM that has itself run for the full deadline SHALL count against that JVM. Repeated such calls SHALL mark that JVM degraded; it is then skipped until a health probe succeeds.
- **A held bridge.** While the bridge is held for at least the deadline, every further call SHALL fail at once instead of waiting. That holds whether a call or the bridge's own message handling holds it. It also holds as soon as the caller of the call that holds the bridge has stopped waiting for it. The Access Bridge's part of a discovery pass therefore costs at most one deadline per stall, however many windows the pass covers; a pass that asks a held window again may cost one deadline more.
- **A failing call.** A failure inside one bridge call SHALL fail that call only; the bridge SHALL stay usable for the next one.
- **Everything else.** The runtime and other providers MUST remain responsive throughout.

#### Scenario: Frozen JVM does not freeze the runtime
- **WHEN** the fixture app's event-dispatch thread is suspended and a query touches its tree
- **THEN** the query returns (with errors or without JAB results) within the configured deadline margin, and a concurrent UIA query completes normally

#### Scenario: A call waiting behind another JVM's stuck call does not count against its own JVM

- **GIVEN** a call for JVM A that is stuck on the bridge past its deadline
- **WHEN** calls for a healthy JVM B are made repeatedly during that time
- **THEN** each of them fails as "bridge busy" without running, and B is never marked degraded
- **AND** B's next call succeeds as soon as A's call has returned
- **NOTE:** Verifiable at unit level with a controllable pump.

#### Scenario: A held bridge fails further calls at once

- **GIVEN** a call that holds the bridge, whose caller has just stopped waiting for it
- **WHEN** another call is made
- **THEN** it fails within a quarter of the deadline, without being queued
- **NOTE:** Verifiable at unit level with a controllable pump, and live with the wedged fixture.

#### Scenario: A bridge held by its own message handling fails further calls at once

- **GIVEN** the bridge busy with its own message handling for longer than the deadline
- **WHEN** a call is made
- **THEN** it fails within a quarter of the deadline, and the backend knows that the bridge's message handling holds it
- **NOTE:** Verifiable at unit level with a controllable pump.

#### Scenario: Calls given up during a stall are not run afterwards

- **GIVEN** calls whose callers stopped waiting before the calls started
- **WHEN** the stuck call returns
- **THEN** none of those calls runs
- **NOTE:** Verifiable at unit level with a controllable pump.

#### Scenario: A JVM whose own calls run through the deadline is degraded

- **GIVEN** a JVM whose own calls each run for the full deadline
- **WHEN** that happens as often as the degraded threshold
- **THEN** the JVM is marked degraded, and further calls for it fail at once until a health probe succeeds
- **NOTE:** Verifiable at unit level with a controllable pump.

#### Scenario: A failing call does not take the bridge down

- **GIVEN** a bridge call that fails inside the bridge thread
- **WHEN** the next call is made
- **THEN** that call runs and succeeds, and no call is reported as held
- **NOTE:** Verifiable at unit level with a controllable pump.

#### Scenario: A discovery pass during a stall is bounded

- **GIVEN** two bridge-enabled fixture JVMs, the event thread of one of them wedged
- **WHEN** the desktop is enumerated repeatedly during the wedge
- **THEN** every pass that starts at least two deadlines after the fixture reports its event queue wedged, and that asks no window the bridge has not answered about since the wedge began, returns within half a deadline
- **AND** a concurrent UIA query completes normally

### Requirement: Enablement diagnostics without configuration mutation

When a top-level window's class name starts with `SunAwt` and `isJavaWindow` reports false within 250 ms, the JAB backend SHALL report that window as one it cannot serve. The Java provider SHALL log one actionable diagnostic per window, naming both enablement paths (the `-Djavax.accessibility.assistive_technologies` launch flag and `jabswitch`), for every such window no backend serves.

- **A slow or missing answer.** A window about which the bridge answered more slowly, or not at all, is set aside instead and not reported (see *A Java window that does not answer does not hide the other Java windows*).
- **No configuration mutation.** Neither the backend nor the provider SHALL write `.accessibility.properties`, registry keys, or any other target-side configuration.
- **Who emits it.** Whether a window is truly unreachable is a question only the provider that knows all backends can answer, so the Java provider emits the diagnostic.

#### Scenario: Bridge not enabled
- **WHEN** the fixture app runs without the enablement flag and the desktop is queried
- **THEN** the diagnostic is logged exactly once for that window and no file or registry mutation occurs

### Requirement: Single appearance of Java windows

When the Java provider has claimed a Java top-level window through its JAB backend, the merged desktop tree SHALL show that window at most once. A window is claimed through the JAB backend after a successful `GetAccessibleContextFromHWND`, and stays claimed while it is held after the bridge served it (see *A Java window that does not answer does not hide the other Java windows*).

- **While served,** the window SHALL be shown exactly once.
- **While held,** it SHALL NOT be shown at all.
- **The UIA provider** skips claimed windows (config `providers.windows-uia.honor_window_claims`, default true — keyed by the UIA provider's id per the config convention). It SHALL therefore never show the shell of a claimed window while claims are honored.
- **With the kill switch off,** both representations MAY appear and remain distinguishable via `@Technology`.

#### Scenario: No duplicates in the merged tree
- **WHEN** the fixture app runs with the bridge enabled and claims are honored
- **THEN** exactly one window node with the fixture's title exists under the desktop, and it carries `@Technology = "JAB"`

#### Scenario: Kill switch restores the UIA shell
- **WHEN** `providers.windows-uia.honor_window_claims` is false
- **THEN** the UIA shell window reappears alongside the JAB window (both locatable, distinguishable via `@Technology`)

#### Scenario: A held window is shown by no provider

- **GIVEN** a window the bridge served, now held, with claims honored
- **WHEN** the desktop is enumerated by the Java and UI Automation providers
- **THEN** neither a JAB node nor a UI Automation shell is shown for it
- **NOTE:** Verifiable at unit level with a stub backend in the Java provider; live, *A Swing window keeps its place while the bridge does not answer* checks it.
