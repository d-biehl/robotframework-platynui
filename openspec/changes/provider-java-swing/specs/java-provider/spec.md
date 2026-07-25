## ADDED Requirements

### Requirement: Agent-backed Java UI tree
The Java provider SHALL surface the UI tree of a Java application through an **agent backend**: an agent running inside the target JVM, whose element model is mapped onto PlatynUI `UiNode`s with normalized roles/namespaces, `native:*` attributes, and the applicable interaction patterns — so a Java app is queried with the same XPath and appears in the same Inspector/picker as any other provider. The agent backend SHALL support Swing/AWT; the backend core, wire protocol, and routing SHALL be toolkit-neutral so further toolkit adapters (JavaFX, SWT) can be added without a protocol or routing break. The backend SHALL be platform-neutral (one implementation for Windows/Linux/macOS). (Real-provider-only: requires a JVM with the agent loaded; runs in an acceptance lane against the fixture.)

#### Scenario: A Swing table cell has full fidelity through the agent
- **WHEN** a `JTable` data cell is inspected via the agent backend
- **THEN** the cell node has its correct name, bounds, and selection state, and a stable identity-based `RuntimeId` (unlike the JAB renderer-alias)

### Requirement: Automatic, keyword-free backend selection
Backend selection SHALL be automatic and internal to the Java provider: a JVM window is served via the agent backend exactly when an agent is present in that window's JVM (detected via `java-app-classification`), with no explicit attach/connect keyword and **no change to the boolean `window_claims` semantics** — the Java provider remains the single Java claimant. A Java JVM with no agent SHALL continue to be served by the JAB backend (Windows) or the platform's native provider (elsewhere). When an agent appears in an already-running JVM, the serving backend SHALL switch on the next enumeration pass without re-claiming.

#### Scenario: Agent backend preferred over JAB for the same window
- **WHEN** a Swing window's JVM has the agent loaded and the JAB bridge is also enabled
- **THEN** the window resolves through the agent backend (higher fidelity), and exactly one representation appears (one Java claim, one tree)

#### Scenario: No agent falls back to the JAB backend
- **WHEN** a Swing window's JVM has no agent
- **THEN** the window is served by the JAB backend (Windows) exactly as before

### Requirement: Injection paths and bounded behavior
The agent SHALL be usable both when loaded at launch (`-javaagent`) and when attached to a running JVM (via the native attach transport), the latter documented as subject to JEP 451 (works with a warning on current JDKs, opt-in via `-XX:+EnableDynamicAgentLoading`, disallowed by default in a future JDK). Automatic attachment SHALL be opt-in via `providers.java.agent.auto_attach` (default off); with it off, the agent is used only for JVMs the operator launched with `-javaagent`. All agent reads SHALL be marshaled onto the toolkit thread under a per-call deadline; an unresponsive agent SHALL degrade like a frozen JVM (bounded, no runtime hang), never blocking other providers.

#### Scenario: Auto-attach is off by default
- **WHEN** the provider is enabled with default config and a Swing JVM without an agent is detected
- **THEN** no attach/instrumentation occurs; the window falls back to JAB, and the classifier's diagnostic names the agent as an available opt-in

#### Scenario: Unresponsive agent does not hang the runtime
- **WHEN** an agent stops responding
- **THEN** agent calls return within the deadline margin as errors, the vm is marked degraded, and a concurrent query via another provider completes normally

### Requirement: Delivery as an opt-in package with exact version pairing
The agent JAR SHALL be delivered in the separate `platynui-provider-java` package and discovered through the `platynui.providers` entry-point group — in-process for the test-run provider, via the co-located environment interpreter for the Inspector/CLI binaries; an explicit `providers.java.agent.jar` config SHALL override discovery. When the package is not installed, Java agent support SHALL be reported unavailable with an actionable diagnostic (no silent absence, no claim). The provider↔agent version pairing SHALL be exact: a mismatch SHALL abort the connection with a diagnostic naming both versions and the remedy (restart the JVM with the current agent).

#### Scenario: Missing package yields an actionable diagnostic
- **WHEN** the provider is enabled but `platynui-provider-java` is not installed
- **THEN** the agent backend stays inert (JAB/native serve Java windows unchanged), and the diagnostic names the `robotframework-platynui[java]` install as the remedy

#### Scenario: Version mismatch aborts the connection
- **WHEN** a provider connects to an agent of a different version (e.g. from another virtual environment)
- **THEN** the connection is aborted with a diagnostic naming both versions and the remedy — no degraded or partial operation

### Requirement: Quiescence without Java involvement
When the agent backend is inactive (the `platynui-provider-java` package absent, or `providers.java.agent.enabled` false), the runtime SHALL perform **no Java-related activity** beyond the passive classification facts: no handshake-directory scanning, no attach, no agent JAR resolution. The test host SHALL never require a JVM, JDK, or `jattach` for any of this — the attach transport is implemented natively and only ever talks to the target application's own JVM.

#### Scenario: Non-Java test session touches nothing Java
- **WHEN** a runtime serves a session with the agent backend inactive and no Java windows present
- **THEN** no handshake directory is scanned, no attach is attempted, and no Java-related file or process access occurs beyond the classifier's passive per-window module check
