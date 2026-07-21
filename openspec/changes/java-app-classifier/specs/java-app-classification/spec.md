## ADDED Requirements

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
