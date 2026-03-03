# PlatynUI Wayland Compositor

A Wayland compositor for automated UI testing with [PlatynUI](../../README.md) on Linux.
It provides a controlled display server environment where GTK, Qt, and X11 applications
can run — headless in CI or nested in a window during development.

## Why?

Automated UI tests on Linux need a display server. Running tests inside an existing
desktop session is fragile and non-reproducible. This compositor solves that by providing:

- **Reproducible environments** — same resolution, scale, keyboard layout, every time
- **Headless operation** — no physical display needed, ideal for CI/CD pipelines
- **XWayland support** — test X11 applications alongside native Wayland apps
- **Automation protocols** — programmatic input injection, screenshots, window enumeration
- **Test-control IPC** — query and control the compositor from test code

## Features

- Three backends: **headless** (CI), **winit** (nested window), **DRM** (bare metal)
- Server-side and client-side window decorations
- XWayland for X11 application support
- Multi-monitor setups with per-output scaling
- Configurable keyboard layouts
- TOML configuration file with theming support
- Child program lifecycle management (`--exit-with-child`)
- Readiness notification for CI synchronization (`--ready-fd`, `--print-env`)
- Test-control IPC via Unix socket (JSON protocol)
- Automation protocols: virtual keyboard/pointer, screencopy, foreign-toplevel,
  layer-shell, output management, clipboard control

## Quick Start

```bash
# Build
cargo build -p platynui-wayland-compositor

# Run nested in a window (development)
cargo run -p platynui-wayland-compositor -- --backend winit

# Run headless (CI)
cargo run -p platynui-wayland-compositor -- --backend headless --exit-with-child -- your-test-suite

# With XWayland for X11 apps
cargo run -p platynui-wayland-compositor -- --backend winit --xwayland --print-env
```

## CI Usage

```bash
# Self-contained: compositor starts, runs tests, exits when done
platynui-wayland-compositor --backend headless --exit-with-child -- python -m pytest tests/

# Or start in background with readiness notification
platynui-wayland-compositor --backend headless --print-env --ready-fd 3 3>&1 &
```

## Documentation

- [Usage Reference](docs/usage.md) — backends, CLI flags, CI patterns, window management
- [Configuration](docs/configuration.md) — TOML config file, theming, keyboard layouts, multi-monitor
- [IPC Protocol](docs/ipc-protocol.md) — test-control socket, JSON commands
- [Full Project Plan](../../docs/plan-waylandCompositor.md) — roadmap and design decisions
