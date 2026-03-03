# PlatynUI Wayland Compositor

Smithay-based Wayland compositor for PlatynUI integration testing.
A minimal but functional Wayland compositor that can host GTK/Qt/X11 applications
for automated UI testing.

## Status

**Phase 2 complete** — the compositor is fully functional with 24 Wayland protocols,
three backends (headless, winit, DRM), server-side decorations with close/maximize/minimize
buttons, XWayland support, test-control IPC, multi-monitor, client
cursor shapes, keyboard layout configuration, TOML configuration file support,
and child program lifecycle management.

Verified working with: Kate (Qt6), gtk4-demo, gnome-text-editor, Nautilus (LibAdwaita/CSD),
X11 apps via XWayland.

## Quick Start

```bash
# Build (all backends compiled by default)
cargo build -p platynui-wayland-compositor

# Run nested in a window (development)
cargo run -p platynui-wayland-compositor -- --backend winit

# Run nested with XWayland + environment output
cargo run -p platynui-wayland-compositor -- --backend winit --xwayland --print-env

# Run headless (CI)
cargo run -p platynui-wayland-compositor -- --backend headless --timeout 300

# Launch child program inside the compositor (exits when child exits)
cargo run -p platynui-wayland-compositor -- --backend headless --exit-with-child -- gtk4-demo

# Use a custom configuration file
cargo run -p platynui-wayland-compositor -- --config /path/to/compositor.toml --backend winit

# Launch an app inside the compositor (manually, from another terminal)
WAYLAND_DISPLAY=wayland-1 gtk4-demo
```

## Backends

### Headless (`--backend headless`)

Off-screen rendering. No window is displayed. Ideal for CI pipelines
and automated testing where no visual output is needed.
Set `LIBGL_ALWAYS_SOFTWARE=1` for environments without a hardware GPU.

```bash
cargo run -p platynui-wayland-compositor -- --backend headless
```

### Winit (`--backend winit`)

Nested compositor running inside a window on your current desktop (X11 or Wayland).
Best for development and interactive testing.

```bash
cargo run -p platynui-wayland-compositor -- --backend winit
```

### DRM (`--backend drm`)

Direct hardware rendering on a real TTY using DRM/KMS + libinput + libseat.
This is the production backend for running on bare metal without another display server.

**Requirements:**
- A running seat manager: **systemd-logind**, **elogind**, or **seatd**
- Must be started from a **real TTY** (e.g. Ctrl+Alt+F2), not from within a graphical session
- Exclusive GPU access (DRM master) — no other compositor can run on the same GPU simultaneously

```bash
# Switch to a free TTY first (Ctrl+Alt+F3), then:
cargo run -p platynui-wayland-compositor -- --backend drm
```

**Starting from another session:**

The DRM backend cannot run inside an existing graphical session (GNOME, KDE, Sway, etc.)
because it needs exclusive DRM master access to the GPU. Options:

| Method | Command | Notes |
|--------|---------|-------|
| **Free TTY** (recommended) | `Ctrl+Alt+F3` → login → run | Simplest approach |
| **seatd-launch** | `seatd-launch -- target/release/platynui-wayland-compositor --backend drm` | Starts a dedicated seat daemon; install `seatd` first |
| **SSH + VT switch** | `ssh host` → `sudo chvt 3` → run | Needs logind session on the target VT |

**Troubleshooting:**

| Error | Cause | Fix |
|-------|-------|-----|
| `Function not implemented (os error 38)` | No seat manager available | Install/start `seatd` or ensure `logind` is running |
| `Permission denied` | Not on a real TTY or missing seat access | Switch to a TTY with Ctrl+Alt+F2 |
| `Device or resource busy` | Another compositor holds DRM master | Stop the other compositor or switch VT |

> **Tip:** For development and functional testing, use `--backend winit` instead. Reserve
> the DRM backend for hardware-specific testing or production use.

## CLI Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--backend <headless\|winit\|drm>` | `headless` | Rendering backend |
| `--width <px>` | `1920` | Virtual output width |
| `--height <px>` | `1080` | Virtual output height |
| `--socket-name <name>` | auto | Wayland socket name |
| `--log-level <level>` | `warn` | Log level (error/warn/info/debug/trace) |
| `--ready-fd <N>` | — | Write `READY\n` to file descriptor N |
| `--print-env` | `false` | Print `WAYLAND_DISPLAY=...` (and `DISPLAY=...` with XWayland) on stdout when ready |
| `--timeout <secs>` | `0` | Auto-shutdown after N seconds (0 = off) |
| `--xwayland` | `false` | Enable XWayland for running X11 applications |
| `--control-socket` | `false` | Enable test-control IPC socket |
| `--outputs <N>` | `1` | Number of virtual monitors |
| `--output-layout <horizontal\|vertical>` | `horizontal` | Multi-monitor arrangement |
| `--scale <factor>` | `1.0` | Scale factor for virtual outputs. Applied to all outputs created via `--outputs`. For per-output scale, use the TOML configuration file. |
| `--window-scale <factor>` | `1.0` | Scale the winit preview window (e.g. `0.5` to halve). Only affects rendering resolution, not client-visible output scale. Useful to fit large multi-output setups on screen. |
| `--restrict-protocols <ids>` | — | Comma-separated app-ID whitelist for privileged protocols |
| `--config <path>` | — | Path to TOML configuration file (see [Configuration File](#configuration-file)) |
| `--exit-with-child` | `false` | Shut down compositor when the child program exits |
| `-- <command> [args...]` | — | Child program to launch after compositor readiness |

### Keyboard Layout

| Flag | Env Variable | Description |
|------|-------------|-------------|
| `--keyboard-layout` | `XKB_DEFAULT_LAYOUT` | Layout(s), comma-separated (e.g. `de,us,fr`) |
| `--keyboard-variant` | `XKB_DEFAULT_VARIANT` | Variant(s), positionally paired (e.g. `nodeadkeys,,neo`) |
| `--keyboard-model` | `XKB_DEFAULT_MODEL` | Physical model (e.g. `pc105`) |
| `--keyboard-rules` | `XKB_DEFAULT_RULES` | XKB rules file (e.g. `evdev`) |
| `--keyboard-options` | `XKB_DEFAULT_OPTIONS` | Options (e.g. `grp:alt_shift_toggle,compose:ralt`) |

Priority: CLI flag > config file (`[keyboard]`) > environment variable > XKB default.

### Log Level Priority

1. `RUST_LOG` env var (highest, fine-grained per-crate)
2. `--log-level` CLI flag
3. `PLATYNUI_LOG_LEVEL` env var
4. Default: `warn`

## CI Usage

```bash
# Start compositor in background with readiness notification
platynui-wayland-compositor --backend headless --timeout 300 --print-env --ready-fd 3 3>&1 &

# Wait for READY line, then run tests
WAYLAND_DISPLAY=wayland-1 your-test-command

# With XWayland for X11 apps
platynui-wayland-compositor --backend headless --xwayland --print-env --ready-fd 3 3>&1 &
# Output includes both WAYLAND_DISPLAY and DISPLAY
```

### Child Program Pattern

The `--exit-with-child` flag combined with trailing arguments (`-- <command>`) provides
a self-contained lifecycle for CI:

```bash
# Compositor starts → waits for readiness → launches pytest → exits when pytest exits
platynui-wayland-compositor --backend headless --exit-with-child -- python -m pytest tests/

# With XWayland and a custom config
platynui-wayland-compositor --backend headless --xwayland --exit-with-child \
    --config ci-compositor.toml -- your-test-suite
```

The child program inherits the compositor's environment (`WAYLAND_DISPLAY`, `DISPLAY` if
XWayland is enabled). The compositor polls the child process every 100 ms and shuts down
automatically when the child exits.

## Configuration File

The compositor can be configured via a TOML file. All settings are optional —
missing values use built-in defaults. CLI flags always override config file values.

### File Discovery

The config file is discovered in this order (first match wins):

1. `--config <path>` CLI flag (explicit path)
2. `$XDG_CONFIG_HOME/platynui/compositor.toml`
3. `~/.config/platynui/compositor.toml` (if `XDG_CONFIG_HOME` is unset)
4. No file → built-in defaults (the compositor runs fine without any config file)

### Complete Example

```toml
# ~/.config/platynui/compositor.toml

# ── Font ─────────────────────────────────────────────────────────────
# Font for compositor-rendered UI (title bars, panel).
# Resolved via fontconfig at runtime; falls back to egui's built-in font.
[font]
family = "Noto Sans"     # Font family name (default: "Noto Sans")
size = 13.0              # Font size in logical pixels (default: 13.0)

# ── Theme ────────────────────────────────────────────────────────────
# Colors for window decorations. CSS-style hex strings: #rrggbb or #rrggbbaa.
# Invalid values silently fall back to the built-in defaults.
[theme]
titlebar-background         = "#33333f"   # Inactive window title bar
titlebar-background-focused = "#404d73"   # Focused window title bar
titlebar-text               = "#ffffff"   # Title bar text
button-close                = "#d94040"   # Close (✕) button
button-maximize             = "#40bf59"   # Maximize (□) button
button-minimize             = "#e6bf33"   # Minimize (─) button
active-border               = "#7380b3"   # Focused window border
inactive-border             = "#595966"   # Unfocused window border

# ── Keyboard ─────────────────────────────────────────────────────────
# XKB keyboard configuration. Equivalent to XKB_DEFAULT_* env vars and
# --keyboard-* CLI flags.
[keyboard]
model   = "pc105"                        # XKB model (e.g. "pc105")
rules   = "evdev"                        # XKB rules file
options = "grp:alt_shift_toggle,compose:ralt"  # XKB options, comma-separated

# Multiple layouts as an array of tables. Each entry has a name and an
# optional variant. Layouts are joined with commas for XKB (de,us → "de,us").
[[keyboard.layout]]
name    = "de"
variant = "nodeadkeys"

[[keyboard.layout]]
name    = "us"
# variant omitted → default variant

# ── Outputs ──────────────────────────────────────────────────────────
# Virtual monitor definitions. Overrides --outputs/--width/--height CLI flags
# when present. Output entries are applied in order.
[[output]]
width  = 1920            # Width in pixels (default: 1920)
height = 1080            # Height in pixels (default: 1080)
x      = 0               # X position in the combined output space (default: 0)
y      = 0               # Y position in the combined output space (default: 0)
scale  = 1.0             # Scale factor, e.g. 1.0, 1.5, 2.0 (default: 1.0)

[[output]]
width  = 2560
height = 1440
x      = 1920
y      = 0
scale  = 1.5
```

### Section Reference

#### `[font]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `family` | string | `"Noto Sans"` | Font family name. Resolved via fontconfig; falls back to egui built-in. |
| `size` | float | `13.0` | Font size in logical pixels. |

#### `[theme]`

All theme values are CSS-style hex color strings: `#rrggbb` (opaque) or `#rrggbbaa` (with alpha).
Invalid values silently fall back to the built-in defaults listed below.

| Key | Default | Description |
|-----|---------|-------------|
| `titlebar-background` | `#33333f` | Title bar background for inactive windows |
| `titlebar-background-focused` | `#404d73` | Title bar background for the focused window |
| `titlebar-text` | `#ffffff` | Title bar text color |
| `button-close` | `#d94040` | Close button color |
| `button-maximize` | `#40bf59` | Maximize button color |
| `button-minimize` | `#e6bf33` | Minimize button color |
| `active-border` | `#7380b3` | Border color for the focused window |
| `inactive-border` | `#595966` | Border color for unfocused windows |

#### `[keyboard]`

| Key | Type | Default | Equivalent CLI / Env |
|-----|------|---------|---------------------|
| `model` | string | — | `--keyboard-model` / `XKB_DEFAULT_MODEL` |
| `rules` | string | — | `--keyboard-rules` / `XKB_DEFAULT_RULES` |
| `options` | string | — | `--keyboard-options` / `XKB_DEFAULT_OPTIONS` |

#### `[[keyboard.layout]]`

Array of tables. Each entry defines one XKB layout. Multiple entries are joined
with commas (e.g. `de,us`) and passed to XKB as a combined layout string.

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `name` | string | yes | XKB layout name (e.g. `de`, `us`, `fr`) |
| `variant` | string | no | XKB variant (e.g. `nodeadkeys`, `neo`). Omit for default. |

**Priority chain:** CLI flag (`--keyboard-layout`) > config file (`[[keyboard.layout]]`) > environment variable (`XKB_DEFAULT_LAYOUT`) > XKB compiled-in default.

#### `[[output]]`

Array of tables. Each entry defines one virtual monitor. When `[[output]]` entries
are present, they override `--outputs`, `--width`, and `--height` CLI flags.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `width` | integer | `1920` | Width in pixels |
| `height` | integer | `1080` | Height in pixels |
| `x` | integer | `0` | X position in combined output space |
| `y` | integer | `0` | Y position in combined output space |
| `scale` | float | `1.0` | Output scale factor (e.g. `1.0`, `1.5`, `2.0`) |

### Minimal Configurations

Only specify what you need — everything has sensible defaults:

```toml
# Just change the font
[font]
family = "DejaVu Sans"
```

```toml
# German keyboard with nodeadkeys
[[keyboard.layout]]
name = "de"
variant = "nodeadkeys"
```

```toml
# Dark red theme for close buttons
[theme]
button-close = "#cc3333"
```

## Window Decorations

The compositor supports both decoration modes:

- **Server-Side Decorations (SSD):** Compositor renders title bars with close (✕), maximize (□),
  and minimize (─) buttons. Used by Qt/KDE apps and X11 apps via XWayland.
- **Client-Side Decorations (CSD):** Apps draw their own decorations (GTK4/LibAdwaita/GNOME apps).
  The compositor correctly handles CSD resize via shadow regions and client-initiated grabs.

### Window Management

- Click on title bar → drag to move
- Click close button → `xdg_toplevel.close()` / X11 `WM_DELETE_WINDOW`
- Click maximize → toggle maximize (saves/restores position)
- Click minimize → hide window (restore via click on empty desktop; Phase 3 adds taskbar restore)
- Resize via invisible borders (8px) around SSD windows
- CSD apps handle their own resize via `xdg_toplevel.resize()`

## Architecture

```
src/
├── main.rs                           ← Binary entry point
├── lib.rs                            ← CLI (clap), tracing init, run()
├── state.rs                          ← State struct + all protocol initialization
├── config.rs                         ← TOML configuration file support
├── child.rs                          ← Child program spawning + exit monitoring
├── client.rs                         ← Per-client data (ClientState)
├── focus.rs                          ← KeyboardFocusTarget, PointerFocusTarget
├── workspace.rs                      ← Window placement (cascade), Space wrapper
├── input.rs                          ← Keyboard + pointer event routing + hit-testing
├── grabs.rs                          ← Interactive move + resize grabs (Wayland + X11)
├── decorations.rs                    ← SSD rendering, hit-testing, cursor shapes
├── render.rs                         ← Interleaved per-window render element collection
├── signals.rs                        ← SIGTERM/SIGINT handling + watchdog timer
├── ready.rs                          ← Readiness notification for CI
├── environment.rs                    ← XDG_RUNTIME_DIR, WAYLAND_DISPLAY setup
├── control.rs                        ← Test-control IPC (Unix socket + JSON)
├── multi_output.rs                   ← Multi-monitor support
├── security.rs                       ← Client permission filtering
├── xwayland.rs                       ← XWayland integration (X11 window management)
├── handlers/                         ← Wayland protocol handlers
│   ├── compositor.rs                 ← wl_compositor + wl_subcompositor
│   ├── shm.rs                        ← wl_shm buffer management
│   ├── dmabuf.rs                     ← linux-dmabuf-v1 (GPU buffers)
│   ├── output.rs                     ← wl_output + xdg-output-manager-v1
│   ├── seat.rs                       ← wl_seat (keyboard, pointer)
│   ├── xdg_shell.rs                  ← xdg_shell toplevels + popups + positioning
│   ├── decoration.rs                 ← xdg-decoration (SSD/CSD negotiation)
│   ├── selection.rs                  ← Clipboard (data_device + primary_selection)
│   ├── viewporter.rs                 ← wp-viewporter
│   ├── fractional_scale.rs           ← wp-fractional-scale-v1
│   ├── xdg_activation.rs            ← xdg-activation-v1
│   ├── pointer_constraints.rs        ← pointer-constraints-v1 + relative-pointer-v1
│   ├── single_pixel_buffer.rs        ← wp-single-pixel-buffer-v1
│   ├── presentation_time.rs          ← wp-presentation-time
│   ├── keyboard_shortcuts_inhibit.rs ← keyboard-shortcuts-inhibit-v1
│   ├── text_input.rs                 ← text-input-v3 + input-method-v2
│   ├── idle_notify.rs                ← ext-idle-notify-v1
│   ├── session_lock.rs               ← ext-session-lock-v1
│   ├── xdg_foreign.rs               ← xdg-foreign-v2
│   ├── security_context.rs           ← wp-security-context-v1
│   └── cursor_shape.rs               ← wp-cursor-shape-v1
└── backend/
    ├── mod.rs                        ← Backend module
    ├── winit.rs                      ← Nested compositor in a window
    ├── headless.rs                   ← Off-screen rendering (EGL on render node)
    └── drm.rs                        ← Direct hardware rendering (DRM/KMS + libinput)
```

## Implemented Protocols (24)

### Core Protocols

| Protocol | Purpose |
|----------|---------|
| `wl_compositor` + `wl_subcompositor` | Surface creation and sub-surfaces |
| `wl_shm` | Shared memory buffer management |
| `linux-dmabuf-v1` | GPU buffer sharing (DMA-BUF) |
| `wl_output` + `xdg-output-manager-v1` | Output info (size, scale, position) |
| `wl_seat` | Input devices (keyboard, pointer) |
| `xdg_shell` | Window management (toplevels + popups) |
| `xdg-decoration` | SSD/CSD decoration negotiation |
| `wl_data_device` + `primary_selection` | Clipboard and primary selection |

### App Compatibility Protocols

| Protocol | Purpose |
|----------|---------|
| `wp-viewporter` | Surface scaling (GTK4/Qt6/Chromium) |
| `wp-fractional-scale-v1` | HiDPI fractional scaling |
| `xdg-activation-v1` | Focus-stealing prevention |
| `pointer-constraints-v1` + `relative-pointer-v1` | Pointer lock/confine |
| `wp-single-pixel-buffer-v1` | Efficient solid-color surfaces |
| `wp-presentation-time` | Frame timing for video/animation |
| `keyboard-shortcuts-inhibit-v1` | Pass all keys to client |
| `text-input-v3` + `input-method-v2` | IME support (CJK/Compose/Emoji) |
| `ext-idle-notify-v1` | Idle detection |
| `ext-session-lock-v1` | Screen locking |
| `xdg-foreign-v2` | Cross-app window relationships |
| `wp-security-context-v1` | Sandboxed client support |
| `wp-cursor-shape-v1` | Server-side cursor shapes |

## Cargo Features

All backends (Winit, DRM/KMS, Headless) and XWayland support are compiled unconditionally — there are no optional Cargo features.

## Roadmap

- [x] **Phase 1**: Functional compositor (24 protocols, input, popups, clipboard)
- [x] **Phase 2**: SSD + XWayland + DRM + test-control IPC + multi-monitor + keyboard layout config
- [ ] **Phase 3**: Automation protocols (layer-shell, foreign-toplevel, EIS/libei, virtual-pointer/keyboard, screencopy, data-control, output-management)
- [ ] **Phase 4**: Platform crate (`crates/platform-linux-wayland/`)
- [ ] **Phase 5**: Built-in VNC/RDP for headless remote access
- [ ] **Phase 6**: Integration & CI test suite
- [ ] **Phase 7**: Documentation
- [ ] **Phase 8**: Portal & PipeWire *(optional)*
- [ ] **Phase 9**: Built-in panel + app launcher *(optional — layer-shell allows external panels like waybar)*

See [docs/plan-waylandCompositor.md](../../docs/plan-waylandCompositor.md) for the full plan.
