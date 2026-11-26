# Spec Delta

## Purpose

Window management under Wayland compositors other than the PlatynUI compositor, each through the compositor's own mechanism: how a node's top-level window is found, which geometry the window manager reports for it, which operations each compositor supports, and how an operation a compositor cannot perform fails. It covers niri and cosmic-comp.

## ADDED Requirements

### Requirement: A node's window is found without guessing

Under niri and cosmic-comp, the window manager SHALL resolve a node to the compositor's top-level window by narrowing the candidates in this order: the node's process ID where the compositor reports one, then the exact title, then a title that contains the node's name, then a size that only one candidate has. When the candidates stay ambiguous, it SHALL fail with an error that names what it compared, and SHALL NOT pick one.

#### Scenario: niri tells two equal windows apart by process

- **GIVEN** a niri session with two instances of the egui test app, each with the same title and size
- **WHEN** a node of the second instance is resolved to its window
- **THEN** the window manager SHALL return the second instance's window, chosen by its process ID

Verified in the local niri lane, against the real compositor.

#### Scenario: cosmic-comp refuses to guess between equal windows

- **GIVEN** a cosmic-comp session with two windows that have the same title and the same size
- **WHEN** a node of one of them is resolved to its window
- **THEN** the window manager SHALL fail with an error that names the title and size it compared, because cosmic-comp reports no process ID

Verified in CI with a recorded cosmic event sequence.

#### Scenario: A unique size decides when titles do not match

- **GIVEN** a cosmic-comp session with two windows of the same application whose accessible names differ from their titles but whose sizes differ
- **WHEN** a node of the smaller window is resolved
- **THEN** the window manager SHALL return the smaller window

Verified in CI with a recorded cosmic event sequence.

### Requirement: Window bounds come from the compositor's report

The window manager SHALL report a window's bounds as the visible frame the compositor reports, converted to desktop coordinates by adding the position of the output (cosmic-comp) or of the workspace view's output (niri) it refers to. It SHALL report the bounds' top-left corner as the window's surface origin, because neither compositor reports where a client's surface starts.

#### Scenario: niri bounds include the output's position

- **GIVEN** a niri session with two outputs side by side, the second at logical x 1920, and a window shown on the second output at view position (100, 50) with a window offset of (4, 4) in its tile
- **WHEN** the window's geometry is read
- **THEN** its bounds SHALL start at (2024, 54) with the window size niri reports, and its surface origin SHALL be (2024, 54)

Verified in CI with a recorded niri reply.

#### Scenario: cosmic bounds include the output's position

- **GIVEN** a cosmic-comp session whose second output is at logical x 1920, and a window whose geometry event reports (30, 40, 800, 600) relative to that output
- **WHEN** the window's geometry is read
- **THEN** its bounds SHALL be (1950, 40, 800, 600)

Verified in CI with a recorded cosmic event sequence.

#### Scenario: A niri window outside the workspace view has no bounds

- **GIVEN** a niri window that is not visible in its workspace view, so niri reports no view position for it
- **WHEN** the window's geometry is read
- **THEN** the read SHALL fail with an unavailable error saying the window is not on screen

Verified in CI with a recorded niri reply.

#### Scenario: Elements of a server-side-decorated window land where they are drawn

- **GIVEN** the egui test app in the local niri lane
- **WHEN** a button is clicked by its accessible element
- **THEN** the click SHALL activate the button, shown by its click counter

Verified in the local niri lane; the same suite runs in the local cosmic lane.

### Requirement: Each compositor's operations are offered where it can perform them

Under niri, the window manager SHALL activate, close, minimize, maximize, restore and resize windows, and SHALL move floating windows. Under cosmic-comp, it SHALL activate, close, minimize, maximize and restore windows. Every operation a compositor cannot perform SHALL fail with an unavailable error that names the operation and the compositor.

#### Scenario: Window operations under niri

- **GIVEN** the egui test app in the local niri lane
- **WHEN** its window is minimized, activated, maximized, restored and resized in turn
- **THEN** each operation SHALL succeed, and the state read after each SHALL be minimized, normal, maximized, normal, and normal with the new size

Verified in the local niri lane.

#### Scenario: Moving a tiled niri window is unavailable

- **GIVEN** a tiled (not floating) window in a niri session
- **WHEN** the window is moved
- **THEN** the call SHALL fail with an unavailable error that names moving, niri and that the window is tiled

Verified in CI with a recorded niri reply; the move request is never sent.

#### Scenario: Moving and resizing under cosmic-comp are unavailable

- **GIVEN** any window in a cosmic-comp session
- **WHEN** the window is moved or resized
- **THEN** each call SHALL fail with an unavailable error that names the operation and cosmic-comp

Verified in CI.

#### Scenario: Window operations under cosmic-comp

- **GIVEN** the egui test app in the local cosmic lane
- **WHEN** its window is minimized, activated, maximized and restored in turn
- **THEN** each operation SHALL succeed, and the state read after each SHALL be minimized, normal, maximized and normal

Verified in the local cosmic lane.

### Requirement: Popups are not listed under niri and cosmic-comp

Under niri and cosmic-comp, the window manager's popup query SHALL fail with an unavailable error naming popups and the compositor, because neither compositor lists a client's popups. It SHALL NOT return an empty list, which would read as "this application has no open popup".

#### Scenario: The popup query is unavailable

- **GIVEN** a niri or cosmic-comp session
- **WHEN** the popups of a process are queried
- **THEN** the query SHALL fail with an unavailable error naming popups and the compositor

Verified in CI.

### Requirement: A closed window stops resolving

When a resolved window has closed, every later operation on it SHALL fail with an error saying the window no longer exists, under niri and cosmic-comp alike. It SHALL NOT act on another window that took its place.

#### Scenario: Operating on a closed window fails

- **GIVEN** a window resolved in the local niri lane, then closed
- **WHEN** it is activated
- **THEN** the call SHALL fail with an error saying the window no longer exists

Verified in the local niri lane, and in CI with recorded replies for both compositors.
