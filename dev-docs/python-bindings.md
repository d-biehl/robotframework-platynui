# Python Bindings

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document covers the Python/PyO3 bindings for PlatynUI (`platynui_native`). For the platform-agnostic architecture, see `dev-docs/architecture.md`.

## Architecture

Single native wheel `platynui_native` built with PyO3 + maturin. All types are registered in a single flat module (`platynui_native._native`), re-exported via `platynui_native` — except `PatternName`, which is intentionally not re-exported to avoid colliding with the `PlatynUI.core.types.PatternName: TypeAlias = str` alias (Python user code talks the str alias; the wrapper stays internal at `platynui_native._native.PatternName`):

- Core types: `Point`, `Size`, `Rect`, `PatternName` (internal-only wrapper), `RuntimeId`, `TechnologyId`, `Namespace` enum. All implement `__eq__`/`__ne__`/`__hash__`.
- Runtime types: `Runtime`, `UiNode`, decomposed pattern wrappers (`Focusable`, `Activatable`, `Minimizable`, `Maximizable`, `Restorable`, `Closeable`, `Movable`, `Resizable`, `Responsive`), pointer/keyboard APIs, evaluation iterators.

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

## Runtime Configuration

`Runtime(config=None)` takes an optional construction-time dict that binds the runtime to a specific session. `None` or an empty dict reproduces the environment-derived default (platform auto-detected, each provider discovers its own connection). The dict has two id-keyed buckets — `platform` and `providers` — fanned out to the matching platform/provider factory (see `architecture.md` §3–§4):

```python
Runtime({
    "platform": {"backend": "x11", "x11": {"display": ":1"}},
    "providers": {"atspi": {"bus_address": "unix:path=…"}},
})
```

Leaf values convert `str`→string, `bool`→boolean (checked *before* `int`, since Python `bool` subclasses `int`), `int`→integer, `float`→float, `dict`→nested map, `list`/`tuple`→list. Keys, ids, or whole sections a backend does not recognize — another OS's block, a typo, a non-dict section — are ignored with a debug-level log rather than raising, so one dict stays portable across platforms. The config is consumed once at construction and is immutable for the runtime's life; there is no re-bind. The Robot Framework surface exposes it as `BareMetal(config=…)`.

## Threading & GIL

- `Runtime`: `Send + Sync`
- XDM Cache: a single runtime-owned `xpath_cache: Mutex<XdmCache>` where `XdmCache` wraps `Arc<Mutex<Option<(RuntimeId, RuntimeXdmNode)>>>` and is `Send + Sync + Clone`, so the one cache is shared across threads while preserving explicit invalidation semantics
- `UiNode`: `Send + Sync` (wraps `Arc<dyn UiNode>`)

## Exceptions

All custom exceptions inherit from `PlatynUiError` (which extends `Exception`):

| Exception | Description |
|-----------|-------------|
| `PlatynUiError` | Base exception for all PlatynUI errors |
| `ProviderError` | UI tree provider errors |
| `EvaluationError` | XPath evaluation failures |
| `PointerError` | Pointer/mouse operation failures |
| `KeyboardError` | Keyboard input failures |
| `PatternError` | Pattern action failures (focus, window, etc.) |
| `AttributeNotFoundError` | Requested attribute does not exist on node |

## Build & Distribution

- Backend: maturin with PyO3 (`extension-module`, `abi3-py312`)
- Feature: `mock-provider` for local development
- Developer workflow: `uv sync --dev` + `maturin develop -m packages/native/Cargo.toml --release`
- CI builds wheels for Linux/macOS/Windows
