# platynui-wayland-compositor-ctl

CLI tool for controlling a running PlatynUI Wayland compositor via its test-control IPC socket.

Analogous to `swaymsg` or `hyprctl`, but for the PlatynUI compositor.

## Usage

```bash
# Compositor status (version, uptime, backend, outputs)
platynui-wayland-compositor-ctl status

# List windows (human-readable table)
platynui-wayland-compositor-ctl list-windows

# List windows (JSON output for scripting)
platynui-wayland-compositor-ctl --json list-windows

# Get window details (by index, app_id, or title)
platynui-wayland-compositor-ctl get-window 0
platynui-wayland-compositor-ctl get-window firefox
platynui-wayland-compositor-ctl get-window "My Document"

# Focus/close windows by flexible identifier
platynui-wayland-compositor-ctl focus firefox
platynui-wayland-compositor-ctl close 1

# Screenshot (auto-generated filename)
platynui-wayland-compositor-ctl screenshot

# Screenshot (explicit filename)
platynui-wayland-compositor-ctl screenshot -o screenshot.png

# Shutdown
platynui-wayland-compositor-ctl shutdown
```

## Window Identifiers

Window commands (`get-window`, `focus`, `close`) accept flexible identifiers:

- A **number** (e.g. `0`, `2`) refers to the window index from `list-windows`
- A **string** (e.g. `firefox`, `foot`) matches first by `app_id` (exact match),
  then by window title (case-insensitive substring match)

## Output Format

By default, output is human-readable with colored formatting (when connected
to a terminal). Use `--json` / `-j` for machine-readable JSON output.

## Socket Discovery

The tool finds the control socket automatically:

1. `--socket <path>` — explicit path
2. `PLATYNUI_CONTROL_SOCKET` environment variable (set by the compositor)
3. `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY.control` — derived from environment
