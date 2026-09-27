## MODIFIED Requirements

### Requirement: Agent-backed Java UI tree
The Java provider SHALL surface the UI tree of a Java application through an **agent backend**: an agent running inside the target JVM (delivered and injected per the `java-agent` capability), whose element model is mapped onto PlatynUI `UiNode`s with normalized roles/namespaces, `native:*` attributes, and the applicable interaction patterns — so a Java app is queried with the same XPath and appears in the same Inspector/picker as any other provider. The agent backend SHALL support Swing/AWT; the client, mapping layer and backend routing SHALL be toolkit-neutral so further toolkit adapters (JavaFX, SWT) can be added without a protocol or routing break. The backend SHALL be platform-neutral (one implementation for Windows/Linux/macOS). (Real-provider-only: requires a JVM with the agent loaded; runs in an acceptance lane against the fixture.)

The tree SHALL cover the JVM's **whole** UI, independently of how the application was launched into it. A JVM's top-level windows are not necessarily reachable from any one thread: AWT partitions them per `AppContext`, and the Java Web Start and applet runtimes give each application one of their own, so a window list read from the agent's own threads is empty for exactly the applications the agent exists to reach. Window enumeration SHALL therefore span every toolkit world in the JVM, and windows belonging to the runtime that launched the application (splash screens, download dialogs, shared owner frames) SHALL be excluded by the same showing/visibility rules that already apply — the launcher's furniture is not part of the application's tree.

#### Scenario: A Swing table cell has full fidelity through the agent
- **WHEN** a `JTable` data cell is inspected via the agent backend
- **THEN** the cell node has its correct name, bounds, and selection state, and a stable identity-based `RuntimeId` (unlike the JAB renderer-alias)

#### Scenario: A running Swing application is served without launch changes
- **WHEN** a Swing application that was started by its own script, with no PlatynUI arguments, is queried while the agent package is installed
- **THEN** its tree is served through the agent backend, without the application having been restarted

#### Scenario: A Web Start application is served like any other
- **WHEN** a Swing application launched through Java Web Start — sandboxed by its runtime and running in its own `AppContext` — is enumerated while the agent package is installed
- **THEN** its window appears under the desktop with the agent backend's `@Technology`, its tree is readable to the same depth and fidelity as the same application launched directly, and it is claimed once

#### Scenario: The launcher's own windows are not part of the application
- **WHEN** a Web Start application is enumerated while its runtime's splash and download windows exist in the same JVM
- **THEN** only the application's own showing windows appear in the tree, and no invisible owner or launcher window is surfaced as a top-level node
