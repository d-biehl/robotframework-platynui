## ADDED Requirements

### Requirement: Agent-presence signal
The classification SHALL report whether a PlatynUI agent is present in a JVM window's process, derived from the agent's per-user handshake file (existing for that PID and answering the connection handshake). A handshake file whose PID no longer runs SHALL be treated as stale (not present, eligible for cleanup). The signal feeds the Java provider's backend selection and SHALL be observable like the other classification facts; consuming it SHALL NOT itself trigger any attach or instrumentation.

#### Scenario: Agent-backed JVM is flagged
- **WHEN** a Swing JVM launched with `-javaagent` is classified
- **THEN** the classification reports agent-present for that window's process, observable like the other classification facts

#### Scenario: Stale handshake file is not reported as present
- **WHEN** a handshake file exists for a PID that is no longer running
- **THEN** the classification reports no agent for that PID and the stale file is ignored (eligible for cleanup), so no provider attempts a connection
