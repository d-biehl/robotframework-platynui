# Spec Delta

## Purpose

Defines how the PlatynUI Wayland compositor accepts emulated input from clients over EIS (libei), `zwlr-virtual-pointer-v1`, `zwp-virtual-keyboard-v1` and `wp-pointer-warp-v1`. It covers where that input is processed, which keymap and coordinate space apply, what happens to held input when an emulating device goes away, and how the restrictive protocol policy gates it. Test tools and the PlatynUI platform's fallback input paths depend on it behaving like a real seat device.

## ADDED Requirements

### Requirement: Emulated pointer input is handled like device input

Pointer events from an EIS client or a virtual pointer SHALL be handled exactly like pointer events from a physical device or the control socket. That covers the window focused and raised on a press, server-side decoration hits (titlebar buttons, move, resize), the pointer bridge to X11 clients, the cursor shape, and the pointer focus that motion sets. No emulated path SHALL skip these effects.

#### Scenario: A virtual-pointer click activates the window under it

- **GIVEN** a compositor with two mapped toplevels A and B, where B is focused
- **WHEN** a virtual-pointer client moves the pointer onto A and sends a left-button press and release
- **THEN** the window listing SHALL report A as focused, and A's client SHALL receive the button press and release on its surface

#### Scenario: An EIS click activates the window under it

- **GIVEN** a compositor with two mapped toplevels A and B, where B is focused, and an EI sender client bound to an absolute pointer and a button device
- **WHEN** the client moves the absolute pointer onto A and sends a left-button press and release
- **THEN** the window listing SHALL report A as focused

### Requirement: Relative emulated motion stays on the outputs

Relative motion from an EIS client or a virtual pointer SHALL move the pointer by the given delta and SHALL keep the pointer inside the union of the output geometries, just as for relative motion from a physical device.

#### Scenario: Relative motion past the right edge stops at the edge

- **GIVEN** a compositor with one 1280×720 output at the origin and the pointer at (1270, 100)
- **WHEN** a virtual-pointer client sends a relative motion of (+50, 0) and a frame
- **THEN** the reported pointer position SHALL have x within the output (≤ 1279) and y = 100

### Requirement: Absolute emulated motion maps onto the advertised space

Absolute motion from a virtual pointer SHALL map its position, relative to the extent the client sends, onto the output the pointer was created for. For a pointer created without an output it SHALL map onto the union of all outputs. Motion with a zero extent SHALL be ignored. Absolute motion and touch from an EIS client SHALL be interpreted in the global logical coordinates of the regions the compositor advertised.

#### Scenario: An output-bound virtual pointer reaches that output's center

- **GIVEN** a compositor with two 1280×720 outputs side by side, the second at x = 1280
- **WHEN** a virtual-pointer client created for the second output sends absolute motion (500, 500) with extent (1000, 1000)
- **THEN** the reported pointer position SHALL be (1920, 360)

#### Scenario: Zero extent is ignored

- **GIVEN** a compositor with the pointer at (10, 10)
- **WHEN** a virtual-pointer client sends absolute motion with an x extent of 0
- **THEN** the pointer position SHALL remain (10, 10) and the compositor SHALL keep serving the client

#### Scenario: EIS absolute motion lands on the requested global position

- **GIVEN** a compositor with two 1280×720 outputs side by side and an EI sender client bound to an absolute pointer
- **WHEN** the client sends absolute motion to (1500, 200)
- **THEN** the reported pointer position SHALL be (1500, 200)

### Requirement: The EIS endpoint is discoverable

Unless started with `--no-eis`, the compositor SHALL listen for EI clients on the socket `eis-platynui` in `$XDG_RUNTIME_DIR` and SHALL export its path as `LIBEI_SOCKET` to the child it launches. With `--print-env` it SHALL also print `LIBEI_SOCKET=<path>` in its readiness output. With `--no-eis` it SHALL do none of these.

#### Scenario: The child process finds the EIS socket

- **GIVEN** the compositor started headless with a child command that prints `LIBEI_SOCKET`
- **WHEN** the child runs
- **THEN** it SHALL print a path ending in `eis-platynui` that accepts an EI connection

#### Scenario: EIS can be switched off

- **WHEN** the compositor is started with `--no-eis` and `--print-env`
- **THEN** no `eis-platynui` socket SHALL exist in its runtime directory and the readiness output SHALL NOT name `LIBEI_SOCKET`

### Requirement: EIS serves sender clients only

The compositor SHALL accept only EI clients that connect as senders, meaning clients that emulate input. A client that connects as a receiver SHALL be disconnected, and the compositor SHALL keep serving every other client.

#### Scenario: A receiver context is refused

- **GIVEN** a running compositor with EIS enabled
- **WHEN** an EI client completes the handshake as a receiver
- **THEN** the client SHALL be disconnected, and a following control-socket status request SHALL succeed

### Requirement: EIS offers pointer, keyboard and touch but not text

To a sender client the compositor SHALL offer a seat with relative pointer, absolute pointer, button, scroll, keyboard and touch capabilities, and SHALL NOT offer the text capability. A client SHALL get a device for each offered capability it binds. Absolute-pointer and touch devices SHALL carry one region per output, with that output's global logical geometry and scale.

#### Scenario: Binding every capability yields every device except text

- **GIVEN** a compositor with two outputs and an EI sender client
- **WHEN** the client binds all capabilities the seat offers
- **THEN** it SHALL receive pointer, absolute pointer, button, scroll, keyboard and touch devices, SHALL NOT receive a text device, and its absolute pointer SHALL carry two regions matching the outputs

### Requirement: The EIS keyboard uses the compositor's keymap

The keyboard device the compositor gives an EI client SHALL carry the compositor's configured XKB keymap. A key code sent through EIS SHALL produce the same keysym in the focused client as the same key code injected through the control socket.

#### Scenario: The EI keymap matches the compositor keymap

- **GIVEN** a compositor configured with a non-default keyboard layout
- **WHEN** an EI sender client binds the keyboard capability
- **THEN** the keymap the client receives SHALL describe the same layout as the keymap the control socket's `get_keymap` reports

#### Scenario: A key typed through EIS reaches the focused client

- **GIVEN** a focused raw Wayland client and an EI sender client bound to a keyboard
- **WHEN** the EI client presses and releases the key code for `a`
- **THEN** the focused client SHALL receive a press and a release of that key code with the compositor's keymap active

### Requirement: Several EIS clients are served at once

The compositor SHALL serve several EI sender clients at the same time, each with its own devices. A new connection SHALL NOT disconnect an existing one.

#### Scenario: A second EI client does not displace the first

- **GIVEN** a compositor with an EI sender client A bound to an absolute pointer
- **WHEN** a second EI sender client B connects and binds an absolute pointer, and then A moves its pointer to (100, 100)
- **THEN** A SHALL still be connected and the reported pointer position SHALL be (100, 100)

### Requirement: A departing emulated device releases what it holds

When an EI client disconnects, or a virtual keyboard or virtual pointer is destroyed or its client disconnects, the compositor SHALL release every key and button that device still holds, and SHALL clear the modifier state that device set. Input held by other devices SHALL stay held.

#### Scenario: A crashed EI client leaves no key pressed

- **GIVEN** a focused raw Wayland client and an EI sender client bound to a keyboard
- **WHEN** the EI client presses Shift and then disconnects without releasing it
- **THEN** the focused client SHALL receive the release of Shift, and the control socket's `get_modifiers` SHALL report `shift` as inactive

#### Scenario: A destroyed virtual keyboard clears its modifiers

- **GIVEN** a virtual-keyboard client that has set Ctrl through its modifier request
- **WHEN** the client destroys its virtual keyboard
- **THEN** `get_modifiers` SHALL report `ctrl` as inactive

#### Scenario: Another device's held key survives

- **GIVEN** Shift held through the control socket and Ctrl held through an EI client
- **WHEN** the EI client disconnects
- **THEN** `get_modifiers` SHALL report `shift` active and `ctrl` inactive

### Requirement: A virtual keyboard's keys use the keymap it supplied

Key events from a virtual keyboard SHALL be interpreted with the keymap that client supplied, and the focused client SHALL receive that keymap before such a key. Afterwards, input from every other device SHALL again use the compositor's configured keymap.

#### Scenario: A custom virtual-keyboard keymap is honored

- **GIVEN** a focused raw Wayland client and a virtual-keyboard client whose keymap maps one key code to a character the compositor's layout does not have on it
- **WHEN** the virtual keyboard presses and releases that key code, and afterwards the control socket injects the same key code
- **THEN** the focused client SHALL receive the virtual keyboard's keymap before the first key, and the compositor's configured keymap before the injected key

### Requirement: Pointer warps are honored only for the focused surface

A pointer-warp request SHALL move the pointer only when the surface has pointer focus (including during an implicit grab), the serial is that of an enter event sent to the requesting client, and the position lies inside the surface. The surface-local position SHALL be converted to global coordinates, for popups and subsurfaces too. Other requests SHALL be ignored without a protocol error.

#### Scenario: A warp inside the focused surface moves the pointer

- **GIVEN** a raw Wayland client whose toplevel at global (100, 100) has pointer focus, with the enter serial it received
- **WHEN** it requests a warp to surface-local (20, 30) with that serial
- **THEN** the reported pointer position SHALL be (120, 130) and the client SHALL receive a motion event at (20, 30)

#### Scenario: A warp with a stale serial is ignored

- **GIVEN** the same client after the pointer left its surface and entered it again
- **WHEN** it requests a warp with the serial of the first enter
- **THEN** the pointer position SHALL be unchanged and the client SHALL stay connected

#### Scenario: A warp outside the surface or to an unfocused surface is ignored

- **GIVEN** a raw Wayland client with a 200×200 toplevel that does not have pointer focus
- **WHEN** it requests a warp to (20, 30), and after gaining focus a warp to (500, 500)
- **THEN** neither request SHALL move the pointer

### Requirement: The restrictive policy hides the virtual-input globals

When the compositor runs with `--restrict-protocols`, it SHALL NOT advertise the virtual-keyboard, virtual-pointer and pointer-warp globals to any client. Without that option it SHALL advertise them to every client.

#### Scenario: Restricted clients see no virtual-input globals

- **GIVEN** a compositor started with `--restrict-protocols`
- **WHEN** a raw Wayland client lists the registry
- **THEN** the listing SHALL contain none of `zwp_virtual_keyboard_manager_v1`, `zwlr_virtual_pointer_manager_v1` and `wp_pointer_warp_v1`

#### Scenario: Unrestricted clients see them

- **GIVEN** a compositor started without `--restrict-protocols`
- **WHEN** a raw Wayland client lists the registry
- **THEN** the listing SHALL contain all three globals
