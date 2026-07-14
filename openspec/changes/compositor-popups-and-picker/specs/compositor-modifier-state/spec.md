## ADDED Requirements

### Requirement: The compositor exposes the seat's keyboard modifier state

The PlatynUI Wayland compositor SHALL expose the seat keyboard's current modifier state (ctrl, alt, shift, logo) over its control socket. The reported state SHALL reflect all input paths the compositor accepts — physical input, control-socket injection, EIS, and virtual-input protocols — so an observer sees exactly what applications on that seat see.

#### Scenario: Held modifiers are observable

- **WHEN** Ctrl+Alt+Shift are held on the compositor seat (e.g. injected via the control socket or EIS)
- **THEN** the modifier query SHALL report ctrl, alt, and shift as active, and SHALL report them released once they are released

### Requirement: The Inspector live picker works under the PlatynUI compositor

The Inspector's live mouse picker SHALL be available under the PlatynUI compositor: its modifier observation SHALL use the compositor's modifier state, and its reader selection SHALL prefer the compositor path when the session provides a PlatynUI control socket — never binding to a leaked host X11 `DISPLAY` in that case. Pointer position continues to come from the existing compositor query. On X11 and Windows the picker SHALL keep its current behavior.

#### Scenario: The picker is enabled and picks under the compositor

- **WHEN** the Inspector runs inside a PlatynUI compositor session, the picker is armed, Ctrl+Alt+Shift are held, and the cursor is over a widget of another application
- **THEN** the picker SHALL resolve, select, and reveal that element in the Inspector's tree (the Inspector-picker acceptance test SHALL pass on the compositor lane)

#### Scenario: A leaked host DISPLAY does not misbind the modifier reader

- **WHEN** the compositor session environment still carries the host's X11 `DISPLAY`
- **THEN** the picker SHALL still observe the compositor seat's modifiers (not the host X server's), and the local compositor lane SHALL behave like the isolated CI lane

#### Scenario: X11 and Windows picker behavior is unchanged

- **WHEN** the Inspector runs on X11 or Windows
- **THEN** modifier observation SHALL keep using the existing platform readers and the picker suites SHALL keep passing there
