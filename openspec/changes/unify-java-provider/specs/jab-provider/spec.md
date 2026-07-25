## MODIFIED Requirements

### Requirement: Provider registration and inert absence
The JAB functionality SHALL be provided as a backend of the single Java provider (`java-provider`), not as an independently registered provider. It SHALL be inert — yielding no nodes and failing nothing — when `WindowsAccessBridge-64.dll` cannot be discovered, or `providers.java.enabled` is false, or `providers.java.jab.enabled` is false. Runtime construction MUST NOT fail because of JAB availability. All other JAB behavior (tree exposure, roles, attributes, patterns, handle hygiene, robustness, diagnostics) is unchanged by the backend refactor.

#### Scenario: Runtime without a JDK on the machine
- **WHEN** a runtime is created on Windows with no discoverable JAB client DLL
- **THEN** the runtime comes up normally, the JAB backend contributes no children, and exactly one actionable diagnostic (discovery paths tried) is logged

#### Scenario: Kill switch
- **WHEN** `providers.java.jab.enabled` is set to `false`
- **THEN** the backend performs no DLL loading and contributes no nodes

### Requirement: Robustness against unresponsive JVMs
JAB calls SHALL run on the backend's dedicated pump thread with a per-call deadline (`providers.java.jab.call_timeout_ms`); a timeout SHALL surface as a provider error for the affected node only, and repeated timeouts SHALL mark that `vmID` degraded (skipped until a health probe succeeds). Other providers and the runtime MUST remain responsive throughout. The behavior is unchanged by the backend refactor — only the configuration key moves into the `providers.java.jab.*` namespace.

#### Scenario: Frozen JVM does not freeze the runtime
- **WHEN** the fixture app's event-dispatch thread is suspended and a query touches its tree
- **THEN** the query returns (with errors or without JAB results) within the configured deadline margin, and a concurrent UIA query completes normally

### Requirement: Single appearance of Java windows
When the Java provider has claimed a Java top-level window through its JAB backend (successful `GetAccessibleContextFromHWND`), the merged desktop tree SHALL show that window exactly once: the UIA provider skips claimed windows (config `providers.windows-uia.honor_window_claims`, default true — keyed by the UIA provider's id per the config convention). With the kill switch off, both representations MAY appear and remain distinguishable via `@Technology`.

#### Scenario: No duplicates in the merged tree
- **WHEN** the fixture app runs with the bridge enabled and claims are honored
- **THEN** exactly one window node with the fixture's title exists under the desktop, and it carries `@Technology = "JAB"`

#### Scenario: Kill switch restores the UIA shell
- **WHEN** `providers.windows-uia.honor_window_claims` is false
- **THEN** the UIA shell window reappears alongside the JAB window (both locatable, distinguishable via `@Technology`)
