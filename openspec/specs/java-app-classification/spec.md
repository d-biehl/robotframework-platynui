# java-app-classification

## Purpose

The `java-app-classification` capability provides provider-independent detection of JVM-backed top-level windows: is the window backed by a JVM, which UI toolkit renders it (Swing/AWT, SWT, JavaFX), and is it reachable through the platform's native accessibility stack at all. The classification is a platform-bundle capability (`platynui_core::platform::java`, Windows backend in `platform-windows`), surfaced as observable facts (`native:IsJvm`, `native:JvmToolkit`, `native:JvmAccessibilityReachable` attributes) and an actionable "JVM window absent from native accessibility" diagnostic — so the Inspector, selectors, and logs can distinguish an accessible Java app from one whose accessibility is not enabled. Detection only: no window-ownership change, no injection, no target-side mutation. The cross-platform signal table lives in `dev-docs/java-toolkits.md`.
## Requirements
### Requirement: Provider-independent JVM window classification
The system SHALL provide a platform-bundle capability that classifies a top-level window as JVM-backed or not, using a signal independent of any accessibility bridge (on Windows: the `jvm.dll` module loaded in the owning process). When the platform allows, it SHALL also report the UI toolkit (Swing/AWT, SWT, JavaFX) and whether the window is reachable through the native accessibility stack. Where a platform backend is absent or a signal is unavailable, the corresponding field SHALL be reported as unknown rather than guessed, and callers SHALL degrade gracefully.

#### Scenario: A Swing window is classified as a JVM app
- **WHEN** the classifier inspects the fixture's Swing top-level window on Windows
- **THEN** it reports is-JVM true and toolkit Swing/AWT (via the `SunAwt*` window class and the `jvm.dll` module), independent of whether the Access Bridge is enabled

#### Scenario: A non-JVM window is not misclassified
- **WHEN** the classifier inspects a native (non-Java) top-level window
- **THEN** it reports is-JVM false and toolkit unknown, and issues no Java diagnostic

#### Scenario: Toolkit is unknown where the platform cannot tell (mock-lane)
- **WHEN** the classification logic is given signals with no reliable toolkit discriminator
- **THEN** the toolkit field is unknown (not a guess), while is-JVM still reflects the JVM signal

### Requirement: Observable classification facts
The classification SHALL be surfaced as `native:*` attributes on the node the owning provider already emits for the application/window, so the Inspector, XPath selectors, and logs can distinguish an accessible Java app from one whose accessibility is not reachable. Emitting these attributes SHALL NOT change which provider owns the window.

#### Scenario: Inspector can see a Swing app with accessibility disabled
- **WHEN** a Swing app is running on Windows without the Access Bridge enabled and its process/window is observed
- **THEN** the classification facts report a JVM Swing app that is not reachable through native accessibility, visible as native attributes (not only a log line)

### Requirement: Cross-platform enablement diagnostic
The Windows-only `SunAwt`-suspect warning SHALL be generalized into a single "JVM window absent from native accessibility" diagnostic, emitted at most once per window, naming the actionable enablement path for the detected toolkit/platform (and, once available, the agent provider). The JAB provider SHALL emit this shared diagnostic instead of its own.

#### Scenario: Bridge-less Swing window yields the actionable diagnostic once
- **WHEN** a JVM-backed Swing window is detected on Windows but is not reachable through native accessibility
- **THEN** the shared diagnostic fires once for that window, naming how to enable accessibility (the launch flag / `jabswitch`), and does not repeat on subsequent enumeration passes

### Requirement: Agent-presence signal
The classification SHALL report whether a PlatynUI agent is present in a JVM window's process, derived from the agent's per-user handshake file (existing for that PID and answering the connection handshake). A handshake file whose PID no longer runs SHALL be treated as stale (not present, eligible for cleanup). The signal feeds the Java provider's backend selection and SHALL be observable like the other classification facts; consuming it SHALL NOT itself trigger any attach or instrumentation.

#### Scenario: Agent-backed JVM is flagged
- **WHEN** a Swing JVM launched with `-javaagent` is classified
- **THEN** the classification reports agent-present for that window's process, observable like the other classification facts

#### Scenario: Stale handshake file is not reported as present
- **WHEN** a handshake file exists for a PID that is no longer running
- **THEN** the classification reports no agent for that PID and the stale file is ignored (eligible for cleanup), so no provider attempts a connection
