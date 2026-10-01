# Spec Delta

## MODIFIED Requirements

### Requirement: Single Java provider with toolkit backends

The system SHALL register exactly one Java UiTree provider (`provider-java`), which routes each claimed Java top-level window to a toolkit backend. Backends serve trees, patterns, and diagnostics under their own `@Technology` value.

- **When a window is claimed.** A window SHALL be claimed exactly when one of the available backends can serve it. Backends do not all cover the same windows, so a Java window no backend can serve SHALL be left to the platform's native provider rather than claimed and served empty.
- **A held window.** A window that a backend has served, and now holds because it cannot ask about it (its JVM does not answer, or the backend's channel is held by another JVM), counts as one that backend can serve (`jab-provider`, *A Java window that does not answer does not hide the other Java windows*). It SHALL stay claimed, SHALL yield no node, and SHALL NOT be reported as unreachable while it is held.
- **Claims are boolean.** Window claims SHALL be boolean, so a window appears exactly once in the tree: adding or enabling a backend changes which backend serves a window, never the claim semantics.
- **A missing or disabled backend** SHALL be inert (no nodes, no failures), and runtime construction MUST NOT fail because of backend availability.

#### Scenario: The JAB backend serves the Swing fixture
- **WHEN** the Swing fixture runs with the bridge enabled and the desktop is enumerated
- **THEN** the fixture window appears exactly once with `@Technology = "JAB"`, and the JAB acceptance suites pass against it

#### Scenario: A Java window no backend can serve is left alone
- **WHEN** a JVM-backed window that JAB cannot serve is enumerated and no other backend is available for it (an SWT or JavaFX window with no agent)
- **THEN** the Java provider does not claim it, and it is served by the platform's native provider

#### Scenario: Umbrella kill switch
- **WHEN** `providers.java.enabled` is `false`
- **THEN** no Java provider is active, no backend loads anything, and Java windows are served by the platform's native provider (UIA shell on Windows)

#### Scenario: Backend kill switch
- **WHEN** `providers.java.jab.enabled` is `false`
- **THEN** the JAB backend performs no DLL loading and contributes no nodes, while the umbrella and other backends are unaffected

#### Scenario: A held window stays claimed without a node

- **GIVEN** a backend that reports a window it served before as held
- **WHEN** the Java provider enumerates the desktop
- **THEN** the window stays claimed, the provider yields no node for it, reports no enablement diagnostic for it, and offers its process to the in-JVM agent
- **NOTE:** Verifiable at unit level with the Java provider's stub backend.
