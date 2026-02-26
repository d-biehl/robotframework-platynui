# Platform Linux Wayland – Feasibility Study

> **Status:** Research / Planning
> **Date:** 2026-02-26
> **Crate:** `crates/platform-linux-wayland` (planned)

This document captures the current state (Feb 2026) of Wayland protocol support, compositor landscape, Rust ecosystem, and implementation strategies for the `platform-linux-wayland` crate. It serves as both a reference and a decision guide for implementation.

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
- **No client-to-client input injection** — by design, for security. Addressed by libei (supported by GNOME 45+, KDE Plasma 6.1+) and uinput.
- **No equivalent to EWMH** — window management is compositor-specific. The `ext-foreign-toplevel-list-v1` standard (staging) covers listing; management operations remain fragmented.
- **Protocol standardization in progress** — critical `ext-*` protocols have reached staging in wayland-protocols (screenshots, toplevel list, clipboard), but layer-shell is not yet standardized and Mutter implements none of these `ext-*` protocols.

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

### Broader Wayland Accessibility Landscape

The limitations above are not unique to PlatynUI. The Wayland accessibility situation is widely recognized as problematic ([reddit discussion, May 2025](https://www.reddit.com/r/linux/comments/1kkuafo/wayland_an_accessibility_nightmare/)). Key issues affecting the entire ecosystem:

- **No standardized system-wide input simulation** — Wayland's security model intentionally prevents it. libei is the emerging solution but is not yet universally supported.
- **No cursor position tracking API** — assistive technologies that need to monitor cursor position (dwell clickers, eye-tracking tools) have no Wayland-native solution.
- **No screen coordinate exposure** — neither AT-SPI under Wayland nor the draft Newton protocol expose screen-absolute coordinates for accessible objects.
- **Compositor fragmentation** — each compositor implements a different subset of accessibility-relevant protocols, forcing tools to maintain multiple backends.

#### Newton Project (Wayland-Native Accessibility)

[Newton](https://blogs.gnome.org/a11y/2024/06/18/update-on-newton-the-wayland-native-accessibility-project/) is a GNOME-led project (funded by the Sovereign Tech Fund) to replace AT-SPI with a Wayland-native accessibility architecture. Key design points:

- **Push-based model:** Toolkits push accessibility tree updates through a [draft Wayland protocol](https://gitlab.freedesktop.org/mwcampbell/wayland-protocols/tree/accessibility), synchronized with surface commits.
- **Compositor as mediator:** The compositor passes tree updates from apps to ATs via file descriptor passing — apps cannot claim focus or bypass the compositor.
- **Sandboxing-friendly:** Newton-enabled apps work inside Flatpak sandboxes without the AT-SPI bus exception.
- **AccessKit integration:** GTK 4 apps use [AccessKit](https://github.com/AccessKit/accesskit) for the toolkit side, which also enables accessibility on Windows and macOS.
- **AT protocol via D-Bus:** Assistive technologies connect to the compositor via D-Bus (not Wayland), making it easy to restrict access from sandboxed apps.

**Current status (as of Jun 2024, last public update):**
- Orca screen reader basically usable with GTK 4 apps (Nautilus, Text Editor, Podcasts, Fractal).
- Keyboard commands, mouse review, flat review working.
- **Not yet working:** Screen coordinates for accessible objects, synthesized mouse events on Wayland, explore-by-touch, overlay drawing for AT cursors.
- **Not yet upstream** — draft protocol not accepted into wayland-protocols. Matt Campbell's GNOME Foundation contract ended; continuation uncertain.

**Impact on PlatynUI:** Newton is not yet usable for UI automation (no screen coordinates, no mouse synthesis, not upstream). However, it represents the long-term direction. If Newton's Wayland protocol is adopted, PlatynUI could eventually use it instead of AT-SPI for Wayland-native applications. The AccessKit integration is particularly interesting — it could provide a more reliable accessibility tree than AT-SPI for GTK 4 apps. **Monitor this project closely.**

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
| **Compositor support** | Mutter (GNOME 45+), KWin (KDE Plasma 6.1+), Sway (via [sway-libei fork](https://github.com/tytan652/sway-libei)), Weston (planned) |
| **Portal integration** | [`org.freedesktop.portal.InputCapture`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.InputCapture.html) and [`org.freedesktop.portal.RemoteDesktop`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html) |
| **Permissions** | Via xdg-desktop-portal (user consent dialog or test token) |
| **Rust crate** | [`reis`](https://crates.io/crates/reis) 0.6.1 — pure Rust libei/libeis implementation; EI client + EIS server; `tokio` and `calloop` async integration |
| **Maturity** | Actively developed, API maturing (0.6.1 is 8th release); best long-term bet for GNOME and KDE |

### 4.4. Recommendation

| Approach | Universal | Clean API | No Root | Long-term |
|---|---|---|---|---|
| **uinput** | ✅ | ❌ | ❌ | ⚠️ |
| **wlr-protocols** | ❌ | ✅ | ✅ | ⚠️ |
| **libei** | ⚠️ (GNOME, KDE, Sway-fork) | ✅ | ✅ | ✅ |

**Recommended strategy:** Start with **uinput** for universal coverage (CI headless environments typically run as root or have `/dev/uinput` access), then add **libei** as the preferred path for desktop sessions. With `reis` 0.6.1 providing a mature, pure-Rust libei/libeis implementation with `tokio` integration, the libei path is production-viable. The wlr-protocols can serve as a middle layer for wlroots-based compositors.

---

## 5. Screenshots & Screen Capture

### 5.1. ext-image-copy-capture-v1 (Standard)

The [ext-image-copy-capture-v1](https://wayland.app/protocols/ext-image-copy-capture-v1) protocol is the established standard for screen capture under Wayland (staging since wayland-protocols 1.37, Aug 2024).

- **Source selection:** via [ext-image-capture-source-v1](https://wayland.app/protocols/ext-image-capture-source-v1) — supports output (monitor), toplevel (window), or workspace capture.
- **Compositor support:** KWin, Sway (1.11+), Mir, Hyprland, cosmic-comp, niri, Weston (partial).
- **Buffer types:** `wl_shm` (CPU-accessible), DMA-BUF (GPU-accessible).
- **Adoption:** Broad — only Mutter (GNOME) and labwc do not support it yet.

### 5.2. wlr-screencopy-unstable-v1 (Legacy)

The [wlr-screencopy-unstable-v1](https://wayland.app/protocols/wlr-screencopy-unstable-v1) protocol is the older wlroots-specific approach.

- **Compositor support:** Sway, Hyprland, wlroots-based compositors.
- **Status:** Superseded by `ext-image-copy-capture-v1`. Sway 1.11+ supports both but the standard protocol is preferred. Legacy-only for older wlroots (<0.19).
- **Limitation:** Output-level only (full monitor), no per-window capture.

### 5.3. xdg-desktop-portal Screenshot

The [org.freedesktop.portal.Screenshot](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html) D-Bus portal provides a compositor-agnostic screenshot API.

- **Compositor support:** Any compositor with an xdg-desktop-portal backend (GNOME, KDE, Sway, Hyprland).
- **Limitation:** Triggers user consent dialog (unless test token is used); returns a file path, not a buffer.
- **Use case:** Fallback for compositors without direct protocol support.

### 5.4. Recommendation

Use **ext-image-copy-capture-v1** as primary — it has broad adoption (KWin, Sway 1.11+, Hyprland, Mir, cosmic-comp, niri) and is the clear standard. Fall back to **wlr-screencopy** only for older wlroots-based compositors predating v0.19, and **portal** as last resort for Mutter/GNOME (which still does not implement `ext-image-copy-capture`).

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

The [ext-foreign-toplevel-list-v1](https://wayland.app/protocols/ext-foreign-toplevel-list-v1) protocol provides a standardized read-only list of toplevel windows (staging since wayland-protocols 1.36, Apr 2024).

- **Information:** `title`, `app_id`, `identifier`
- **No management operations** — list and observe only.
- **Compositor support:** KWin, Sway (1.10+), Hyprland, cosmic-comp, niri.

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

- **Compositor support:** KWin (KDE Plasma 6.2+), cosmic-comp, niri.
- **Status:** Implemented by several compositors, but **not yet accepted into wayland-protocols** (as of Feb 2026). Adoption is growing independently of formal standardization. `wlr-layer-shell-unstable-v1` remains the primary option for wlroots-based compositors.

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

> **Current versions:** wayland-protocols **1.47** (Dec 2025), Wayland core **1.24.0** (Jul 2025).

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
| [`ext-data-control-v1`](https://wayland.app/protocols/ext-data-control-v1) | Clipboard access (read/write) | Staging (since wayland-protocols 1.45) |
| `ext-layer-shell-v1` | Layer surfaces (overlay) | Implemented by KWin, niri, cosmic, but **not yet in wayland-protocols** |
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

> **Current compositor versions:** Sway 1.11 (Jun 2025), Hyprland 0.45+, KWin 6.4.3 / KDE Plasma 6.6 (Feb 2026), Mutter 49.2 / GNOME 49 (Nov 2025), Weston 15.0.0 (Feb 2026), labwc 0.8+, niri 25.x, Mir 2.x, cosmic-comp (beta, Pop!_OS 24.04 LTS).

| Protocol | Sway | Hyprland | KWin | Mutter | Weston | labwc | niri | Mir | cosmic |
|---|---|---|---|---|---|---|---|---|---|
| `ext-image-copy-capture-v1` | ✅ (1.11+) | ✅ | ✅ | ❌ | ⚠️ | ❌ | ✅ | ✅ | ✅ |
| `ext-data-control-v1` | ✅ (1.11+) | ❓ | ❓ | ❌ | ❌ | ❓ | ❓ | ❓ | ✅ |
| `wlr-screencopy-v1` | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ | ❌ | ✅ |
| `wlr-foreign-toplevel-mgmt` | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ |
| `ext-foreign-toplevel-list` | ✅ (1.10+) | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ | ✅ |
| `wlr-layer-shell-v1` | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ |
| `ext-layer-shell-v1` | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ | ✅ |
| `zwlr_virtual_pointer_v1` | ✅ | ✅ | ❌ | ❌ | ⚠️ | ✅ | ❌ | ✅ | ❌ |
| `zwp_virtual_keyboard_v1` | ✅ | ✅ | ❌ | ❌ | ✅ | ✅ | ❌ | ✅ | ✅ |
| libei (EIS) | [Fork](https://github.com/tytan652/sway-libei) | ❌ | ✅ (6.1+) | ✅ (45+) | ❌ | ❌ | ❌ | ❌ | ❌ |

**Key insights:**
- No single protocol set covers all compositors. Mutter (GNOME) is the most restrictive — it only supports libei and portals.
- **KWin** (KDE) is the most cooperative desktop compositor for UI automation — it supports `ext-image-copy-capture`, `ext-foreign-toplevel-list`, `ext-layer-shell`, **and** libei. KDE Plasma 6.2+ added full Sticky Keys support, 6.4+ added pointer-keys via numpad and improved screen reader usability.
- **cosmic-comp** (System76/Pop!_OS) has the broadest protocol coverage of any compositor — it supports both `wlr-*` and `ext-*` variants for screenshots, toplevel management, layer-shell, data-control, and virtual keyboard. Built on Smithay (Rust), it includes built-in accessibility features (zoom, color filters). However, it does not yet support libei or `zwlr_virtual_pointer`. As Pop!_OS 24.04 LTS ships with COSMIC, it is becoming a relevant target desktop.
- **Mutter** (GNOME) relies exclusively on portals and libei. However, GNOME 48+ improved Orca accessibility on Wayland, and GNOME 49 enhanced Remote Desktop capabilities.
- **Sway 1.11+** supports all standard `ext-*` protocols — no fallback to wlr-specific protocols needed.

---

## 11. Compositors for CI / Headless Testing

For automated testing in CI, a headless-capable compositor is essential.

### 11.1. Weston (Current Choice)

[Weston](https://gitlab.freedesktop.org/wayland/weston) is the Wayland reference compositor (**v15.0.0**, Feb 2026). Note: Weston has dropped the "reference compositor" designation in recent releases.

- **Backends:** `headless-backend.so` (no display), `wayland-backend.so` (nested), `x11-backend.so` (in X11 window), `drm-backend.so` (hardware).
- **Already configured:** see [`scripts/startwaylandsession.sh`](../scripts/startwaylandsession.sh)
- **Protocol support:** Limited — no `wlr-foreign-toplevel`, no `wlr-layer-shell`, no `ext-image-copy-capture`. Versions 14 and 15 did not add any of these critical protocols.
- **Best for:** Basic Wayland session setup, AT-SPI testing, application launching.

### 11.2. Sway

[Sway](https://github.com/swaywm/sway) is a wlroots-based tiling compositor with excellent protocol support (**v1.11**, Jun 2025; based on wlroots 0.19).

- **Headless mode:** `WLR_BACKENDS=headless sway` (via wlroots backend).
- **Virtual outputs:** `swaymsg create_output` to add virtual monitors.
- **Protocol support:** Richest of any compositor — supports nearly all wlr and ext protocols. Sway 1.10 added `ext-foreign-toplevel-list-v1`; Sway 1.11 added `ext-image-copy-capture-v1`, `ext-data-control-v1`, and security-context metadata in IPC.
- **IPC:** Full window management via [i3-compatible IPC](https://i3wm.org/docs/ipc.html).
- **Best for:** Comprehensive testing with full window management.

### 11.3. Mutter (GNOME)

[Mutter](https://gitlab.gnome.org/GNOME/mutter) is GNOME's compositor (**Mutter 49.2** / GNOME 49, Nov 2025; GNOME 50 planned for Mar 2026).

- **Headless mode:** `mutter --headless --virtual-monitor 1920x1080` (since GNOME 45).
- **Protocol support:** Minimal custom protocols; relies on portals and libei. Does not implement `ext-image-copy-capture`, `ext-foreign-toplevel-list`, or any layer-shell protocol.
- **Accessibility:** GNOME 48 fixed Orca screen reader shortcuts on Wayland (Caps Lock modifier) and enabled accessible web content in Flatpak. GNOME 49 added enhanced Remote Desktop with multitouch input forwarding and relative mouse input.
- **Best for:** Testing GNOME-specific applications, portal-based workflows.

### 11.4. labwc

[labwc](https://github.com/labwc/labwc) is a lightweight wlroots-based stacking compositor.

- **Headless mode:** Via wlroots backends (`WLR_BACKENDS=headless`).
- **Protocol support:** Good wlr protocol coverage, lightweight footprint.
- **Best for:** Lightweight CI testing environments.

### 11.5. cosmic-comp (COSMIC Desktop)

[cosmic-comp](https://github.com/pop-os/cosmic-comp) is System76's Smithay-based compositor for the [COSMIC desktop environment](https://system76.com/cosmic), shipping with Pop!_OS 24.04 LTS (beta as of Sep 2025).

- **Built with:** [Smithay](https://github.com/Smithay/smithay) (Rust), MSRV 1.90, Rust 2024 edition — same toolchain as PlatynUI.
- **Protocol support:** Exceptionally broad — supports both `wlr-*` and `ext-*` protocol families:
  - Screenshots: `ext-image-copy-capture-v1` + `wlr-screencopy`
  - Toplevel: `ext-foreign-toplevel-list` + `wlr-foreign-toplevel-management` (with activate, close, maximize, minimize, move-to-workspace)
  - Layer shell: `wlr-layer-shell` + `ext-layer-shell`
  - Data control: `ext-data-control` + `wlr-data-control`
  - Input: `zwp_virtual_keyboard` (but **no** `zwlr_virtual_pointer`, **no** libei)
- **Accessibility:** Built-in zoom (`accessibility_zoom`) and color filters (inversion, color blindness). `A11yState` and `A11yKeyboardMonitorState` in compositor state. Screen reader toggle shortcut added recently.
- **Headless mode:** Not explicitly documented. Backends include X11, Winit, and KMS — no dedicated headless backend, but Winit backend may work for testing.
- **License:** GPL-3.0 — note: this may affect whether PlatynUI can link against cosmic-comp code directly.
- **Relevance:** As Pop!_OS gains market share, COSMIC becomes a third major desktop target alongside GNOME and KDE. Its Smithay/Rust foundation makes it architecturally close to PlatynUI.

### 11.6. Cage

[cage](https://github.com/cage-kiosk/cage) is a kiosk compositor (runs a single application fullscreen).

- **Headless mode:** Via wlroots backends.
- **Use case:** Single-app testing scenarios.

### 11.7. Custom Test Compositor (Future Option)

A purpose-built Wayland compositor for PlatynUI's CI testing could address limitations of all existing compositors. Built with [Smithay](https://github.com/Smithay/smithay) (Rust Wayland compositor library, already a transitive dependency via `eframe`/`egui`), it would implement exactly the protocols PlatynUI needs:

- **All required protocols** in one compositor (ext-image-copy-capture, ext-foreign-toplevel-list, wlr-foreign-toplevel-management, layer-shell, virtual pointer/keyboard, EIS via `reis`).
- **Test-control IPC** — a side-channel for tests to set window positions, query compositor state, control timing deterministically.
- **Window bounds exposure** — the compositor knows window positions; it can provide them without IPC hacks.
- **No GPU required** — headless-only, CPU rendering, fast startup (<50ms).
- **Estimated effort:** ~2-4K LoC, 2-3 weeks initial development.

This would complement (not replace) testing against real compositors. It fills the gap between `platform-mock` (no protocol at all) and real compositors (non-deterministic, limited control).

### 11.8. Recommendation

CI testing should prioritize the compositors that PlatynUI's users actually run:

| Use Case | Compositor | Reason |
|---|---|---|
| **CI primary** | Mutter (headless) | GNOME is the most common Linux desktop; validates portal/libei path |
| **CI secondary** | Weston (headless) | Reference implementation; already configured; validates basic Wayland |
| **CI extended** | KWin (headless) | KDE is the second most common desktop; validates ext-* protocols + libei |
| **CI extended** | cosmic-comp | Growing Pop!_OS user base; broadest dual-protocol coverage (wlr + ext) |
| **Protocol testing** | Sway (headless) | Useful for validating wlr-* protocol clients in isolation |
| **Future** | Custom compositor | Deterministic protocol-level testing (see §11.7) |

**Rationale:** Sway is a niche tiling WM used by a small community. While it has excellent protocol support, testing primarily against Sway would validate protocols that real users' desktops (GNOME, KDE, COSMIC) may not support. Mutter, KWin, and cosmic-comp should be the primary CI targets because they represent the actual user base.

---

## 12. Rust Crate Ecosystem

### 12.1. Wayland Client

| Crate | Version | Purpose | Link |
|---|---|---|---|
| [`wayland-client`](https://crates.io/crates/wayland-client) | 0.31.12 | Core Wayland client library | [Docs](https://docs.rs/wayland-client) |
| [`wayland-protocols`](https://crates.io/crates/wayland-protocols) | 0.32.10 | Stable + staging protocol bindings | [Docs](https://docs.rs/wayland-protocols) |
| [`wayland-protocols-wlr`](https://crates.io/crates/wayland-protocols-wlr) | 0.3.10 | wlroots-specific protocol bindings | [Docs](https://docs.rs/wayland-protocols-wlr) |
| [`smithay-client-toolkit`](https://crates.io/crates/smithay-client-toolkit) | 0.20.0 | High-level toolkit (layer-shell, SHM, etc.); Rust Edition 2024 | [Docs](https://docs.rs/smithay-client-toolkit) |

### 12.2. Input

| Crate | Version | Purpose | Link |
|---|---|---|---|
| [`evdev`](https://crates.io/crates/evdev) | | uinput virtual device creation (pure Rust) | [Docs](https://docs.rs/evdev) |
| [`reis`](https://crates.io/crates/reis) | 0.6.1 | Pure Rust libei/libeis protocol; EI client (`reis::ei`) + EIS server (`reis::eis`); high-level event/request API; `tokio` and `calloop` async features; 7.7K SLoC; MIT license. Repo: [`ids1024/reis`](https://github.com/ids1024/reis) | [Docs](https://docs.rs/reis) |
| [`xkbcommon`](https://crates.io/crates/xkbcommon) | | Keymap handling (needed for virtual keyboard) | [Docs](https://docs.rs/xkbcommon) |

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

**Goal:** Basic automation under Mutter and Weston headless — targeting the most common user desktop (GNOME) and the reference implementation from day one.

| Component | Implementation | Effort |
|---|---|---|
| `PlatformModule` | `wayland-client` connection, protocol negotiation via `wl_registry` | S |
| `DesktopInfoProvider` | `wl_output` enumeration | S |
| `PointerDevice` | uinput via `evdev` crate (universal, works on all compositors) | M |
| `KeyboardDevice` | uinput via `evdev` crate | M |
| `PointerDevice` + `KeyboardDevice` (libei) | `reis` 0.6.1 crate integration — required for Mutter/GNOME (only input path) | L |
| `ScreenshotProvider` (portal) | `org.freedesktop.portal.Screenshot` via `zbus` — works on Mutter | M |
| `WindowManager` (minimal) | AT-SPI PID matching + limited D-Bus (Mutter) | M |
| CI setup (Mutter) | `mutter --headless --virtual-monitor 1920x1080` | S |
| CI setup (Weston) | Existing `startwaylandsession.sh` — reference implementation baseline | S |

**Rationale:** GNOME/Mutter is the most restrictive compositor but also the most common desktop. By targeting it first, we solve the hardest problems early (portal-only screenshots, libei-only input). Weston provides a reference baseline. uinput serves as universal fallback for CI environments where libei portals are unavailable.

### Phase 2: KDE & Standard Protocols

**Goal:** KWin support via standard `ext-*` protocols, highlight overlays.

| Component | Implementation | Effort |
|---|---|---|
| `ScreenshotProvider` (ext) | `ext-image-copy-capture-v1` — standard protocol, supported by KWin, Sway, Hyprland, niri, cosmic | M |
| `WindowManager` (ext) | `ext-foreign-toplevel-list-v1` (standard) + `wlr-foreign-toplevel-management` (management ops) | M |
| `HighlightProvider` | `ext-layer-shell-v1` for KWin + `wlr-layer-shell` for wlroots-based + `tiny-skia` rendering | M |
| `WindowManager` (KWin IPC) | Window bounds/move/resize via KWin D-Bus scripting API | M |
| CI setup (KWin) | KWin headless testing | M |

### Phase 3: Broad Coverage & Test Infrastructure

**Goal:** Full compositor coverage, platform mediation, custom test compositor.

| Component | Implementation | Effort |
|---|---|---|
| `ScreenshotProvider` (legacy) | `wlr-screencopy` fallback for older wlroots (<0.19) | S |
| `WindowManager` (compositor IPC) | Sway i3-IPC, Hyprland hyprctl for bounds/move/resize | M |
| Platform mediation crate | `platform-linux` routing X11/Wayland per-application (§13) | L |
| Custom test compositor | Smithay-based, all required protocols, test-control IPC (§11.7) | L |
| CI matrix | Sway headless (protocol validation), labwc (lightweight) | S |

**Effort legend:** S = small (1-2 days), M = medium (3-5 days), L = large (1-2 weeks).

---

## 16. Open Questions

1. **Window bounds under Wayland:** Accept `None` from `WindowManager::window_bounds()` or implement compositor-specific IPC backends? Currently leaning toward compositor IPC (KWin D-Bus, Sway i3-IPC, Hyprland hyprctl) as opt-in backends, with `None` as the default. A custom test compositor (§11.7) could solve this for CI.
2. **reis API stability:** `reis` 0.6.1 is the 8th release with significant API changes between versions. Before integrating, verify the 0.6.x API is stable enough for production use. This is now **Phase 1 critical** since Mutter requires libei as the only input path.
3. **uinput permissions in CI:** Docker containers and GitHub Actions runners — is `/dev/uinput` typically accessible? Needs testing with actual CI runners. uinput is the universal fallback when libei portals are unavailable.
4. **Highlight without coordinates:** If we can't know window screen position, should highlights be rendered as image overlays in screenshots rather than live compositor overlays? Compositor IPC can provide window geometry for KWin/Sway/Hyprland, leaving only Mutter without a solution.
5. **Mutter headless + portals:** Does `mutter --headless` properly support xdg-desktop-portal for screenshots and libei for input injection? Test tokens may be needed to bypass consent dialogs in CI. Needs validation.
6. **Mixed session routing:** How should the runtime decide per-application whether to use X11 or Wayland platform when XWayland is available? See §13 for the proposed detection algorithm.
7. **Mutter protocol gap:** Mutter/GNOME still does not implement `ext-image-copy-capture`, `ext-foreign-toplevel-list`, or layer-shell protocols. Portal-based fallbacks remain the only option for GNOME. Monitor whether GNOME 50+ adds any of these.
8. **Newton project status:** The Newton Wayland-native accessibility project (§3) has had no public updates since Jun 2024. The draft Wayland accessibility protocol is not accepted into wayland-protocols. Monitor whether this project continues and whether it could eventually replace AT-SPI for Wayland-native apps.
9. **Custom test compositor scope:** Should the custom compositor (§11.7) be a standalone binary or a library that tests can embed in-process? The latter would enable true unit-test-level protocol testing but is more complex.

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

### Accessibility
- [Newton — Wayland-native accessibility project (GNOME blog, Jun 2024)](https://blogs.gnome.org/a11y/2024/06/18/update-on-newton-the-wayland-native-accessibility-project/)
- [Draft Wayland accessibility protocol (Matt Campbell)](https://gitlab.freedesktop.org/mwcampbell/wayland-protocols/tree/accessibility)
- [AccessKit (cross-platform accessibility toolkit)](https://github.com/AccessKit/accesskit)
- ["Wayland: An Accessibility Nightmare" (reddit discussion, May 2025)](https://www.reddit.com/r/linux/comments/1kkuafo/wayland_an_accessibility_nightmare/)

### Tools & Libraries
- [ydotool (uinput-based input automation)](https://github.com/ReimuNotMoe/ydotool)
- [wtype (Wayland keyboard input)](https://github.com/atx/wtype)
- [wlrctl (wlroots window management)](https://git.sr.ht/~brocellous/wlrctl)
- [AT-SPI2 (accessibility)](https://gitlab.gnome.org/GNOME/at-spi2-core)
- [Smithay (Rust Wayland compositor library)](https://github.com/Smithay/smithay)

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
