# Spec Delta

## MODIFIED Requirements

### Requirement: Point-based hit-testing of Java windows

The JAB provider SHALL implement `element_at_point`.

- **Finding the window.** The window under a point is resolved via `WindowFromPoint`, then its root window (`GetAncestor(GA_ROOT)`), then its window class, then `isJavaWindow`.
- **A point over a Java window.** For such a point the provider SHALL return the deepest accessible node at that point, using the bridge's native hit-test (`getAccessibleContextAt`), as a `control:`/`item:` node with `@Technology = "JAB"`.
- **A bridge that reports no context.** The JDK's native hit-test answers null for every point until the target JVM has observed a mouse event (`EventQueueMonitor.currentMousePosition`). The provider SHALL therefore fall back to a bounded geometric descent over calibrated child bounds when the bridge reports no context.
- **The frame area.** It SHALL resolve a point over the window but outside every child to the window node itself.
- **A point not over a Java window.** For such a point, and for a point over the host process's own window, the provider SHALL report `UnsupportedOperation`, so other providers handle the point.
  - For a root window whose class does not mark it as an AWT window, that is whose class name does not start with `SunAwt`, the provider SHALL report `UnsupportedOperation` without a bridge call.
- **Which scenarios need a real provider.** A scenario needs a live JVM with the bridge enabled, and runs in the Windows acceptance lane against the Swing fixture app with the in-JVM agent disabled, unless it says otherwise.

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

### Requirement: Single appearance under hit-testing

When the Java provider has claimed a Java top-level window through its JAB backend, a point over that window SHALL resolve to the JAB node, or to a provider error while the window is held. It SHALL never resolve to the UIA shell, regardless of provider order.

- **The UIA provider** SHALL abstain from `element_at_point` for windows claimed by another provider (config `providers.windows-uia.honor_window_claims`, default true). With the kill switch off, UIA MAY resolve the shell.
- **The Java provider** SHALL route a point to the first backend that does not abstain. It SHALL pass the abstention on when no backend answers, so a point over a window it does not claim falls through to the platform's native provider.

#### Scenario: Claimed Java window resolves to the JAB node
- **WHEN** the fixture app runs with the bridge enabled and claims are honored, and a point inside its window is hit-tested
- **THEN** the resolved node carries `@Technology = "JAB"` (not the UIA shell)

#### Scenario: Kill switch lets UIA hit-test the shell again
- **WHEN** `providers.windows-uia.honor_window_claims` is false and the UIA provider hit-tests a point over a Java window
- **THEN** the UIA provider resolves an element for that window (the shell), distinguishable via `@Technology`

#### Scenario: A point over a held window yields a provider error, not the UIA shell

- **GIVEN** a window the bridge served, now held because the bridge is held past its deadline, with claims honored
- **WHEN** a point inside that window is hit-tested through the runtime
- **THEN** the result is a provider error, and the UIA provider does not resolve the shell
- **NOTE:** Verifiable at unit level in two halves: the JAB gate with a scripted probe, and the Java provider's router with its stub backend passing a backend's provider error on. UI Automation's abstention for claimed windows is covered by *Claimed Java window resolves to the JAB node*.

### Requirement: Bounded hit-testing against unresponsive JVMs

JAB hit-testing SHALL run on the backend's pump thread under the per-call deadline (`providers.java.jab.call_timeout_ms`). It follows the same rules for waiting and failing as every other JAB call (`jab-provider`, *Robustness against unresponsive JVMs*).

- **An unresponsive JVM.** A hit-test against an unresponsive JVM SHALL return within the deadline margin as a provider error.
- **A held bridge.** A hit-test while the bridge is held past its deadline SHALL fail at once.
- **Everything else.** The hit-test MUST NOT hang the runtime or other providers in either case.

#### Scenario: Frozen JVM does not hang the picker
- **WHEN** the fixture app's event-dispatch thread is suspended and a point over its window is hit-tested
- **THEN** the call returns (with an error or no JAB hit) within the configured deadline margin, and a concurrent UIA hit-test elsewhere completes normally

#### Scenario: A held bridge fails a hit-test at once

- **GIVEN** the bridge held past its deadline by a call about another JVM's window
- **WHEN** a point over a served JVM's window is hit-tested
- **THEN** the call returns a provider error within a quarter of the deadline
- **NOTE:** Verifiable at unit level with a controllable pump. Live only when a stuck bridge call outlasts the deadline, which the wedged fixture shows or not.
