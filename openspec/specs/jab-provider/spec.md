# jab-provider

## Purpose

The `jab-provider` capability exposes Java Swing/AWT UI trees on Windows to PlatynUI through the Java Access Bridge (JAB), strictly out-of-process: it reads the JDK-owned `WindowsAccessBridge-64.dll` client and never loads code into, or mutates the configuration of, target JVMs. It surfaces Java top-level windows and their accessibility subtrees as normal PlatynUI nodes (normalized roles, standard attributes, interaction patterns, and window capabilities), coexisting with the UIA provider so a Java window appears exactly once in the merged desktop tree.
## Requirements
### Requirement: Provider registration and inert absence
The JAB functionality SHALL be provided as a backend of the single Java provider (`java-provider`), not as an independently registered provider. It SHALL be inert — yielding no nodes and failing nothing — when `WindowsAccessBridge-64.dll` cannot be discovered, or `providers.java.enabled` is false, or `providers.java.jab.enabled` is false. Runtime construction MUST NOT fail because of JAB availability. All other JAB behavior (tree exposure, roles, attributes, patterns, handle hygiene, robustness, diagnostics) is unchanged by the backend refactor.

#### Scenario: Runtime without a JDK on the machine
- **WHEN** a runtime is created on Windows with no discoverable JAB client DLL
- **THEN** the runtime comes up normally, the JAB backend contributes no children, and exactly one actionable diagnostic (discovery paths tried) is logged

#### Scenario: Kill switch
- **WHEN** `providers.java.jab.enabled` is set to `false`
- **THEN** the backend performs no DLL loading and contributes no nodes

### Requirement: Java top-level window discovery
The provider SHALL discover Java top-level windows via `EnumWindows` + `isJavaWindow` and expose each as a `control:Window` (dialogs as `Dialog`) under the desktop, with `Technology` reported as "JAB", plus `app:Application` grouping by process id. (All scenarios in this spec are real-provider-only: they require a live JVM with the bridge enabled and run in the Windows acceptance lane against the Swing fixture app.)

#### Scenario: Fixture window appears
- **WHEN** the Swing fixture app runs with the bridge enabled and the desktop children are enumerated
- **THEN** exactly one `control:Window` with the fixture's title exists with `@Technology = "JAB"`, and an `app:Application` node for the fixture's PID groups it

#### Scenario: Late-started JVM appears on a later poll
- **WHEN** the fixture app starts after the runtime already answered a query
- **THEN** a subsequent query finds the fixture window without recreating the runtime

### Requirement: Tree exposure with normalized roles
The provider SHALL expose the full accessibility subtree beneath each Java window lazily, with roles normalized to PascalCase from JAB's `role_en_US` (unknown roles PascalCased generically), and originals preserved under `native:*` (`native:Role`, `native:LocalizedRole`, `native:States`, `native:Description`, `native:IndexInParent`, `native:Interfaces`).

#### Scenario: Stage-1 controls reachable with normalized roles
- **WHEN** the fixture window's subtree is walked
- **THEN** the stage-1 button, text field, menu bar, and label are present with roles `Button`, `Text`, `MenuBar`, `Label` (exact mapping pinned by the spike's role table) and their designated `@Name`s

#### Scenario: Roles map into the PlatynUI vocabulary
- **WHEN** the role-mapping unit test runs over the spike-harvested role list
- **THEN** every known role maps to a name from the PlatynUI role vocabulary and unknown roles fall back to generic PascalCase (mock-lane verifiable — pure mapping test; alignment with the AT-SPI2 mapping is followed where the vocabularies coincide, as guidance rather than a requirement)

### Requirement: Standard attributes and RuntimeId
Nodes SHALL provide `Name`, `Role`, `Bounds` in desktop coordinates, `IsEnabled`/`IsVisible` (and `IsFocused` where applicable) derived from `states_en_US`, and a `RuntimeId` of the form `jab://<vmID>/<hwnd>[/<child-index-path>]` stable for the element's lifetime; nodes in the `app:Application`-grouped view carry an `app/<pid>` scope prefix (`jab://app/<pid>/…`) so RuntimeIds stay unique across the desktop and app views (mirroring the UIA provider's scoping). `control:Id` SHALL never be emitted (JAB has no developer-id source).

#### Scenario: Bounds match reality
- **WHEN** the fixture button's `@Bounds` is compared with a pointer click at the bounds center
- **THEN** the click lands on the button (observable via the fixture's click counter, read back on the same runtime), including on a DPI-scaled monitor
- **NOTE** Top-level `@Bounds` is sourced from the injected `WindowManager` (live `GetWindowRect`), not from JAB, because JAB frame bounds lag out-of-band window moves; descendant bounds are JAB-sourced and DPI-calibrated.

#### Scenario: RuntimeId stable across repeated queries
- **WHEN** the same unchanged control is located twice in separate queries
- **THEN** both results carry the identical RuntimeId

### Requirement: Core interaction patterns
The provider SHALL support: Focusable (`requestFocus`), ActivationTarget (bounds center), TextContent (chunked `getAccessibleTextRange`), TextEditable (capability marker from the text interface and `editable` state — no write action, per the text-input-policy capability), Toggleable (`checked` state), StatefulValue (numeric value/min/max), Selectable/SelectionProvider (`AccessibleSelection`), and Expandable — each advertised only when the underlying JAB interfaces/states genuinely back it.

#### Scenario: TextEditable is a marker without an action
- **WHEN** the fixture's editable stage-1 text field is inspected
- **THEN** it advertises `TextEditable` with `IsReadOnly` = false, and `pattern_by_name(TextEditable)` returns no action instance

#### Scenario: Toggle state reflects reality
- **WHEN** the fixture checkbox is activated once (e.g. a pointer click)
- **THEN** its `ToggleState` changes from Off to On, read back on the same runtime (the provider reads JAB state live per access)

#### Scenario: Honest pattern lists
- **WHEN** a node without an accessible-text interface is inspected
- **THEN** it advertises no TextContent pattern and exposes no `control:Text`

### Requirement: Window capabilities via WindowManager delegation
Top-level Java windows SHALL expose `native:NativeWindowHandle` and implement the window capability patterns (Activatable, Minimizable, Maximizable, Restorable, Closeable, Movable, Resizable) by delegating to the runtime's injected `WindowManager`, not via JAB.

#### Scenario: Activate and move
- **WHEN** the fixture window is activated and then moved to a target position
- **THEN** it becomes the foreground window and its reported `@Bounds` origin matches the target position

#### Scenario: Close
- **WHEN** the Closeable pattern closes the fixture window
- **THEN** the fixture process exits and the window disappears from the tree on the next query

### Requirement: Handle hygiene
Every `AccessibleContext` (and any other `JOBJECT64`) obtained from JAB SHALL be released via `releaseJavaObject` when its owning node/value is dropped; identity checks SHALL use `isSameObject`, never raw handle equality.

#### Scenario: Repeated full walks stay stable
- **WHEN** the fixture window's full subtree is walked ten times in one session
- **THEN** every walk returns the same structure and the target JVM keeps responding normally (leak regression guard)

### Requirement: Robustness against unresponsive JVMs
JAB calls SHALL run on the backend's dedicated pump thread with a per-call deadline (`providers.java.jab.call_timeout_ms`); a timeout SHALL surface as a provider error for the affected node only, and repeated timeouts SHALL mark that `vmID` degraded (skipped until a health probe succeeds). Other providers and the runtime MUST remain responsive throughout. The behavior is unchanged by the backend refactor — only the configuration key moves into the `providers.java.jab.*` namespace.

#### Scenario: Frozen JVM does not freeze the runtime
- **WHEN** the fixture app's event-dispatch thread is suspended and a query touches its tree
- **THEN** the query returns (with errors or without JAB results) within the configured deadline margin, and a concurrent UIA query completes normally

### Requirement: Enablement diagnostics without configuration mutation
When a top-level window's class name starts with `SunAwt` but `isJavaWindow` reports false, the JAB backend SHALL report that window as one it cannot serve, and the Java provider SHALL log one actionable diagnostic per window naming both enablement paths (the `-Djavax.accessibility.assistive_technologies` launch flag and `jabswitch`) for every such window no backend serves. Neither SHALL write `.accessibility.properties`, registry keys, or any other target-side configuration. The observable behavior is unchanged by the backend refactor — only the emitter moves, because whether a window is truly unreachable is a question only the provider that knows all backends can answer.

#### Scenario: Bridge not enabled
- **WHEN** the fixture app runs without the enablement flag and the desktop is queried
- **THEN** the diagnostic is logged exactly once for that window and no file or registry mutation occurs

### Requirement: Single appearance of Java windows
When the Java provider has claimed a Java top-level window through its JAB backend (successful `GetAccessibleContextFromHWND`), the merged desktop tree SHALL show that window exactly once: the UIA provider skips claimed windows (config `providers.windows-uia.honor_window_claims`, default true — keyed by the UIA provider's id per the config convention). With the kill switch off, both representations MAY appear and remain distinguishable via `@Technology`.

#### Scenario: No duplicates in the merged tree
- **WHEN** the fixture app runs with the bridge enabled and claims are honored
- **THEN** exactly one window node with the fixture's title exists under the desktop, and it carries `@Technology = "JAB"`

#### Scenario: Kill switch restores the UIA shell
- **WHEN** `providers.windows-uia.honor_window_claims` is false
- **THEN** the UIA shell window reappears alongside the JAB window (both locatable, distinguishable via `@Technology`)

### Requirement: XPath end-to-end
Standard PlatynUI XPath queries SHALL work against JAB-provided subtrees, including name-based location and process-scoped queries.

#### Scenario: Locate and click by name
- **WHEN** `//control:Window[@Name='PlatynUI Swing TestApp']//control:Button[@Name='<fixture button name>']` is evaluated and the result is clicked via ActivationTarget
- **THEN** exactly one node matches and the fixture's click counter increments (read back on the same runtime)

#### Scenario: Process-scoped query
- **WHEN** `app:Application[@ProcessId=<fixture pid>]//control:Window` is evaluated
- **THEN** the fixture window is the sole match
