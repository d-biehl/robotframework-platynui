# Spec Delta

## Purpose

Defines how a pointer move turns into motion events: the path ends with a paced approach, so the
toolkit under the pointer sees separate motion over the target before a click, even when it merges
the input it reads per frame. Also defines the parameters that control the approach on every
surface.

## ADDED Requirements

### Requirement: A pointer move along a path ends with a paced approach

When a pointer move follows a path (every motion mode except `Direct`), its last `approach_steps`
motion events SHALL each be delivered no sooner than `approach_interval` after the motion event
before them, including the first of them. Each SHALL be at a different integer pixel position, and
the last SHALL be exactly at the target. The approach SHALL apply to every action that moves the
pointer along a path: move, click, multi-click, scroll with a target, and drag.

#### Scenario: Default move ends with three paced steps

- **GIVEN** the default pointer profile
- **WHEN** the pointer moves 50 px in the `Linear` mode
- **THEN** the last 3 motion events SHALL each come at least 20 ms after the one before them
- **AND** they SHALL be at 3 different integer pixel positions, the last one exactly at the target

Verified with the runtime's recording pointer device and recorded sleeps (mock-verifiable).

#### Scenario: Click target is approached before the press

- **GIVEN** the default pointer profile
- **WHEN** a click targets a point 50 px away
- **THEN** the paced approach SHALL finish before the button press, and the press SHALL happen at
  the target

Verified with the runtime's recording pointer device (mock-verifiable).

#### Scenario: Every path mode ends with the approach

- **GIVEN** the default pointer profile with the motion mode set to `Bezier`, `Overshoot` or `Jitter`
- **WHEN** the pointer moves 50 px
- **THEN** the move SHALL end with the same paced approach as in the `Linear` mode, the last motion
  event exactly at the target

Verified with the runtime's recording pointer device (mock-verifiable).

### Requirement: The approach is skipped when there is nothing to approach

A move in the `Direct` motion mode SHALL stay one motion event straight to the target, with no
approach. A move of zero distance SHALL produce no paced approach. A move shorter than
`approach_steps` pixels SHALL pace only as many steps as it has different integer pixel positions
to visit.

#### Scenario: Direct mode is a single jump

- **GIVEN** the motion mode `Direct`
- **WHEN** the pointer moves 50 px
- **THEN** exactly one motion event SHALL be delivered, at the target, with no approach interval
  waited

#### Scenario: Pointer already on the target

- **GIVEN** the pointer already at the target point
- **WHEN** a move or click targets that point
- **THEN** no paced approach SHALL be waited

#### Scenario: Very short move

- **GIVEN** the default pointer profile
- **WHEN** the pointer moves 1 px
- **THEN** exactly one paced step SHALL be delivered, at the target

### Requirement: The approach is configurable and can be turned off

The pointer profile SHALL have two parameters for the approach: `approach_steps` (a whole number,
default 3) and `approach_interval` (a duration, default 20 ms). Either set to 0 SHALL turn the
approach off, so the move follows only its path timing. Both SHALL be settable on the profile and
per action, and both SHALL be reported by the profile like the other pointer parameters.

#### Scenario: Approach turned off by steps

- **GIVEN** a profile with `approach_steps` set to 0
- **WHEN** the pointer moves 50 px in the `Linear` mode
- **THEN** no motion event SHALL wait for an approach interval

#### Scenario: Approach turned off by interval

- **GIVEN** a profile with `approach_interval` set to 0
- **WHEN** the pointer moves 50 px in the `Linear` mode
- **THEN** no motion event SHALL wait for an approach interval

#### Scenario: Per-action override wins over the profile

- **GIVEN** a profile with the default approach
- **WHEN** a single move is given the override `approach_steps` 5 and `approach_interval` 30 ms
- **THEN** that move SHALL end with 5 paced steps at least 30 ms apart
- **AND** the next move without overrides SHALL use the profile's 3 steps at 20 ms again

### Requirement: The approach is part of the move's duration

The approach SHALL count toward the move's duration, not come on top of it. A move SHALL take at
least `approach_steps × approach_interval`, so a short move still gets the full interval. A longer
move SHALL keep its usual duration, with the approach taken from its end. A non-zero
`max_move_duration` SHALL cap the whole move, approach included, shortening the intervals evenly.
With `move_time_per_pixel` and `max_move_duration` both 0, the move SHALL run without an approach.

#### Scenario: Short move takes at least the approach time

- **GIVEN** the default pointer profile (800 µs per pixel, so 40 ms of path time for 50 px)
- **WHEN** the pointer moves 50 px in the `Linear` mode
- **THEN** the move SHALL take at least 60 ms, with the 3 approach steps at least 20 ms apart

Verified with the recorded sleeps (mock-verifiable).

#### Scenario: Long move keeps its duration

- **GIVEN** the default pointer profile (400 ms of path time for 500 px)
- **WHEN** the pointer moves 500 px in the `Linear` mode
- **THEN** the move SHALL take its usual 400 ms, not 460 ms
- **AND** its last 3 steps SHALL be at least 20 ms apart

Verified with the recorded sleeps (mock-verifiable).

#### Scenario: A short cap shortens the intervals

- **GIVEN** a profile with `max_move_duration` 30 ms and the default approach
- **WHEN** the pointer moves 50 px in the `Linear` mode
- **THEN** the move SHALL take no more than 30 ms in total
- **AND** its 3 approach steps SHALL be evenly spaced, at least 10 ms apart

Verified with the recorded sleeps (mock-verifiable).

#### Scenario: Zero move time means no approach

- **GIVEN** a profile with `move_time_per_pixel` 0 and `max_move_duration` 0
- **WHEN** the pointer moves 50 px in the `Linear` mode
- **THEN** no motion event SHALL wait for an approach interval

Verified with the recorded sleeps (mock-verifiable).

### Requirement: The approach parameters are available on every pointer surface

The approach parameters SHALL be usable wherever the other pointer timing parameters are: the
Python bindings (profile dict and per-call overrides, the interval in milliseconds as
`approach_interval_ms`), the CLI pointer commands (`--approach-steps`, `--approach-interval` in
milliseconds), the BareMetal `pointer_profile` import setting and `overrides` argument, and the
high-level library's pointer settings.

#### Scenario: Robot Framework turns the approach off for one click

- **GIVEN** a BareMetal suite on the mock provider
- **WHEN** `Pointer Click` is called with `overrides=${{ {"approach_steps": 0} }}`
- **THEN** the click SHALL succeed and land on its target

Verified in the mock lane; the timing itself is covered by the runtime scenarios above.

#### Scenario: CLI accepts the approach flags

- **WHEN** `platynui-cli pointer move --point 100,100 --approach-steps 2 --approach-interval 10`
  is run
- **THEN** the move SHALL end with 2 paced steps at least 10 ms apart

### Requirement: Invalid approach values are handled like other pointer parameters

A step count that is not a non-negative whole number SHALL be rejected with an error that names
the parameter, before the pointer moves. A negative interval SHALL count as 0, like every other
pointer duration, and so turn the approach off.

#### Scenario: Negative interval turns the approach off

- **WHEN** an override sets `approach_interval_ms` to a negative number
- **THEN** the move SHALL run without an approach, as with an interval of 0

#### Scenario: Negative step count is rejected

- **WHEN** an override sets `approach_steps` to a negative number or a non-integer
- **THEN** the call SHALL fail with an error that names `approach_steps`, and the pointer SHALL
  not move

### Requirement: A click straight after opening a GTK 3 menu activates the item

With the default pointer profile, clicking a GTK 3 menu item straight after the click that opened
its menu SHALL activate the item, even when the path to the item arrives at the toolkit as a burst.

#### Scenario: Menu bar item activates right after opening the menu

- **GIVEN** the GTK 3 test app in a real lane (compositor or X11)
- **WHEN** the test clicks `menu-file` and then straight away `menu-file-new`
- **THEN** the `last-action-menu-file-new` label SHALL appear

Only verifiable against the real GTK 3 toolkit, never the mock. Covered by the existing test
`Activating A Menu Bar Item Updates The Last Action`.

#### Scenario: Burst path still activates the item

- **GIVEN** the GTK 3 test app in a real lane, and the click on `menu-file-new` given the override
  `move_time_per_pixel_us` 1, so the path before the approach is sent as one burst
- **WHEN** the test clicks `menu-file` and then straight away `menu-file-new`
- **THEN** the `last-action-menu-file-new` label SHALL appear

Only verifiable against the real GTK 3 toolkit, as a check during implementation (not a committed
test). Whether GTK 3 merges the burst depends on how busy it is: a fast local machine reads the
motion in time even without the approach, so the scenario passes there but does not exercise the
merging. The CI compositor lane is where the merging happens.
