## ADDED Requirements

### Requirement: Agent-backed Java UI tree
The Java provider SHALL surface the UI tree of a Java application through an **agent backend**: an agent running inside the target JVM (delivered and injected per the `java-agent` capability), whose element model is mapped onto PlatynUI `UiNode`s with normalized roles/namespaces, `native:*` attributes, and the applicable interaction patterns — so a Java app is queried with the same XPath and appears in the same Inspector/picker as any other provider. The agent backend SHALL support Swing/AWT; the client, mapping layer and backend routing SHALL be toolkit-neutral so further toolkit adapters (JavaFX, SWT) can be added without a protocol or routing break. The backend SHALL be platform-neutral (one implementation for Windows/Linux/macOS). (Real-provider-only: requires a JVM with the agent loaded; runs in an acceptance lane against the fixture.)

#### Scenario: A Swing table cell has full fidelity through the agent
- **WHEN** a `JTable` data cell is inspected via the agent backend
- **THEN** the cell node has its correct name, bounds, and selection state, and a stable identity-based `RuntimeId` (unlike the JAB renderer-alias)

#### Scenario: A running Swing application is served without launch changes
- **WHEN** a Swing application that was started by its own script, with no PlatynUI arguments, is queried while the agent package is installed
- **THEN** its tree is served through the agent backend, without the application having been restarted

### Requirement: Node validity is answered, not assumed
Nodes served by the agent backend SHALL report their validity: a node SHALL be valid only while its element is still live in the target JVM and still attached to a showing window, and SHALL report invalid once the element is gone, detached, its window closed, or the JVM ended. When the agent is unreachable or degraded, validity SHALL be reported as invalid rather than assumed — consumers that hold a node (notably a scoped root in the Robot Framework library, which reuses a resolved element while it reports valid) then re-resolve instead of pinning a dead element.

#### Scenario: A pinned root survives its window closing and reopening
- **WHEN** a scoped root is pinned to an agent-served element and that element's window is closed and reopened
- **THEN** the node reports invalid while gone, and the root is resolved again against the new window instead of staying bound to the dead element

#### Scenario: A vanished JVM does not leave valid nodes
- **WHEN** the target JVM ends while a consumer still holds one of its nodes
- **THEN** the node reports invalid (never valid-by-default), and no call blocks beyond the deadline margin

### Requirement: Automatic, keyword-free backend selection
Backend selection SHALL be automatic and internal to the Java provider: a JVM window is served via the agent backend exactly when an agent is present in that window's JVM (detected via `java-app-classification`), with no explicit attach/connect keyword and **no change to the boolean `window_claims` semantics** — the Java provider remains the single Java claimant. A Java JVM with no agent SHALL continue to be served by the JAB backend (Windows) or the platform's native provider (elsewhere). When an agent appears in an already-running JVM, the serving backend SHALL switch on the next enumeration pass without re-claiming.

#### Scenario: Agent backend preferred over JAB for the same window
- **WHEN** a Swing window's JVM has the agent loaded and the JAB bridge is also enabled
- **THEN** the window resolves through the agent backend (higher fidelity), and exactly one representation appears (one Java claim, one tree)

#### Scenario: No agent falls back to the JAB backend
- **WHEN** a Swing window's JVM has no agent
- **THEN** the window is served by the JAB backend (Windows) exactly as before

#### Scenario: An unresponsive agent does not hang the runtime
- **WHEN** an agent stops responding
- **THEN** agent calls return within the deadline margin as errors, the vm is marked degraded, and a concurrent query via another provider completes normally

### Requirement: Automatic attachment to detected JVMs, on by default
When a Java window's JVM carries no agent, the Java provider SHALL attach one automatically, without any keyword or per-application call — attaching to a running application is the normal path, not an exception. This SHALL be governed by `providers.java.agent.auto_attach`, defaulting to **on**; the deliberate consent is the installation of the agent package, not this flag. Setting it to `false` SHALL limit the agent to JVMs launched with `-javaagent`, leaving agent-less JVMs to the JAB backend or the platform's native provider.

#### Scenario: An already-running application is attached automatically
- **WHEN** a Swing application that was started by its own script is enumerated and its JVM carries no agent
- **THEN** the agent is injected into that JVM and the window is served through the agent backend, with no keyword called and no restart

#### Scenario: Automatic attachment can be switched off
- **WHEN** `providers.java.agent.auto_attach` is `false` and an agent-less Swing JVM is detected
- **THEN** no injection occurs, the window is served by the JAB backend, and the diagnostic names the agent as an available option
