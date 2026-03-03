# platynui-wayland-compositor-ctl

CLI tool for controlling a running [PlatynUI Wayland compositor](../wayland-compositor/README.md)
from the command line or from scripts.

## What It Does

- Query compositor status (version, uptime, backend, outputs)
- List and inspect windows
- Focus, close, or manipulate windows by index, app-ID, or title
- Take screenshots
- Shut down the compositor

## Examples

```bash
# Compositor status
platynui-wayland-compositor-ctl status

# List windows
platynui-wayland-compositor-ctl list-windows

# JSON output for scripting
platynui-wayland-compositor-ctl --json list-windows

# Focus a window by app-ID
platynui-wayland-compositor-ctl focus firefox

# Screenshot
platynui-wayland-compositor-ctl screenshot -o screenshot.png

# Shutdown
platynui-wayland-compositor-ctl shutdown
```

## Socket Discovery

The tool connects to the compositor’s control socket. Discovery order:

1. `--socket <path>` — explicit path
2. `PLATYNUI_CONTROL_SOCKET` env var (set automatically by the compositor)
3. Derived from `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY.control`

See the [IPC Protocol documentation](../wayland-compositor/docs/ipc-protocol.md)
for the full command reference.
