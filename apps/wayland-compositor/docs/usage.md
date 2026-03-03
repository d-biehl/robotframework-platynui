# Usage Reference

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
| `--config <path>` | — | Path to TOML configuration file (see [Configuration](configuration.md)) |
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

## Window Decorations

The compositor supports both decoration modes:

- **Server-Side Decorations (SSD):** Compositor renders title bars with close (✕), maximize (□),
  and minimize (─) buttons. Used by Qt/KDE apps and X11 apps via XWayland.
- **Client-Side Decorations (CSD):** Apps draw their own decorations (GTK4/LibAdwaita/GNOME apps).
  The compositor correctly handles CSD resize via shadow regions and client-initiated grabs.

### Window Management

- Click on title bar → drag to move
- Click close button → close window
- Click maximize → toggle maximize (saves/restores position)
- Click minimize → hide window
- Resize via invisible borders (8px) around SSD windows
- CSD apps handle their own resize
