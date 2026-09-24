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

### Process Identity and PID Namespaces

The normal deployment runs everything in one PID namespace: the runtime, the application under test, the display server and the accessibility bus daemon all see the same process table, and a process ID means the same process to each of them. Nothing in this section changes anything there. It explains why the provider still handles process IDs with care, and what happens in the one special deployment where that care matters: PlatynUI running as a **sidecar** in a PID namespace of its own.

#### What process IDs are used for

The provider needs process IDs for four things:

- **Hiding its own user interface.** A host such as the Inspector registers its own accessible application on the same bus it inspects. The provider recognises it and leaves it out of the application list, and out of the event-driven popup candidates.
- **Reporting an application's identity.** `@ProcessId` on `app:Application`, and the node identifier a consumer reads (`element.id` in Python), which follows it.
- **Reading the local process table.** The `app:*` attributes (process name, executable path, command line, user name, start time) come from `/proc/<pid>`.
- **Correlating a native window with an application.** The point hit-test maps a window to its application, and the window manager maps an application node back to its windows and popups, through the process ID both sides report.

#### Why a process ID only means something where it was issued

The bus daemon reports every peer's process ID as the kernel translates it into the daemon's own namespace. A peer the daemon cannot see (a process in a namespace outside the daemon's view) has no number there, and daemons answer for it in one of three ways: credentials carrying `0`, credentials omitting the field while the dedicated query answers a successful `0`, or an explicit "process ID unknown". Which one you get depends on the implementation and version.

Two numbers from different namespaces can be equal without naming the same process. Suppose the application is PID 4 inside its container, and the runtime happens to be PID 4 inside its own. A provider that compares the number the daemon reports for the application with `getpid()` takes the application for its own UI, hides it, and returns an empty tree. Two unresolvable peers that both report `0` look like the same process in the same way. So the provider never compares a process ID from another namespace with its own, and never treats `0` or an absent value as an identity. An identity that cannot be resolved never matches anything, not even another unresolved one.

#### One question per bus connection

For each bus connection it holds, the provider asks the daemon once what process ID it reports for **the provider's own connection**, and compares the answer with `getpid()`. It asks through `GetConnectionCredentials`, where a peer the daemon cannot see comes back with the process ID `0` or without one, depending on the implementation. The provider reads both as "cannot tell". It can check this one answer, because it already knows the right one. The outcome is one of two:

- **Local numbering.** The daemon reported our own PID. Its numbers are values in the runtime's namespace, so another connection is ours exactly when the daemon reports our PID for it, and a reported number can be used to read `/proc`.
- **No identity.** The daemon reported nothing, `0`, "unknown", or a number that is not ours (a daemon in an ancestor namespace does that). No own-process check is performed at all, and no number the daemon reports is treated as valid locally.

| | Local numbering | No identity |
|---|---|---|
| Own UI hidden from the tree and the popup candidates | yes, on a positive match | no. The host's own application may appear, and nothing else is hidden |
| `@ProcessId` and `element.id` | the daemon's number for the application | the daemon's number for the application |
| `app:*` process-table attributes | read through that number | absent |

`@ProcessId` does not depend on the outcome. It reports the number the application's own environment knows it by, whether or not that number is valid in the runtime's namespace, and it is **absent** (never `0`) when the daemon cannot tell. Reporting a number is not a comparison. `element.id` is that number when present, otherwise the toolkit's accessible-id or nothing.

A locally valid PID makes the `app:*` attributes possible, but it does not guarantee them. Each one is reported only when its value was actually read for that process. An unreadable value is left out rather than answered with an empty string, a placeholder, or a value describing the automation host. For example, the start time is absent when `/proc/<pid>/stat` cannot be read, instead of being an empty string.

The provider reports no architecture at all. Linux keeps none per process: `/proc/<pid>/status` has no such field, and the platform string in the auxiliary vector sits in the process's own memory. The only source left would be the executable's ELF header, which takes a hand-kept table of machine types, for a value nothing in PlatynUI needs on Linux.

Each connection is identified by the bus instance it is connected to and by its unique name there, because unique names repeat across buses. A process can hold several runtimes, each bound to its own bus through `providers.atspi.bus_address`, and each connection decides for itself. The outcome is logged once per connection, together with its inputs. It is an info line naming local numbering, or a warning that own-process exclusion is inactive on that connection. In a sidecar deployment the warning is the expected state. On an ordinary desktop it is not, and it means the bus daemon does not number processes the way the runtime does. Only definitive answers are cached, per connection: a resolved number, or the daemon's explicit "cannot resolve". A timed-out or failed lookup is retried the next time it is needed. A daemon that is out of memory or over a quota counts as failed too. What is cached for an application is the daemon's answer, not the verdict drawn from it. An application first seen while the provider's own lookup was still failing therefore gets the right verdict once that lookup succeeds. Shutting a provider down discards the cache of its own connections, and other runtimes in the process keep theirs.

#### The supported sidecar topology

The display server, the accessibility bus daemon and the application share one PID namespace, and the runtime runs in a sibling namespace (a Kubernetes pod without `shareProcessNamespace`). The deployment must provide three things. PlatynUI expects them and does not work around their absence:

- **Absolute socket paths that resolve identically on both sides.** This covers the accessibility bus (`providers.atspi.bus_address` or `AT_SPI_BUS_ADDRESS`), the display-server socket and, on Wayland, the compositor control socket (`PLATYNUI_CONTROL_SOCKET`). A bus address that cannot be reached is reported as an error naming it, not as an empty application list.
- **The bus daemon in the application's namespace**, so that it sees the application.
- **The same uid on both sides**, because the daemon authenticates peers by their credentials.

What the daemon can see decides two things independently. Whether it sees **the application** decides whether that application has an identity at all. Whether it sees **the runtime** decides whether the provider has one. The supported topology is the combination where the first holds and the second does not. The outcome is *no identity*, every application appears with its `@ProcessId`, and there are no `app:*` attributes, because there is no process ID the runtime could read `/proc` with. The two other combinations are outside the supported topology, and the rules above still hold in them:

- **A daemon in a namespace of its own** loses both identities for every application on its bus.
- **A daemon that shares the runtime's namespace but not the application's** keeps the provider's identity and loses the application's.

Nothing in the provider detects which combination it is in.

Window correlation stays inside one namespace. The window's process ID (`_NET_WM_PID`, or the compositor's client PID) is compared with the process ID the daemon reports for the application, and in the supported topology both are numbers in the application's namespace. Neither is ever compared with the runtime's own PID. When either side has no number, nothing is resolved, which is better than resolving the wrong application. Skipping the runtime's own windows at a point is the window system backend's decision, and the provider does not re-derive ownership from a reported number on that path.

#### Where it lives and how it is tested

The decision, the peer classification and the per-connection cache are in `crates/provider-atspi/src/identity.rs`. The application enumeration and the window correlation (`lib.rs`), the popup filter (`popups.rs`) and the attribute path (`node.rs`) all consume that one decision. The decision is a pure function of injected numbers, so its unit tests cover every daemon answer shape without a bus.

The things only a real daemon can show are covered by `crates/provider-atspi/src/pidns_harness.rs`, which builds real PID-namespace topologies with `unshare`: the sidecar, a runtime that shares the daemon's namespace, and a forced PID collision. It is `#[ignore]`d and local only. Run it once per bus implementation:

```sh
just test-atspi-pidns dbus-daemon
just test-atspi-pidns dbus-broker
```

The harness has hard prerequisites: unprivileged user namespaces, control over the next PID inside them (`/proc/sys/kernel/ns_last_pid`, used by the forced collision), the selected daemon, and for dbus-broker a user session. A missing one fails the run with a message naming it. It never skips.

## 3. WindowManager (EWMH)

- XID resolution: `_NET_CLIENT_LIST` + `_NET_WM_PID` matching with `_NET_WM_NAME` fallback for multi-window PIDs.
- EWMH actions: `_NET_ACTIVE_WINDOW`, `_NET_CLOSE_WINDOW`.
- Window state: read from the client window's `_NET_WM_STATE`. `_NET_WM_STATE_HIDDEN` (or ICCCM `WM_STATE` = Iconic) means minimized and wins over everything else, since an iconified window keeps its maximized atoms; both `_NET_WM_STATE_MAXIMIZED_VERT` and `_HORZ` mean maximized; `_NET_WM_STATE_ABOVE` means kept on top. A window that no longer exists fails the property read, so the query reports an error instead of a state.
- `activate()`: EWMH does not require a window manager to de-iconify on `_NET_ACTIVE_WINDOW`, so an iconified window is first mapped (ICCCM §4.1.4, the client-side way out of Iconic state). The window manager handles the resulting MapRequest and leaves `_NET_WM_STATE` alone, so a window minimized while maximized comes back maximized. `_NET_ACTIVE_WINDOW` follows as before.
- The AT-SPI provider exposes `IsMinimized`, `IsMaximized` and `IsTopmost` on top-level windows from this state, next to `IsActive`; each read asks the window manager again, and a window that cannot be resolved reads `false`.
- WindowSurface pattern on Frame/Window/Dialog roles: `activate()`, `close()`, `accepts_user_input()`.
- `IsTopmost` via EWMH, `AcceptsUserInput` via AT-SPI State.

**Own windows in the hit-test.** Resolving the element at a point skips the runtime's own windows, so the Inspector's live picker never selects itself and resolves the window behind instead. A window reports its owner through `_NET_WM_PID`, a number the client writes from its own PID namespace, so an unrelated application in another namespace can carry the runtime's number. A window therefore counts as the runtime's own only when two witnesses agree:

- It reports the runtime's PID.
- The X server attributes it to the same process as the runtime's own connection.

The server's attribution is X-Resource's `LocalClientPID`, which the server derives from socket credentials (see [`java-toolkits.md`](java-toolkits.md), *Window → process*). Both of its answers are numbers in the server's namespace, so they compare wherever the server runs. The server is asked about the runtime's own connection once per connection, and about a window only when that window reports the runtime's PID.

- **An ordinary desktop, or a runtime in a container on the host's display**: the runtime's own window is skipped, and another window that reuses its number is resolved.
- **WSLg**, where the server cannot see the runtime or its applications: the server answers `0` for both, so the reported PID decides.
- **The sidecar**, where the server sits with the application: the application's window is resolved even when it reports the runtime's number.
- **A server without X-Resource 1.2**: the reported PID alone decides, as before, and the log warns once that the exclusion is unverified on that display.

The log states once per connection what the server said about the runtime. This verdict is the only own-window exclusion in the hit-test: the AT-SPI provider does not re-derive ownership from the window's reported PID. The local checks for these cases run with `just test-x11-pidns`.
