## MODIFIED Requirements

### Requirement: Single appearance under hit-testing
When the Java provider has claimed a Java top-level window through its JAB backend, a point over that window SHALL resolve to the JAB node, not the UIA shell, regardless of provider order: the UIA provider SHALL abstain from `element_at_point` for windows claimed by another provider (config `providers.windows-uia.honor_window_claims`, default true). With the kill switch off, UIA MAY resolve the shell. The Java provider SHALL route a point to the first backend that does not abstain, and SHALL pass the abstention on when no backend answers, so a point over a window it does not claim falls through to the platform's native provider exactly as before.

#### Scenario: Claimed Java window resolves to the JAB node
- **WHEN** the fixture app runs with the bridge enabled and claims are honored, and a point inside its window is hit-tested
- **THEN** the resolved node carries `@Technology = "JAB"` (not the UIA shell)

#### Scenario: Kill switch lets UIA hit-test the shell again
- **WHEN** `providers.windows-uia.honor_window_claims` is false and the UIA provider hit-tests a point over a Java window
- **THEN** the UIA provider resolves an element for that window (the shell), distinguishable via `@Technology`

### Requirement: Bounded hit-testing against unresponsive JVMs
JAB hit-testing SHALL run on the backend's pump thread under the per-call deadline (`providers.java.jab.call_timeout_ms`); a hit-test against an unresponsive JVM SHALL return within the deadline margin as a provider error, and MUST NOT hang the runtime or other providers. The behavior is unchanged by the backend refactor — only the configuration key moves into the `providers.java.jab.*` namespace.

#### Scenario: Frozen JVM does not hang the picker
- **WHEN** the fixture app's event-dispatch thread is suspended and a point over its window is hit-tested
- **THEN** the call returns (with an error or no JAB hit) within the configured deadline margin, and a concurrent UIA hit-test elsewhere completes normally
