# Development Task Runner

PlatynUI uses [just](https://github.com/casey/just) as a command runner for common development tasks.

## Installation

Install `just` via one of these methods:

```bash
# Via cargo
cargo install just

# Via Homebrew (macOS/Linux)
brew install just

# Via system package manager
# Arch: pacman -S just
# Debian/Ubuntu (24.04+): apt install just
```

## Quick Start

```bash
# List all available recipes
just

# Bootstrap the development environment
just bootstrap

# Run the full pre-commit check sequence
just pre-commit
```

## Available Recipes

### Bootstrap

| Recipe | Description |
|---|---|
| `just bootstrap` | Install all Python and Rust dev dependencies via `uv sync` |

### Build

| Recipe | Description |
|---|---|
| `just build` | Build all Rust crates |
| `just build-native` | Build the native Python package |
| `just build-native mock-provider` | Build the native package with mock provider |
| `just build-cli` | Build the CLI Python package |
| `just build-inspector` | Build the Inspector Python package |

### Check & Lint

| Recipe | Description |
|---|---|
| `just fmt` | Format all Rust code |
| `just fmt-check` | Check Rust formatting without applying changes |
| `just clippy` | Run clippy with strict warnings |
| `just ruff` | Lint Python code |
| `just mypy` | Type-check Python code |
| `just check` | Run `fmt` + `clippy` + `ruff` |

### Test

| Recipe | Description |
|---|---|
| `just test` | Run all Rust tests |
| `just test-crate platynui-xpath` | Run tests for a specific crate |
| `just test-python` | Run Python tests |

### Desktop Integration (Linux)

PlatynUI ships `.desktop` files and icons for proper integration with Linux desktop environments (GNOME, KDE Plasma, etc.). This ensures window icons and application names display correctly under Wayland compositors that resolve icons via `app_id`.

| Recipe | Description |
|---|---|
| `just install-desktop` | Install `.desktop` files and icons to `$XDG_DATA_HOME` |
| `just uninstall-desktop` | Remove installed desktop files and icons |
| `just update-icon-cache` | Refresh the GTK icon cache |

The install targets respect `$XDG_DATA_HOME` (defaults to `~/.local/share`).

**Application IDs:**

| Application | App ID |
|---|---|
| Wayland Compositor | `org.platynui.compositor` |
| Inspector | `org.platynui.inspector` |
| Test App (egui) | `org.platynui.test.egui` |

### CI

| Recipe | Description |
|---|---|
| `just pre-commit` | Run the full check sequence: bootstrap, fmt, build, clippy, test, ruff |
