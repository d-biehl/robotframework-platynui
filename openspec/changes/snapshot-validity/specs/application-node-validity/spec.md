# Spec Delta

## Purpose

An application node (`app:Application`) that a provider creates for a process reports whether that process is still running. A consumer that holds one — a pinned scoped root, a captured element, a cached snapshot — then notices when the application has ended instead of treating it as alive forever.

## ADDED Requirements

### Requirement: An application node is valid while its process runs

An application node that a provider creates for a process — UI Automation, the Java Access Bridge and the Java agent create one per process — SHALL report itself valid while the process it was created for is running, and invalid once that process has ended. The process SHALL be identified by its pid together with its start time, both recorded when the node is created, so that a later process that receives the same pid does not make the node valid again. An application node for a process that no longer exists when the node is created SHALL be invalid from the start. Validity SHALL NOT depend on whether the process still has windows.

#### Scenario: The application's process ends

- **GIVEN** an application node for a running application
- **WHEN** the application's process exits
- **THEN** the node SHALL report itself invalid, and `bool(node)` SHALL be `False`
- **NOTE:** Verifiable only against a real provider (UI Automation, the Java Access Bridge, the Java agent), not the mock. For UI Automation it is also exercised at the provider's unit level with a spawned child process.

#### Scenario: The application closes its windows but keeps running

- **GIVEN** an application node for a running application
- **WHEN** the application closes its last window and its process keeps running
- **THEN** the node SHALL still report itself valid
- **NOTE:** Exercised at the provider's unit level with a spawned child process that has no window.

#### Scenario: A reused pid is another process

- **GIVEN** an application node recorded for pid `P` with start time `T`
- **WHEN** a process with pid `P` but a different start time is running
- **THEN** the node SHALL report itself invalid
- **NOTE:** Exercised at the unit level with a recorded start time that does not match the running process. A real reuse of a pid cannot be provoked on demand.

#### Scenario: The process was gone before the node was created

- **GIVEN** a top-level window whose process exits after the window was listed
- **WHEN** the provider creates the application node for that process
- **THEN** the node SHALL report itself invalid from the start
- **NOTE:** Exercised at the unit level by creating the node for a pid that no running process has.

#### Scenario: A root pinned to an application is looked up again after the application ended

- **GIVEN** a BareMetal suite whose scoped root is `/app:Application[@ProcessId=${pid}]`, resolved while the application runs
- **WHEN** the application's process exits and a keyword looks up a relative target with a short timeout
- **THEN** the keyword SHALL fail with `RootNotFoundError` naming the root selector, not with `ElementNotFoundError` for the target
- **NOTE:** Verifiable only against a real provider: on the Windows lane for an application served by UI Automation and for a Swing application served by the Java Access Bridge and by the Java agent.

### Requirement: An application node whose process cannot be inspected stays valid

When the provider cannot tell whether the process is still the one the node was created for — the process cannot be opened because access is denied, or the platform offers no way to read a process's start time — the node SHALL report itself valid, as an application node did before this requirement. The node SHALL report itself invalid only when the provider knows that the process has ended or has been replaced.

#### Scenario: Access to the process is denied

- **GIVEN** an application node for a process whose start time cannot be read because access is denied
- **WHEN** the node is asked whether it is valid
- **THEN** it SHALL report itself valid
- **NOTE:** Exercised at the unit level with an identity whose check answers that it cannot tell.

#### Scenario: The start time could not be recorded, and the process is gone

- **GIVEN** an application node whose start time could not be recorded at creation because access was denied
- **WHEN** no process with the node's pid is running any more
- **THEN** the node SHALL report itself invalid
- **NOTE:** Exercised at the unit level. That the pid is free is something a denied process still reveals.

### Requirement: The agent's application node follows its agent session

The application node served by the Java agent SHALL also report itself invalid while its agent session is closed or degraded, as `java-provider` requires of every node the agent backend serves (*Node validity is answered, not assumed*). It SHALL answer from the state the provider already holds, without a call into the target JVM.

#### Scenario: The agent is degraded

- **GIVEN** an application node served by the Java agent
- **WHEN** its agent session becomes degraded
- **THEN** the node SHALL report itself invalid, and a root pinned to it SHALL be looked up again
- **NOTE:** Exercised at the unit level with a fake session. Against a real JVM the live Java tests cover it.

#### Scenario: The agent session is closed

- **GIVEN** an application node served by the Java agent
- **WHEN** its agent session is closed because the agent's handshake is gone
- **THEN** the node SHALL report itself invalid

#### Scenario: A validity check does not call into the JVM

- **GIVEN** an application node served by the Java agent whose JVM does not answer
- **WHEN** the node is asked whether it is valid
- **THEN** the answer SHALL come back without waiting for the JVM
- **NOTE:** Exercised at the unit level with a fake session that fails every call.
