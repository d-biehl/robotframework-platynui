# java-agent Specification

## Purpose

The `java-agent` capability is the toolkit-neutral foundation every Java UI-toolkit adapter needs before it can read a single node: a PlatynUI-owned agent artifact that runs **inside** the target JVM, the transports that get it there and talk to it, and the way it is shipped and found. Native accessibility leaves hard gaps for Java toolkits (see `dev-docs/java-toolkits.md`); an in-JVM agent reads the toolkit's own in-process model instead and bypasses them.

The design splits a control plane from a data plane: injection happens once — via the native attach transport into a running JVM (the primary path) or via `-javaagent` at launch (the durable fallback) — after which all traffic flows over the agent's own loopback NDJSON-RPC connection, discovered and authenticated through a per-user handshake file. The attach protocol is spoken directly from Rust (`crates/java-agent`), so no JDK, no `jattach` and no bundled foreign binary is needed on the test host. The agent JAR is built by the Gradle product project `java/agent` and delivered by the pure-data wheel `platynui-provider-java`; installing that wheel is the consent for in-JVM instrumentation.

This capability ends where the adapters begin: the agent is in the JVM and answers. It surfaces no PlatynUI node of its own — mapping answers onto nodes belongs to `provider-java-swing` and its `-javafx`/`-swt` follow-ups.

## Requirements
### Requirement: Injection into a running JVM without launch changes
The agent SHALL be loadable into an **already-running** JVM through a natively implemented attach transport — the primary path, since Java applications are commonly launched by scripts, installers or Web Start and are inspected while running. It SHALL equally be loadable at launch via `-javaagent`, the durable fallback. Neither path SHALL require a JDK, a bundled foreign binary, or any other Java tooling on the test host. The attach path is documented as subject to JEP 451 (works with a warning on current JDKs, opt-in via `-XX:+EnableDynamicAgentLoading`, disallowed by default in a future JDK). Failure to load SHALL distinguish "attach failed" from "agent rejected inside the target" (a JEP 451 refusal, or a JVM that declines the agent library). A restrictive security policy in the target is **not** such a rejection — the agent operates independently of it. (Real-target-only: these scenarios need a live JVM and run against the fixture.)

#### Scenario: A running application is instrumented without being restarted
- **WHEN** a Java application started by its own script, with no PlatynUI arguments, is targeted while the agent package is installed
- **THEN** the agent is loaded into that JVM through the attach transport and becomes reachable, without restarting or relaunching the application

#### Scenario: No JDK on the test host
- **WHEN** the test host has only a JRE, or no Java installation at all beyond the application under test
- **THEN** injection still succeeds, because the attach protocol is implemented natively rather than delegated to a JDK tool or a bundled binary

#### Scenario: Agent rejected inside the target is distinguishable
- **WHEN** injection reaches the target JVM but the JVM declines to load or initialise the agent library at all — a JEP 451 refusal, or an artifact it will not accept
- **THEN** the failure is reported as agent-init-refused, distinct from a failed attach, so the diagnostic names the actual cause
- **AND** an agent that *was* loaded and then failed in its own start-up is not reported this way, because it deliberately keeps its failure out of the target — that case surfaces as the missing handshake instead

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

"The toolkit thread" SHALL be resolved **per element**, not assumed to be one per JVM. A JVM may partition its UI into several independent toolkit worlds — AWT's `AppContext` is the case in the field, created per application by the Java Web Start and applet runtimes — each with its own event queue, and posting to the wrong one runs the work on a thread that will never see the element. Every call SHALL therefore be dispatched to the toolkit thread belonging to the element it concerns, and the deadline and abandonment semantics above SHALL apply unchanged to each of them. An agent thread's own toolkit affiliation SHALL never be used as a stand-in for an element's.

#### Scenario: Two clients share one agent
- **WHEN** an Inspector session and a test run are connected to the same agent simultaneously
- **THEN** both are served and neither blocks the other

#### Scenario: A wedged toolkit thread does not pin the agent
- **WHEN** the target's toolkit thread stops processing while a call is in flight
- **THEN** the call is abandoned at its deadline, the client receives a bounded error, and the agent remains able to serve later calls once the thread recovers

#### Scenario: Liveness is answerable per element
- **WHEN** an element identity is queried for liveness after its object has been detached or its window closed
- **THEN** the agent reports it as no longer live, without walking the whole tree

#### Scenario: An element outside the agent's own toolkit world is served
- **WHEN** a call concerns an element that belongs to a different `AppContext` than the agent's threads do — the shape a Web Start application has
- **THEN** the call runs on that element's own toolkit thread and returns its answer, and is bounded by the same deadline as any other call

#### Scenario: One wedged toolkit world does not disable the others
- **WHEN** a JVM has several toolkit worlds and one of their event queues stops processing
- **THEN** calls concerning elements of that world fail at their deadline while calls concerning the others continue to be answered

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

### Requirement: Operation independent of the target's security policy

The agent SHALL function in a target JVM whose security policy grants the application's own code nothing beyond a sandbox — the normal state of a Java Web Start application, whose `SecurityManager` denies environment reads, property reads, file access and socket accept to code it does not trust. The agent's own classes SHALL therefore be defined by a class loader that is not subject to that policy, so that whether PlatynUI works is not a decision the target gets to make. This grants the agent no reach that injecting it did not already grant: loading an agent into a JVM is a fully privileged act by construction, which is what JEP 451 exists to restrict, and the JNLP runtime itself loads its own privileged code the same way.

A start that fails nevertheless SHALL fail **inside** the agent. No `Throwable` may escape the agent's entry point into the target application, and the agent's diagnostic channel SHALL remain usable under a policy that denies it — a failure whose report throws its own failure leaves the operator with no cause at all. The client side SHALL NOT report this state at debug level: an agent that was injected but published no handshake is an actionable failure whose cause was printed in the target's log, and the diagnostic SHALL say so.

A failure SHALL NOT be permanent. The agent is idempotent per JVM — a second injection into a JVM that already carries a running agent leaves the one it has — but "already tried" is not "already running": a start that failed SHALL leave the JVM open to another attempt, so that a retry after a transient cause can succeed. Otherwise the client's retry budget is spent on a target that has silently decided never to answer.

(Real-target-only: verified against a JVM started under a restrictive policy; the mock provider has no security manager. That verification is bound to a JDK that still has one — JEP 486 permanently disabled the security manager in JDK 24, where such a JVM cannot be started at all — so this requirement outlives the automated evidence for it, and what remains then is a real Web Start target exercised by hand.)

#### Scenario: A sandboxed application still gets a working agent

- **WHEN** the agent is attached into a JVM running under a security manager that denies its code environment reads, property reads and socket accept
- **THEN** the agent starts, publishes its handshake file, accepts a client connection and answers calls, exactly as in an unrestricted JVM

#### Scenario: A failed start never reaches the application

- **WHEN** the agent's start-up fails for any reason, including one raised while it is reporting an earlier failure
- **THEN** the target application observes no exception or error propagating out of the agent, keeps running, and receives one diagnostic line naming the cause

#### Scenario: An agent that never came up is reported, not swallowed

- **WHEN** an attach succeeds but no handshake appears within the readiness budget
- **THEN** the client reports it as a warning that names the target's own log as where the cause was printed, rather than leaving the JVM to look simply agent-less

#### Scenario: A failed start does not disable the JVM for later attempts

- **WHEN** an agent start fails inside a JVM and the same JVM is injected again afterwards
- **THEN** the second attempt runs the start again rather than returning as a no-op, while a JVM whose agent is already running still keeps that one agent
