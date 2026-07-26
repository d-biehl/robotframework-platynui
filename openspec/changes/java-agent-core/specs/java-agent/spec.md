## ADDED Requirements

### Requirement: Injection into a running JVM without launch changes
The agent SHALL be loadable into an **already-running** JVM through a natively implemented attach transport — the primary path, since Java applications are commonly launched by scripts, installers or Web Start and are inspected while running. It SHALL equally be loadable at launch via `-javaagent`, the durable fallback. Neither path SHALL require a JDK, a bundled foreign binary, or any other Java tooling on the test host. The attach path is documented as subject to JEP 451 (works with a warning on current JDKs, opt-in via `-XX:+EnableDynamicAgentLoading`, disallowed by default in a future JDK). Failure to load SHALL distinguish "attach failed" from "agent rejected inside the target" (e.g. a sandboxed-JNLP `SecurityManager`). (Real-target-only: these scenarios need a live JVM and run against the fixture.)

#### Scenario: A running application is instrumented without being restarted
- **WHEN** a Java application started by its own script, with no PlatynUI arguments, is targeted while the agent package is installed
- **THEN** the agent is loaded into that JVM through the attach transport and becomes reachable, without restarting or relaunching the application

#### Scenario: No JDK on the test host
- **WHEN** the test host has only a JRE, or no Java installation at all beyond the application under test
- **THEN** injection still succeeds, because the attach protocol is implemented natively rather than delegated to a JDK tool or a bundled binary

#### Scenario: Agent rejected inside the target is distinguishable
- **WHEN** injection reaches the target JVM but the agent cannot initialise there
- **THEN** the failure is reported as agent-init-refused, distinct from a failed attach, so the diagnostic names the actual cause

### Requirement: Rendezvous, authentication and discovery via a handshake file
On startup the agent SHALL publish a handshake file in a per-user, owner-only directory, keyed by process id, carrying the loopback port it bound, a random token, the set of active toolkits, and its version; it SHALL remove the file on shutdown. Clients SHALL discover agents by reading these files and SHALL present the token when connecting. A handshake file whose process no longer exists SHALL be treated as stale. The token SHALL NOT be passed as a launch argument. Provider and agent versions SHALL match exactly; a mismatch SHALL abort the connection with a diagnostic naming both versions and the remedy.

#### Scenario: Two concurrent target JVMs stay distinct
- **WHEN** two instrumented JVMs run at the same time
- **THEN** each publishes its own handshake file with its own port and token, and a client reaches each one under its correct process id

#### Scenario: Stale handshake file is ignored
- **WHEN** a handshake file exists for a process that is no longer running
- **THEN** no connection is attempted and the file is eligible for cleanup

#### Scenario: Version mismatch aborts the connection
- **WHEN** a client connects to an agent of a different version (e.g. from another virtual environment)
- **THEN** the connection is aborted with a diagnostic naming both versions and the remedy — no degraded or partial operation

### Requirement: Bounded, multi-client agent runtime
The agent SHALL accept multiple concurrent client connections, so that an Inspector session and a test run do not lock each other out. All work SHALL be marshaled onto the toolkit thread under a per-call deadline **on the agent side as well as the client side**, and results abandoned by a deadline SHALL be discarded rather than block the handler. An unresponsive agent SHALL surface as bounded errors to its clients, never as a runtime hang. Elements handed out SHALL carry agent-assigned identities backed by weak references, and the agent SHALL expose a cheap liveness answer for such an identity, so clients can report node validity honestly.

#### Scenario: Two clients share one agent
- **WHEN** an Inspector session and a test run are connected to the same agent simultaneously
- **THEN** both are served and neither blocks the other

#### Scenario: A wedged toolkit thread does not pin the agent
- **WHEN** the target's toolkit thread stops processing while a call is in flight
- **THEN** the call is abandoned at its deadline, the client receives a bounded error, and the agent remains able to serve later calls once the thread recovers

#### Scenario: Liveness is answerable per element
- **WHEN** an element identity is queried for liveness after its object has been detached or its window closed
- **THEN** the agent reports it as no longer live, without walking the whole tree

### Requirement: Delivery as an opt-in package
The agent artifact SHALL be delivered in a separate installable package, discovered through the `platynui.providers` entry-point group — resolved in-process where the runtime already runs inside Python, and via the co-located environment interpreter for standalone binaries; an explicit configuration setting SHALL override discovery. Installing that package SHALL be the consent for in-JVM instrumentation: when it is absent, Java agent support SHALL be reported unavailable with an actionable diagnostic and nothing SHALL be injected. (Whether a detected JVM is then attached *automatically* is the consuming provider's policy — see `java-provider`.)

#### Scenario: Missing package yields an actionable diagnostic
- **WHEN** the agent package is not installed and a Java application is encountered
- **THEN** nothing is injected, existing providers serve the application unchanged, and the diagnostic names the install as the remedy

#### Scenario: The agent artifact is found from a standalone binary
- **WHEN** a standalone binary installed in an environment resolves the agent artifact with no explicit configuration
- **THEN** it finds the artifact belonging to that environment, without embedding a Python interpreter

### Requirement: Quiescence when inactive
When the agent support is inactive — its package absent or explicitly disabled — the runtime SHALL perform no Java-related activity: no handshake-directory scanning, no attach, no agent artifact resolution. Machine-wide JVM enumeration SHALL NOT be performed at any time; only processes owning windows already under consideration are relevant.

#### Scenario: A non-Java session touches nothing Java
- **WHEN** a session runs with agent support inactive and no Java application present
- **THEN** no handshake directory is scanned, no attach is attempted, and no Java-related file or process access occurs
