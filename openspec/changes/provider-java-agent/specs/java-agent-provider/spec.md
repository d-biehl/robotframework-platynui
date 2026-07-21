## ADDED Requirements

### Requirement: Agent-backed Java UI tree
The provider SHALL surface the UI tree of a Java application through an agent running inside its JVM, mapping the agent's element model onto PlatynUI `UiNode`s with normalized roles/namespaces, `native:*` attributes, and the applicable interaction patterns — so a Java app is queried with the same XPath and appears in the same Inspector/picker as any other provider. It SHALL support Swing/AWT and JavaFX at minimum; SWT where the agent exposes it. The provider SHALL be platform-neutral (one implementation for Windows/Linux/macOS). (Real-provider-only: requires a JVM with the agent loaded; runs in an acceptance lane against the fixture.)

#### Scenario: A Swing table cell has full fidelity through the agent
- **WHEN** a `JTable` data cell is inspected via the agent provider
- **THEN** the cell node has its correct name, bounds, and selection state, and a stable identity-based `RuntimeId` (unlike the JAB renderer-alias)

#### Scenario: A JavaFX app is reachable on Linux
- **WHEN** a JavaFX application runs on Linux (where no native accessibility exists) with the agent loaded
- **THEN** its scene graph is surfaced as a PlatynUI tree through the agent provider

### Requirement: Automatic, keyword-free routing with claims priority
Routing to the agent provider SHALL be automatic: it claims a Java window only when an agent is present in that window's JVM (detected via `java-app-classification`), with no explicit attach/connect keyword. The `window_claims` registry SHALL honor a provider priority so that, for the same native window, the agent outranks JAB, which outranks the native provider serving Java (agent > JAB > native-for-SWT/JavaFX). A Java JVM with no agent SHALL continue to be served by its existing provider (JAB on Windows, native elsewhere).

#### Scenario: Agent outranks JAB for the same window
- **WHEN** a Swing window's JVM has the agent loaded and the JAB bridge is also enabled
- **THEN** the window resolves to the agent provider's node (higher fidelity), not the JAB node, and only one representation appears

#### Scenario: No agent falls back to JAB
- **WHEN** a Swing window's JVM has no agent
- **THEN** the window is served by JAB (Windows) exactly as before — the agent provider does not claim it

### Requirement: Injection paths and bounded behavior
The agent SHALL be usable both when loaded at launch (`-javaagent`) and when attached to a running JVM (Attach API / `jattach`), the latter documented as subject to JEP 451 (works with a warning on current JDKs, opt-in via `-XX:+EnableDynamicAgentLoading`, disallowed by default in a future JDK). Automatic attachment SHALL be opt-in via `providers.java-agent.auto_attach` (default off); with it off, the agent is used only for JVMs the operator launched with `-javaagent`. All agent reads SHALL be marshaled onto the toolkit thread under a per-call deadline; an unresponsive agent SHALL degrade like a frozen JVM (bounded, no runtime hang), never blocking other providers.

#### Scenario: Auto-attach is off by default
- **WHEN** the provider is enabled with default config and a Swing JVM without an agent is detected
- **THEN** no attach/instrumentation occurs; the window falls back to JAB, and the classifier's diagnostic names the agent as an available opt-in

#### Scenario: Unresponsive agent does not hang the runtime
- **WHEN** an agent stops responding
- **THEN** agent calls return within the deadline margin as errors, the vm is marked degraded, and a concurrent query via another provider completes normally
