# java-provider Specification

## Purpose

The `java-provider` capability is the single place Java applications enter PlatynUI's tree. Java is reached through more than one channel — the Java Access Bridge for Swing/AWT on Windows, an in-JVM agent for the toolkits and platforms native accessibility does not cover — and those channels do not reach the same windows. Exactly one provider is registered for all of them, and a channel is a **backend** of it: the provider routes each Java top-level window to the backend that can serve it, and backend nodes reach the runtime unwrapped, so `@Technology`, patterns and node validity stay the backend's own answers.

The single-claimant shape is the point. Because there is only ever one Java claimant, window claims stay a boolean question ("is someone else representing this window?") instead of becoming rank-based ownership between competing Java providers: gaining a backend changes which backend *serves* a window, never who *claims* it, so no registry protocol and no generic consumer is affected. A Java window that no backend reaches is deliberately left unclaimed and served by the platform's native provider rather than claimed and shown empty — and the provider says why, through the shared enablement diagnostic, because only it knows whether some other backend got there.
## Requirements
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

### Requirement: Agent-backed Java UI tree
The Java provider SHALL surface the UI tree of a Java application through an **agent backend**: an agent running inside the target JVM (delivered and injected per the `java-agent` capability), whose element model is mapped onto PlatynUI `UiNode`s with normalized roles/namespaces, `native:*` attributes, and the applicable interaction patterns — so a Java app is queried with the same XPath and appears in the same Inspector/picker as any other provider. The agent backend SHALL support Swing/AWT; the client, mapping layer and backend routing SHALL be toolkit-neutral so further toolkit adapters (JavaFX, SWT) can be added without a protocol or routing break. The backend SHALL be platform-neutral (one implementation for Windows/Linux/macOS). (Real-provider-only: requires a JVM with the agent loaded; runs in an acceptance lane against the fixture.)

#### Scenario: A Swing table cell has full fidelity through the agent
- **WHEN** a `JTable` data cell is inspected via the agent backend
- **THEN** the cell node has its correct name, bounds, and selection state, and a stable identity-based `RuntimeId` (unlike the JAB renderer-alias)

#### Scenario: A running Swing application is served without launch changes
- **WHEN** a Swing application that was started by its own script, with no PlatynUI arguments, is queried while the agent package is installed
- **THEN** its tree is served through the agent backend, without the application having been restarted

### Requirement: Node validity is answered, not assumed
Nodes served by the agent backend SHALL report their validity: a node SHALL be valid only while its element is still live in the target JVM and still attached to a showing window, and SHALL report invalid once the element is gone, detached, its window closed, or the JVM ended. When the agent is unreachable or degraded, validity SHALL be reported as invalid rather than assumed — consumers that hold a node (notably a scoped root in the Robot Framework library, which reuses a resolved element while it reports valid) then re-resolve instead of pinning a dead element.

#### Scenario: A pinned root survives its window closing and reopening
- **WHEN** a scoped root is pinned to an agent-served element and that element's window is closed and reopened
- **THEN** the node reports invalid while gone, and the root is resolved again against the new window instead of staying bound to the dead element

#### Scenario: A vanished JVM does not leave valid nodes
- **WHEN** the target JVM ends while a consumer still holds one of its nodes
- **THEN** the node reports invalid (never valid-by-default), and no call blocks beyond the deadline margin

### Requirement: Automatic, keyword-free backend selection
Backend selection SHALL be automatic and internal to the Java provider: a JVM window is served via the agent backend exactly when an agent is present in that window's JVM — detected from the **agent's own handshake rendezvous**, not from a platform Java classifier, so the criterion needs nothing that only some platforms have — with no explicit attach/connect keyword and **no change to the boolean `window_claims` semantics** — the Java provider remains the single Java claimant. A Java JVM with no agent SHALL continue to be served by the JAB backend (Windows) or the platform's native provider (elsewhere). When an agent appears in an already-running JVM, the serving backend SHALL switch without re-claiming: on the *same* pass when that pass is what injected it, and on the next one otherwise.

#### Scenario: Agent backend preferred over JAB for the same window
- **WHEN** a Swing window's JVM has the agent loaded and the JAB bridge is also enabled
- **THEN** the window resolves through the agent backend (higher fidelity), and exactly one representation appears (one Java claim, one tree)

#### Scenario: No agent falls back to the JAB backend
- **WHEN** a Swing window's JVM has no agent
- **THEN** the window is served by the JAB backend (Windows) exactly as before

#### Scenario: An unresponsive agent does not hang the runtime
- **WHEN** an agent stops responding
- **THEN** agent calls return within the deadline margin as errors, the vm is marked degraded, and a concurrent query via another provider completes normally

#### Scenario: An agent that answers again is usable again
- **WHEN** a JVM that stopped answering resumes
- **THEN** the provider serves its nodes again without the runtime being restarted — a call abandoned at its deadline SHALL NOT leave the connection permanently unusable

### Requirement: Automatic attachment to detected JVMs, on by default
When a Java window's JVM carries no agent, the Java provider SHALL attach one automatically, without any keyword or per-application call — attaching to a running application is the normal path, not an exception. This SHALL be governed by `providers.java.agent.auto_attach`, defaulting to **on**; the deliberate consent is the installation of the agent package, not this flag. Setting it to `false` SHALL limit the agent to JVMs launched with `-javaagent`, leaving agent-less JVMs to the JAB backend or the platform's native provider.

#### Scenario: An already-running application is attached automatically
- **WHEN** a Swing application that was started by its own script is enumerated and its JVM carries no agent
- **THEN** the agent is injected into that JVM and the window is served through the agent backend **in that same enumeration**, with no keyword called and no restart — a consumer never sees the weaker backend for a window that is about to be taken over

#### Scenario: Automatic attachment can be switched off
- **WHEN** `providers.java.agent.auto_attach` is `false` and an agent-less Swing JVM is detected
- **THEN** no injection occurs, the window is served by the JAB backend, and the diagnostic names the agent as an available option

### Requirement: Tabular content served by the agent is structured by row
A table surfaced through the agent backend SHALL place its cells beneath **row** nodes rather than directly beneath the table, so a row is addressable in its own right. Each row SHALL carry its own identity, its own selection state, and — when it is in view — its own on-screen rectangle; each cell SHALL remain reachable and keep the coordinates it reports today, so a cell's position stays knowable both structurally and by attribute.

A table is routinely larger than the viewport showing it, and the toolkit answers geometry questions from the model regardless of what is scrolled into view. Rows and cells that are **not** on screen SHALL therefore report no rectangle at all and SHALL NOT claim to be in view, rather than publishing the position they would occupy — a rectangle outside the window would aim pointer input at whatever is there instead.

The row level SHALL come from the toolkit's own model rather than from the accessibility view — which for Swing offers only a flat list of cells, and is the reason the flat shape existed at all. This aligns the Java tree with what the other providers already surface for tabular content, and the alignment is the point: a table should not have a different shape merely because of which technology reads it.

#### Scenario: A table's children are its rows
- **WHEN** a Swing table with four rows and three columns is enumerated through the agent backend
- **THEN** the table node has four children, each a row, and each row has three cells — not twelve cells directly under the table

#### Scenario: A cell still knows where it is
- **WHEN** a cell inside a row is inspected
- **THEN** it reports the same row and column coordinates as before the row level existed, and its name, bounds and selection state are unchanged

#### Scenario: A row is addressable and locatable
- **WHEN** a row that is in view is inspected
- **THEN** it has an on-screen rectangle spanning its cells, a selection state that reflects whether the row is selected, and an identity that stays the same across repeated enumerations of the unchanged table

#### Scenario: Content scrolled out of view is present but has no place on screen
- **WHEN** a row far below the visible part of a scrolling table, and one of its cells, are inspected
- **THEN** both are in the tree with their names, coordinates and identity intact
- **AND** neither reports a rectangle or anything to aim pointer input at, and both report that they are not in view

#### Scenario: Hit-testing passes through the row
- **WHEN** a point inside a cell is hit-tested through the agent backend
- **THEN** the returned chain reaches the cell by way of its row, so a consumer revealing the result can place it in the tree

#### Scenario: Selected cells are still named correctly
- **WHEN** a table reports its selection while its cells sit beneath rows
- **THEN** the identifiers it publishes for the selected items resolve to nodes that exist in the tree, or are omitted — never identifiers assembled from a position that no longer addresses what it used to
