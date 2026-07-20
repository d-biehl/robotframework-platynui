## ADDED Requirements

### Requirement: Point-based hit-testing of Java windows
The JAB provider SHALL implement `element_at_point`: for a desktop point over a Java top-level window (resolved via `WindowFromPoint` → root owner → `isJavaWindow`), it SHALL return the deepest accessible node at that point using the bridge's native hit-test (`getAccessibleContextAt`), as a `control:`/`item:` node with `@Technology = "JAB"`. Because the JDK's native hit-test answers null for every point until the target JVM has observed a mouse event (`EventQueueMonitor.currentMousePosition`), the provider SHALL fall back to a bounded geometric descent over calibrated child bounds when the bridge reports no context, and SHALL resolve a point over the window but outside every child (frame area) to the window node itself. For a point not over a Java window (or over the host process's own window) it SHALL report `UnsupportedOperation` (or no hit for its own process) so other providers handle the point. (Real-provider-only: requires a live JVM with the bridge enabled; runs in the Windows acceptance lane against the Swing fixture app.)

#### Scenario: Pick a control inside a Swing window
- **WHEN** `element_at_point` is evaluated at the bounds-center of the fixture's stage-1 button
- **THEN** exactly one node is returned, it is the stage-1 button (its designated `@Name`, role `Button`), and it carries `@Technology = "JAB"`

#### Scenario: Point outside any Java window is deferred
- **WHEN** `element_at_point` is evaluated at a point over a non-Java window
- **THEN** the JAB provider does not claim the hit (it reports the operation unsupported), so another provider resolves the point

### Requirement: Reveal-ready hit result
A node returned by JAB hit-testing SHALL carry the same `RuntimeId` that top-down traversal produces for that element (app-scoped, `jab://app/<pid>/…`) and a walkable parent chain up to its `app:Application`, so a consumer can reveal-and-select it in the tree by walking `parent()`.

#### Scenario: Picked node reveals to the same tree node
- **WHEN** a control is resolved by point and then located by top-down XPath
- **THEN** both carry the identical `RuntimeId`, and the picked node's ancestors resolve up to the `app:Application` node for the fixture's PID

### Requirement: Single appearance under hit-testing
When the JAB provider has claimed a Java top-level window, a point over that window SHALL resolve to the JAB node, not the UIA shell, regardless of provider order: the UIA provider SHALL abstain from `element_at_point` for windows claimed by another provider (config `providers.windows-uia.honor_window_claims`, default true). With the kill switch off, UIA MAY resolve the shell.

#### Scenario: Claimed Java window resolves to the JAB node
- **WHEN** the fixture app runs with the bridge enabled and claims are honored, and a point inside its window is hit-tested
- **THEN** the resolved node carries `@Technology = "JAB"` (not the UIA shell)

#### Scenario: Kill switch lets UIA hit-test the shell again
- **WHEN** `providers.windows-uia.honor_window_claims` is false and the UIA provider hit-tests a point over a Java window
- **THEN** the UIA provider resolves an element for that window (the shell), distinguishable via `@Technology`

### Requirement: Bounded hit-testing against unresponsive JVMs
JAB hit-testing SHALL run on the provider's pump thread under the per-call deadline (`providers.jab.call_timeout_ms`); a hit-test against an unresponsive JVM SHALL return within the deadline margin as a provider error, and MUST NOT hang the runtime or other providers.

#### Scenario: Frozen JVM does not hang the picker
- **WHEN** the fixture app's event-dispatch thread is suspended and a point over its window is hit-tested
- **THEN** the call returns (with an error or no JAB hit) within the configured deadline margin, and a concurrent UIA hit-test elsewhere completes normally
