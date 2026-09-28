## Purpose

Lets the platform's window manager report a top-level window's state: minimized, maximized or normal, and whether it is kept on top. Accessibility providers that cannot read this from their toolkit use the report to expose `IsMinimized`, `IsMaximized` and `IsTopmost` on every platform.

## ADDED Requirements

### Requirement: The window manager reports a window's state

The platform window manager SHALL report, for a resolved top-level window:
- its visual state, which is exactly one of *normal*, *minimized* or *maximized*;
- whether the window is kept above other windows (*topmost*).

A minimized window SHALL report *minimized*, even if it will come back maximized when activated. A window manager that has no notion of topmost windows SHALL report *not topmost*. A window manager that cannot read window state at all SHALL report the query as an unavailable capability and SHALL NOT guess a state.

#### Scenario: Normal window

- **GIVEN** a top-level window that was never minimized or maximized
- **WHEN** its window state is queried
- **THEN** the visual state SHALL be *normal* and topmost SHALL be false
- **NOTE** real provider only (per backend: Win32 in the Windows lane, X11 and PlatynUI compositor in the Linux lanes); the platform mock window manager records calls but keeps no window state

#### Scenario: Maximized window

- **GIVEN** a top-level window that has been maximized
- **WHEN** its window state is queried
- **THEN** the visual state SHALL be *maximized*
- **NOTE** real provider only

#### Scenario: Minimized window that was maximized

- **GIVEN** a top-level window that has been maximized and then minimized
- **WHEN** its window state is queried
- **THEN** the visual state SHALL be *minimized*
- **NOTE** real provider only

#### Scenario: Window manager without state support

- **GIVEN** a window manager that does not implement the state query
- **WHEN** a window's state is queried through it
- **THEN** the query SHALL fail with a capability-unavailable error naming the window-state capability

#### Scenario: Window that no longer exists

- **GIVEN** a window that was resolved and has since been closed
- **WHEN** its window state is queried
- **THEN** the query SHALL fail with an error instead of reporting a state
- **NOTE** real provider only (the mock window manager has no window lifecycle)

### Requirement: Minimized windows stay resolvable

A top-level window that is currently minimized SHALL still be resolvable by the window manager from its accessibility node. This keeps its state readable and lets activation bring it back.

#### Scenario: Resolving a minimized window on the PlatynUI compositor

- **GIVEN** a top-level window on the PlatynUI Wayland compositor that has been minimized
- **WHEN** `Activate Window` is called on that window's accessibility node
- **THEN** the window SHALL be resolved and brought back, and its `@IsMinimized` SHALL be `False`
- **NOTE** real provider only (Wayland acceptance lane)

### Requirement: Window-manager-backed providers expose window state attributes

Providers that take window operations from the platform window manager rather than from their toolkit (AT-SPI, JAB) SHALL expose `control:IsMinimized`, `control:IsMaximized` and `control:IsTopmost` on every top-level window node that exposes `control:IsActive`. The values SHALL be read live from the window-state report on each access. `IsMinimized` and `IsMaximized` SHALL never both be `True`. Nodes that are not top-level windows SHALL NOT expose these attributes. When the window cannot be resolved or its state cannot be read, each attribute SHALL read `False`, matching how `IsActive` behaves in that situation.

#### Scenario: Maximized window on Linux

- **GIVEN** an application's top-level window on X11 or on the PlatynUI Wayland compositor
- **WHEN** `Maximize Window` is called on it and `@IsMaximized` is then read
- **THEN** `@IsMaximized` SHALL be `True` and `@IsMinimized` SHALL be `False`
- **NOTE** real provider only (AT-SPI in the X11 and Wayland acceptance lanes)

#### Scenario: Minimized window on Linux

- **GIVEN** an application's top-level window on X11 or on the PlatynUI Wayland compositor
- **WHEN** `Minimize Window` is called on it and `@IsMinimized` is then read
- **THEN** `@IsMinimized` SHALL be `True` and `@IsMaximized` SHALL be `False`
- **NOTE** real provider only (AT-SPI in the X11 and Wayland acceptance lanes)

#### Scenario: State is read live

- **GIVEN** a top-level window whose `@IsMaximized` has been read as `False`
- **WHEN** the window is maximized and `@IsMaximized` is read again on the same node
- **THEN** the second read SHALL be `True`
- **NOTE** real provider only

#### Scenario: Non-window element

- **GIVEN** a button inside an application's top-level window
- **WHEN** its attributes are enumerated
- **THEN** they SHALL contain none of `control:IsMinimized`, `control:IsMaximized`, `control:IsTopmost`

#### Scenario: Topmost on the PlatynUI compositor

- **GIVEN** an application's top-level window on the PlatynUI Wayland compositor
- **WHEN** `@IsTopmost` is read
- **THEN** it SHALL be `False`
- **NOTE** real provider only (Wayland acceptance lane)

#### Scenario: Java window on Windows

- **GIVEN** the Swing fixture's top-level window reached through JAB
- **WHEN** the window is maximized and `@IsMaximized` is read, then minimized and `@IsMinimized` is read
- **THEN** both reads SHALL be `True`
- **NOTE** real provider only (Windows acceptance lane with JAB)
