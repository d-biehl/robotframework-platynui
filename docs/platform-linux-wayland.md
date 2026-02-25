# Platform Linux Wayland – Feasibility Study

> **Status:** Research / Planning  
> **Date:** 2026-02-25  
> **Crate:** `crates/platform-linux-wayland` (planned)

This document captures the current state of Wayland protocol support, compositor landscape, Rust ecosystem, and implementation strategies for the `platform-linux-wayland` crate. It serves as both a reference and a decision guide for implementation.

---

## Table of Contents

1. [Overview & Motivation](#1-overview--motivation)
2. [Platform Trait Requirements](#2-platform-trait-requirements)
3. [AT-SPI under Wayland: Capabilities & Limitations](#3-at-spi-under-wayland-capabilities--limitations)
4. [Input Injection](#4-input-injection)
5. [Screenshots & Screen Capture](#5-screenshots--screen-capture)
6. [Window Management](#6-window-management)
7. [Highlight Overlays](#7-highlight-overlays)
8. [Desktop Info (Monitors)](#8-desktop-info-monitors)
9. [Wayland Protocol Landscape](#9-wayland-protocol-landscape)
10. [Compositor Support Matrix](#10-compositor-support-matrix)
11. [Compositors for CI / Headless Testing](#11-compositors-for-ci--headless-testing)
12. [Rust Crate Ecosystem](#12-rust-crate-ecosystem)
13. [Detecting Wayland vs X11 per Application](#13-detecting-wayland-vs-x11-per-application)
14. [Implementation Architecture](#14-implementation-architecture)
15. [Prioritized Roadmap](#15-prioritized-roadmap)
16. [Open Questions](#16-open-questions)
17. [References](#17-references)

---

## 1. Overview & Motivation

PlatynUI's X11 platform implementation (`platform-linux-x11`) relies on X11-specific APIs (XTest, EWMH, XGetImage, override-redirect windows) that are unavailable under Wayland's security model. To support Wayland-native automation, a dedicated `platform-linux-wayland` crate is needed.

Key challenges under Wayland:
- **No global coordinate space** — clients don't know their absolute screen position.
- **No client-to-client input injection** — by design, for security.
- **No equivalent to EWMH** — window management is compositor-specific.
- **Protocol fragmentation** — critical protocols are either unstable, compositor-specific, or still being standardized.

---

## 2. Platform Trait Requirements

The following traits from `platynui-core` must be implemented (see [crates/core/src/platform/](../crates/core/src/platform/)):

| Trait | X11 Mechanism | Wayland Equivalent | Feasibility |
|---|---|---|---|
| `PlatformModule` | `x11rb` connection | `wayland-client` connection | ✅ Straightforward |
| `PointerDevice` | XTest `XTestFakeMotionEvent/ButtonEvent` | uinput / `zwlr_virtual_pointer_v1` / libei | ✅ Multiple options |
| `KeyboardDevice` | XTest `XTestFakeKeyEvent` + XKB | uinput / `zwp_virtual_keyboard_v1` / libei | ✅ Multiple options |
| `ScreenshotProvider` | `XGetImage` | `ext-image-copy-capture-v1` / `wlr-screencopy` | ✅ Well-supported |
| `HighlightProvider` | Override-redirect X11 window | `wlr-layer-shell-v1` / `ext-layer-shell-v1` | ⚠️ Compositor-dependent |
| `DesktopInfoProvider` | XRandR | `wl_output` (core protocol) | ✅ Built-in |
| `WindowManager` | EWMH (`_NET_WM_*`) | `wlr-foreign-toplevel-management` | ⚠️ Partial, no bounds/move/resize |

---

## 3. AT-SPI under Wayland: Capabilities & Limitations

[AT-SPI2](https://gitlab.gnome.org/GNOME/at-spi2-core) works under Wayland for accessibility tree traversal, role queries, actions, and text content. However, several capabilities are degraded:

### What works
- Full accessibility tree traversal via D-Bus
- Component roles, names, states, actions
- Text content, selection, value queries
- Process identification via PID

### What doesn't work
- **`Component::GetExtents(SCREEN)`** returns `(0, 0)` for window-relative-to-screen — Wayland clients don't know their absolute screen position.
- **`Component::GetExtents(WINDOW)`** still works (position relative to the window's own origin).
- **`Component::GetPosition`** same limitation as `GetExtents`.
- **`Component::ScrollTo`** and **`Component::GrabFocus`** work at toolkit level.

**Impact on PlatynUI:** The `WindowManager::window_bounds()` and `WindowManager::move_to()` / `resize()` methods cannot use AT-SPI data for screen-absolute coordinates under Wayland. The `ActivationPoint` calculation in the runtime must fall back to window-relative coordinates plus compositor-provided window position (if available via foreign-toplevel protocols).

---

## 4. Input Injection

Three viable approaches exist, each with distinct tradeoffs:

### 4.1. uinput / evdev (Kernel-Level)

Injects input events at the Linux kernel level via `/dev/uinput`, bypassing the Wayland compositor entirely. This is the approach used by [ydotool](https://github.com/ReimuNotMoe/ydotool).

| Aspect | Detail |
|---|---|
| **Mechanism** | Create virtual input device via `/dev/uinput`, write `EV_KEY`, `EV_REL`, `EV_ABS` events |
| **Compositor support** | Universal — works on any Wayland compositor (and X11) |
| **Permissions** | Requires access to `/dev/uinput` (typically `root` or `input` group) |
| **Coordinate system** | Absolute coordinates via `ABS_X`/`ABS_Y` (needs screen resolution for mapping) |
| **Limitations** | Cannot target specific windows; operates on the focused surface |
| **Rust crate** | [`evdev`](https://crates.io/crates/evdev) (pure Rust, well-maintained) |
| **License concern** | None (evdev crate is MIT; ydotool is AGPL but we'd use evdev directly) |

**ydotool architecture reference:**
- Daemon (`ydotoold`) holds the uinput device open
- Client sends commands via Unix socket
- We would implement the uinput device directly, no daemon needed

### 4.2. Wayland Protocols (Compositor-Level)

Protocol-based input injection through the compositor. Requires compositor support.

| Protocol | Purpose | Compositors |
|---|---|---|
| [`zwlr_virtual_pointer_v1`](https://wayland.app/protocols/wlr-virtual-pointer-unstable-v1) | Virtual pointer device (motion, buttons, axis) | Sway, wlroots-based, Mir, Weston (partial) |
| [`zwp_virtual_keyboard_v1`](https://wayland.app/protocols/virtual-keyboard-unstable-v1) | Virtual keyboard (keymap + key events) | Sway, wlroots-based, Mir, Weston |

- **Advantages:** Clean integration, no special permissions, compositor-aware.
- **Disadvantages:** wlroots/Mir only. Not supported by Mutter (GNOME) or KWin (KDE).

### 4.3. libei (Input Emulation Interface)

[libei](https://gitlab.freedesktop.org/libinput/libei) is a freedesktop.org standard for input emulation, designed specifically for Wayland. Initiated by Red Hat / Peter Hutterer (libinput maintainer).

| Aspect | Detail |
|---|---|
| **Mechanism** | Client connects to EIS (Emulated Input Server) in the compositor via a portal or direct socket |
| **Compositor support** | Mutter (GNOME 45+), KWin (KDE 6.1+), Sway (via [sway-libei fork](https://github.com/tytan652/sway-libei)), Weston (planned) |
| **Portal integration** | [`org.freedesktop.portal.InputCapture`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.InputCapture.html) and [`org.freedesktop.portal.RemoteDesktop`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html) |
| **Permissions** | Via xdg-desktop-portal (user consent dialog or test token) |
| **Rust crate** | [`reis`](https://crates.io/crates/reis) (Rust bindings for libei) |
| **Maturity** | Actively developed, API stabilizing; best long-term bet |

### 4.4. Recommendation

| Approach | Universal | Clean API | No Root | Long-term |
|---|---|---|---|---|
| **uinput** | ✅ | ❌ | ❌ | ⚠️ |
| **wlr-protocols** | ❌ | ✅ | ✅ | ⚠️ |
| **libei** | ⚠️ (growing) | ✅ | ✅ | ✅ |

**Recommended strategy:** Start with **uinput** for universal coverage (CI headless environments typically run as root or have `/dev/uinput` access), then add **libei** as the preferred path for desktop sessions. The wlr-protocols can serve as a middle layer for wlroots-based compositors.

---

## 5. Screenshots & Screen Capture

### 5.1. ext-image-copy-capture-v1 (Standard)

The [ext-image-copy-capture-v1](https://wayland.app/protocols/ext-image-copy-capture-v1) protocol is the emerging standard for screen capture under Wayland.

- **Source selection:** via [ext-image-capture-source-v1](https://wayland.app/protocols/ext-image-capture-source-v1) — supports output (monitor), toplevel (window), or workspace capture.
- **Compositor support:** KWin, Sway (0.41+), Mir, Hyprland, cosmic-comp, niri, Weston (partial).
- **Buffer types:** `wl_shm` (CPU-accessible), DMA-BUF (GPU-accessible).

### 5.2. wlr-screencopy-unstable-v1 (Legacy)

The [wlr-screencopy-unstable-v1](https://wayland.app/protocols/wlr-screencopy-unstable-v1) protocol is the older wlroots-specific approach.

- **Compositor support:** Sway, Hyprland, wlroots-based compositors.
- **Status:** Being superseded by `ext-image-copy-capture-v1`; Sway 0.41+ supports both.
- **Limitation:** Output-level only (full monitor), no per-window capture.

### 5.3. xdg-desktop-portal Screenshot

The [org.freedesktop.portal.Screenshot](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html) D-Bus portal provides a compositor-agnostic screenshot API.

- **Compositor support:** Any compositor with an xdg-desktop-portal backend (GNOME, KDE, Sway, Hyprland).
- **Limitation:** Triggers user consent dialog (unless test token is used); returns a file path, not a buffer.
- **Use case:** Fallback for compositors without direct protocol support.

### 5.4. Recommendation

Use **ext-image-copy-capture-v1** as primary (widest support, standard), fall back to **wlr-screencopy** for older wlroots, and **portal** as last resort.

---

## 6. Window Management

This is the most challenging area. Wayland has **no equivalent to X11's EWMH** for programmatic window management.

### 6.1. wlr-foreign-toplevel-management-unstable-v1

The [wlr-foreign-toplevel-management-unstable-v1](https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1) protocol provides limited window management.

**Supported operations:**
- `activate` — bring window to front
- `close` — close window
- `set_minimized` / `unset_minimized`
- `set_maximized` / `unset_maximized`
- `set_fullscreen` / `unset_fullscreen`
- List all toplevel windows with `title`, `app_id`, PID

**NOT supported:**
- Window bounds (position + size) — ❌
- Move window — ❌
- Resize window — ❌
- Window stacking order — ❌

**Compositor support:** Sway, Hyprland, wlroots-based, Mir, cosmic-comp, niri, labwc.

### 6.2. ext-foreign-toplevel-list-v1 (Read-Only)

The [ext-foreign-toplevel-list-v1](https://wayland.app/protocols/ext-foreign-toplevel-list-v1) protocol provides a read-only list of toplevel windows.

- **Information:** `title`, `app_id`, `identifier`
- **No management operations** — list and observe only.
- **Compositor support:** KWin, Sway, Hyprland, cosmic-comp, niri.

### 6.3. Compositor-Specific IPC

Some compositors expose their own IPC for window management:

| Compositor | IPC | Capabilities |
|---|---|---|
| **Sway** | [swaymsg / i3 IPC](https://github.com/swaywm/sway/wiki) | Full: move, resize, focus, layout, scratchpad |
| **Hyprland** | [hyprctl](https://wiki.hyprland.org/Configuring/Using-hyprctl/) | Full: move, resize, focus, workspaces |
| **KWin** | [KWin scripting / D-Bus](https://develop.kde.org/docs/plasma/kwin/api/) | Full via D-Bus or JavaScript scripting |
| **Mutter** | Limited D-Bus | Minimal |

### 6.4. Impact on `WindowManager` Trait

| Method | Feasibility | Mechanism |
|---|---|---|
| `resolve_window()` | ✅ | `wlr-foreign-toplevel` + AT-SPI PID matching |
| `is_active()` | ✅ | `wlr-foreign-toplevel` state events |
| `activate()` | ✅ | `wlr-foreign-toplevel` activate request |
| `close()` | ✅ | `wlr-foreign-toplevel` close request |
| `minimize()` / `restore()` | ✅ | `wlr-foreign-toplevel` set/unset minimized |
| `maximize()` | ✅ | `wlr-foreign-toplevel` set/unset maximized |
| `window_bounds()` | ❌ | **Not available** — must return `None` or use compositor IPC |
| `move_to()` | ❌ | **Not available** — compositor IPC only |
| `resize()` | ❌ | **Not available** — compositor IPC only |

---

## 7. Highlight Overlays

Under X11, PlatynUI uses override-redirect windows to draw highlight rectangles over UI elements. Wayland equivalents:

### 7.1. wlr-layer-shell-unstable-v1

The [wlr-layer-shell-unstable-v1](https://wayland.app/protocols/wlr-layer-shell-unstable-v1) protocol allows creating overlay surfaces.

- **Layers:** background, bottom, top, **overlay** (highest, above all windows).
- **Features:** Anchoring to edges, exclusive zones, keyboard interactivity control.
- **Compositor support:** Sway, Hyprland, wlroots-based, Mir, cosmic-comp, niri, labwc.
- **Not supported by:** Mutter (GNOME) — uses its own shell extensions, KWin — partial via `ext-layer-shell-v1`.

### 7.2. ext-layer-shell-v1

The [ext-layer-shell-v1](https://wayland.app/protocols/ext-layer-shell-v1) protocol is the standardized version.

- **Compositor support:** KWin (KDE 6.x), cosmic-comp, niri.
- **Status:** Newer standard; growing adoption.

### 7.3. Recommendation

Use **wlr-layer-shell** for wlroots-based compositors, **ext-layer-shell** for KWin, and fall back to a separate Wayland client window (less ideal) for Mutter/GNOME.

**Limitation:** Without global window coordinates, placing a highlight overlay at the correct screen position requires knowing the target window's screen-space bounds — which is unavailable via standard Wayland protocols. Possible workarounds:
- Use `ext-image-copy-capture` to screenshot + AT-SPI window-relative coords to compute overlay position.
- Use compositor IPC (Sway/Hyprland) for window geometry.
- Accept the limitation and only highlight within the captured screenshot image (software overlay).

---

## 8. Desktop Info (Monitors)

Monitor information is available via the core Wayland protocol's [`wl_output`](https://wayland.app/protocols/wayland#wl_output) interface:

- Physical size (mm), make, model, subpixel arrangement
- Current mode (resolution, refresh rate)
- Transform (rotation)
- Scale factor
- Position in compositor coordinate space (since `wl_output` v4)

Additionally, [`xdg_output_manager`](https://wayland.app/protocols/xdg-output-unstable-v1) provides logical coordinates and size.

**Feasibility:** ✅ Fully supported on all compositors — this is core protocol.

---

## 9. Wayland Protocol Landscape

### 9.1. Stable / Core Protocols

| Protocol | Purpose | Part of |
|---|---|---|
| [`wl_output`](https://wayland.app/protocols/wayland#wl_output) | Monitor info | Core |
| [`wl_seat`](https://wayland.app/protocols/wayland#wl_seat) | Input seat (capabilities) | Core |
| [`xdg_shell`](https://wayland.app/protocols/xdg-shell) | Window management (client-side) | wayland-protocols stable |
| [`xdg_output`](https://wayland.app/protocols/xdg-output-unstable-v1) | Logical monitor info | wayland-protocols unstable (widely adopted) |

### 9.2. Unstable / Staging Protocols (wayland-protocols)

| Protocol | Purpose | Status |
|---|---|---|
| [`ext-image-copy-capture-v1`](https://wayland.app/protocols/ext-image-copy-capture-v1) | Screenshot / screen capture | Staging — wide adoption |
| [`ext-image-capture-source-v1`](https://wayland.app/protocols/ext-image-capture-source-v1) | Capture source selection | Staging — pairs with above |
| [`ext-foreign-toplevel-list-v1`](https://wayland.app/protocols/ext-foreign-toplevel-list-v1) | Read-only toplevel list | Staging |
| [`ext-layer-shell-v1`](https://wayland.app/protocols/ext-layer-shell-v1) | Layer surfaces (overlay) | Staging |
| [`zwp_virtual_keyboard_v1`](https://wayland.app/protocols/virtual-keyboard-unstable-v1) | Virtual keyboard input | Unstable |

### 9.3. wlroots-Specific Protocols

| Protocol | Purpose | Compositors |
|---|---|---|
| [`wlr-foreign-toplevel-management-v1`](https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1) | Window management (activate, close, min/max) | wlroots-based, Mir |
| [`wlr-layer-shell-v1`](https://wayland.app/protocols/wlr-layer-shell-unstable-v1) | Overlay/panel surfaces | wlroots-based, Mir |
| [`wlr-screencopy-v1`](https://wayland.app/protocols/wlr-screencopy-unstable-v1) | Screen capture (legacy) | wlroots-based |
| [`zwlr_virtual_pointer_v1`](https://wayland.app/protocols/wlr-virtual-pointer-unstable-v1) | Virtual pointer input | wlroots-based, Mir |

### 9.4. Desktop Portal APIs (D-Bus)

| Portal | Purpose | Link |
|---|---|---|
| `org.freedesktop.portal.Screenshot` | Compositor-agnostic screenshots | [Docs](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html) |
| `org.freedesktop.portal.RemoteDesktop` | Remote input + screen sharing | [Docs](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html) |
| `org.freedesktop.portal.InputCapture` | Input capture (libei) | [Docs](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.InputCapture.html) |

---

## 10. Compositor Support Matrix

Support for protocols relevant to UI automation (as of early 2026):

| Protocol | Sway | Hyprland | KWin | Mutter | Weston | labwc | niri | Mir | cosmic |
|---|---|---|---|---|---|---|---|---|---|
| `ext-image-copy-capture-v1` | ✅ (0.41+) | ✅ | ✅ | ❌ | ⚠️ | ❌ | ✅ | ✅ | ✅ |
| `wlr-screencopy-v1` | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ | ❌ | ✅ |
| `wlr-foreign-toplevel-mgmt` | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ |
| `ext-foreign-toplevel-list` | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ | ✅ |
| `wlr-layer-shell-v1` | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ |
| `ext-layer-shell-v1` | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ | ✅ |
| `zwlr_virtual_pointer_v1` | ✅ | ✅ | ❌ | ❌ | ⚠️ | ✅ | ❌ | ✅ | ❌ |
| `zwp_virtual_keyboard_v1` | ✅ | ✅ | ❌ | ❌ | ✅ | ✅ | ❌ | ✅ | ❌ |
| libei (EIS) | [Fork](https://github.com/tytan652/sway-libei) | ❌ | ✅ (6.1+) | ✅ (45+) | ❌ | ❌ | ❌ | ❌ | ❌ |

**Key insight:** No single protocol set covers all compositors. Mutter (GNOME) is the most restrictive — it only supports libei and portals.

---

## 11. Compositors for CI / Headless Testing

For automated testing in CI, a headless-capable compositor is essential.

### 11.1. Weston (Current Choice)

[Weston](https://gitlab.freedesktop.org/wayland/weston) is the Wayland reference compositor.

- **Backends:** `headless-backend.so` (no display), `wayland-backend.so` (nested), `x11-backend.so` (in X11 window), `drm-backend.so` (hardware).
- **Already configured:** see [`scripts/startwaylandsession.sh`](../scripts/startwaylandsession.sh)
- **Protocol support:** Limited — no `wlr-foreign-toplevel`, no `wlr-layer-shell`, no `ext-image-copy-capture`.
- **Best for:** Basic Wayland session setup, AT-SPI testing, application launching.

### 11.2. Sway

[Sway](https://github.com/swaywm/sway) is a wlroots-based tiling compositor with excellent protocol support.

- **Headless mode:** `WLR_BACKENDS=headless sway` (via wlroots backend).
- **Virtual outputs:** `swaymsg create_output` to add virtual monitors.
- **Protocol support:** Richest of any compositor — supports nearly all wlr and ext protocols.
- **IPC:** Full window management via [i3-compatible IPC](https://i3wm.org/docs/ipc.html).
- **Best for:** Comprehensive testing with full window management.

### 11.3. Mutter (GNOME)

[Mutter](https://gitlab.gnome.org/GNOME/mutter) is GNOME's compositor.

- **Headless mode:** `mutter --headless --virtual-monitor 1920x1080` (GNOME 45+).
- **Protocol support:** Minimal custom protocols; relies on portals and libei.
- **Best for:** Testing GNOME-specific applications, portal-based workflows.

### 11.4. labwc

[labwc](https://github.com/labwc/labwc) is a lightweight wlroots-based stacking compositor.

- **Headless mode:** Via wlroots backends (`WLR_BACKENDS=headless`).
- **Protocol support:** Good wlr protocol coverage, lightweight footprint.
- **Best for:** Lightweight CI testing environments.

### 11.5. Cage

[cage](https://github.com/cage-kiosk/cage) is a kiosk compositor (runs a single application fullscreen).

- **Headless mode:** Via wlroots backends.
- **Use case:** Single-app testing scenarios.

### 11.6. Recommendation

| Use Case | Compositor | Reason |
|---|---|---|
| **CI primary** | Sway (headless) | Best protocol coverage, IPC for window mgmt |
| **CI simple** | Weston (headless) | Already configured, reference compositor |
| **GNOME testing** | Mutter (headless) | Portal/libei testing |
| **Lightweight CI** | labwc (headless) | Simple, fast startup |

---

## 12. Rust Crate Ecosystem

### 12.1. Wayland Client

| Crate | Version | Purpose | Link |
|---|---|---|---|
| [`wayland-client`](https://crates.io/crates/wayland-client) | 0.31 | Core Wayland client library | [Docs](https://docs.rs/wayland-client) |
| [`wayland-protocols`](https://crates.io/crates/wayland-protocols) | 0.32 | Stable + staging protocol bindings | [Docs](https://docs.rs/wayland-protocols) |
| [`wayland-protocols-wlr`](https://crates.io/crates/wayland-protocols-wlr) | 0.3 | wlroots-specific protocol bindings | [Docs](https://docs.rs/wayland-protocols-wlr) |
| [`smithay-client-toolkit`](https://crates.io/crates/smithay-client-toolkit) | 0.19 | High-level toolkit (layer-shell, SHM, etc.) | [Docs](https://docs.rs/smithay-client-toolkit) |

### 12.2. Input

| Crate | Purpose | Link |
|---|---|---|
| [`evdev`](https://crates.io/crates/evdev) | uinput virtual device creation (pure Rust) | [Docs](https://docs.rs/evdev) |
| [`reis`](https://crates.io/crates/reis) | libei client bindings (Rust) | [Docs](https://docs.rs/reis) |
| [`xkbcommon`](https://crates.io/crates/xkbcommon) | Keymap handling (needed for virtual keyboard) | [Docs](https://docs.rs/xkbcommon) |

### 12.3. System / IPC

| Crate | Purpose | Link |
|---|---|---|
| [`nix`](https://crates.io/crates/nix) | Unix system calls (permissions, file descriptors) | [Docs](https://docs.rs/nix) |
| [`zbus`](https://crates.io/crates/zbus) | D-Bus client (for portals; already in workspace) | [Docs](https://docs.rs/zbus) |

### 12.4. Rendering (for highlight overlays)

| Crate | Purpose | Link |
|---|---|---|
| [`tiny-skia`](https://crates.io/crates/tiny-skia) | CPU-based 2D rendering (draw rectangles into SHM buffer) | [Docs](https://docs.rs/tiny-skia) |
| [`softbuffer`](https://crates.io/crates/softbuffer) | Software-rendered frame buffer display | [Docs](https://docs.rs/softbuffer) |

---

## 13. Detecting Wayland vs X11 per Application

AT-SPI provides no direct attribute indicating whether an application uses Wayland natively or runs under XWayland. However, the display protocol can be heuristically detected by reading `/proc/{pid}/environ`:

| Environment Variable | Wayland Native | XWayland / X11 |
|---|---|---|
| `GDK_BACKEND` | `wayland` | `x11` |
| `QT_QPA_PLATFORM` | `wayland` | `xcb` |
| `WAYLAND_DISPLAY` | present | may be present (inherited) |
| `DISPLAY` | may be absent | present |
| `XDG_SESSION_TYPE` | `wayland` | inherited, unreliable |

**Implementation location:** [`crates/provider-atspi/src/process.rs`](../crates/provider-atspi/src/process.rs) already reads `/proc/{pid}/` for process metadata. Adding environ parsing is straightforward.

**Algorithm:**
1. Read `/proc/{pid}/environ` (null-byte-separated key=value pairs).
2. Check `GDK_BACKEND` / `QT_QPA_PLATFORM` for explicit toolkit settings.
3. If absent, check for `WAYLAND_DISPLAY` without `DISPLAY` → likely Wayland.
4. If both `WAYLAND_DISPLAY` and `DISPLAY` present → ambiguous (toolkit may use either).

**Use case:** When both `platform-linux-x11` and `platform-linux-wayland` are loaded, the runtime can route window management calls to the appropriate platform based on the target application's display protocol.

---

## 14. Implementation Architecture

### 14.1. Layered Backend Approach

```
┌──────────────────────────────────────────────────┐
│              platform-linux-wayland               │
│                  PlatformModule                   │
├────────────┬────────────┬────────────┬────────────┤
│  Pointer   │  Keyboard  │ Screenshot │  Highlight │
│  Device    │  Device    │  Provider  │  Provider  │
├────────────┴────────────┴────────────┴────────────┤
│              Backend Selection Layer              │
├─────────────┬───────────────┬─────────────────────┤
│   uinput    │  wlr-proto    │      libei          │
│  (evdev)    │ (wayland-cl)  │     (reis)          │
│  [fallback] │ [wlroots]     │ [GNOME/KDE]         │
└─────────────┴───────────────┴─────────────────────┘
```

**Runtime detection:** On `initialize()`, the crate probes the compositor for supported protocols via `wl_registry`. Based on available globals, it selects the best backend for each capability:

1. Check for `zwlr_virtual_pointer_v1` → use wlr pointer
2. Check for EIS socket / portal → use libei
3. Fall back to uinput

### 14.2. Mediation Crate (platform-linux)

A future `platform-linux` crate could mediate between X11 and Wayland:

```
platform-linux
├── Detects session type ($XDG_SESSION_TYPE, $WAYLAND_DISPLAY)
├── Loads platform-linux-x11 if X11 session
├── Loads platform-linux-wayland if Wayland session
└── Routes per-window calls based on app detection (§13)
```

This would allow a single platform registration that handles both X11 and Wayland applications in a mixed XWayland session.

---

## 15. Prioritized Roadmap

### Phase 1: Foundation (MVP)

**Goal:** Basic automation under Sway/wlroots headless.

| Component | Implementation | Effort |
|---|---|---|
| `PlatformModule` | `wayland-client` connection, protocol negotiation | S |
| `DesktopInfoProvider` | `wl_output` enumeration | S |
| `PointerDevice` | uinput via `evdev` crate | M |
| `KeyboardDevice` | uinput via `evdev` crate | M |
| `ScreenshotProvider` | `ext-image-copy-capture-v1` | M |
| `WindowManager` (partial) | `wlr-foreign-toplevel-management` | M |
| CI setup | Sway headless in `startwaylandsession.sh` | S |

### Phase 2: Enrichment

**Goal:** Highlight overlays, libei support, broader compositor coverage.

| Component | Implementation | Effort |
|---|---|---|
| `HighlightProvider` | `wlr-layer-shell` + `tiny-skia` rendering | M |
| `PointerDevice` + `KeyboardDevice` (libei) | `reis` crate integration | L |
| `ScreenshotProvider` (fallback) | `wlr-screencopy` for older wlroots | S |
| `WindowManager` (Sway IPC) | Full bounds/move/resize via Sway IPC | M |

### Phase 3: Broad Desktop Support

**Goal:** GNOME, KDE, and portal-based workflows.

| Component | Implementation | Effort |
|---|---|---|
| `ScreenshotProvider` (portal) | `org.freedesktop.portal.Screenshot` via `zbus` | M |
| `HighlightProvider` (ext) | `ext-layer-shell-v1` for KWin | S |
| Platform mediation crate | `platform-linux` routing X11/Wayland | L |
| Mutter/KWin CI testing | Additional CI matrix entries | M |

**Effort legend:** S = small (1-2 days), M = medium (3-5 days), L = large (1-2 weeks).

---

## 16. Open Questions

1. **Window bounds under Wayland:** Accept `None` from `WindowManager::window_bounds()` or implement compositor-specific IPC backends?
2. **libei maturity:** Is the `reis` crate production-ready? Does libei's API still have breaking changes?
3. **uinput permissions in CI:** Docker containers and GitHub Actions runners — is `/dev/uinput` typically accessible?
4. **Highlight without coordinates:** If we can't know window screen position, should highlights be rendered as image overlays in screenshots rather than live compositor overlays?
5. **Compositor selection for CI:** Should we standardize on Sway instead of Weston for better protocol coverage?
6. **Mixed session routing:** How should the runtime decide per-application whether to use X11 or Wayland platform when XWayland is available?

---

## 17. References

### Specifications & Protocols
- [Wayland Protocol Explorer (wayland.app)](https://wayland.app/protocols/)
- [wayland-protocols repository (freedesktop.org)](https://gitlab.freedesktop.org/wayland/wayland-protocols)
- [wlr-protocols repository](https://gitlab.freedesktop.org/wlroots/wlr-protocols)
- [xdg-desktop-portal specification](https://flatpak.github.io/xdg-desktop-portal/)
- [libei specification & source](https://gitlab.freedesktop.org/libinput/libei)

### Compositors
- [Weston (reference compositor)](https://gitlab.freedesktop.org/wayland/weston)
- [Sway (wlroots tiling compositor)](https://github.com/swaywm/sway)
- [Hyprland (dynamic compositor)](https://github.com/hyprwm/Hyprland)
- [labwc (wlroots stacking compositor)](https://github.com/labwc/labwc)
- [cage (kiosk compositor)](https://github.com/cage-kiosk/cage)
- [niri (scrollable tiling compositor)](https://github.com/YaLTeR/niri)
- [Mir (Canonical display server)](https://github.com/canonical/mir)
- [cosmic-comp (System76 compositor)](https://github.com/pop-os/cosmic-comp)

### Tools & Libraries
- [ydotool (uinput-based input automation)](https://github.com/ReimuNotMoe/ydotool)
- [wtype (Wayland keyboard input)](https://github.com/atx/wtype)
- [wlrctl (wlroots window management)](https://git.sr.ht/~brocellous/wlrctl)
- [AT-SPI2 (accessibility)](https://gitlab.gnome.org/GNOME/at-spi2-core)

### Rust Crates
- [wayland-client](https://crates.io/crates/wayland-client) — Wayland client library
- [wayland-protocols](https://crates.io/crates/wayland-protocols) — Protocol bindings
- [wayland-protocols-wlr](https://crates.io/crates/wayland-protocols-wlr) — wlroots protocol bindings
- [smithay-client-toolkit](https://crates.io/crates/smithay-client-toolkit) — High-level Wayland toolkit
- [evdev](https://crates.io/crates/evdev) — Linux evdev/uinput (input devices)
- [reis](https://crates.io/crates/reis) — libei Rust bindings
- [xkbcommon](https://crates.io/crates/xkbcommon) — XKB keymap handling
- [tiny-skia](https://crates.io/crates/tiny-skia) — CPU 2D rendering
- [zbus](https://crates.io/crates/zbus) — D-Bus client

### Wiki & Guides
- [Arch Wiki: Wayland](https://wiki.archlinux.org/title/Wayland)
- [Arch Wiki: Sway](https://wiki.archlinux.org/title/Sway)
- [Sway Wiki](https://github.com/swaywm/sway/wiki)
- [Hyprland Wiki](https://wiki.hyprland.org/)

### Project Documentation
- [PlatynUI Architecture](architecture.md)
- [PlatynUI Planning](planning.md)
- [PlatynUI Platform Linux (X11)](platform-linux.md)
