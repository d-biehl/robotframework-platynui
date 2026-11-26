# Spec Delta

## Purpose

How element highlights are drawn under Wayland: as a client surface on the overlay layer of `zwlr_layer_shell_v1`, on every compositor that offers it, with the same frame as on the other platforms, without ever taking input or moving windows, and with an explicit error where the protocol is missing.

## ADDED Requirements

### Requirement: Highlights are drawn on the overlay layer

Under a Wayland compositor that offers `zwlr_layer_shell_v1`, including the PlatynUI compositor, a highlight SHALL be drawn as an overlay-layer surface. Each rectangle SHALL get a red frame 3 px wide at a 1 px gap outside it, in desktop coordinates. The edges where the frame is clipped at the desktop's edge SHALL be dashed, as on X11 and Windows.

#### Scenario: A highlighted rectangle shows its frame

- **GIVEN** a headless PlatynUI compositor with one 1920×1080 output
- **WHEN** the rectangle (100, 100, 200, 50) is highlighted for 5 s
- **THEN** a screenshot taken during the highlight SHALL show red pixels on the frame line from (96, 96) to (303, 153), and the rectangle's own pixels SHALL be unchanged

Verified in CI by an integration test against the headless PlatynUI compositor.

#### Scenario: A rectangle across two outputs is framed on both

- **GIVEN** a headless PlatynUI compositor with two 1920×1080 outputs side by side
- **WHEN** the rectangle (1800, 100, 240, 50) is highlighted
- **THEN** a screenshot SHALL show the frame's left part on the first output and its right part on the second, without a gap at the outputs' shared edge

Verified in CI by an integration test against the headless PlatynUI compositor.

#### Scenario: An element is highlighted through the library

- **GIVEN** the egui test app in the compositor acceptance lane
- **WHEN** `BM.Highlight` is called on its window for 1 s
- **THEN** the keyword SHALL succeed

Covered by the existing egui smoke suite.

### Requirement: A highlight never takes input or moves windows

The highlight surface SHALL accept no pointer or touch input, SHALL never take keyboard focus, and SHALL claim no exclusive zone, so it neither moves nor is moved by panels or windows.

#### Scenario: A click goes through a highlight

- **GIVEN** the egui test app in the compositor acceptance lane, with its button highlighted for 10 s
- **WHEN** the button is clicked by its accessible element while the highlight is shown
- **THEN** the button's click counter SHALL increase

Verified in the compositor acceptance lane, against the real compositor.

#### Scenario: Keyboard focus stays with the window

- **GIVEN** the egui test app's text field focused in the compositor acceptance lane
- **WHEN** the window is highlighted and text is typed
- **THEN** the text SHALL arrive in the text field

Verified in the compositor acceptance lane.

### Requirement: A highlight lasts for its duration or until it is replaced or cleared

A highlight with a duration SHALL disappear when the duration has elapsed. A new highlight SHALL replace the one shown before it. Clearing SHALL remove the shown highlight at once. A highlight SHALL also disappear when the runtime that drew it ends.

#### Scenario: A highlight disappears after its duration

- **GIVEN** a headless PlatynUI compositor
- **WHEN** a rectangle is highlighted for 0.5 s
- **THEN** a screenshot after 1 s SHALL show no frame pixels

Verified in CI by the integration test.

#### Scenario: A new highlight replaces the previous one

- **GIVEN** a rectangle highlighted for 10 s
- **WHEN** a second, separate rectangle is highlighted
- **THEN** a screenshot SHALL show only the second rectangle's frame

Verified in CI by the integration test.

#### Scenario: Clearing removes the highlight at once

- **GIVEN** a rectangle highlighted for 10 s
- **WHEN** the highlight is cleared
- **THEN** a screenshot taken right after SHALL show no frame pixels

Verified in CI by the integration test.

### Requirement: Highlighting without a layer shell is unavailable

When the compositor does not offer `zwlr_layer_shell_v1`, a highlight request with at least one rectangle SHALL fail with an unavailable error that names the layer shell and the compositor. Clearing SHALL still succeed.

#### Scenario: Mutter has no layer shell

- **GIVEN** a Wayland session whose compositor's globals include no `zwlr_layer_shell_v1`
- **WHEN** a rectangle is highlighted
- **THEN** the request SHALL fail with an unavailable error naming `zwlr_layer_shell_v1` and the compositor
- **AND** clearing SHALL succeed

Unit-tested over the availability decision; the PlatynUI compositor always offers the protocol.
