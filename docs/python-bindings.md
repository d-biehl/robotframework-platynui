# Python Bindings

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document covers the Python/PyO3 bindings for PlatynUI (`platynui_native`). For the platform-agnostic architecture, see `docs/architecture.md`.

## Architecture

Single native wheel `platynui_native` built with PyO3 + maturin. All types are registered in a single flat module (`platynui_native._native`), re-exported via `platynui_native`:

- Core types: `Point`, `Size`, `Rect`, `PatternId`, `RuntimeId`, `TechnologyId`, `Namespace` enum. All implement `__eq__`/`__ne__`/`__hash__`.
- Runtime types: `Runtime`, `UiNode`, pattern wrappers (`Focusable`, `WindowSurface`), pointer/keyboard APIs, evaluation iterators.

## Type Conversion

| Rust (`UiValue`) | Python |
|------------------|--------|
| Null | `None` |
| Bool | `bool` |
| Integer | `int` |
| Float/Number | `float` |
| String | `str` |
| Point | `Point` |
| Size | `Size` |
| Rect | `Rect` |
| Array | `list` |
| Object | `dict` |

## Threading & GIL

- `Runtime`: `Send + Sync`
- XDM Cache: thread-local `HashMap<u64, XdmCache>` per `PyRuntime` instance (because `XdmCache` is `!Send`)
- `UiNode`: `Send + Sync` (wraps `Arc<dyn UiNode>`)

## Exceptions

All custom exceptions inherit from `PlatynUiError` (which extends `Exception`):

| Exception | Description |
|-----------|-------------|
| `PlatynUiError` | Base exception for all PlatynUI errors |
| `EvaluationError` | XPath evaluation failures |
| `ProviderError` | UI tree provider errors |
| `PointerError` | Pointer/mouse operation failures |
| `KeyboardError` | Keyboard input failures |
| `PatternError` | Pattern action failures (focus, window, etc.) |
| `AttributeNotFoundError` | Requested attribute does not exist on node |

## Build & Distribution

- Backend: maturin with PyO3 (`extension-module`, `abi3-py310`)
- Feature: `mock-provider` for local development
- Developer workflow: `uv sync --dev` + `maturin develop -m packages/native/Cargo.toml --release`
- CI builds wheels for Linux/macOS/Windows
