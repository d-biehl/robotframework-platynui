# Spec Delta

## ADDED Requirements

### Requirement: A Java window that does not answer does not hide the other Java windows

When the bridge does not answer for a top-level window within the per-call deadline, discovery SHALL NOT abandon its pass. It SHALL set that window aside and go on with the windows it can still ask.

- **Asking again.** A set-aside window SHALL NOT be asked again until its re-probe is due, at a bounded rate. It SHALL be served again once a re-probe answers.
- **Which windows are held.** A window is held while it is set aside, or while the bridge is busy so that a window served in the previous pass could not be asked.
- **What a held window does.**
  - It SHALL keep its window claim.
  - It SHALL yield no node.
  - It SHALL NOT be reported as a window whose bridge is not enabled.
  - Its process SHALL stay a candidate for the in-JVM agent.
- **Other JVMs.** Once the bridge answers again, the windows of every other JVM SHALL be served in the next pass.

#### Scenario: A JVM that is starting up does not hide the JVMs already running

- **GIVEN** a served JVM B, and a JVM A whose window does not answer its first bridge call within the deadline
- **WHEN** the desktop is enumerated again after A's call has returned
- **THEN** B's window and B's `app:Application` are present, and A's window is set aside and not asked in that pass
- **NOTE:** Verifiable at unit level with scripted windows, scripted bridge answers and a controlled clock. Live, the two-instance Swing suites of the Windows lane exercise it.

#### Scenario: A set-aside window is asked again at a bounded rate

- **GIVEN** a window that was set aside
- **WHEN** the desktop is enumerated repeatedly before its re-probe is due
- **THEN** the bridge is not asked about that window in those passes
- **AND WHEN** its re-probe is due and the bridge answers
- **THEN** the window is served again in that pass
- **NOTE:** Verifiable at unit level with scripted bridge answers and a controlled clock.

#### Scenario: A window that does not answer is not reported as bridge-not-enabled

- **GIVEN** a set-aside window whose class starts with `SunAwt`
- **WHEN** the Java provider emits its enablement diagnostics for the pass
- **THEN** no enablement diagnostic is logged for that window, and it stays claimed
- **NOTE:** Verifiable at unit level with a stub backend in the Java provider.

#### Scenario: A Swing window keeps its place while the bridge does not answer

- **GIVEN** two bridge-enabled fixture JVMs, the event thread of one of them wedged, and window claims honored
- **WHEN** the Java and UI Automation providers enumerate the desktop during the wedge
- **THEN** no UI Automation shell appears for either Swing window

#### Scenario: The other JVM's windows return once the bridge answers

- **GIVEN** two bridge-enabled fixture JVMs, the event thread of one of them wedged
- **WHEN** the wedge ends
- **THEN** the healthy JVM's window and its `app:Application` are present in the first pass that starts after the bridge answers again
- **AND** the wedged JVM's window is served again within its re-probe interval

### Requirement: A JVM that does not answer is reported once per episode, naming its process

A JVM does not answer the bridge when a call for it has run past its deadline, whether that call still holds the bridge or has marked the JVM degraded. The JAB backend SHALL report such a JVM once at warning level.

- **Content.** The report SHALL name:
  - the process, by its pid and, where known, its name;
  - the call that did not answer;
  - the deadline.
- **Consequence.** It SHALL state that the JVM's windows are missing from query results until it answers, and, while it holds the bridge, the windows of every JVM served through the bridge as well.
- **While it persists.** The condition SHALL be recorded at debug level only.
- **End of an episode.** The episode ends when the JVM answers again or its process ends. The end SHALL be recorded once, at debug level. A later failure SHALL start a new episode.
- **No repeat.** The failed calls themselves SHALL NOT be reported again at warning level.

#### Scenario: A wedged JVM is named once

- **GIVEN** a bridge-enabled fixture JVM whose event thread is wedged
- **WHEN** the desktop is enumerated many times during the wedge
- **THEN** exactly one warning names that JVM's pid, the call and the deadline, and states what is missing from query results
- **AND** the later passes of the same wedge add debug records only
- **NOTE:** Verifiable at unit level with a controllable pump, and live with the wedged fixture.

#### Scenario: An episode ends at debug level, and a new stall warns again

- **GIVEN** a JVM that was reported as not answering
- **WHEN** a call for it completes within the deadline
- **THEN** one debug record marks the end of the episode, and no record at info or warning level is added
- **AND WHEN** it stops answering again later
- **THEN** a new warning names it
- **NOTE:** Verifiable at unit level.

#### Scenario: A healthy JVM is never named

- **GIVEN** two bridge-enabled fixture JVMs, the event thread of one of them wedged
- **WHEN** the desktop is enumerated during and after the wedge
- **THEN** no warning names the healthy JVM's pid, and the healthy JVM is never marked degraded

#### Scenario: A healthy run reports no JAB warning

- **GIVEN** the Windows acceptance lane
- **WHEN** it runs to completion with every fixture healthy
- **THEN** its output holds no warning from the JAB backend

## MODIFIED Requirements

### Requirement: Java top-level window discovery

The provider SHALL discover Java top-level windows via `EnumWindows` and `isJavaWindow`, and expose each as a `control:Window` (dialogs as `Dialog`) under the desktop, with `Technology` reported as "JAB", plus `app:Application` grouping by process id.

- **Which windows are asked.** The provider SHALL ask `isJavaWindow` only about visible top-level windows whose window class marks them as AWT windows, that is whose class name starts with `SunAwt`. A window of any other class SHALL cost no bridge call:
  - in the desktop pass;
  - when the windows of an `app:Application` are listed;
  - in the wait for the bridge's first rendezvous after connecting. That wait SHALL be skipped when no AWT window is visible.
- **Which scenarios need a real provider.** Every scenario in this spec is real-provider-only unless it says otherwise: it requires a live JVM with the bridge enabled and runs in the Windows acceptance lane against the Swing fixture app.

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

#### Scenario: No rendezvous wait without an AWT window

- **GIVEN** a connected bridge and a desktop without a visible AWT window
- **WHEN** the first enumeration after connecting runs
- **THEN** it returns without waiting for the bridge's rendezvous
- **NOTE:** Verifiable at unit level.

#### Scenario: A modern JDK's window is discovered through its AWT class

- **GIVEN** the Swing fixture running on a Java 21 runtime with the bridge enabled
- **WHEN** the desktop children are enumerated
- **THEN** the fixture's window is present with `@Technology = "JAB"` and reports the Swing/AWT toolkit

### Requirement: Robustness against unresponsive JVMs

JAB calls SHALL run on the backend's dedicated pump thread with a per-call deadline (`providers.java.jab.call_timeout_ms`). The deadline bounds how long a caller waits, counted from the moment it asks.

- **A call that never started.** A call that had not started when its caller stopped waiting SHALL NOT run afterwards, and SHALL NOT count against the JVM it targets. It SHALL fail with an error saying that the bridge was busy.
- **A call that ran through its deadline.** A call that has itself run for the full deadline SHALL count against its JVM. Repeated such calls SHALL mark that JVM degraded; it is then skipped until a health probe succeeds.
- **A held bridge.** While the bridge is held by a call that has already run past its deadline, every further call SHALL fail at once instead of waiting. A stall therefore costs a discovery pass at most one deadline, however many windows the pass covers.
- **Everything else.** The runtime and other providers MUST remain responsive throughout.

#### Scenario: Frozen JVM does not freeze the runtime
- **WHEN** the fixture app's event-dispatch thread is suspended and a query touches its tree
- **THEN** the query returns (with errors or without JAB results) within the configured deadline margin, and a concurrent UIA query completes normally

#### Scenario: A call waiting behind another JVM's stuck call does not count against its own JVM

- **GIVEN** a call for JVM A that is stuck on the bridge past its deadline
- **WHEN** calls for a healthy JVM B are made repeatedly during that time
- **THEN** each of them fails as "bridge busy" without running, and B is never marked degraded
- **AND** B's calls succeed as soon as A's call has returned
- **NOTE:** Verifiable at unit level with a controllable pump.

#### Scenario: A held bridge fails further calls at once

- **GIVEN** a call that has held the bridge past its deadline
- **WHEN** another call is made
- **THEN** it fails in well under the deadline, without being queued
- **NOTE:** Verifiable at unit level with a controllable pump, and live with the wedged fixture.

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

#### Scenario: A discovery pass during a stall is bounded

- **GIVEN** two bridge-enabled fixture JVMs, the event thread of one of them wedged
- **WHEN** the desktop is enumerated repeatedly during the wedge
- **THEN** every pass that starts after the stall has lasted one deadline returns within half a deadline, and a concurrent UIA query completes normally
