# Linux X11 Platform Implementation

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document covers the Linux/X11-specific implementation details for PlatynUI: platform devices, AT-SPI2 provider, and EWMH WindowManager. For the platform-agnostic architecture, see `docs/architecture.md`.

## 1. Platform Devices

**Initialization** (`PlatformModule::initialize()`):
- Eager X11 connection via `x11rb::RustConnection` (pure Rust, no libX11).
- Extension probing: XTEST is mandatory (error if missing), RANDR is optional (graceful fallback to root geometry).
- `XInitThreads` not needed (`x11rb` is pure Rust).

**Desktop & Monitors**: XRandR for monitor enumeration (ID, name, bounds, primary). Fallback to root window geometry if RANDR unavailable.

**Pointer**: XTest (`FakeMotion`, `FakeButtonEvent`). `QueryPointer` for current position. Buttons 1-3 (primary/middle/secondary), 8/9 (back/forward), 4-7 (scroll).

**Keyboard**: XTest injection (`FakeKeyEvent`) with keysym-to-keycode resolution via `GetKeyboardMapping`. Named keys (modifiers, function keys, navigation, numpad) resolved from a static lookup table; single characters resolved via keysym mapping with CapsLock-aware shift management. Characters not present in the active keyboard layout are injected through dynamic remapping of a spare (unmapped) keycode via `ChangeKeyboardMapping`. Control characters encountered in text input (e.g. `\n`, `\t`) are mapped to their corresponding X11 TTY function keysyms:

| Character | Code | X11 Keysym |
|-----------|------|------------|
| `\n` (LF) | U+000A | `XK_RETURN` |
| `\r` (CR) | U+000D | `XK_RETURN` |
| `\t` (TAB) | U+0009 | `XK_TAB` |
| `\b` (BS) | U+0008 | `XK_BACKSPACE` |
| ESC | U+001B | `XK_ESCAPE` |
| DEL | U+007F | `XK_DELETE` |

Other C0 control characters (U+0000–U+001F) have no standard keyboard equivalent and are not mapped.

**Screenshot**: `XGetImage` returning RGBA. Optional XShm acceleration planned.

**Highlight**: Multiple small override-redirect windows per segment (solid red borders). Clamping to desktop bounds; clipped edges drawn dashed (8px on / 4px off). Thread + `mpsc` channel for show/clear with deadline-based duration timer.

**Shutdown**: Highlight thread cleanup + X11 connection FD close.

**X11 Utilities**: Connection pooling via `Mutex<X11Handle>` in `x11util.rs`.

## 2. AT-SPI2 Provider

**Connection**: D-Bus/AT-SPI2 via `zbus` 5 + `atspi-*` 0.13. Blocking tree queries.

**Node Model** (`AtspiNode`):
- Lazy `children()` and streaming `attributes()`.
- Role mapping to `control`/`item` namespaces via AT-SPI role enum.
- `app:Application` nodes for processes with the Application interface.

**Standard Attributes**: `Role`, `Name`, `RuntimeId` (from D-Bus object path), `Technology` = "AT-SPI2", optional `Id` (from `accessible_id`).

**Component-gated Attributes**: `Bounds`, `ActivationPoint`, `IsEnabled`, `IsVisible`, `IsOffscreen`, `IsFocused` — only present when the AT-SPI Component interface is available.

**Native Attributes**: `Native/<Interface>.<Property>` for all AT-SPI interfaces, including `Accessible.GetAttributes` mapping.

**Patterns**: `Focusable` via `grab_focus()` + AT-SPI State flags.

## 3. WindowManager (EWMH)

- XID resolution: `_NET_CLIENT_LIST` + `_NET_WM_PID` matching with `_NET_WM_NAME` fallback for multi-window PIDs.
- EWMH actions: `_NET_ACTIVE_WINDOW`, `_NET_CLOSE_WINDOW`.
- WindowSurface pattern on Frame/Window/Dialog roles: `activate()`, `close()`, `accepts_user_input()`.
- `IsTopmost` via EWMH, `AcceptsUserInput` via AT-SPI State.
