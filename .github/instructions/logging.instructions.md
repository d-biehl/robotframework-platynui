---
description: 'Logging checklist for Rust and Python code: levels, log-or-return, once per episode, fields, loggers, entry points.'
applyTo: '**/*.rs,**/*.py'
---

# Logging Checklist

These are the non-negotiable logging rules, one line each. The concept behind them — why each rule
exists, with worked examples — is [`dev-docs/logging.md`](../../dev-docs/logging.md); read it
before adding a new kind of record. The rules apply to the PlatynUI library (native core,
platforms, providers, Python bindings, Robot Framework library), the CLI and the Inspector. The
development tools (Wayland compositor, EIS test client, egui test app) keep their own setup.

## 1. Dependencies

Every Rust crate that logs adds `tracing` to its own `Cargo.toml` — **not** through
`[workspace.dependencies]`, which maturin cannot handle:

```toml
[dependencies]
tracing = { version = "0.1", default-features = false, features = ["std"] }
```

- `platynui-core` has no `tracing` dependency. Its diagnostic building blocks
  (`platynui_core::diagnostics::Transitions`, `platynui_core::ui::describe`) return facts; the
  caller logs.
- The entry points build their filter with `platynui-log-filter` (`crates/log-filter`):
  - the CLI enables its `fmt` feature, for `init_stderr`;
  - the Inspector enables `fmt` and `log`. Its GUI stack (eframe, egui, wgpu, winit) logs through
    the `log` crate, and `log` makes `try_init` install the `LogTracer` that forwards those records
    into the same filter. Never call `LogTracer::init` by hand;
  - the Python extension (`packages/native`) depends on it without features, and on
    `tracing-subscriber` with `["registry", "env-filter", "std"]` for its queueing subscriber.
- Always `default-features = false` with exactly the features shown; add others only with a written
  reason. Workspace builds unify features, so a workspace build of the CLI forwards `log` records
  too; that only widens which crates' records it can show, and is accepted.

## 2. Levels

One meaning per level, the same in Rust and Python (`dev-docs/logging.md` §3):

- **error** — something PlatynUI had to do failed unexpectedly and was swallowed, so results or state are wrong and nothing else reports it; state the consequence; never for a failure that is returned or raised.
- **warn** — PlatynUI knowingly works with less than it was asked for (environment, configuration, target application) and says what the user loses; never in a healthy session; never for a returned or raised failure. A fallback that is normal in some sessions is debug, not warn.
- **info** — a lifecycle transition of a long-lived native resource (runtime created or shut down, Java agent injected), once per transition, never per operation; Python code logs nothing at info.
- **debug** — what one operation did or decided; a fallback normal in some sessions; a call slower than its call class's threshold; a returned failure with its context; a keyword action line.
- **trace** — one record per element, tick, key or character, or message.

## 3. Where and how often

- **Log or return:** a layer that returns a failure (`Err`, exception, failed keyword) logs it at most at debug; only the layer that swallows it or decides its consequence logs it above debug.
- **Once per episode:** a warning or error that can recur on every call goes through `platynui_core::diagnostics::Transitions` — warn/error when the episode starts, debug while it continues, one debug record naming the subject when it ends; once-per-process reports use `std::sync::Once`.
- **Consequence:** a warning or error names its subject and states what the user loses, after a semicolon: `"provider failed to list its top-level elements; its elements are missing from query results"`.
- **Typed text:** a record that names a key, character, key code or keysym, or carries a keyboard device's error text, is `trace` only, however rarely it fires; keywords never repeat the text they are given to type.
- **Elements:** describe an element with `platynui_core::ui::describe` (Python: `UiNode.describe()`), in an `element` or `window` field; never make `repr`/`str` ask the provider.
- **Decisions:** a record of a choice names what was chosen and why; the rejected alternatives go to debug.
- **Configuration:** a platform or provider checks its own configuration section at the start of its build, before anything can fail (`ConfigMap::unknown_keys`, `try_str`/`try_bool`/`try_i64`/`try_map`), and warns per unknown key and per wrongly typed value, naming `component` and `key`, then applies the default.

## 4. Emitting events (Rust)

### Import Style

For modules that log a lot, import the macros:

```rust
use tracing::{debug, error, info, trace, warn};
```

Where calls are sparse, use the fully qualified path:

```rust
tracing::debug!(count = entries.len(), "discovered provider factories");
```

Both forms are fine; stay consistent within a module.

### Structured Fields

Values go into fields, never into the message. Messages are lower-case English fragments without a
trailing period and without a type, function or module prefix; proper names (AT-SPI, X11, JAB) keep
their case. Use the field names of `dev-docs/logging.md` §11 (`error`, `provider`, `component`,
`key`, `application`, `bus_name`, `pid`, `call`, `elapsed_ms`, `timeout_ms`, `element`, `window`,
`position`, …).

```rust
// Good — structured and filterable
tracing::debug!(xpath, cached = options.cache().is_some(), "xpath evaluate");
tracing::info!(backend = "x11", forced = false, providers = ?ids, desktop = %bounds, monitors = 2, "runtime initialized");
// The lookup failure is returned to the caller: debug, not warn.
tracing::debug!(pid, "no X11 window found for the process");

// Avoid — values interpolated into the message
tracing::debug!("xpath evaluate: {} (cached={})", xpath, cached);
```

### Field Formatting

- **Direct value**: `field = value` — for types implementing `tracing::Value` (integers, bools, `&str`).
- **Display**: `field = %value` — uses `Display`.
- **Debug**: `field = ?value` — uses `Debug`.
- **Errors**: always `error = %err` — never `err = …`, `e = …` or a bare `%err`.

```rust
tracing::debug!(display = %disp, screen = screen_num, root, "X11 connection established");
tracing::debug!(button = ?target_button, clicks, target = ?target, "pointer click");
// Swallowed by the enumeration, reported once per episode: error, with its consequence.
tracing::error!(provider = id, error = %err, "provider failed to list its top-level elements; its elements are missing from query results");
```

### Naming Pitfall

Do **not** name a local variable `display` when using it with the `%` specifier — the macro reads
`%display` as a call to `tracing::field::display()`. Rename the local, for example to `disp`:

```rust
// BAD — conflicts with tracing::field::display()
let display = get_display();
tracing::debug!(%display, "connected");

// GOOD
let disp = get_display();
tracing::debug!(display = %disp, "connected");
```

## 5. Producer rules

- A native component logs under a target that starts with `platynui` (its module path does that by default); an overridden `target:` outside that prefix is not reached by the level knob.
- Diagnostics are events that carry their context in their own fields, not spans: spans are not carried to Python or Robot Framework.
- Expensive message parts (an element description, a formatted tree) are built only when their level is enabled — inside the macro's fields, or behind `isEnabledFor` in Python.

## 6. Entry points

- Library crates never install a subscriber.
- The CLI and the Inspector install theirs with `platynui_log_filter::init_stderr`; output goes to stderr, stdout is for command output.
- Their `--log-level` is an optional argument with `value_parser = platynui_log_filter::parse_level`: case-insensitive, `off` and the Python spellings (`warning`, `critical`, `fatal`) included.
- The Python extension builds its filter through `platynui-log-filter` too; its queueing subscriber lives in `packages/native/src/log_bridge.rs`.

## 7. Python

- Log through `logging.getLogger("platynui.<module path>")` in lower case — the module's `__name__.lower()`, such as `platynui.core.adapter_devices` or `platynui.baremetal`; keyword modules included.
- `robot.api.logger` only for keyword output that needs Robot Framework (such as an embedded screenshot), never for diagnostics.
- No diagnostic at INFO — Robot Framework shows INFO by default; lifecycle records go to DEBUG.
- Values go into the message through lazy `%` arguments; guard arguments that cost a provider call (such as `describe()`) with `isEnabledFor`.
- `warnings.warn` only for the Python API (`platynui_native`, `PlatynUI.core`, `PlatynUI.ui`): `DeprecationWarning` or a `UserWarning` subclass. A deprecation a Robot Framework user can hit is a warning in the log, once per run and use site.
