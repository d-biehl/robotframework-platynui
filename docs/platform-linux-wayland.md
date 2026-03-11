# Platform Linux Wayland – Feasibility Study

> **Status:** Phase 1 complete (input injection on all compositor families), Phase 1b in progress (screenshots, window management)
> **Date:** 2026-02-26 (initial), 2026-03-10 (last updated)
> **Crate:** `crates/platform-linux-wayland`

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
- **No client-to-client input injection** — by design, for security. Addressed by libei (GNOME 45+, KDE Plasma 6.1+) and wlr virtual-pointer/keyboard protocols (wlroots-based compositors).
- **No equivalent to EWMH** — window management is compositor-specific. The `ext-foreign-toplevel-list-v1` standard (staging) covers listing; management operations remain fragmented.
- **Protocol standardization in progress** — critical `ext-*` protocols have reached staging in wayland-protocols (screenshots, toplevel list, clipboard), but layer-shell is not yet standardized and Mutter implements none of these `ext-*` protocols.

---

## 2. Platform Trait Requirements

The following traits from `platynui-core` must be implemented (see [crates/core/src/platform/](../crates/core/src/platform/)):

| Trait | X11 Mechanism | Wayland Equivalent | Feasibility |
|---|---|---|---|
| `PlatformModule` | `x11rb` connection | `wayland-client` connection | ✅ Straightforward |
| `PointerDevice` | XTest `XTestFakeMotionEvent/ButtonEvent` | libei / `zwlr_virtual_pointer_v1` | ✅ libei (GNOME, KDE) + wlr (wlroots) |
| `KeyboardDevice` | XTest `XTestFakeKeyEvent` + XKB | libei / `zwp_virtual_keyboard_v1` | ✅ libei (GNOME, KDE) + wlr (wlroots) |
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

| Approach | GNOME + KDE | wlroots | Clean API | No Root | Long-term |
|---|---|---|---|---|---|
| **libei** | ✅ (only path for Mutter) | ⚠️ (Sway-fork only) | ✅ | ✅ | ✅ |
| **wlr-protocols** | ❌ | ✅ | ✅ | ✅ | ⚠️ |
| **uinput** | ❌ (bypasses compositor) | ✅ | ❌ | ❌ | ❌ |

**Decision:** Four input backends implemented (see §14.1):

1. **libei (EIS)** — primary for Mutter (GNOME) + KWin (KDE), via direct EIS socket or XDG Desktop Portal `RemoteDesktop.ConnectToEIS()`.
2. **wlr virtual-pointer/keyboard** — fallback for wlroots-based compositors (Sway, Hyprland, labwc) that don't expose EIS.
3. **Control socket** — exclusive backend for the PlatynUI custom compositor, using JSON-over-Unix-socket IPC.
4. **Portal** — wraps EIS via `RemoteDesktop` portal, used when direct EIS socket is unavailable but the compositor supports the portal (Mutter, KWin).

No uinput — see §4.1 for rationale.

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

This is the most challenging area. Wayland has **no equivalent to X11's EWMH** for programmatic window management. PlatynUI's `WindowManager` trait (see [`crates/core/src/platform/window_manager.rs`](../crates/core/src/platform/window_manager.rs)) requires 10 methods. Under Wayland, **7 of 10** are solvable via standard protocols; the remaining 3 (`bounds`, `move_to`, `resize`) require compositor-specific IPC.

### 6.1. Available Protocols

#### wlr-foreign-toplevel-management-unstable-v1

The [wlr-foreign-toplevel-management-unstable-v1](https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1) protocol provides window lifecycle management.

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

#### ext-foreign-toplevel-list-v1 (Read-Only)

The [ext-foreign-toplevel-list-v1](https://wayland.app/protocols/ext-foreign-toplevel-list-v1) protocol provides a standardized read-only list of toplevel windows (staging since wayland-protocols 1.36, Apr 2024).

- **Information:** `title`, `app_id`, `identifier`
- **No management operations** — list and observe only.
- **Compositor support:** KWin, Sway (1.10+), Hyprland, cosmic-comp, niri.

### 6.2. Compositor-Specific IPC for Full Window Control

No Wayland protocol exposes window bounds or allows move/resize. The only path to these capabilities is compositor-specific IPC:

| Compositor | IPC Mechanism | bounds | move_to | resize | Rust Integration |
|---|---|---|---|---|---|
| **KWin** | [D-Bus `org.kde.KWin`](https://develop.kde.org/docs/plasma/kwin/api/) | ✅ | ✅ | ✅ | `zbus` (already in workspace) |
| **Sway** | [i3 IPC](https://github.com/swaywm/sway/wiki) (Unix socket, JSON) | ✅ | ✅ | ✅ | [`swayipc`](https://crates.io/crates/swayipc) or raw socket + serde |
| **Hyprland** | [hyprctl](https://wiki.hyprland.org/Configuring/Using-hyprctl/) (Unix socket, JSON) | ✅ | ✅ | ✅ | [`hyprland`](https://crates.io/crates/hyprland) or raw socket + serde |
| **Mutter** | Limited D-Bus | ❌ | ❌ | ❌ | No viable API — see §6.5 |
| **cosmic-comp** | No public IPC (yet) | ❌ | ❌ | ❌ | May add IPC in future releases |

### 6.3. Implementation Strategy: Two-Layer Architecture

```
WindowManager (Wayland)
├── Protocol Layer (standard — all compositors)
│   ├── wlr-foreign-toplevel-management → activate, close, minimize, maximize, restore
│   ├── ext-foreign-toplevel-list → resolve_window (title/app_id matching)
│   └── AT-SPI PID matching → resolve_window (fallback)
│
└── Compositor IPC Layer (pluggable — per compositor)
    ├── KWinIpcBackend    → bounds, move_to, resize  (D-Bus)
    ├── SwayIpcBackend    → bounds, move_to, resize  (i3 IPC socket)
    ├── HyprlandIpcBackend → bounds, move_to, resize  (hyprctl socket)
    └── (none for Mutter) → bounds/move_to/resize return PlatformError::NotSupported
```

**Runtime detection:** On initialization, the `WindowManager` probes which compositor is running (via `wl_registry` globals + environment variables like `SWAYSOCK`, `HYPRLAND_INSTANCE_SIGNATURE`, or D-Bus name `org.kde.KWin`). It loads the matching IPC backend. If no IPC backend is available, `bounds`/`move_to`/`resize` return `PlatformError::NotSupported`.

**Trait compatibility:** The existing `WindowManager` trait returns `Result<T, PlatformError>` for all methods. No trait changes are needed — the Wayland implementation returns `Err(PlatformError::not_supported("bounds not available on this compositor"))` when no IPC backend is loaded. The `WindowSurfacePattern` in `provider-atspi` already handles errors gracefully and propagates them to the Python/RF layer.

### 6.4. Impact on `WindowManager` Trait

| Method | Protocol Layer | Compositor IPC Layer | Net Result |
|---|---|---|---|
| `resolve_window()` | ✅ `ext-foreign-toplevel-list` + AT-SPI PID | — | ✅ All compositors |
| `is_active()` | ✅ `wlr-foreign-toplevel` state events | — | ✅ All compositors |
| `activate()` | ✅ `wlr-foreign-toplevel` activate | — | ✅ All compositors |
| `close()` | ✅ `wlr-foreign-toplevel` close | — | ✅ All compositors |
| `minimize()` / `restore()` | ✅ `wlr-foreign-toplevel` set/unset | — | ✅ All compositors |
| `maximize()` | ✅ `wlr-foreign-toplevel` set/unset | — | ✅ All compositors |
| `bounds()` | ❌ | ✅ KWin, Sway, Hyprland | ⚠️ Not on Mutter/cosmic |
| `move_to()` | ❌ | ✅ KWin, Sway, Hyprland | ⚠️ Not on Mutter/cosmic |
| `resize()` | ❌ | ✅ KWin, Sway, Hyprland | ⚠️ Not on Mutter/cosmic |

**Coverage:** KWin + Sway + Hyprland account for the vast majority of Wayland desktops other than GNOME. Together with Mutter (7 of 10 methods), this gives >95% of Linux desktops at least basic window management, and KDE/Sway/Hyprland users get full parity with X11.

### 6.5. The Mutter Problem and `ActivationPoint`

Mutter (GNOME) provides no window geometry API for external processes. This means `bounds()`, `move_to()`, and `resize()` are genuinely unsupported on GNOME Wayland. However, the impact on PlatynUI's core automation workflow is manageable:

**Why `bounds()` matters for automation:**

The runtime uses `ActivationPoint` (from AT-SPI `Bounds`) to calculate absolute screen coordinates for pointer clicks. The pointer subsystem (see [`crates/runtime/src/pointer.rs`](../crates/runtime/src/pointer.rs)) supports three `PointOrigin` modes:
- `Desktop` — absolute screen coordinates
- `Bounds(Rect)` — relative to an element's bounding rectangle
- `Absolute(Point)` — offset from an anchor point

Under X11, AT-SPI `GetExtents(SCREEN)` returns screen-absolute coordinates → `PointOrigin::Desktop` works.
Under Wayland, AT-SPI `GetExtents(SCREEN)` returns `(0, 0)` for the window origin → the element's absolute position is unknown.

**Mutter workaround — focus-based pointer injection via libei:**

The key insight: **we don't need to know the window's screen position to click on an element inside it.** libei injects input events through the compositor, which routes them to the focused surface. Combined with AT-SPI window-relative coordinates, this enables a focus-based automation flow:

1. **`activate()`** — bring target window to front (works via `wlr-foreign-toplevel` or D-Bus on Mutter)
2. **AT-SPI `GetExtents(WINDOW)`** — returns the element's position *relative to its own window* (this works under Wayland!)
3. **libei absolute pointer move** — move the pointer to an absolute screen position. Since the window is focused and (typically) placed by the compositor, we can combine window-relative coordinates with either:
   - **Heuristic:** Assume the window is centered/maximized (works for simple CI scenarios)
   - **Screenshot correlation:** Take a screenshot of the focused window via portal, compare with AT-SPI-reported element positions to derive the window's screen offset
   - **Accept reduced precision:** For tests where exact positioning isn't critical, the runtime logs a warning and clicks relative to the monitor center

4. **`move_to()` / `resize()`** — return `PlatformError::NotSupported` with clear error message

**Implementation in the runtime:**

The `ActivationPoint` resolution in [`crates/runtime/src/xpath.rs`](../crates/runtime/src/xpath.rs) already reads the element's `Bounds` attribute. Under Wayland, the `provider-atspi` crate would provide `Bounds` from `GetExtents(WINDOW)` instead of `GetExtents(SCREEN)`:

```rust
// In provider-atspi, when running under Wayland:
// - Use GetExtents(WINDOW) for Bounds attribute → window-relative coords
// - Store the coordinate type so the runtime knows it's relative
//
// In the runtime pointer logic:
// - If Bounds are window-relative, use PointOrigin::Bounds with the window's
//   screen position (from compositor IPC) or fall back to the focused
//   window position
```

The `PointOrigin::Bounds(Rect)` already supports this pattern — the `Rect` serves as the coordinate origin. Under Wayland, this Rect would be `Rect::new(window_x, window_y, 0.0, 0.0)` (where `window_x/y` comes from compositor IPC or is `(0, 0)` as fallback on Mutter).

### 6.6. Recommendation

| Compositor | Window Lifecycle | bounds/move/resize | Pointer Click Strategy |
|---|---|---|---|
| **KWin** | `wlr-foreign-toplevel` (if available) or D-Bus | D-Bus IPC → full support | Standard: screen-absolute via IPC bounds |
| **Sway** | `wlr-foreign-toplevel` | i3 IPC → full support | Standard: screen-absolute via IPC bounds |
| **Hyprland** | `wlr-foreign-toplevel` | hyprctl IPC → full support | Standard: screen-absolute via IPC bounds |
| **Mutter** | D-Bus limited (activate, close) + AT-SPI actions | ❌ not available | Focus-based: activate → window-relative AT-SPI coords + libei |
| **cosmic-comp** | `wlr-foreign-toplevel` | ❌ (no IPC yet) | Focus-based (same as Mutter) |

**Priority:** Start with KWin + Sway IPC backends (covers KDE + tiling WM users with full functionality). Mutter's focus-based approach requires no compositor IPC — it works with what's already available (AT-SPI + libei). This means even the Phase 1 MVP can support pointer clicks on GNOME.

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

### 11.7. Custom Test Compositor — ✅ Implemented

The PlatynUI project includes a purpose-built Wayland compositor for CI testing and development, implemented with [Smithay](https://github.com/Smithay/smithay) (~15K LoC):

- **Crate:** `apps/wayland-compositor` (`platynui-wayland-compositor`)
- **Control CLI:** `apps/wayland-compositor-ctl` (`platynui-wayland-compositor-ctl`)
- **Test application:** `apps/test-app-egui` (`platynui-test-app-egui`) — egui app with AccessKit accessibility
- **Backends:** Headless (no display), Winit (windowed), DRM (hardware)
- **EIS server:** Built-in via `reis` — supports libei-based input injection
- **XWayland:** Supported for X11 application testing
- **Control IPC:** Unix socket for test orchestration (keyboard/pointer injection, compositor state queries)
- **Protocol coverage:** Virtual pointer/keyboard, layer-shell, foreign-toplevel management, and more
- **No GPU required** for headless mode — CPU rendering, fast startup

The compositor is the primary target for deterministic integration testing. The `ControlSocketBackend` in `platform-linux-wayland` communicates with it via the control socket IPC.

### 11.8. Recommendation

CI testing should prioritize the compositors that PlatynUI's users actually run:

| Use Case | Compositor | Reason |
|---|---|---|
| **Dev/CI primary** | PlatynUI compositor (headless) | Deterministic, all protocols, control IPC, already implemented |
| **CI integration** | Mutter (headless) | GNOME is the most common Linux desktop; validates portal/libei path |
| **CI integration** | Weston (headless) | Reference implementation; already configured; validates basic Wayland |
| **CI extended** | KWin (headless) | KDE is the second most common desktop; validates ext-* protocols + libei |
| **CI extended** | cosmic-comp | Growing Pop!_OS user base; broadest dual-protocol coverage (wlr + ext) |
| **Protocol testing** | Sway (headless) | Validates wlr-* protocol clients; `startsway.sh` already available |

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
| [`evdev`](https://crates.io/crates/evdev) | | Linux evdev input device access (pure Rust). Not used for input injection (see §4.4) but may be useful for input monitoring/diagnostics. | [Docs](https://docs.rs/evdev) |
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

### 12.5. Diagnostic & Development Tools (CLI)

These command-line tools are useful during development, debugging, and CI validation to inspect the running compositor's capabilities.

| Tool | Package | Purpose |
|---|---|---|
| [`wayland-info`](https://gitlab.freedesktop.org/wayland/wayland-utils) | `wayland-utils` | Lists all Wayland globals (= registered protocol interfaces) with name and version. The primary tool for verifying which protocols a compositor supports. |
| [`wlr-randr`](https://sr.ht/~emersion/wlr-randr/) | `wlr-randr` | Monitor configuration (resolution, position, scale, transform) on wlroots-based compositors. Similar to `xrandr` for X11. |
| [`wlrctl`](https://git.sr.ht/~brocellous/wlrctl) | `wlrctl` | Window management and input via wlr-foreign-toplevel and virtual-keyboard/pointer protocols. Useful for scripting and testing protocol interactions. |
| [`wev`](https://git.sr.ht/~sircmpwn/wev) | `wev` | Wayland event viewer — displays all input events (keyboard, pointer, touch) received by a surface. The Wayland equivalent of X11's `xev`. |
| [`wl-clipboard`](https://github.com/bugaevc/wl-clipboard) | `wl-clipboard` | Clipboard access (`wl-copy`, `wl-paste`) via `wl_data_device` or `ext-data-control`. |

**Installation:**

```bash
# Debian / Ubuntu
sudo apt install wayland-utils wev wl-clipboard

# Fedora
sudo dnf install wayland-utils wev wl-clipboard

# Arch
sudo pacman -S wayland-utils wev wl-clipboard
```

**Example: Checking compositor protocol support**

```bash
$ wayland-info | grep -E 'interface:.*zwlr_|interface:.*ext_|interface:.*zwp_|interface:.*ei_'
interface: 'zwlr_virtual_pointer_manager_v1', version: 1, name: 42
interface: 'zwp_virtual_keyboard_manager_v1', version: 1, name: 43
interface: 'zwlr_foreign_toplevel_manager_v1', version: 3, name: 44
interface: 'ext_image_copy_capture_manager_v1', version: 1, name: 45
interface: 'ext_foreign_toplevel_list_v1', version: 1, name: 46
...
```

This output directly maps to the protocols in §10 (Compositor Support Matrix). In CI, `wayland-info` can validate that the headless compositor under test actually advertises the expected protocols before running the test suite.

**Programmatic equivalent:** PlatynUI's runtime performs the same protocol discovery via `wl_registry` global enumeration during compositor connection (see §6.3 and §14). The `wayland-info` output is the human-readable version of what the runtime sees.

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
├────────────┬──────────────┬───────────┬───────────┤
│  Control   │    libei     │ wlr-proto │  Portal   │
│  Socket    │    (reis)    │(wayland-cl)│  (zbus)  │
│ [PlatynUI] │ [GNOME+KDE] │ [wlroots] │ [fallbk] │
└────────────┴──────────────┴───────────┴───────────┘
```

**Runtime detection:** On `initialize()`, the crate probes the compositor for supported protocols via `wl_registry`, checks for an EIS socket, and detects the compositor type (via `SO_PEERCRED` on the Wayland socket). Based on the results, it selects the best backend for each capability using a priority chain per compositor type:

| Compositor Type | Backend Priority |
|---|---|
| PlatynUI | ControlSocket → EIS → Portal |
| Mutter, KWin | Portal → EIS |
| Other (Sway, Hyprland, …) | EIS → Portal → VirtualInput |

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

> **Last updated:** 2026-03-10

### Phase 1: Foundation (MVP) — ✅ Complete

**Goal:** Basic automation under Mutter, KWin, and wlroots-based compositors — input injection on all major compositor families.

| Component | Implementation | Status |
|---|---|---|
| `PlatformModule` | `wayland-client` connection, protocol negotiation via `wl_registry`, compositor type detection via `SO_PEERCRED` | ✅ Done |
| `DesktopInfoProvider` | `wl_output` + `zxdg_output_manager_v1` enumeration, physical pixel positioning, D-Bus enrichment (Mutter/KWin metadata) | ✅ Done |
| `PointerDevice` + `KeyboardDevice` (libei) | `reis` 0.6 crate, direct EIS socket + XDG Portal `RemoteDesktop.ConnectToEIS()` — input path for Mutter + KWin | ✅ Done |
| `PointerDevice` + `KeyboardDevice` (wlr) | `zwlr_virtual_pointer_v1` + `zwp_virtual_keyboard_v1` with compositor seat keymap, local XKB state for modifier tracking — input path for wlroots-based compositors | ✅ Done |
| `PointerDevice` + `KeyboardDevice` (control socket) | Custom JSON-over-Unix-socket protocol for PlatynUI compositor | ✅ Done |
| Backend selection | Compositor-type-aware priority chain with automatic fallback | ✅ Done |
| CI setup (Weston) | `scripts/startwaylandsession.sh` | ✅ Done |
| CI setup (Sway) | `scripts/startsway.sh` — isolated Sway session for wlr-protocol testing | ✅ Done |

**Implementation notes:**
- The EIS backend connects directly via `$LIBEI_SOCKET` or well-known paths in `$XDG_RUNTIME_DIR`. Key resolution uses the same `KEY_MAP` (80+ named keys) + `KeymapLookup` as the wlr backend.
- The Portal backend wraps EIS: it acquires an EIS file descriptor via `RemoteDesktop.ConnectToEIS()` and delegates all input operations to `EisBackend`. Persistence tokens avoid repeated consent dialogs.
- The wlr virtual-keyboard backend uploads the compositor's own keymap (received via `wl_keyboard::Event::Keymap`) and maintains a local `xkb::State` to send explicit `modifiers` events after each key — required because wlroots sets `update_state = false` for virtual keyboards.
- The control-socket backend uses fire-and-forget JSON messages over a Unix domain socket, designed for the custom PlatynUI Wayland compositor.

### Phase 1b: Screenshots & Window Basics — Not Started

**Goal:** Screenshot capability and basic window management for automation workflows.

| Component | Implementation | Effort |
|---|---|---|
| `ScreenshotProvider` (portal) | `org.freedesktop.portal.Screenshot` via `zbus` — works on Mutter, KWin, Sway | M |
| `ScreenshotProvider` (ext) | `ext-image-copy-capture-v1` — standard protocol for KWin, Sway 1.11+, Hyprland, niri, cosmic | M |
| `WindowManager` (protocol layer) | `wlr-foreign-toplevel-management` + `ext-foreign-toplevel-list` + AT-SPI PID matching → activate, close, minimize, maximize, restore, resolve_window, is_active | M |
| `WindowManager` (Mutter focus-based) | activate via D-Bus/AT-SPI + pointer clicks use window-relative AT-SPI coords + libei (no compositor IPC needed — see §6.5) | M |
| CI setup (Mutter) | `mutter --headless --virtual-monitor 1920x1080` | S |

### Phase 2: Full Window Management & Highlights

**Goal:** KWin/Sway IPC for window bounds, highlight overlays.

| Component | Implementation | Effort |
|---|---|---|
| `WindowManager` (KWin IPC) | Window bounds/move/resize via KWin D-Bus scripting API (see §6.2) | M |
| `WindowManager` (Sway IPC) | Window bounds/move/resize via i3 IPC socket (see §6.2) | M |
| `WindowManager` (Hyprland IPC) | Window bounds/move/resize via hyprctl socket (see §6.2) | S |
| `HighlightProvider` | `ext-layer-shell-v1` for KWin + `wlr-layer-shell` for wlroots-based + `tiny-skia` rendering | M |
| `ScreenshotProvider` (legacy) | `wlr-screencopy` fallback for older wlroots (<0.19) | S |
| CI setup (KWin) | KWin headless testing | M |

### Phase 3: Platform Mediation & Broad Coverage

**Goal:** Unified X11/Wayland platform, full CI matrix.

| Component | Implementation | Effort |
|---|---|---|
| Platform mediation crate | `platform-linux` routing X11/Wayland per-application (§13) | L |
| CI matrix | Mutter + KWin + Sway headless matrix in CI pipeline | M |

**Effort legend:** S = small (1-2 days), M = medium (3-5 days), L = large (1-2 weeks).

### Custom Test Compositor — ✅ Implemented Separately

The custom Smithay-based compositor described in §11.7 has been implemented as `apps/wayland-compositor` (~15K LoC). It provides:
- Headless and windowed (Winit) backends
- EIS server via `reis` for libei-based input injection
- XWayland support
- Control socket IPC (`apps/wayland-compositor-ctl`) for test orchestration
- Full protocol coverage (virtual pointer/keyboard, layer-shell, foreign-toplevel, etc.)
- Egui test application (`apps/test-app-egui`) with AccessKit accessibility for integration testing

This compositor is already usable for development and will become the primary deterministic test environment once the CI matrix is set up.

---

## 16. Open Questions

1. ~~**Window bounds on Mutter:**~~ Accepted: Mutter does not expose window geometry. Focus-based pointer strategy (§6.5) with window-relative AT-SPI coordinates is the production approach. Screenshot correlation remains a future enhancement option.
2. ~~**reis API stability:**~~ Pinned to `reis` 0.6.x. The EIS backend wraps `reis` behind the `InputBackend` trait, isolating the rest of the crate from API changes.
3. **Mutter headless + libei + portals in CI:** Does `mutter --headless` provide a functional EIS endpoint for libei-based input injection and xdg-desktop-portal for screenshots — both without user consent dialogs? Test tokens via `org.freedesktop.portal.RemoteDesktop` may be needed. Needs validation with actual CI runners.
4. **Highlight without coordinates:** If we can't know window screen position (Mutter, cosmic), should highlights be rendered as image overlays in screenshots rather than live compositor overlays? Compositor IPC provides window geometry for KWin/Sway/Hyprland. On Mutter, software overlay on a captured screenshot or no highlight are the options.
5. ~~**Mixed session routing:**~~ Deferred to Phase 3 (`platform-linux` mediation crate). See §13 for the proposed detection algorithm.
6. **Mutter protocol gap:** Mutter/GNOME still does not implement `ext-image-copy-capture`, `ext-foreign-toplevel-list`, or layer-shell protocols. Portal-based fallbacks remain the only option for GNOME. Monitor whether GNOME 50+ adds any of these.
7. **Newton project status:** The Newton Wayland-native accessibility project (§3) has had no public updates since Jun 2024. The draft Wayland accessibility protocol is not accepted into wayland-protocols. Monitor whether this project continues and whether it could eventually replace AT-SPI for Wayland-native apps.
8. ~~**Custom test compositor scope:**~~ Resolved: Implemented as standalone binary (`apps/wayland-compositor`) with companion control CLI (`apps/wayland-compositor-ctl`). Supports headless, Winit, and DRM backends. ~15K LoC, Smithay-based, with EIS, XWayland, and full protocol support.
9. **AT-SPI `GetExtents` coordinate mode switching:** The `provider-atspi` crate currently uses `GetExtents(SCREEN)`. Under Wayland, it should use `GetExtents(WINDOW)` and combine with window position from compositor IPC (or `(0, 0)` fallback). This requires detecting Wayland vs X11 inside the provider — see §13 for the proposed detection algorithm.

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
- [wayland-info / wayland-utils (protocol inspector)](https://gitlab.freedesktop.org/wayland/wayland-utils)
- [wev (Wayland event viewer)](https://git.sr.ht/~sircmpwn/wev)
- [wlr-randr (monitor configuration)](https://sr.ht/~emersion/wlr-randr/)
- [wl-clipboard (clipboard access)](https://github.com/bugaevc/wl-clipboard)
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
