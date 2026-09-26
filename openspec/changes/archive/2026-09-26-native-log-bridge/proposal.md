# Proposal

## Why

Every diagnostic the Rust core emits is thrown away when PlatynUI runs from Python. The native extension (`packages/native`) installs no `tracing` subscriber, so every `tracing` event in the process goes to no one (`packages/native/src/lib.rs:12-21` only registers types; nothing in `packages/native/src` or its library crates installs a subscriber). That includes messages written precisely so that a user can diagnose a failure:

- the warning `wayland-sidecar-capabilities` adds when a window's bounds fall back to the toolkit's geometry — on Wayland the only thing that explains a click missing by the window offset;
- the Wayland compositor identification record, and the AT-SPI process-identity decision;
- the debug records `runtime-session-config` *requires* for ignored config keys (`openspec/specs/runtime-session-config/spec.md`: "SHALL be recorded at debug log level") — today a mistyped key is silent from Python;
- the result of the user-facing XPath function `fn:trace()`, which exists only to be seen (`crates/xpath/src/engine/functions/diagnostics.rs:23`).

The CLI and the Inspector show all of this on stderr (`crates/cli/src/lib.rs:33-59`); a Robot Framework run shows none of it. Robot Framework users debug from the RF log, so a diagnostic that does not reach that log does not exist for them.

Getting it there is not a matter of plugging in a stock bridge. Robot Framework records library messages only from its main thread and silently drops everything else (`robot/output/librarylogger.py:45` in RF 7.5), and several native threads log — the Wayland event loop, the AT-SPI popup watcher, zbus executors. Native calls also hold the GIL for their whole duration (no `detach` anywhere in `packages/native/src`), and some of them join those threads while holding it, so a bridge that calls into Python from the logging thread would deadlock the run.

## What Changes

- **The native extension installs a `tracing` subscriber that forwards events to Python's `logging` module.** Events are converted into log records carrying the level, the originating Rust module, the message and its structured fields, and are emitted under the logger hierarchy `platynui.native.*` (for example `platynui.native.provider_atspi.extents`).
- **Records are delivered on the Python thread that calls into PlatynUI, never from the thread that logged.** The subscriber only queues a record — it never touches Python, so it cannot block or deadlock a native thread. Queued records are delivered on the calling thread when a native call returns, and at the start and end of every PlatynUI keyword, so an event from a background thread appears in the log of the keyword during which it happened (or the next PlatynUI keyword at the latest). A record from a thread other than the delivering one names its thread and the time it was logged.
- **Native messages appear in the Robot Framework log with their own level.** Native `ERROR` becomes an RF `ERROR`, `WARN` an RF `WARN` — so they also appear on the console and under *Test Execution Errors* — `INFO`/`DEBUG`/`TRACE` their RF counterparts. RF's `--loglevel` and `Set Log Level` still decide what the log keeps.
- **By default only native `WARN` and `ERROR` are produced** (maintainer decision: a fixed floor, independent of RF's log level). More detail is opt-in:
  - a new `PlatynUI.BareMetal` import argument, `native_log_level` (`error`/`warn`/`info`/`debug`/`trace`), which lowers the floor for PlatynUI's own crates;
  - the environment variables the CLI already uses, `PLATYNUI_LOG_LEVEL` and `RUST_LOG`, with the CLI's precedence and directive syntax, for per-crate control including third-party crates.
  Third-party crates (zbus, async-io, …) stay at `WARN` unless an environment directive says otherwise. Levels that are not enabled cost nothing: their events are neither formatted nor queued.
- **The queue is bounded.** When it overflows, further records are dropped and the next delivery reports how many were lost as a warning, so a flood cannot exhaust memory and never goes unnoticed.
- **Native warnings in healthy runs are reviewed.** Because native warnings now become RF warnings, the change reviews every native warning the mock, X11 and compositor lanes produce and lowers the level of those that are not actionable in a healthy session, so a warning in the RF log means something.
- **Plain Python users benefit too.** Code that uses `platynui_native` directly gets the same records through `logging`; the extension exposes the level setting and an explicit flush for them.

No keyword, return value or existing argument changes. Nothing is removed.

## Capabilities

### New Capabilities

- `native-logging`: how diagnostics emitted by the native core reach Python's `logging` and the Robot Framework log — delivery on the calling Python thread (including records from native background threads), level mapping, the default `WARN` floor and how to lower it, the logger hierarchy and record content, the guarantee that logging never blocks or deadlocks native code, and the bounded queue with reported overflow.

### Modified Capabilities

None. `runtime-session-config` already requires its debug records; this change makes them observable from Python without changing that requirement.

## Impact

- **`packages/native`** (Rust + Python stubs): a new logging module — subscriber layer, bounded queue, level filter with reload, delivery into Python `logging` — installed at module init (`src/lib.rs`); a per-call delivery point after the runtime lock is released (`PyRuntime::runtime()`, `src/runtime.rs:711`, and the equivalent node/pattern paths); new Python-facing functions for the level and an explicit flush (`_native.pyi`, `platynui_native/__init__.py`). New normal dependency `tracing-subscriber` (features `registry`, `env-filter`, `std`), declared per crate as `.github/instructions/tracing.instructions.md:12` requires. **Needs a native rebuild.**
- **`src/PlatynUI`**: `OurDynamicCore` (`src/PlatynUI/_our_libcore.py`) delivers pending records before and after every keyword it dispatches, which covers `PlatynUI.BareMetal` and the high-level `PlatynUI` library; `BareMetal.__init__` gains the `native_log_level` import argument and its documentation.
- **Rust library crates**: no API change. Individual `warn!` calls may be lowered to `debug!`/`info!` where the lane review finds them non-actionable.
- **Rules and docs**: `.github/instructions/tracing.instructions.md` (the Python extension becomes the third place allowed to install a subscriber, and the level table gains "a WARN reaches the RF console"), `dev-docs/python-bindings.md` (logging section), the BareMetal import-argument documentation.
- **Tests**: Rust unit tests for the layer, queue and filter; pytest for delivery into `logging` (calling thread, background thread, overflow, precedence); a pytest-driven Robot Framework run that checks the RF output for native records inside the right keyword; lane review in the mock, X11 and compositor lanes.
- **Platforms**: all. The bridge lives in the extension and is platform-neutral; the threads that log in the background differ per platform (Wayland event loop and AT-SPI popup watcher on Linux, the JAB pump on Windows). The Windows lane review needs a Windows machine.
- **Behavior change, not breaking:** RF logs start containing native `WARN`/`ERROR` messages, and RF runs that were silent may now show warnings on the console and under *Test Execution Errors*. No test outcome changes because of it.
