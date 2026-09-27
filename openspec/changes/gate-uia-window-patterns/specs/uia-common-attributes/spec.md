# Spec Delta

## ADDED Requirements

### Requirement: Window capability patterns are gated on the window surface

The windows-uia provider SHALL advertise the window capability patterns (Activatable, Minimizable, Maximizable, Restorable, Closeable, Movable, Resizable, Responsive) on an element exactly when UI Automation reports a WindowPattern or a TransformPattern for it. `pattern_by_name` SHALL return an instance of one of these patterns exactly when the element advertises it. An element without that window surface SHALL NOT yield an instance of any window capability pattern, whether or not it has a native window handle of its own. The window of an inner element SHALL therefore be the nearest ancestor that serves Activatable, never the element itself.

#### Scenario: A top-level window serves the window patterns it advertises

- **GIVEN** a top-level window with three standard buttons, shown off screen and without activation by a child process of the test
- **WHEN** the window's node is read
- **THEN** its `SupportedPatterns` SHALL contain all eight window capability patterns, and `pattern_by_name` SHALL return an instance for each of them
- **NOTE:** Real provider only: a UI Automation unit test in `just test` on a Windows host; nothing is activated. The window is of the predefined `STATIC` class, so its role is not `Window`, and the test does not select it by role.

#### Scenario: A button with a window handle of its own serves no window pattern

- **GIVEN** one of the test window's buttons, a child window with a native window handle of its own
- **WHEN** `pattern_by_name` is called for each of the eight window capability patterns
- **THEN** none SHALL return an instance, and none of them SHALL be in its `SupportedPatterns`
- **NOTE:** Real provider only. Fails before this change, when every element yields every instance.

#### Scenario: No element below the window serves a window pattern

- **GIVEN** every node listed below the test window, elements without a native window handle included, such as the title bar
- **WHEN** the eight window capability patterns are requested from each
- **THEN** no instance SHALL be returned
- **NOTE:** Real provider only. The test iterates the listed nodes instead of naming the window's non-client elements, whose set and names vary between Windows versions and languages.

#### Scenario: An inner element's window is its window, not itself

- **GIVEN** a button listed from the test window, which keeps its parent alive
- **WHEN** the nearest ancestor-or-self that yields an Activatable instance is looked up, as the runtime does to bring an element to the front, and when `top_level_or_self` is called on the button
- **THEN** both SHALL be the test window, and the button itself SHALL yield no Activatable instance
- **NOTE:** Real provider only. The ancestor lookup fails before this change, when it returns the button.
