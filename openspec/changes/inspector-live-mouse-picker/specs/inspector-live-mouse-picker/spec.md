## ADDED Requirements

### Requirement: Live picking is armed by an explicit toggle

The Inspector SHALL provide an explicit on/off toggle that arms and disarms live mouse-picking. Holding the activation modifiers SHALL have a picking effect only while the toggle is on (armed). Turning the toggle off SHALL immediately stop picking and leave the current selection unchanged. The Inspector SHALL NOT observe or act on the activation modifiers while the toggle is off, so the feature never watches the keyboard behind the user's back.

#### Scenario: Holding the modifiers does nothing while disarmed

- **WHEN** the picker toggle is off and the user holds the activation modifier combination over another application
- **THEN** the selection SHALL NOT change and no picking SHALL occur

#### Scenario: Arming enables picking

- **WHEN** the user turns the picker toggle on and then holds the activation modifier combination
- **THEN** picking SHALL occur (the element under the cursor is resolved and selected)

#### Scenario: Disarming stops picking

- **WHEN** picking is active and the user turns the toggle off
- **THEN** picking SHALL stop immediately and the last-selected element SHALL remain selected

### Requirement: While armed, holding the configured modifier combination picks the element under the cursor

While the picker is armed and the user holds the configured activation modifier combination (default Ctrl+Alt+Shift), the Inspector SHALL continuously resolve the UI element under the mouse cursor and make it the selected node in the tree, revealing it (expanding ancestors as needed), showing its attributes, and highlighting it on screen. Selection SHALL update as the cursor moves so that the currently-highlighted element always corresponds to the element under the pointer. When the modifier combination is released, picking SHALL stop and the last-picked element SHALL remain selected. Picking is a held-modifier state, evaluated continuously; it is not a one-shot key press.

#### Scenario: Element under the cursor becomes selected while held

- **WHEN** the picker is armed and available and the user holds the configured combination with the mouse over a control in another application
- **THEN** that control SHALL be revealed and selected in the Inspector tree, its attributes shown, and it SHALL be highlighted on screen

#### Scenario: Selection follows the moving cursor

- **WHEN** the user keeps the combination held and moves the mouse from one control to another
- **THEN** the selected/highlighted element SHALL update to the control now under the cursor

#### Scenario: Releasing the modifiers stops picking and keeps the last pick

- **WHEN** the user releases any modifier of the configured combination
- **THEN** the Inspector SHALL stop updating the selection from the cursor, and the element picked last SHALL stay selected

#### Scenario: A resolve in flight when picking stops does not move the selection

- **WHEN** an element resolution or tree reveal is still in progress and the user releases the modifiers (or disarms the toggle) before it completes
- **THEN** the late result SHALL be discarded and SHALL NOT move the selection after picking has stopped

#### Scenario: Pointing at empty space makes no spurious selection

- **WHEN** picking is active and the cursor is over a location where hit-test resolves no element
- **THEN** the current selection SHALL be left unchanged rather than cleared or moved to an arbitrary node

### Requirement: The activation modifier combination is configurable

The activation modifier combination SHALL be configurable by the user, defaulting to Ctrl+Alt+Shift. The Inspector SHALL evaluate the currently-configured combination when deciding whether picking is active, and a configured combination SHALL require all of its modifiers to be held (and only that set) to activate picking. The configured combination SHALL persist across restarts consistent with how the Inspector stores its other settings.

#### Scenario: A changed combination takes effect

- **WHEN** the user configures a different activation combination and then holds it while armed
- **THEN** picking SHALL activate for the newly-configured combination

#### Scenario: The old combination no longer activates after reconfiguration

- **WHEN** the user has changed the combination away from the default and then holds the former (default) combination
- **THEN** picking SHALL NOT activate

#### Scenario: A partial modifier combination does not trigger picking

- **WHEN** the user holds only some of the configured modifiers (e.g. Ctrl+Shift when the combination is Ctrl+Alt+Shift)
- **THEN** picking SHALL NOT activate

### Requirement: Picking reuses the existing reveal, select, and highlight behavior

The picker SHALL drive the Inspector's existing reveal-and-select path (which walks the resolved node's ancestor chain to synchronize the tree) and its existing highlight overlay, rather than introducing a parallel selection or highlight mechanism. Repeatedly resolving the same element while the cursor is stationary SHALL NOT thrash the tree or restart in-flight work.

#### Scenario: A picked element off-screen in the tree is scrolled into view

- **WHEN** the picker resolves an element whose tree row is not currently expanded or visible
- **THEN** the Inspector SHALL expand its ancestors, select its row, and scroll it into view, the same way selecting a search result does

#### Scenario: Re-resolving the same element is idempotent

- **WHEN** consecutive picker ticks resolve the same element (the cursor has not moved to a different element)
- **THEN** the Inspector SHALL keep it selected without repeatedly clearing/reloading its attributes or highlight

### Requirement: The picker never selects its own overlay or window

The picker's own highlight overlay SHALL NOT be resolvable by the hit-test — it MUST be excluded from picking (e.g. rendered click/hit-through, or filtered from the result) so that moving the cursor over the highlighted region continues to resolve the target element beneath, not the overlay. Picking SHALL NOT get "stuck" selecting the highlight. Resolving a point over the Inspector's own window SHALL behave deterministically: it SHALL either resolve the Inspector's own controls or resolve nothing, and SHALL NOT produce flicker or a feedback loop.

#### Scenario: The highlight overlay is not picked

- **WHEN** picking is active and the cursor moves within the currently-highlighted region (which the overlay covers)
- **THEN** hit-test SHALL continue to resolve the target element beneath the overlay, and the overlay itself SHALL never become the selected element
- **NOTE** Verifiable only against a real platform where the overlay is a real on-screen window, not the mock.

#### Scenario: Hovering the Inspector's own window is stable

- **WHEN** picking is active and the cursor moves over the Inspector's own window
- **THEN** the Inspector SHALL not enter a selection feedback loop or flicker; the result SHALL be deterministic (own controls or nothing)

### Requirement: The picker is only active where cursor position and hit-test are both available

The Inspector SHALL treat live picking as available only when the active platform can report the real, live cursor position AND can hit-test a point. Where either capability is unavailable, the picker SHALL be disabled: its toggle SHALL be shown in a disabled (greyed-out) state with an indication of why, and it SHALL NOT be armable. The Inspector SHALL NOT move the selection based on a cursor position that the platform reports as unavailable.

#### Scenario: Picker disabled on a platform without live cursor position

- **WHEN** the Inspector runs on a backend that cannot report the physical cursor position (e.g. a generic Wayland session using the EIS/virtual-input path)
- **THEN** the picker toggle SHALL be greyed out and not armable, and the reason SHALL be discoverable to the user
- **NOTE** Verifiable only against a real generic-Wayland session, not the mock.

#### Scenario: Picker disabled where hit-test is unsupported

- **WHEN** the provider reports hit-test as unsupported (e.g. the macOS AX stub, or any provider returning the "unsupported" signal)
- **THEN** the picker toggle SHALL be greyed out and not armable
- **NOTE** The availability decision (unsupported hit-test → disabled) is unit-testable with a mock provider that reports "unsupported"; the macOS case is the real-world instance.

#### Scenario: An unavailable cursor position never drives selection

- **WHEN** a picker tick occurs and the platform reports the cursor position as unavailable
- **THEN** the Inspector SHALL skip that tick without moving the selection (in particular, it SHALL NOT select whatever is at the screen origin)

### Requirement: Modifier state is read per platform without a global hotkey registration

While armed, the Inspector SHALL determine whether the configured activation combination is held by reading the current modifier state each tick, not by registering a system global hotkey. On X11 the modifier state SHALL be taken from the same pointer query used to read the cursor position; on Windows it SHALL be read via the asynchronous key-state API; in the PlatynUI test compositor it SHALL be reported over the compositor control protocol alongside the pointer position. This reading is internal to the Inspector and SHALL NOT require a new shared core/platform capability trait.

#### Scenario: Modifier state is observed while the Inspector is not focused

- **WHEN** the picker is armed, another application has keyboard focus, and the user holds the configured combination over it
- **THEN** the Inspector SHALL still observe that the combination is held and pick the element under the cursor
- **NOTE** Verifiable only against a real platform (X11 / Windows / test compositor), not the mock.

### Requirement: The picker is discoverable and its state is visible

The Inspector SHALL expose the picker through a visible toggle (e.g. a toolbar switch/button) that communicates the current activation gesture (the configured combination, e.g. "hold Ctrl+Alt+Shift") and reflects the current state: disabled where unsupported, armed/disarmed, and indicating when picking is actively following the cursor. A status hint SHALL communicate what is happening (e.g. "armed — hold Ctrl+Alt+Shift to pick", "picking…", or the reason picking is unavailable).

#### Scenario: Toggle communicates availability and gesture

- **WHEN** the Inspector starts on a platform where picking is supported
- **THEN** the picker toggle SHALL be shown enabled, in the disarmed state, with the configured activation gesture indicated

#### Scenario: Active picking is reflected in the UI

- **WHEN** the picker is armed, the user is holding the combination, and picking is updating the selection
- **THEN** the Inspector SHALL visibly indicate that picking is currently active
