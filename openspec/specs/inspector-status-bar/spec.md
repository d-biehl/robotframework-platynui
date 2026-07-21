# inspector-status-bar Specification

## Purpose

The Inspector's bottom status bar as a segmented strip: a transient left segment carrying the background-activity indicator and one-shot messages (search/result statuses, picker events such as a completed pick), and a persistent right segment that always reflects the live picker's state. The split exists so persistent state and transient messages can coexist — neither may overwrite or suppress the other.

## Requirements

### Requirement: The status bar is split into a transient and a persistent segment

The Inspector's bottom status bar SHALL consist of two segments: a left segment for the activity indicator and transient messages (search/result status and one-off events), and a right-aligned segment for persistent state. A transient message SHALL NOT overwrite or hide the persistent segment, and the persistent segment SHALL NOT suppress transient messages.

#### Scenario: Both segments visible at once

- **WHEN** a search completes (producing a transient result message) while the picker is armed (a persistent state)
- **THEN** the status bar SHALL show the result message in the left segment and the picker state in the right segment simultaneously

### Requirement: The persistent segment shows the picker state

The right status-bar segment SHALL reflect the picker's current state at all times: disarmed, armed (including the configured activation combination, e.g. "armed — hold Ctrl+Alt+Shift"), actively picking, or the reason picking is unavailable on this platform. The segment SHALL update when the state changes. The picker state SHALL NOT be rendered as an inline label next to the toolbar toggle.

#### Scenario: Arming the picker updates the segment

- **WHEN** the user arms the Pick Element toggle
- **THEN** the right segment SHALL show the armed state including the configured modifier combination

#### Scenario: Active picking is shown

- **WHEN** the picker is armed and the user holds the activation combination
- **THEN** the right segment SHALL indicate that picking is active
- **NOTE** Verifiable only against a real platform / test compositor where global modifier state is readable, not the mock.

#### Scenario: Unavailability reason is shown

- **WHEN** the Inspector runs on a platform where live picking is unsupported
- **THEN** the right segment SHALL state that picking is unavailable and why
- **NOTE** Verifiable against a backend without cursor/hit-test support (e.g. generic Wayland), not the mock.

#### Scenario: Reconfigured combination is reflected

- **WHEN** the user changes the activation combination in Settings while the picker is armed
- **THEN** the armed-state text SHALL show the newly configured combination

### Requirement: Picker events appear as transient messages

A completed pick SHALL produce a transient message in the left segment identifying the picked element (at minimum its accessible name). Transient picker messages SHALL use the same message slot as search/result statuses.

#### Scenario: Completed pick is announced

- **WHEN** the user picks an element and releases the activation combination
- **THEN** the left segment SHALL show a message identifying the picked element
- **NOTE** End-to-end verifiable via the BareMetal egui acceptance lane reading the Inspector's own AccessKit tree.

### Requirement: Existing status behavior is preserved

The left segment SHALL keep the existing behavior: the activity indicator reflects running background work, result statuses render as today, and error statuses remain visually distinguished from informational ones.

#### Scenario: Error status still distinguished

- **WHEN** a search fails with an error
- **THEN** the left segment SHALL render the error message in the error styling, alongside an unchanged persistent segment
