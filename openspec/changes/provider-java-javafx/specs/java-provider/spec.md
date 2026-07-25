## ADDED Requirements

### Requirement: JavaFX scene-graph tree via the agent
The provider SHALL surface JavaFX applications through the agent: the scene graph (`Stage`→`Scene`→`Node`, including popups) mapped onto PlatynUI `UiNode`s with normalized roles, FX accessibility enrichment (`queryAccessibleAttribute`), the FX automation id (`Node.getId()`) as an addressable attribute, and identity-stable `RuntimeId`s. All reads SHALL be marshaled onto the FX Application Thread under the agent's per-call deadline. On Windows, the Java provider SHALL claim an FX window it serves via the agent backend so the UIA provider skips it (boolean `window_claims`, as for JAB-served windows); agent-less FX apps remain served by UIA. (Real-provider-only: requires the FX fixture with the agent loaded; runs in an acceptance lane.)

#### Scenario: A JavaFX app is reachable on Linux
- **WHEN** a JavaFX application runs on Linux (where no native accessibility exists) with the agent loaded
- **THEN** its scene graph is surfaced as a PlatynUI tree — nodes have correct roles, names, bounds, and stable `RuntimeId`s, and locators/actions work as for any other provider

#### Scenario: Java provider claims an FX window on Windows
- **WHEN** an FX window's JVM has the agent loaded and FX's native UIA tree is also visible
- **THEN** the window resolves through the agent backend and only one representation appears (UIA skips the claimed window); without an agent, UIA serves it exactly as before

#### Scenario: Virtualized table cells resolve through the model
- **WHEN** a `TableView` cell that is scrolled out of the virtualized viewport is addressed by locator
- **THEN** the agent resolves it from the table model with correct name and selection state
