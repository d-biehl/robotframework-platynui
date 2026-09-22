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

### `show_highlight`

Shows one or more compositor-rendered highlight frames in logical desktop coordinates.
This is currently used by the Wayland platform backend for the PlatynUI compositor.

**Request:**
```json
{
  "command": "show_highlight",
  "rects": [
    {"x": 100, "y": 50, "width": 800, "height": 600},
    {"x": 950, "y": 200, "width": 300, "height": 120}
  ],
  "duration_ms": 1200
}
```

**Parameters:**

| Field         | Type   | Required | Description |
|---------------|--------|----------|-------------|
| `rects`       | array  | yes*     | Highlight rectangles in logical compositor coordinates |
| `duration_ms` | int    | no       | Optional auto-clear timeout in milliseconds |

\* As a convenience, a single rectangle can also be sent via top-level `x`, `y`, `width`, `height` fields.

**Response:**
```json
{"status": "ok", "message": "highlight updated", "rects": 2}
```

---

### `clear_highlight`

Removes any active compositor-rendered highlight frames.

**Request:**
```json
{"command": "clear_highlight"}
```

**Response:**
```json
{"status": "ok", "message": "highlight cleared"}
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
| `pid`        | int |null | Process the window's client belongs to, or `null` (see below) |
| `focused`    | bool   | Whether this window has keyboard focus                 |
| `maximized`  | bool   | Whether this window is maximized                       |
| `fullscreen` | bool   | Whether this window is fullscreen                      |

#### The `pid` field

Every entry that names a process — a window, a minimized window, a popup, and the
window at a point — carries `pid` with the same meaning and the same two sources:

- For a **Wayland client**, it is the process the compositor itself established
  from the peer credentials of the client's connection, when it accepted that
  connection.
- For an **X11 client through XWayland**, it is the process the client declared
  for itself in `_NET_WM_PID`. The compositor cannot verify that value and
  reports it as given; it is never the process of XWayland's own connection.

`pid` is `null` when the process is unknown. For a Wayland client that means the
compositor could not identify it — typically because the client lives in a PID
namespace the compositor cannot see, which is the normal situation when the
compositor and the automation runtime run in different containers; for an X11
client it means the client declared no process id, or declared `0`, which is not
a process.

The field is always present, and the compositor never reports `0`, a negative
number or any other placeholder: a consumer that filters by process can tell
"not identified" from a real process id without knowing how the identification
failed. Everything else about such an entry is reported as usual.

---

### `get_window`

Get details of a specific window.

Windows can be identified by stable `window_id`, numeric list index, `app_id` (exact match), or
`title` (case-insensitive substring match).  When multiple selectors are
provided, priority is: `window_id` → `id` → `app_id` → `title`.  If `app_id` does not
match, the compositor falls through to `title` matching.

**Request (by index):**
```json
{"command": "get_window", "id": 0}
```

**Request (by stable window id):**
```json
{"command": "get_window", "window_id": 123456789}
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
| `window_id` | int | no*      | Stable opaque window identifier for this compositor session |
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
    "window_id": 123456789,
    "title": "Kate",
    "app_id": "org.kde.kate",
    "pid": 4242,
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

### `window_at_point`

Ask which window is at a point in logical compositor coordinates.

**Request:**
```json
{"command": "window_at_point", "x": 640.0, "y": 480.0}
```

**Parameters:**

| Field | Type  | Required | Description                                |
|-------|-------|----------|--------------------------------------------|
| `x`   | float | yes      | X position in logical compositor coordinates |
| `y`   | float | yes      | Y position in logical compositor coordinates |

**Response:** `window` carries the same fields as `get_window`'s, or `null` when
no window is reported for that point:

```json
{"status": "ok", "window": {"id": 0, "window_id": 123456789, "app_id": "org.kde.kate", "pid": 4242}}
```

```json
{"status": "ok", "window": null}
```

A missing coordinate is the command's own error:

```json
{"status": "error", "message": "window_at_point requires x and y"}
```

A coordinate that is not a number never reaches that check — it fails request
parsing like any other malformed request, and the answer is
`{"status": "error", "message": "invalid JSON"}`.

**The asking process's own windows are skipped.** The answer is the frontmost
window at the point that the *asking* process does not own — the window
**behind** its own one, and `null` when there is none. "Asking" means the peer of
the control connection the request arrived on, so the same point can yield
different answers to different callers. This is what lets a picker resolve the
window under its own overlay.

A window is skipped **only when both identities are known and equal**. Nothing is
skipped when the compositor could not identify the window's client, when it could
not identify the caller (both are the case across sibling PID namespaces), or
when the window's process is only what an XWayland client declared about itself —
that value is not an identity the compositor established, so an X11 client cannot
make its window unpickable by claiming the caller's process id.

The skipping applies to this command alone. `list_windows`, `get_window` and
`list_popups` keep reporting the caller's own windows: they answer what exists,
not what is under a point.

---

### `close_window`

Send a close request to a window (the application may show a "save?" dialog).
Accepts the same window selectors as `get_window`.

### `focus_window`, `minimize_window`, `maximize_window`, `restore_window`, `move_window`, `resize_window`

Window-management actions using the same selectors as `get_window`.

**Requests:**
```json
{"command": "focus_window", "window_id": 123456789}
```

```json
{"command": "minimize_window", "window_id": 123456789}
```

```json
{"command": "maximize_window", "window_id": 123456789}
```

```json
{"command": "restore_window", "window_id": 123456789}
```

```json
{"command": "move_window", "window_id": 123456789, "x": 120, "y": 80}
```

```json
{"command": "resize_window", "window_id": 123456789, "width": 640, "height": 360}
```

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
