## ADDED Requirements

### Requirement: Pointer Scroll turns the mouse wheel by direction and ticks

The library SHALL provide a `Pointer Scroll` keyword that emits mouse-wheel scroll events in a given direction by a given number of notches. The direction SHALL be one of `UP`, `DOWN`, `LEFT`, `RIGHT` (default `DOWN`). The amount SHALL be given as `ticks` (default 1), where one tick is one mouse-wheel notch (120 units). The keyword SHALL own the sign and axis convention so the requested direction is the visible scroll direction, and SHALL NOT require the caller to provide signed deltas.

#### Scenario: Scroll down by a number of notches

- **WHEN** `Pointer Scroll` is called with direction `DOWN` and `ticks` N
- **THEN** it SHALL emit a downward wheel scroll equivalent to N notches

#### Scenario: Default direction and amount

- **WHEN** `Pointer Scroll` is called without a direction or ticks
- **THEN** it SHALL scroll one notch downward

#### Scenario: Each direction scrolls the corresponding axis

- **WHEN** `Pointer Scroll` is called with `UP`, `DOWN`, `LEFT` or `RIGHT`
- **THEN** it SHALL scroll vertically for `UP`/`DOWN` and horizontally for `LEFT`/`RIGHT`, in the visually corresponding direction

### Requirement: Pointer Scroll targets like the other pointer keywords

`Pointer Scroll` SHALL accept a target the same way as the other pointer keywords: an optional `descriptor` as the first argument (a selector or a captured element), optional keyword-only `x`/`y`, and `activate`, `overrides` and `query_overrides`. When a target (descriptor or coordinates) is given, the pointer SHALL be moved over it before scrolling, so the wheel acts on the widget under the cursor. With no target it SHALL scroll at the current pointer position. Per-call waiting SHALL be configurable only via `query_overrides`, and window activation SHALL follow `activate` / the library default, consistent with the other pointer keywords.

#### Scenario: Scroll over an element

- **WHEN** `Pointer Scroll` is called with an element selector
- **THEN** the pointer SHALL be moved over that element and then scrolled

#### Scenario: Scroll at the current position

- **WHEN** `Pointer Scroll` is called with no target (`descriptor` `${None}` and no coordinates)
- **THEN** it SHALL scroll at the current pointer position without first moving

#### Scenario: Missing target fails like the other pointer keywords

- **WHEN** `Pointer Scroll` is given a selector that never resolves within the timeout
- **THEN** it SHALL raise the same not-found error as the other pointer keywords, honoring the query settings and a per-call `query_overrides`

### Requirement: Scrolling produces a real, reversible effect

`Pointer Scroll` SHALL drive the platform's real scroll input so that a scrollable container actually scrolls. Scrolling a container by several notches SHALL change its scroll offset in the scrolled direction, and scrolling the same amount in the opposite direction SHALL return the offset to approximately its starting value.

#### Scenario: Scrolling a container moves and restores its offset

- **WHEN** `Pointer Scroll` scrolls a scrollable container `DOWN` by several notches and then `UP` by the same amount
- **THEN** the container's scroll offset SHALL increase after the downward scroll and return to approximately its starting value after the upward scroll
