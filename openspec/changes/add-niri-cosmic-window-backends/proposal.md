# Proposal

## Why

Under Wayland, PlatynUI can only place elements, click them and manage windows under its own test compositor. Every other compositor is "unsupported": the Wayland window manager has a single backend, the PlatynUI control socket, and `backend()` refuses every other compositor. Without a window's position on the desktop, the AT-SPI provider cannot place a window's elements, so pointer actions and highlights do not work, and window operations (activate, close, minimize, maximize, restore) fail.

Two desktop compositors now expose enough to change that, as checked on 2026-11-26 against niri 26.04 and cosmic-comp 1.11:
- **niri** has a JSON IPC socket (`$NIRI_SOCKET`). Its window list carries each window's process ID, title and, since 25.08, its position and size. Its actions focus, close, minimize, restore, maximize and resize windows by ID, and move floating windows.
- **cosmic-comp** has no IPC socket. Its own Wayland protocols instead report each window's geometry (`zcosmic_toplevel_info_v1`, version 2 and later) and manage windows (`zcosmic_toplevel_manager_v1`: activate, close, minimize, maximize, fullscreen).

Both are Smithay-based desktops that are gaining users (cosmic ships with Pop!_OS 24.04 LTS). They are the first foreign compositors on which PlatynUI can offer window management.

## What Changes

- **Identify niri and cosmic-comp.** The Wayland backend recognises both: from the Wayland socket's peer binary (`niri`, `cosmic-comp`) and from `XDG_CURRENT_DESKTOP` (`niri`, `COSMIC`). It records the identification like every other one.
- **A niri window backend** over the niri IPC socket:
  - finding a node's window by process ID, then title, then a unique size;
  - its bounds and whether it is active;
  - its state (normal, minimized, maximized);
  - activate, close, minimize, maximize and restore;
  - resize;
  - move, for floating windows only, because niri tiles every other window. Moving a tiled window fails with an explicit unavailable error.
- **A cosmic-comp window backend** over `ext-foreign-toplevel-list-v1`, `zcosmic_toplevel_info_v1` and `zcosmic_toplevel_manager_v1`:
  - finding a node's window by title, then a unique size, because cosmic-comp reports no process ID; it never guesses between equal candidates;
  - its bounds, whether it is active, and its state;
  - activate, close, minimize, maximize and restore.

  Moving and resizing fail with an explicit unavailable error, because cosmic-comp offers neither.
- **Input under niri** uses the virtual pointer and virtual keyboard niri offers, which PlatynUI's existing virtual-input backend already drives.
- **Input under cosmic-comp** tries the RemoteDesktop portal first, the only pointer path cosmic-comp offers. When the portal is unavailable, it falls back to the virtual-input backend, which there has only the keyboard, so pointer actions fail with an explicit unavailable error.
- **Local acceptance lanes for niri and cosmic-comp.** New Robot Framework profiles start each compositor nested in the developer's session and run the window management and pointer suites against it. Like the Windows VM lane, they run locally, not in CI. The backends' parsing and decisions are unit-tested in CI against recorded IPC replies and protocol events.
- **Documented limits** (design D3, D4). Both compositors report a window's visible frame, not where its surface starts:
  - A toolkit that counts element positions from the surface while drawing a shadow around its window is off by that shadow. That is GTK 3 with client-side decorations.
  - Neither compositor lists popups, so items of GTK menus cannot be placed.

  Both limits are stated in the docs and the support matrix, not hidden.
- **No new GPL code.** `niri-ipc` and `cosmic-protocols` are GPL-licensed, and PlatynUI is Apache-2.0. The niri backend parses the IPC JSON with its own types. The cosmic backend generates its bindings from the two protocol XML files, which carry a permissive (MIT-style) license and are vendored with their notices.
- **Not BREAKING.** Under niri and cosmic-comp, operations that failed with "unsupported compositor" start to work or fail with a more precise unavailable error. Nothing changes under the PlatynUI compositor or other compositors.

## Capabilities

### New Capabilities

- `wayland-window-management`: window management under Wayland compositors other than the PlatynUI compositor, through each compositor's own mechanism. It covers how a node's window is found, which geometry it reports, which operations each compositor supports, and how unsupported operations fail. It starts with niri and cosmic-comp, and KWin, Sway and Hyprland can join it later.

### Modified Capabilities

- `wayland-compositor-detection`: the backend also identifies niri and cosmic-comp, from the peer binary and from `XDG_CURRENT_DESKTOP`, and gates their capabilities like those of any identified compositor.

## Impact

- **Rust, `crates/platform-linux-wayland`:**
  - `capabilities.rs`: `CompositorType::Niri` and `CompositorType::Cosmic`, with classification by peer binary and desktop.
  - `window_manager/`: a `niri_ipc.rs` and a `cosmic_toplevel.rs` backend, dispatched from `backend()`. The matching rule of `platynui_ipc.rs` is shared rather than copied.
  - `input/mod.rs`: input candidates for niri (virtual input) and cosmic-comp (portal, then virtual input).
  - The protocol XML files with their notices, and generated bindings through `wayland-scanner`, which is already in the dependency tree.
- **Rust, `crates/core`:** no trait change. The backends implement the existing `WindowManager` operations and return `PlatformError::CapabilityUnavailable` where a compositor cannot act.
- **Tests:**
  - CI unit tests against recorded niri replies and cosmic event sequences.
  - New local Robot Framework profiles, `real-niri` and `real-cosmic`, with a session wrapper that starts the nested compositor. The existing suites they run are chosen to be compositor-independent.
- **Docs:** `dev-docs/platform-linux-wayland.md` §6 (bounds sources, limits), §10 (support matrix) and §15 (roadmap); `dev-docs/architecture.md` §10 platform snapshot; `CONTRIBUTING.md` for running the local lanes.
- **No native Python rebuild beyond the usual** `just build-native`; the Python and Robot Framework surfaces do not change.
- **Platforms:** Linux Wayland only. The niri backend needs niri 25.08 or later for window positions. The cosmic backend needs `zcosmic_toplevel_info_v1` version 2 or later.
