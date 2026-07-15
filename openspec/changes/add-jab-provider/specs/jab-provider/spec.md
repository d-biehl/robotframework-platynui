## ADDED Requirements

### Requirement: Provider registration and inert absence
The JAB provider SHALL register on Windows builds via the inventory mechanism like other OS providers, and SHALL be inert — yielding no nodes and failing nothing — when `WindowsAccessBridge-64.dll` cannot be discovered or `providers.jab.enabled` is false. Runtime construction MUST NOT fail because of JAB availability.

#### Scenario: Runtime without a JDK on the machine
- **WHEN** a runtime is created on Windows with no discoverable JAB client DLL
- **THEN** the runtime comes up normally, the JAB provider contributes no children, and exactly one actionable diagnostic (discovery paths tried) is logged

#### Scenario: Kill switch
- **WHEN** `providers.jab.enabled` is set to `false`
- **THEN** the provider performs no DLL loading and contributes no nodes

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
Nodes SHALL provide `Name`, `Role`, `Bounds` in desktop coordinates, `IsEnabled`/`IsVisible` (and `IsFocused` where applicable) derived from `states_en_US`, and a `RuntimeId` of the form `jab://<vmID>/<hwnd>[/<child-index-path>]` stable for the element's lifetime. `control:Id` SHALL never be emitted (JAB has no developer-id source).

#### Scenario: Bounds match reality
- **WHEN** the fixture button's `@Bounds` is compared with a pointer click at the bounds center
- **THEN** the click lands on the button (observable via the fixture's click counter), including on a DPI-scaled monitor

#### Scenario: RuntimeId stable across repeated queries
- **WHEN** the same unchanged control is located twice in separate queries
- **THEN** both results carry the identical RuntimeId

### Requirement: Core interaction patterns
The provider SHALL support: Focusable (`requestFocus`), ActivationTarget (bounds center), TextContent (chunked `getAccessibleTextRange`), TextEditable (`setTextContents`, `editable` state), Toggleable (`checked` state), StatefulValue (numeric value/min/max), Selectable/SelectionProvider (`AccessibleSelection`), and Expandable — each advertised only when the underlying JAB interfaces/states genuinely back it.

#### Scenario: Text round-trip
- **WHEN** TextEditable writes "hello" into the fixture's stage-1 text field
- **THEN** `control:Text` of that field reads "hello"

#### Scenario: Toggle state reflects reality
- **WHEN** the fixture checkbox is activated once
- **THEN** its `ToggleState` changes from Off to On

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
JAB calls SHALL run on the provider's dedicated pump thread with a per-call deadline (`providers.jab.call_timeout_ms`); a timeout SHALL surface as a provider error for the affected node only, and repeated timeouts SHALL mark that `vmID` degraded (skipped until a health probe succeeds). Other providers and the runtime MUST remain responsive throughout.

#### Scenario: Frozen JVM does not freeze the runtime
- **WHEN** the fixture app's event-dispatch thread is suspended and a query touches its tree
- **THEN** the query returns (with errors or without JAB results) within the configured deadline margin, and a concurrent UIA query completes normally

### Requirement: Enablement diagnostics without configuration mutation
When a top-level window's class name starts with `SunAwt` but `isJavaWindow` reports false, the provider SHALL log one actionable diagnostic per window naming both enablement paths (the `-Djavax.accessibility.assistive_technologies` launch flag and `jabswitch`). The provider SHALL NOT write `.accessibility.properties`, registry keys, or any other target-side configuration.

#### Scenario: Bridge not enabled
- **WHEN** the fixture app runs without the enablement flag and the desktop is queried
- **THEN** the diagnostic is logged exactly once for that window and no file or registry mutation occurs

### Requirement: Single appearance of Java windows
When the JAB provider has claimed a Java top-level window (successful `GetAccessibleContextFromHWND`), the merged desktop tree SHALL show that window exactly once: the UIA provider skips claimed windows (config `providers.uia.honor_window_claims`, default true). With the kill switch off, both representations MAY appear and remain distinguishable via `@Technology`.

#### Scenario: No duplicates in the merged tree
- **WHEN** the fixture app runs with the bridge enabled and claims are honored
- **THEN** exactly one window node with the fixture's title exists under the desktop, and it carries `@Technology = "JAB"`

#### Scenario: Kill switch restores the UIA shell
- **WHEN** `providers.uia.honor_window_claims` is false
- **THEN** the UIA shell window reappears alongside the JAB window (both locatable, distinguishable via `@Technology`)

### Requirement: XPath end-to-end
Standard PlatynUI XPath queries SHALL work against JAB-provided subtrees, including name-based location and process-scoped queries.

#### Scenario: Locate and click by name
- **WHEN** `//control:Window[@Name='PlatynUI Swing TestApp']//control:Button[@Name='<fixture button name>']` is evaluated and the result is clicked via ActivationTarget
- **THEN** exactly one node matches and the fixture's click counter increments

#### Scenario: Process-scoped query
- **WHEN** `app:Application[@ProcessId=<fixture pid>]//control:Window` is evaluated
- **THEN** the fixture window is the sole match
