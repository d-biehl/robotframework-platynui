# Design

## Context

See proposal.md for the motivation and `specs/` for the requirements.

**How the Wayland window manager works today (verified in the code):**
- `WaylandWindowManager` forwards every operation to a `CompositorBackend` (`crates/platform-linux-wayland/src/window_manager/mod.rs:13`). `backend()` (`:107`) returns the PlatynUI control-socket backend (`:36`) and refuses every other compositor with `unsupported_compositor` (`capabilities.rs:142`).
- The PlatynUI backend finds a node's window in `resolve_window` (`window_manager/platynui_ipc.rs:72`) with `match_best_window` (`:267`). It narrows the candidates by process ID, then exact title, then a title that contains the node's name, then a unique size within `SIZE_MATCH_TOLERANCE` (`:265`), and returns nothing rather than guess.
- `WindowGeometry` (`crates/core/src/platform/window_manager.rs:101`) carries the visible `bounds` and the `surface_origin`. The AT-SPI provider places a window's elements from the surface origin. GTK 4 counts from the visible frame and every other toolkit from the surface (`dev-docs/platform-linux-wayland.md` §3).
- `CompositorType` (`capabilities.rs:38`) has no niri or cosmic variant. `decide_compositor` (`:97`) takes a PlatynUI handshake first, then a PlatynUI desktop marker, then the peer binary (`classify_binary_name`, `:277`), then the desktop (`classify_desktop`, `:299`).
- Input backends are chosen per compositor type (`input/mod.rs:152`): Sway, Hyprland, wlroots and unknown compositors try EIS, then the portal, then virtual input. The virtual-input backend accepts a compositor that offers only one of the two protocols (`input/virtual_input.rs:188`), and then fails the missing half per call with "virtual pointer" unavailable.
- The desktop module already reads output names and logical positions from `wl_output` and `zxdg_output_v1` (`desktop/output_info.rs:24-27`).

**What the two compositors offer (checked on 2026-11-26 against niri 26.04 and cosmic-comp 1.11):**
- **niri:**
  - IPC socket at `$NIRI_SOCKET`, one JSON request and one JSON reply per connection.
  - `Windows` lists `id`, `title`, `app_id`, `pid`, `workspace_id`, `is_focused`, `is_floating` and, since 25.08, a `layout` with `tile_pos_in_workspace_view` (absent for windows outside the view), `window_offset_in_tile`, `window_size` and `tile_size`.
  - Actions taking a window `id`: focus, close, minimize, unminimize, maximize to edges, fullscreen, set width and height, and move a floating window.
  - Server modules for `zwlr_virtual_pointer_v1` and `zwp_virtual_keyboard_v1`; no libei.
- **cosmic-comp:**
  - No IPC socket.
  - `ext_foreign_toplevel_list_v1` plus `zcosmic_toplevel_info_v1` (version 3). Its `get_cosmic_toplevel` (version 2 and later) attaches a cosmic handle to each foreign toplevel. That handle sends `title`, `app_id`, `state` and `geometry(output, x, y, width, height)` relative to an output, and no process ID.
  - `zcosmic_toplevel_manager_v1` (version 4): close, activate, set/unset maximized and minimized, fullscreen, move to workspace; no move or resize.
  - `zwp_virtual_keyboard_v1`, no virtual pointer, and libei only behind the RemoteDesktop portal (`cosmic.portal`).
- **Licensing:** `niri-ipc` is GPL-3.0-or-later and `cosmic-protocols` is GPL-3.0-only. The two cosmic XML files themselves carry an MIT-style permission notice.

**Assumed, not yet verified:** how niri's `Windows` reply shows a minimized or maximized window, and which coordinate space `move-floating-window` takes. Tasks 1.1 and 1.2 record the real replies before the backends depend on them.

## Goals / Non-Goals

**Goals:**
- Full window management under niri and cosmic-comp, as far as each compositor allows, behind the existing `WindowManager` operations.
- One matching rule for all Wayland backends that never guesses.
- Tests that run in CI without either compositor, plus local lanes against the real ones.

**Non-Goals:**
- KWin, Sway, Hyprland or Mutter backends. The backend dispatch is built so they can join later.
- Fixing the GTK 3 shadow offset or placing GTK menu items under niri and cosmic-comp (D3, D4).
- Running the niri or cosmic lanes in CI.
- Highlights; they are change `add-layer-shell-highlights`.

## Decisions

### D1 — One backend per compositor behind the existing trait, one shared matching rule

`backend()` dispatches `CompositorType::Niri` to a `niri_ipc` backend and `CompositorType::Cosmic` to a `cosmic_toplevel` backend. Both implement `CompositorBackend` unchanged; an operation a compositor cannot perform returns `CapabilityUnavailable` naming the operation and the compositor.

`match_best_window` moves into a shared module and works over a small view of a candidate: optional process ID, title and size. The PlatynUI backend, niri and cosmic all use it. Under cosmic the process ID is always `None`, so the existing rule drops straight to title and size. Its "single window left" shortcut stays limited to the case without a process ID and exactly one window, which keeps cosmic from attaching a node to the only window of a different app only when that is genuinely the only window.

A resolved window keeps the compositor's stable ID (niri's window `id`, the cosmic foreign-toplevel handle) in the existing `WindowSelector` cache. An operation on an ID the compositor no longer lists fails with "window no longer exists", and it never re-matches to another window.
- *Rejected:* a copy of the matching rule per backend. The three would drift, as the two readiness paths in the compositor once did.

### D2 — niri: own serde types over the IPC socket

The backend opens `$NIRI_SOCKET` per request, writes one JSON line and reads one reply line, like `niri msg`. It uses its own `serde` types with only the fields it needs and `#[serde(default)]` for everything newer than 25.08, so a newer niri with extra fields keeps working.
- **Bounds:** the logical position of the workspace's output (from `Workspaces` and `Outputs`) + `tile_pos_in_workspace_view` + `window_offset_in_tile`, with `window_size`.
- **No position:** a window without `tile_pos_in_workspace_view` is not on screen, and its geometry read fails with unavailable. A reply without `layout` (niri before 25.08) fails with an error that names the niri version needed.
- **Operations:**
  - activate: `FocusWindow`, or `UnminimizeWindow` for a minimized window;
  - close: `CloseWindow`;
  - minimize: `MinimizeWindow`;
  - maximize: `MaximizeWindowToEdges`;
  - restore: per the state recorded in task 1.1;
  - resize: `SetWindowWidth` and `SetWindowHeight` with fixed values;
  - move: `MoveFloatingWindow` only when `is_floating`, otherwise unavailable without sending a request.
- *Rejected:* the `niri-ipc` crate. It is GPL-3.0-or-later, and it is versioned in lockstep with niri, so one crate version would fit only one niri release.

### D3 — The surface origin is the visible frame's corner

Neither compositor reports where a client's surface starts, only the window geometry. The backends report `WindowGeometry::from_bounds(bounds)`, so the surface origin equals the bounds' top-left corner.
- **Where that is right:**
  - server-side-decorated toolkits (egui, Qt under niri, which offers xdg-decoration);
  - GTK 4, which counts from the visible frame anyway.
- **Where it is wrong:** GTK 3 with client-side decorations counts from its surface, which includes the shadow it draws. Its elements land off by the shadow inset (26, 23 px on the default theme). This limit goes into `dev-docs/platform-linux-wayland.md` §3 and §6.
- *Rejected:* estimating the inset from a screenshot of the toplevel. The captured buffer size gives the total inset but not how it splits between left and right or top and bottom.
- *Rejected:* refusing GTK 3 windows. Everything except element placement still works for them, including window management and keyboard input.

### D4 — Popups are unavailable, not empty

Neither compositor lists a client's popups, and the AT-SPI provider places GTK menu items only from such a list. `popups()` keeps the trait's default, an unavailable error, so the provider's existing fallback applies, and a caller can tell "no list" from "no popup open". Items of GTK 3 and GTK 4 menus therefore cannot be placed under niri and cosmic-comp. The docs say so; the lanes exclude the suites that click menu items (D7).

### D5 — cosmic: a client connection that keeps the toplevel list

The cosmic backend keeps one Wayland client connection, owned by a background thread like the screenshot module's. On that connection it:
- binds `ext_foreign_toplevel_list_v1`, `zcosmic_toplevel_info_v1` (version 2 or later), `zcosmic_toplevel_manager_v1`, and the outputs with `zxdg_output_v1`;
- attaches a cosmic handle to every foreign toplevel;
- keeps title, app ID, state and the latest geometry per toplevel.

Each operation reads that list under a lock after a roundtrip, so a request sees the compositor's current state. Bounds are the geometry event's rectangle plus its output's logical position.

The two protocol XML files are vendored under `crates/platform-linux-wayland/protocols/` with their copyright and permission notices, and bindings are generated with `wayland-scanner`'s macros. `wayland-scanner` is already in the dependency tree through `wayland-protocols`.

A compositor without `zcosmic_toplevel_info_v1` version 2 fails every window operation with an error naming the protocol version needed.
- *Rejected:* the `cosmic-protocols` crate (GPL-3.0-only).
- *Rejected:* a new connection per operation. Toplevels and their geometry arrive as events after binding, so every call would pay a full roundtrip cycle and still race with window changes.

### D6 — Identification and input

`classify_binary_name` maps `niri` to `Niri` and `cosmic-comp` to `Cosmic`. `classify_desktop` maps `niri` and `COSMIC` (case-insensitive) to the same. The order of evidence in `decide_compositor` stays as it is.

The input candidates become:
- niri: `[VirtualInput, Portal]`, because niri has no EIS;
- cosmic: `[Portal, VirtualInput]`. The virtual-input fallback drives the keyboard only, and pointer calls fail with the existing "virtual pointer" unavailable error.

### D7 — Local lanes, like the Windows VM lane

- **Start scripts.** `scripts/startniri.sh` and `scripts/startcosmic.sh` follow `scripts/startsway.sh`: a private `XDG_RUNTIME_DIR`, D-Bus and AT-SPI bus, and the compositor nested in the developer's session through its winit backend. niri gets a generated config with `prefer-no-csd` and animations off.
- **Profiles.** `real-niri` and `real-cosmic` in `robot.toml` inherit `real-wayland`. They also exclude a new tag `compositor:platynui`, for suites that need the PlatynUI compositor's control socket or popup list.
- **Recipes.** `just test-acceptance-niri` and `just test-acceptance-cosmic` start them.
- **Not in CI.** CI installs neither compositor, and a nested session needs a parent display.

The RemoteDesktop portal under cosmic normally asks for consent. Task 1.3 checks whether a nested session can grant it without a dialog. If not, the cosmic lane excludes pointer-driven suites and covers window management and keyboard input.

## Risks / Trade-offs

- **[niri's IPC changes between releases]** → Tolerant serde types (D2), fixtures recorded from 26.04, and an explicit error for a reply without `layout`.
- **[cosmic windows with equal titles and sizes cannot be told apart]** → The shared rule fails with a message naming what it compared (spec). Tests that open two equal windows run only in the niri lane.
- **[GTK 3 elements are off by their shadow under both compositors]** → Documented limit (D3). The lanes use egui and GTK 4 for pointer tests, and GTK 3 only for window management.
- **[GTK menu items cannot be placed]** → Documented limit (D4), and the suites are excluded by tag.
- **[A background Wayland connection for cosmic can die with the compositor]** → Every operation checks the connection and fails with an initialization error naming cosmic-comp. It does not report stale geometry.
- **[The local lanes run only on machines with niri or cosmic-comp installed]** → The recipes check for the binary and fail with a message naming it. `CONTRIBUTING.md` lists the packages.

## Migration Plan

- **Kind of change:** additive. Under niri and cosmic-comp, operations that failed as "unsupported compositor" start to work or fail with a more precise error. Nothing changes under the PlatynUI compositor or other compositors.
- **Native rebuild:** yes, the platform crate is part of the native build (`just build-native`). No Python or Robot Framework API changes.
- **Rollback:** revert the change. No configuration or persistent state changes. The new tag and profiles are additive in `robot.toml`.

## Open Questions

- How niri's `Windows` reply marks minimized and maximized windows, and which coordinates `move-floating-window` takes. Task 1.1 records the replies; the mapping in D2 follows them without changing the approach.
- Whether the cosmic RemoteDesktop portal can be authorized in a nested session without a dialog. Task 1.3 decides between the two lane scopes D7 already names.
