# Linux Platform

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document covers the Linux platform implementation for PlatynUI: the session mediator, X11 platform devices, AT-SPI2 provider, and EWMH WindowManager. For the platform-agnostic architecture, see `dev-docs/architecture.md`.

## 0. Session Mediator (`platynui-platform-linux`)

The mediator crate sits between consumers (CLI, Inspector, Python bindings) and the sub-platform backends (`platform-linux-x11` and `platform-linux-wayland`). It is the **only** Linux platform crate that self-registers via `inventory`.

### Design Decisions

1. **Runtime session detection** — Linux sessions can be X11 or Wayland; this is a runtime property (unlike Windows/macOS which have a single display system). The mediator detects the session once via environment variables and caches the result for the process lifetime.

2. **Sub-platforms are libraries, not plugins** — Sub-platform crates do not self-register. They export their device types as public structs and let the mediator decide when and whether to use them. This avoids unnecessary initialization and inventory pollution.

3. **Resolve once, delegate via static references** — Session detection runs exactly once. `LinuxModule::initialize()` resolves the session type and populates a `Resolved` struct that bundles one `&'static dyn` trait-object reference per platform trait (`module`, `pointer`, `keyboard`, `desktop`, `screenshot`, `highlight`, `window_manager`), each pointing at the chosen sub-platform's `static` device. The struct is stored in `Mutex<Option<Resolved>>` (`RESOLVED`); every wrapper device then routes through `resolved().pointer`, `resolved().keyboard`, etc. without re-detecting the session per call. The sub-platform device structs themselves carry no state — expensive resources (e.g., X11 connections) are lazily acquired inside method bodies.

4. **Single registration point** — The mediator registers exactly one implementation per platform trait. Consumers never see the sub-platform crates in the `inventory` registry, which prevents double-registration or ordering issues.

### Session Detection (`session.rs`)

```
$XDG_SESSION_TYPE  ──→  "x11" / "wayland" (authoritative)
        │ (unset or unknown)
        ▼
$WAYLAND_DISPLAY set?  ──→ Wayland
        │ (unset)
        ▼
$DISPLAY set?          ──→ X11
        │ (unset)
        ▼
    Error: cannot detect session type
```

The result is cached in `Mutex<Option<SessionType>>`. `XWayland` environments have both `$DISPLAY` and `$WAYLAND_DISPLAY` set, but `$XDG_SESSION_TYPE=wayland` — hence step 1 takes priority.

### Delegation Example

```rust
struct LinuxPointer;   // mediator's own wrapper, registered via inventory

impl PointerDevice for LinuxPointer {
    fn position(&self) -> Result<Point, PlatformError> {
        resolved().pointer.position()   // dispatch via the resolved static reference
    }
    // … other methods follow the same pattern
}
```

`resolved()` returns the `Resolved` struct populated at initialization, so the `pointer` reference already points at either the X11 or the Wayland sub-platform device — no per-call session check is needed.

### Crate Dependencies

```
platynui-platform-linux
├── platynui-core                   (platform traits)
├── platynui-platform-linux-x11     (X11 sub-platform, library)
├── platynui-platform-linux-wayland (Wayland sub-platform, library)
├── inventory                       (self-registration)
└── tracing                         (diagnostics)
```

Consumers depend only on `platynui-platform-linux`, never on the sub-platform crates directly.

## 1. X11 Platform Devices (`platynui-platform-linux-x11`)

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

**Screenshot**: `XGetImage` returning BGRA8 (X11 ZPixmap 32bpp is typically BGRX/BGRA). Optional XShm acceleration planned.

**Highlight**: Multiple small override-redirect windows per segment (solid red borders). Clamping to desktop bounds; clipped edges drawn dashed (8px on / 4px off). Thread + `mpsc` channel for show/clear with deadline-based duration timer.

**Shutdown**: Highlight thread cleanup + X11 connection FD close.

**X11 Utilities**: Connection pooling via `Mutex<X11Handle>` in `x11util.rs`.

## 2. AT-SPI2 Provider

**Connection**: D-Bus/AT-SPI2 via `zbus` 5 + `atspi-*` 0.14. Blocking tree queries.

**Node Model** (`AtspiNode`):
- Lazy `children()` and streaming `attributes()`.
- Role mapping to `control`/`item` namespaces via AT-SPI role enum.
- `app:Application` nodes for processes with the Application interface.

**Standard Attributes**: `Role`, `Name`, `RuntimeId` (from D-Bus object path), `Technology` = "AT-SPI2", optional `Id` (from `accessible_id`).

**Component-gated Attributes**: `Bounds`, `ActivationPoint`, `IsEnabled`, `IsVisible`, `IsInView`, `IsFocused` — only present when the AT-SPI Component interface is available.

**Native Attributes**: `Native/<Interface>.<Property>` for all AT-SPI interfaces, including `Accessible.GetAttributes` mapping.

**Patterns**: `Focusable` via `grab_focus()` + AT-SPI State flags.

## 3. WindowManager (EWMH)

- XID resolution: `_NET_CLIENT_LIST` + `_NET_WM_PID` matching with `_NET_WM_NAME` fallback for multi-window PIDs.
- EWMH actions: `_NET_ACTIVE_WINDOW`, `_NET_CLOSE_WINDOW`.
- WindowSurface pattern on Frame/Window/Dialog roles: `activate()`, `close()`, `accepts_user_input()`.
- `IsTopmost` via EWMH, `AcceptsUserInput` via AT-SPI State.
