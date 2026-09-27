# Spec Delta

## MODIFIED Requirements

### Requirement: Provider registration and inert absence
The JAB functionality SHALL be provided as a backend of the single Java provider (`java-provider`), not as an independently registered provider. It SHALL be inert — yielding no nodes and failing nothing — when `WindowsAccessBridge-64.dll` cannot be discovered, or `providers.java.enabled` is false, or `providers.java.jab.enabled` is false. Runtime construction MUST NOT fail because of JAB availability. A missing DLL SHALL be recorded at debug level with the discovery paths tried; it becomes a warning only when a Swing or AWT window is found that no Java backend serves (java-app-classification, *Cross-platform enablement diagnostic*). All other JAB behavior (tree exposure, roles, attributes, patterns, handle hygiene, robustness, diagnostics) is unchanged by the backend refactor.

#### Scenario: Runtime without a JDK on the machine
- **WHEN** a runtime is created on Windows with no discoverable JAB client DLL
- **THEN** the runtime comes up normally, the JAB backend contributes no children, the discovery paths tried are recorded at debug level, and no warning is logged until a Swing or AWT window is found that no Java backend serves

#### Scenario: Kill switch
- **WHEN** `providers.java.jab.enabled` is set to `false`
- **THEN** the backend performs no DLL loading and contributes no nodes
