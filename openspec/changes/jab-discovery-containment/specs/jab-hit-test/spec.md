# Spec Delta

## MODIFIED Requirements

### Requirement: Point-based hit-testing of Java windows

The JAB provider SHALL implement `element_at_point`.

- **A point over a Java window.** A Java top-level window is resolved via `WindowFromPoint`, then its root owner, then its window class, then `isJavaWindow`. For a point over such a window, the provider SHALL return the deepest accessible node at that point, using the bridge's native hit-test (`getAccessibleContextAt`), as a `control:`/`item:` node with `@Technology = "JAB"`.
- **A bridge that reports no context.** The JDK's native hit-test answers null for every point until the target JVM has observed a mouse event (`EventQueueMonitor.currentMousePosition`). The provider SHALL therefore fall back to a bounded geometric descent over calibrated child bounds when the bridge reports no context.
- **The frame area.** It SHALL resolve a point over the window but outside every child to the window node itself.
- **A point not over a Java window.** For such a point the provider SHALL report `UnsupportedOperation`, so other providers handle the point. For a point over the host process's own window it SHALL report no hit.
  - A root window whose class does not mark it as an AWT window, that is whose class name does not start with `SunAwt`, is not a Java window.
  - The provider SHALL decide this without a bridge call.
- **Which scenarios need a real provider.** A scenario needs a live JVM with the bridge enabled, and runs in the Windows acceptance lane against the Swing fixture app, unless it says otherwise.

#### Scenario: Pick a control inside a Swing window
- **WHEN** `element_at_point` is evaluated at the bounds-center of the fixture's stage-1 button
- **THEN** exactly one node is returned, it is the stage-1 button (its designated `@Name`, role `Button`), and it carries `@Technology = "JAB"`

#### Scenario: Point outside any Java window is deferred
- **WHEN** `element_at_point` is evaluated at a point over a non-Java window
- **THEN** the JAB provider does not claim the hit (it reports the operation unsupported), so another provider resolves the point

#### Scenario: A point over a non-AWT window is deferred while the bridge is held

- **GIVEN** the bridge held by a call that has run past its deadline
- **WHEN** `element_at_point` is evaluated at a point whose root window's class does not start with `SunAwt`
- **THEN** the JAB provider reports the operation unsupported at once, without a bridge call
- **NOTE:** Verifiable at unit level; the gate needs no bridge answer.

### Requirement: Bounded hit-testing against unresponsive JVMs

JAB hit-testing SHALL run on the backend's pump thread under the per-call deadline (`providers.java.jab.call_timeout_ms`). It follows the same rules for waiting and failing as every other JAB call (`jab-provider`, *Robustness against unresponsive JVMs*).

- **An unresponsive JVM.** A hit-test against an unresponsive JVM SHALL return within the deadline margin as a provider error.
- **A held bridge.** A hit-test while the bridge is held by a call past its deadline SHALL fail at once.
- **Everything else.** The hit-test MUST NOT hang the runtime or other providers in either case.

#### Scenario: Frozen JVM does not hang the picker
- **WHEN** the fixture app's event-dispatch thread is suspended and a point over its window is hit-tested
- **THEN** the call returns (with an error or no JAB hit) within the configured deadline margin, and a concurrent UIA hit-test elsewhere completes normally

#### Scenario: A held bridge fails a hit-test at once

- **GIVEN** two bridge-enabled fixture JVMs, the event thread of one of them wedged and the bridge held past its deadline
- **WHEN** a point over the healthy JVM's window is hit-tested through the Java provider
- **THEN** the call returns a provider error within half a deadline
