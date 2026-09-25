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

## Logging

The Rust core logs through `tracing`. From Python, those diagnostics go to the standard `logging` module and, when PlatynUI runs under Robot Framework, into the Robot Framework log of the keyword during which they happened. The implementation is `packages/native/src/log_bridge.rs` on the Rust side and `src/PlatynUI/core/native_logging.py` on the library side.

**Where records go.** Each record is emitted under a logger named after the Rust module that logged it, below `platynui.native`: an event from `platynui_provider_atspi::extents` arrives on `platynui.native.provider_atspi.extents`, one from zbus on `platynui.native.zbus.…`. So `logging.getLogger("platynui")` covers both the Python-side loggers (`platynui.devices`, …) and the native ones, and a single subsystem can be filtered on its own. Rust levels map one to one onto Python's (`error`→`ERROR`, `warn`→`WARNING`, `info`→`INFO`, `debug`→`DEBUG`). `trace` becomes level 5, named `TRACE` unless something else has already named it. Robot Framework shows level 5 as `TRACE` under `--loglevel TRACE`. The message text carries the module, the message and the structured fields (`[provider_atspi.extents] … window=… call=…`), because Robot Framework shows only the text. The fields are also attached to the record as `native_fields` for Python handlers.

**When records are delivered, and why on the calling thread.** The subscriber the extension installs at import does nothing but queue: it never calls Python from the thread that logged. That is a requirement, not a detail. Robot Framework records library messages only from the thread that runs the keyword and silently drops the rest, and several native threads log — the Wayland event loop, the AT-SPI popup watcher, zbus executors. Native calls also hold the GIL for their whole duration, and some of them join such a thread while holding it, so a logging thread that waited for the GIL would deadlock the call. Queued records are delivered on the calling thread instead, at three points:

- after every `Runtime` method, once the runtime's lock has been released, so a log handler may call back into the same runtime;
- around every keyword of the Robot Framework libraries, before it runs and after it returns (`OurDynamicCore.run_keyword`), so a record appears in the keyword during which it was logged, or at the latest in the next PlatynUI keyword;
- on `platynui_native.flush_logs()`.

Node and pattern methods do not deliver; their records wait for the next delivery point. Plain Python code that works mostly with nodes calls `flush_logs()` when it wants to see them. A record from another thread than the one delivering it names that thread and the time it was logged in its message, because Robot Framework stamps a message with its delivery time. The queue holds 10 000 records. When it is full, newer records are dropped, and the next delivery reports how many were lost.

**How much is produced.** By default only `warn` and `error`, independent of Robot Framework's own `--loglevel`. More detail is requested explicitly, with the command-line tool's sources and precedence:

1. `RUST_LOG` — filter directives, used verbatim;
2. the requested level — the `PlatynUI.BareMetal` import argument `native_log_level`, or `platynui_native.set_log_level()` from plain Python. It lowers PlatynUI's own crates only (`warn,platynui=<level>`); third-party crates stay at `warn`;
3. `PLATYNUI_LOG_LEVEL` — filter directives, used verbatim;
4. `warn`.

An environment value that does not parse is skipped as though absent and reported as a warning naming the variable. The environment is read again whenever the level changes. The level is process-wide, because the subscriber is. Each library instance registers its `native_log_level` as a request, and while several are live, the most verbose one applies. An instance's request ends when Robot Framework closes the instance's scope (the library listener's `close()`), and that also delivers what its runtime logged last. To see native debug output in Robot Framework, both are needed: `native_log_level=debug` and an RF log level that keeps `DEBUG`.

A native `warn` becomes a Robot Framework warning and so also appears on the console and among the run's errors. That is intended for warnings a user should act on. Messages that are expected in a healthy session belong at `debug` (see `.github/instructions/tracing.instructions.md`).

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
