# Spec Delta

## Purpose

Defines what the PlatynUI Wayland compositor serves to screen-capture clients through `ext-image-capture-source-v1` and `ext-image-copy-capture-v1`: which sources can be captured, which buffers are accepted, how cursors are delivered, and how the restrictive protocol policy gates capture. Standard tools such as `grim`, `wayvnc` and `wl-mirror` rely on it. PlatynUI's own screenshots go through the control socket and are not covered here.

## ADDED Requirements

### Requirement: An output can be captured into a shared-memory buffer

The compositor SHALL let a client create a capture source for any advertised output and a capture session on it. The session SHALL announce the output's size in physical pixels and the `wl_shm` formats ARGB8888 and XRGB8888 before `done`. A frame captured into a matching `wl_shm` buffer SHALL be filled with the output's current content and SHALL end with `ready`, preceded by its transform, damage and presentation time.

#### Scenario: A full-output capture succeeds

- **GIVEN** a headless compositor with one 1280×720 output at scale 1 showing a toplevel filled with a known solid color
- **WHEN** a raw Wayland client captures that output into a 1280×720 ARGB8888 `wl_shm` buffer
- **THEN** the frame SHALL end with `ready`, and the pixels inside the toplevel's rectangle SHALL have that color

#### Scenario: A scaled output announces its mode size

- **GIVEN** a headless compositor started with `--scale 2`
- **WHEN** a client creates a capture session for its output
- **THEN** the announced buffer size SHALL equal the size of the output's current mode, which the `status` command reports

### Requirement: A toplevel can be captured on its own

The compositor SHALL let a client create a capture source from an `ext-foreign-toplevel-list` handle. A frame captured from it SHALL contain only that toplevel's content, without the windows that overlap it. Its size SHALL be the toplevel's window geometry in physical pixels at the output scale.

#### Scenario: A covered toplevel is captured without what covers it

- **GIVEN** a headless compositor at scale 1, toplevel A of 300×200 filled with one solid color, and toplevel B of another color mapped over half of A
- **WHEN** a client captures A through its foreign-toplevel handle
- **THEN** the frame SHALL end with `ready` at 300×200, and every pixel SHALL have A's color

### Requirement: A buffer that does not match the constraints fails the frame

A capture into a buffer whose size or format differs from the announced constraints, or into a buffer that is not `wl_shm`, SHALL end that frame with `failed` and reason `buffer_constraints`. The session SHALL stay usable for a following frame with a matching buffer.

#### Scenario: A wrong-sized buffer is rejected and the session recovers

- **GIVEN** a capture session for a 1280×720 output
- **WHEN** the client captures into a 640×480 buffer, and then into a 1280×720 buffer
- **THEN** the first frame SHALL fail with `buffer_constraints` and the second SHALL end with `ready`

### Requirement: Only one frame per session is pending

A client that creates a frame while the session still has one that has not finished SHALL get the protocol error `duplicate_frame`. The compositor SHALL keep serving every other client.

#### Scenario: A second pending frame is a protocol error

- **GIVEN** a capture session with a created frame that has not been captured yet
- **WHEN** the client creates another frame on that session
- **THEN** the client SHALL be disconnected with `duplicate_frame`, and a following control-socket status request SHALL succeed

### Requirement: Cursors are captured on request

When a session is created with the `paint_cursors` option, the compositor SHALL draw the cursor into each captured frame at the pointer position. Without the option the frame SHALL show no cursor. A cursor session SHALL deliver the cursor image separately at the cursor's size, with its hotspot, while the pointer is over the captured source.

#### Scenario: The cursor appears only when asked for

- **GIVEN** a headless compositor with the pointer at a known position over a solid-colored toplevel
- **WHEN** the same output is captured once with `paint_cursors` and once without
- **THEN** the pixels around the pointer position SHALL differ from the toplevel's color only in the frame captured with `paint_cursors`

#### Scenario: A cursor session reports the cursor image and hotspot

- **GIVEN** a headless compositor with the pointer over an output
- **WHEN** a client creates a cursor session for that output and captures a frame of it
- **THEN** the session SHALL report the cursor entering and its hotspot, and the frame SHALL end with `ready` at the announced cursor size

### Requirement: A capture session ends with its source

When the captured toplevel is closed or the captured output is removed, the compositor SHALL send `stopped` on the sessions of that source, and frames created afterwards SHALL fail with reason `stopped`. The compositor SHALL NOT end the client's connection for this.

#### Scenario: Closing the captured toplevel stops its session

- **GIVEN** a capture session for toplevel A
- **WHEN** A is closed and the client then creates and captures a frame
- **THEN** the session SHALL have received `stopped`, the frame SHALL fail with `stopped`, and the client SHALL stay connected

### Requirement: The restrictive policy hides screen capture

When the compositor runs with `--restrict-protocols`, it SHALL NOT advertise the capture-source and copy-capture globals to any client. Without that option it SHALL advertise them to every client.

#### Scenario: Restricted clients cannot find screen capture

- **GIVEN** a compositor started with `--restrict-protocols`
- **WHEN** a raw Wayland client lists the registry
- **THEN** the listing SHALL contain none of `ext_output_image_capture_source_manager_v1`, `ext_foreign_toplevel_image_capture_source_manager_v1` and `ext_image_copy_capture_manager_v1`

#### Scenario: Unrestricted clients find it

- **GIVEN** a compositor started without `--restrict-protocols`
- **WHEN** a raw Wayland client lists the registry
- **THEN** the listing SHALL contain all three globals
