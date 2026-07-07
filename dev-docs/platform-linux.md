# Linux Platform

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document covers the Linux platform implementation for PlatynUI: the session mediator, X11 platform devices, AT-SPI2 provider, and EWMH WindowManager. For the platform-agnostic architecture, see `dev-docs/architecture.md`.

## 0. Session Mediator (`platynui-platform-linux`)

The mediator crate sits between consumers (CLI, Inspector, Python bindings) and the sub-platform backends (`platform-linux-x11` and `platform-linux-wayland`). It is the **only** Linux platform crate that self-registers via `inventory`.

### Design Decisions

1. **Runtime session detection** — Linux sessions can be X11 or Wayland; this is a runtime property (unlike Windows/macOS which have a single display system). The mediator detects the session once via environment variables and caches the result for the process lifetime.

2. **Sub-platforms are libraries, not plugins** — Sub-platform crates do not self-register. Each exports a `create_*_bundle(config)` function (and the device types it assembles) and lets the mediator decide when to call it. This avoids unnecessary initialization and inventory pollution.

3. **Select a backend per runtime, build an owned bundle** — The mediator registers one `PlatformFactory` per session type (X11, Wayland). A factory's `can_serve(config)` is true when the runtime's `config` names that backend (`platform.backend`) or, absent that, when session detection matches. The runtime calls `create(config)` on the first factory that can serve, and the factory builds *that runtime's own* bundle of devices — pointer, keyboard, screenshot, highlight, window manager, desktop info — by calling the sub-crate's `create_*_bundle`. Each runtime owns its bundle and its X11/Wayland connection; there is no cached `Resolved`, no process-global routing, and no per-call session check (the choice is made once, when the bundle is built). A later runtime makes the choice again from scratch.

4. **Single selection point** — The mediator is the only place the X11-vs-Wayland choice is made. Consumers never see the sub-platform crates in the registry, and each sub-crate stays a standalone implementation.

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

### Selection Example

Each factory answers `can_serve` from the config/session and, when chosen, delegates to its sub-crate's bundle builder:

```rust
struct X11Factory;   // registered via register_platform_factory!

impl PlatformFactory for X11Factory {
    fn id(&self) -> &'static str { "x11" }

    fn can_serve(&self, config: &RuntimeConfig) -> bool {
        match config.platform_backend() {
            Some(backend) => backend == "x11",
            None => matches!(session_type(), Ok(SessionType::X11)),
        }
    }

    fn create(&self, config: &RuntimeConfig) -> Result<PlatformBundle, PlatformError> {
        platynui_platform_linux_x11::create_x11_bundle(config)   // owned, per-runtime
    }
}
```

The Wayland factory is identical but for its id, its session match, and `create_wayland_bundle`. `crates/platform-linux` is authoritative for the exact selection.

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

**Highlight**: Multiple small override-redirect windows per segment (solid red borders). Clamping to desktop bounds; clipped edges drawn dashed (8px on / 4px off). Thread + `mpsc` channel for show/clear with deadline-based duration timer. The overlay controller is owned by the runtime's highlight device (not a process-global), so its thread is spawned per runtime and joined when the bundle drops — a runtime built after an earlier one still highlights.

**Shutdown**: Per-runtime — dropping the runtime drops its platform bundle, which joins the highlight thread and closes the X11 connection FD. No process-global teardown.

**X11 Utilities** (`x11util.rs`): `X11Connection { conn, root }` is built per runtime by `create_x11_bundle` (`X11Connection::connect(display)`, display from `platform.x11.display` config → `$DISPLAY`) and shared among the bundle's devices via `Arc`; the connection closes when the last device drops. There is no process-global connection cell — a new runtime always connects fresh. (The keymap and EWMH-atom lookup tables remain process-global caches: they are server-stable and never torn down, so they don't affect reconnection.)

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
