## MODIFIED Requirements

### Requirement: Bounded hit-testing against unresponsive JVMs
JAB hit-testing SHALL run on the backend's pump thread under the per-call deadline (`providers.java.jab.call_timeout_ms`); a hit-test against an unresponsive JVM SHALL return within the deadline margin as a provider error, and MUST NOT hang the runtime or other providers. The behavior is unchanged by the backend refactor — only the configuration key moves into the `providers.java.jab.*` namespace.

#### Scenario: Frozen JVM does not hang the picker
- **WHEN** the fixture app's event-dispatch thread is suspended and a point over its window is hit-tested
- **THEN** the call returns (with an error or no JAB hit) within the configured deadline margin, and a concurrent UIA hit-test elsewhere completes normally
