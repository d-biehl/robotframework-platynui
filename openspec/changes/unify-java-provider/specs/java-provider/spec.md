## ADDED Requirements

### Requirement: Single Java provider with toolkit backends
The system SHALL register exactly one Java UiTree provider (`provider-java`), which routes each claimed Java top-level window to a toolkit backend; backends serve trees, patterns, and diagnostics under their own `@Technology` value. A window SHALL be claimed exactly when one of the available backends can serve it — backends do not all cover the same windows, so a Java window no backend can serve SHALL be left to the platform's native provider rather than claimed and served empty. Window claims SHALL remain boolean (single-appearance as today): adding or enabling a backend changes which backend serves a window, never the claim semantics. A missing or disabled backend SHALL be inert (no nodes, no failures), and runtime construction MUST NOT fail because of backend availability.

#### Scenario: JAB backend serves the Swing fixture unchanged
- **WHEN** the Swing fixture runs with the bridge enabled and the desktop is enumerated
- **THEN** the fixture window appears exactly once with `@Technology = "JAB"`, with the same tree, roles, patterns, and RuntimeIds as before the refactor (existing acceptance suite passes unchanged)

#### Scenario: A Java window no backend can serve is left alone
- **WHEN** a JVM-backed window that JAB cannot serve is enumerated and no other backend is available for it (an SWT or JavaFX window with no agent)
- **THEN** the Java provider does not claim it, and it is served by the platform's native provider exactly as before

#### Scenario: Umbrella kill switch
- **WHEN** `providers.java.enabled` is `false`
- **THEN** no Java provider is active, no backend loads anything, and Java windows are served by the platform's native provider (UIA shell on Windows)

#### Scenario: Backend kill switch
- **WHEN** `providers.java.jab.enabled` is `false`
- **THEN** the JAB backend performs no DLL loading and contributes no nodes, while the umbrella and other backends are unaffected
