## Purpose

Defines what activating a window does to its minimized and maximized state. This applies whether the window is activated directly, brought to the front for one of its elements, or activated implicitly before a pointer or keyboard action. Activation brings a window forward and never resizes it as a side effect.

## ADDED Requirements

### Requirement: Activation brings a minimized window back to its previous state

Activating a minimized window SHALL show it again in the state it had before it was minimized: a window minimized from the normal state SHALL come back normal, and a window minimized from the maximized state SHALL come back maximized. After activation the window SHALL be the active window.

#### Scenario: A window minimized from the normal state comes back normal

- **GIVEN** a top-level window that is neither maximized nor minimized
- **AND** the window has been minimized
- **WHEN** `Activate Window` is called on it
- **THEN** its `@IsMinimized` SHALL be `False`, its `@IsMaximized` SHALL be `False`, and its `@IsActive` SHALL be `True`

#### Scenario: A window minimized from the maximized state comes back maximized

- **GIVEN** a top-level window that has been maximized and then minimized
- **WHEN** `Activate Window` is called on it
- **THEN** its `@IsMinimized` SHALL be `False`, its `@IsMaximized` SHALL be `True`, and its `@IsActive` SHALL be `True`
- **NOTE** verifiable against the mock for the contract; the per-backend behavior (Win32/UIA, X11 window manager, PlatynUI compositor) SHALL be confirmed in the real acceptance lanes

### Requirement: Activation preserves the maximized state

Activating a window that is not minimized SHALL NOT change whether it is maximized, and SHALL NOT move or resize it.

#### Scenario: Activating a maximized background window keeps it maximized

- **GIVEN** two top-level windows, the first maximized and the second active
- **WHEN** `Activate Window` is called on the first window
- **THEN** the first window's `@IsActive` SHALL be `True`, its `@IsMaximized` SHALL stay `True`, and its `@Bounds` SHALL equal the bounds read before activation

#### Scenario: Activating a normal window keeps it normal

- **GIVEN** a top-level window that is neither maximized nor minimized and not active
- **WHEN** `Activate Window` is called on it
- **THEN** its `@IsMaximized` SHALL stay `False` and its `@Bounds` SHALL equal the bounds read before activation

### Requirement: Bring To Front activates the element's window without restoring it

Bringing an element to the front SHALL activate the top-level window that contains the element, following the activation requirements above. It SHALL NOT return a maximized window to the normal state. When the element has no top-level window that can be activated, the operation SHALL fail and name the element. When activation itself fails, the operation SHALL fail with that error.

#### Scenario: Bring To Front on an element of a maximized window

- **GIVEN** a maximized top-level window that is not active
- **WHEN** `Bring To Front` is called with a button inside that window
- **THEN** the window's `@IsActive` SHALL be `True` and its `@IsMaximized` SHALL stay `True`

#### Scenario: Bring To Front on an element of a minimized window

- **GIVEN** a top-level window that has been minimized from the normal state
- **WHEN** `Bring To Front` is called with an element inside that window
- **THEN** the window's `@IsMinimized` SHALL be `False` and its `@IsActive` SHALL be `True`

#### Scenario: Element without an activatable window

- **GIVEN** an element whose ancestors include no window that can be activated
- **WHEN** `Bring To Front` is called with that element
- **THEN** the keyword SHALL fail with an error reporting the missing activation capability for that element

#### Scenario: Activation of the resolved window fails

- **GIVEN** an element whose top-level window rejects activation (for example because the window closed after it was queried)
- **WHEN** `Bring To Front` is called with that element
- **THEN** the keyword SHALL fail with the activation error, not succeed silently
- **NOTE** exercised at the runtime unit level with a window whose activation reports an error

### Requirement: Implicit activation before an action does not change the window's size

The activation that pointer and keyboard keywords perform before acting (governed by `auto_activate` and the per-call `activate` override) SHALL follow the same activation requirements. A maximized window SHALL stay maximized, so the action lands on the element's current position.

#### Scenario: Pointer Click into a maximized background window

- **GIVEN** `auto_activate` is on, a maximized top-level window is not active, and it contains a button
- **WHEN** `Pointer Click` is called on that button
- **THEN** the window's `@IsActive` SHALL be `True` and its `@IsMaximized` SHALL stay `True`

#### Scenario: Pointer Click on the Restore button of a maximized window

- **GIVEN** `auto_activate` is on and a maximized window with its own Maximize/Restore button
- **WHEN** `Pointer Click` is called on that button
- **THEN** the window SHALL return to its size from before it was maximized, exactly once, with no extra un-maximize from the activation step
- **NOTE** real provider only (the Inspector window controls on the PlatynUI Wayland compositor)

### Requirement: Restoring a window stays an explicit operation

Returning a window to its normal state SHALL remain available through `Restore Window`, independently of activation. It SHALL un-maximize a maximized window and bring back a minimized window. Which state a window minimized while maximized comes back in is left to the platform, since what restoring does is unchanged by this capability.

#### Scenario: Restore Window un-maximizes

- **GIVEN** a maximized top-level window
- **WHEN** `Restore Window` is called on it
- **THEN** its `@IsMaximized` SHALL be `False` and its `@IsMinimized` SHALL be `False`

#### Scenario: Restore Window brings back a minimized window

- **GIVEN** a top-level window that has been minimized from the normal state
- **WHEN** `Restore Window` is called on it
- **THEN** its `@IsMinimized` SHALL be `False`
