# Test-Control IPC Protocol

## Overview

The PlatynUI Wayland Compositor exposes a **Unix domain socket** for
programmatic control and introspection.  It is designed for CI test harnesses,
the companion CLI tool (`platynui-wayland-compositor-ctl`), and the future
Platform-Crate (`crates/platform-linux-wayland`).

## Transport

- **Socket type:** Unix stream socket (SOCK_STREAM).
- **Framing:** newline-delimited JSON — one JSON object per line (`\n`).
- **Encoding:** UTF-8.
- **Connection lifetime:** the compositor processes all commands on the
  connection synchronously, then closes it.  Open a new connection for each
  batch of commands.

## Socket Path

The control socket is created by default when the compositor starts.
Use `--no-control-socket` to disable it.

**Convention:**

```
$XDG_RUNTIME_DIR/<WAYLAND_DISPLAY>.control
```

For example, if `WAYLAND_DISPLAY=wayland-0`:

```
/run/user/1000/wayland-0.control
```

### Environment Variable

The compositor exports `PLATYNUI_CONTROL_SOCKET` into the process environment
so that child processes and tools running inside the session can discover the
socket path without deriving it manually.

### Discovery (CLI tool)

1. Explicit path via `--socket <path>`.
2. `PLATYNUI_CONTROL_SOCKET` environment variable (set by the compositor).
3. Derived automatically: `$XDG_RUNTIME_DIR` + `$WAYLAND_DISPLAY` + `.control`.

## Request Format

```json
{"command": "<command_name>", "param1": value1, ...}
```

All requests are JSON objects with a `"command"` field.  Additional parameters
depend on the command.

## Response Format

### Success

```json
{"status": "ok", ...}
```

### Error

```json
{"status": "error", "message": "<human-readable description>"}
```

## Commands

### `status`

Compositor status — returns version, uptime, backend info, window counts, and output configuration.
The `ping` command is an alias for `status`.

**Request:**
```json
{"command": "status"}
```

**Response:**
```json
{
  "status": "ok",
  "version": "0.12.0-dev.5",
  "backend": "winit",
  "uptime_secs": 154,
  "socket": "wayland-0",
  "xwayland": true,
  "windows": 3,
  "minimized": 1,
  "outputs": [
    {
      "index": 0,
      "name": "WL-1",
      "width": 1920,
      "height": 1080,
      "x": 0,
      "y": 0,
      "scale": 1.0
    }
  ]
}
```

**Fields:**

| Field          | Type   | Description                                             |
|----------------|--------|---------------------------------------------------------|
| `version`      | string | Compositor version (from `Cargo.toml`)                  |
| `backend`      | string | Active backend: `"headless"`, `"winit"`, or `"drm"`     |
| `uptime_secs`  | int    | Seconds since compositor started                        |
| `socket`       | string | Wayland socket name                                     |
| `xwayland`     | bool   | Whether XWayland is active                              |
| `windows`      | int    | Number of mapped (visible) windows                      |
| `minimized`    | int    | Number of minimized windows                             |
| `outputs`      | array  | List of output configurations                           |

---

### `shutdown`

Request a graceful compositor shutdown.

**Request:**
```json
{"command": "shutdown"}
```

**Response:**
```json
{"status": "ok", "message": "shutting down"}
```

---

### `list_windows`

List all currently mapped (visible) and minimized windows.

**Request:**
```json
{"command": "list_windows"}
```

**Response:**
```json
{
  "status": "ok",
  "windows": [
    {
      "id": 0,
      "title": "Kate",
      "app_id": "org.kde.kate",
      "x": 100,
      "y": 50,
      "width": 800,
      "height": 600,
      "focused": true,
      "maximized": false,
      "fullscreen": false
    }
  ],
  "minimized": [
    {
      "id": "minimized_0",
      "title": "Terminal",
      "app_id": "org.gnome.Terminal",
      "x": 200,
      "y": 150
    }
  ]
}
```

**Fields per window:**

| Field        | Type   | Description                                            |
|--------------|--------|--------------------------------------------------------|
| `id`         | int    | Window index (0-based, changes when windows open/close)|
| `title`      | string | Window title (from `xdg_toplevel` or X11 `_NET_WM_NAME`)|
| `app_id`     | string | Application ID (from `xdg_toplevel` or X11 class)     |
| `x`          | int    | X position in logical compositor coordinates           |
| `y`          | int    | Y position in logical compositor coordinates           |
| `width`      | int    | Window width in logical pixels                         |
| `height`     | int    | Window height in logical pixels                        |
| `focused`    | bool   | Whether this window has keyboard focus                 |
| `maximized`  | bool   | Whether this window is maximized                       |
| `fullscreen` | bool   | Whether this window is fullscreen                      |

---

### `get_window`

Get details of a specific window.

Windows can be identified by numeric index, `app_id` (exact match), or
`title` (case-insensitive substring match).  When multiple selectors are
provided, priority is: `id` → `app_id` → `title`.  If `app_id` does not
match, the compositor falls through to `title` matching.

**Request (by index):**
```json
{"command": "get_window", "id": 0}
```

**Request (by app\_id):**
```json
{"command": "get_window", "app_id": "org.kde.kate"}
```

**Request (by title):**
```json
{"command": "get_window", "title": "Kate"}
```

**Parameters:**

| Field    | Type   | Required | Description                                  |
|----------|--------|----------|----------------------------------------------|
| `id`     | int    | no*      | Window index                                 |
| `app_id` | string | no*      | Application ID (exact match)                 |
| `title`  | string | no*      | Window title (case-insensitive substring)    |

\* At least one selector must be provided.

**Response (success):**
```json
{
  "status": "ok",
  "window": {
    "id": 0,
    "title": "Kate",
    "app_id": "org.kde.kate",
    "x": 100,
    "y": 50,
    "width": 800,
    "height": 600,
    "focused": true,
    "maximized": false,
    "fullscreen": false
  }
}
```

**Response (not found):**
```json
{"status": "error", "message": "window not found"}
```

---

### `close_window`

Send a close request to a window (the application may show a "save?" dialog).
Accepts the same window selectors as `get_window`.

**Request:**
```json
{"command": "close_window", "id": 1}
```

```json
{"command": "close_window", "app_id": "org.kde.kate"}
```

**Response:**
```json
{"status": "ok", "message": "close sent", "title": "Kate", "app_id": "org.kde.kate"}
```

---

### `focus_window`

Activate and raise a window (gives it keyboard focus).
Accepts the same window selectors as `get_window`.

**Request:**
```json
{"command": "focus_window", "id": 0}
```

```json
{"command": "focus_window", "app_id": "org.kde.kate"}
```

**Response:**
```json
{"status": "ok", "message": "window focused", "title": "Kate", "app_id": "org.kde.kate"}
```

---

### `screenshot`

Capture the entire compositor output as a PNG image.

For multi-output setups with mixed scales, the screenshot uses the maximum
output scale so HiDPI content remains sharp.

**Request:**
```json
{"command": "screenshot"}
```

**Response:**
```json
{
  "status": "ok",
  "format": "png",
  "width": 1920,
  "height": 1080,
  "scale": 1.0,
  "data": "<base64-encoded PNG>"
}
```

**Fields:**

| Field   | Type   | Description                                       |
|---------|--------|---------------------------------------------------|
| `format`| string | Always `"png"`                                    |
| `width` | int    | Image width in physical pixels                    |
| `height`| int    | Image height in physical pixels                   |
| `scale` | float  | Scale factor used (max across all outputs)        |
| `data`  | string | Base64-encoded (RFC 4648) PNG image data           |

## Error Handling

| Condition                 | Response                                                  |
|---------------------------|-----------------------------------------------------------|
| Missing `command` field   | `{"status":"error","message":"missing or invalid command field"}` |
| Unknown command           | `{"status":"error","message":"unknown command: <name>"}` |
| Window not found          | `{"status":"error","message":"window not found"}`        |
| Screenshot failure        | `{"status":"error","message":"screenshot failed: <detail>"}` |

## CLI Tool

The companion CLI tool `platynui-wayland-compositor-ctl` provides a
user-friendly command-line interface to this protocol, with human-readable
output by default and a `--json` flag for machine-readable output.

### Window Identifiers

Window commands accept flexible identifiers:
- A **number** (e.g. `0`, `2`) refers to the window index from `list-windows`
- A **string** (e.g. `firefox`, `foot`) matches first by `app_id` (exact),
  then by window title (case-insensitive substring)

### Examples

```bash
# Compositor status
platynui-wayland-compositor-ctl status

# List windows (human-readable table)
platynui-wayland-compositor-ctl list-windows

# List windows (JSON)
platynui-wayland-compositor-ctl --json list-windows

# Get window details by index, app_id, or title
platynui-wayland-compositor-ctl get-window 0
platynui-wayland-compositor-ctl get-window firefox
platynui-wayland-compositor-ctl get-window "My Document"

# Focus window by app_id
platynui-wayland-compositor-ctl focus firefox

# Close window by title
platynui-wayland-compositor-ctl close "Unsaved Document"

# Take screenshot (auto-generated filename)
platynui-wayland-compositor-ctl screenshot

# Take screenshot (explicit filename)
platynui-wayland-compositor-ctl screenshot -o screenshot.png

# Shutdown
platynui-wayland-compositor-ctl shutdown

# Use explicit socket path
platynui-wayland-compositor-ctl --socket /run/user/1000/wayland-0.control status
```

## Versioning

This protocol is currently unversioned (v0).  A version field will be added
in a future handshake when breaking changes are needed.  For now, unknown
commands return an error response, allowing forward-compatible clients.
