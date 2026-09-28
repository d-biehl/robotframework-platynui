# Spec Delta

## ADDED Requirements

### Requirement: The Wayland lane's session provides XWayland

The session that the `real-wayland` lane profile's wrapper establishes SHALL start its compositor with XWayland. An application that speaks only X11 then runs in the Wayland lane as it does on a Wayland desktop. In that session:

- Applications that support Wayland SHALL keep running on Wayland.
- PlatynUI's runtime SHALL keep using its Wayland platform, although the session also offers an X display.
- When XWayland does not become usable, the session SHALL end the lane with a non-zero exit code before any suite runs, as the compositor's startup contract defines (capability `compositor-session-lifecycle`). It SHALL NOT run the suites without X11.

A run started inside an already established compositor session reuses that session as it is ("A run inside an already established session does not nest"). Such a session provides X11 only when it was opened with `scripts/startcompositor.sh --xwayland`.

The X11 lane and the Windows lane SHALL be unaffected.

#### Scenario: An X11-only application runs in the Wayland lane

- **GIVEN** the session the `real-wayland` profile's wrapper establishes
- **WHEN** an application without Wayland support starts in it
- **THEN** it opens its window, and its accessibility tree is reachable below its application node
- **NOTE** Verifiable only on the real Wayland lane.

#### Scenario: Wayland-capable fixtures stay on Wayland

- **GIVEN** the session the `real-wayland` profile's wrapper establishes
- **WHEN** the egui, Qt Widgets and Qt Quick fixtures start in it
- **THEN** each of their windows is served over the Wayland protocol, not through XWayland
- **NOTE** Verifiable only on the real Wayland lane, for example from the display connections each fixture's process holds.

#### Scenario: PlatynUI stays on its Wayland platform

- **GIVEN** the session the `real-wayland` profile's wrapper establishes, with both a Wayland display and an X display
- **WHEN** PlatynUI's runtime starts in it
- **THEN** it detects a Wayland session and uses the PlatynUI compositor's window management, not X11's

#### Scenario: A failing XWayland fails the lane

- **GIVEN** an `Xwayland` on `PATH` that exits before it becomes ready
- **WHEN** a run with the `real-wayland` profile starts
- **THEN** it ends with a non-zero exit code and the compositor's XWayland startup error
- **AND** no suite has run
- **NOTE** Relies on the startup contract of capability `compositor-session-lifecycle`.

#### Scenario: An interactive session opened with XWayland serves a lane run

- **GIVEN** an interactive session opened with `scripts/startcompositor.sh --xwayland`
- **WHEN** a run with the `real-wayland` profile is started inside it
- **THEN** the run uses that session and its X display, and no second session is created
- **NOTE** Verifiable only in a real compositor session.
