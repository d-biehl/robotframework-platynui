## ADDED Requirements

### Requirement: SWT widget tree via the agent
The provider SHALL surface SWT applications through the agent: the widget tree (`Display`→`Shell`→`Control`, with `TableItem`/`TreeItem`/`MenuItem`/`ToolItem` as virtual children) mapped onto PlatynUI `UiNode`s with normalized roles, enrichment from SWT's `Accessible` API where present, `Widget.setData` string keys surfaced as addressable `native:*` attributes, and identity-stable `RuntimeId`s. All reads SHALL be marshaled onto the SWT UI thread under the agent's per-call deadline. (Real-provider-only: requires the SWT fixture with the agent loaded; runs in an acceptance lane.)

#### Scenario: A setData test id addresses a widget
- **WHEN** an SWT widget carries a `setData` string key (e.g. the SWTBot convention) and is addressed by a locator on that `native:*` attribute
- **THEN** the widget resolves through the agent even though no native accessibility surface exposes that id

#### Scenario: Virtual table items have full fidelity
- **WHEN** a `TableItem` in the SWT fixture is inspected via the agent provider
- **THEN** the item node has its model-backed name, bounds, and selection state and a stable `RuntimeId`

### Requirement: Subtree-scoped claims for native-widget toolkits
For a toolkit whose controls are themselves native windows (SWT), boolean `window_claims` resolution SHALL extend to native descendants: a provider that skips a claimed window SHALL also skip that window's native window subtree, so a shell claimed by the Java provider yields exactly one representation of its whole control tree. JVMs without an agent SHALL keep their current native representation (UIA on Windows, AT-SPI on Linux) unchanged.

#### Scenario: Native provider abstains below an agent-claimed shell
- **WHEN** an SWT shell's JVM has the agent loaded and the native provider enumerates the desktop
- **THEN** the native provider surfaces neither the shell nor any of its child control windows — the shell's tree appears only via the agent provider

#### Scenario: No agent keeps the native representation
- **WHEN** an SWT shell's JVM has no agent
- **THEN** the native provider serves the shell and all its controls exactly as before — the agent provider claims nothing
