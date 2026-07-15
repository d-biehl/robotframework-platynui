# Robot Framework PlatynUI

Cross-platform native UI automation for Robot Framework.

> [!WARNING]
> Preview quality. Packages, keywords, CLI output, and platform behavior may change before the first stable release.

## What is PlatynUI?

PlatynUI is intended to become the Robot Framework automation layer for native desktop applications. It should let test suites inspect real application windows, find controls by stable queries, read UI state, perform user-like actions, and capture diagnostic evidence without binding tests to one operating system or accessibility technology.

Instead of exposing Windows UIA, Linux AT-SPI2, macOS AX, and test doubles as separate worlds, PlatynUI normalizes them into one desktop UI model. Tests and tools query that model with XPath-like selectors and then act through platform capabilities such as focus, window control, pointer input, keyboard input, highlighting, and screenshots.

The project is organized around:

- A Robot Framework-first library surface, with the current low-level `PlatynUI.BareMetal` library and a higher-level `PlatynUI` library still in migration.
- A shared Rust UI model with `control`, `item`, `app`, and `native` namespaces.
- An XPath 2.0-inspired query engine tailored to native desktop UI trees.
- Native providers for Windows UIA and Linux AT-SPI2, an experimental Java Access Bridge provider for Windows Swing/AWT apps, planned macOS AX support, and mock providers for deterministic tests.
- Platform devices for user-like interaction, screenshots, highlights, desktop metadata, and window management.
- A CLI (`platynui-cli`) and GUI inspector (`platynui-inspector`) for diagnostics, exploration, and query development.
- Python bindings (`platynui-native`) that connect the Rust runtime to Robot Framework and Python tests.

## Current status

- **Rust core:** active implementation for the UI model, XPath engine, runtime, CLI, platform devices, and providers.
- **CLI and Inspector:** preview tools are available as binary Python packages and from local source builds.
- **Robot Framework library:** the high-level `Library    PlatynUI` entry point is still a placeholder while the migration continues. Use `Library    PlatynUI.BareMetal` for the current low-level keyword surface.
- **Python requirement:** Python 3.12 or newer. The native extension uses PyO3 `abi3-py312`.
- **Rust requirement:** Rust 1.95 or newer, Rust 2024 edition.

## Install preview tools

Install pre-release tool packages explicitly. The examples below assume `uv` 0.11.7 or newer. For user-level command-line tools, `uv tool` is the most convenient path:

```sh
uv tool install --prerelease allow platynui-cli
uv tool install --prerelease allow platynui-inspector
```

Inside an existing virtual environment, install the packages directly:

```sh
uv pip install --pre platynui-cli platynui-inspector
# or
pip install --pre platynui-cli platynui-inspector
```

The Robot Framework library package (`robotframework-PlatynUI`) is not yet published as the stable end-user package. For local development, see [CONTRIBUTING.md](CONTRIBUTING.md).

Try the tools:

```sh
platynui-cli list-providers
platynui-cli info --format json
platynui-cli query "//control:Window"
platynui-cli keyboard list | head -n 20
platynui-cli keyboard type "Hello <Ctrl+A>\\u00A7"
platynui-cli snapshot "//control:Window" --pretty
platynui-cli snapshot "//control:Window" --format xml --output windows.xml
platynui-inspector
```

Useful Inspector options:

```sh
platynui-inspector --search-result-limit 5000
platynui-inspector --search-result-limit unlimited
PLATYNUI_INSPECTOR_SEARCH_RESULT_LIMIT=5000 platynui-inspector
```

The Inspector shows the first 5000 XPath search results by default. Use `--search-result-limit <COUNT|unlimited>` or `PLATYNUI_INSPECTOR_SEARCH_RESULT_LIMIT=<COUNT|unlimited>` to override that guard. Command-line options take precedence over environment variables.

## Platform support

| Component | Windows | Linux X11 | Linux Wayland | macOS | Mock |
|-----------|---------|-----------|---------------|-------|------|
| UI tree provider | ✅ UIA | ✅ AT-SPI2 | ✅ AT-SPI2 with Wayland coordinate limits | ❌ AX stub | ✅ |
| Java Swing/AWT provider | ⚠️ experimental JAB (Java Access Bridge) | ✅ via AT-SPI2 (java-atk-wrapper) | ✅ via AT-SPI2 (java-atk-wrapper) | — (JDK implements AX natively) | — |
| Pointer | ✅ SendInput | ✅ XTest | ⚠️ EIS / portal / virtual-input / test-compositor backends | ❌ stub | ✅ |
| Keyboard | ✅ SendInput | ✅ XTest | ⚠️ EIS / portal / virtual-input / test-compositor backends | ❌ stub | ✅ |
| Desktop info | ✅ Win32 | ✅ XRandR/root geometry | ⚠️ `wl_output` plus compositor enrichment | ❌ stub | ✅ |
| Screenshot | ✅ GDI | ✅ XGetImage | ❌ not implemented yet | ❌ stub | ✅ |
| Highlight | ✅ layered window | ✅ override-redirect windows | ⚠️ PlatynUI test compositor only | ❌ stub | ✅ |
| Window management | ✅ Win32 | ⚠️ partial EWMH | ⚠️ PlatynUI test compositor only | ❌ stub | ✅ |
| Inspector | ✅ | ✅ | ⚠️ experimental through Linux mediator | ❌ | ✅ with mock feature |
| Inspector live mouse picker | ✅ UIA | ✅ AT-SPI | ❌ not yet (no live cursor position) | ❌ AX stub | — |

Linux uses `platynui-platform-linux` as a runtime session mediator. It detects X11 vs Wayland from the environment and delegates to the matching backend. X11 remains the most complete Linux path today. Wayland support is active but experimental; see the working notes under [dev-docs/](dev-docs/) and [apps/wayland-compositor/docs/](apps/wayland-compositor/docs/) for current protocol work.

## Package docs

- [packages/cli/README.md](packages/cli/README.md) - CLI package and command overview.
- [packages/inspector/README.md](packages/inspector/README.md) - GUI inspector package and usage notes.
- [packages/native/README.md](packages/native/README.md) - native Python bindings and mock-provider setup.
- [crates/xpath/README.md](crates/xpath/README.md) - XPath engine notes.
- [apps/wayland-compositor/README.md](apps/wayland-compositor/README.md) - test compositor overview.
- [apps/wayland-compositor-ctl/README.md](apps/wayland-compositor-ctl/README.md) - compositor control CLI.

## Documentation

Developer, design, and planning documentation lives under [dev-docs/](dev-docs/) — architecture, platform internals, input, testing strategy, Python bindings, and planning notes (see [dev-docs/README.md](dev-docs/README.md) for an index). The [docs/](docs/) directory is reserved for user-facing documentation, which is still being built out.

Additional working docs live next to some components:

- [apps/wayland-compositor/docs/](apps/wayland-compositor/docs/) - Wayland test compositor usage, configuration, and control protocol notes.
- [crates/xpath/docs/](crates/xpath/docs/) - XPath engine coverage notes.

## Contributing

Contributions are welcome. Start with these guides:

- [CONTRIBUTING.md](CONTRIBUTING.md) - setup, contribution expectations, `just` task runner workflow, coding standards, testing guidance, PR checklist, and packaging notes.

The short version is: keep changes focused, use Conventional Commits, run the relevant `just` checks, and update docs when behavior changes.

## License

Apache-2.0. See `LICENSE` in this repository.
