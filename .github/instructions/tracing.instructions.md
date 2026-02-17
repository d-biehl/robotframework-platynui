---
description: 'Logging and tracing conventions for all Rust crates.'
applyTo: '**/*.rs'
---

# Logging & Tracing

PlatynUI uses the [`tracing`](https://docs.rs/tracing) ecosystem for structured, leveled diagnostics across all Rust crates. This document defines the mandatory conventions.

## 1. Dependency Specification

Every crate that emits log messages adds `tracing` to its own `Cargo.toml` (**not** via `[workspace.dependencies]` — maturin is incompatible with workspace deps):

```toml
[dependencies]
tracing = { version = "0.1", default-features = false, features = ["std"] }
```

Binary crates (CLI, Inspector) additionally depend on the subscriber:

```toml
tracing-subscriber = { version = "0.3", default-features = false, features = ["fmt", "env-filter"] }
```

Always use `default-features = false` with the exact feature set shown above. Do not add extra features without explicit justification.

## 2. Log Levels

| Level   | Purpose                                                                                               | Examples                                                       |
|---------|-------------------------------------------------------------------------------------------------------|----------------------------------------------------------------|
| `error` | Unexpected failure that is **not** already surfaced as a `Result::Err` to the caller.                 | `SendInput` failed, `BitBlt` failed, unsupported image depth   |
| `warn`  | Degraded operation, fallbacks in use, slow calls (>200 ms), missing optional capabilities.           | RANDR missing, using fallback desktop info, connect timeout    |
| `info`  | One-time lifecycle events — emitted at most once per program run.                                    | Runtime initialized/shutdown, platform initialized, bus connected |
| `debug` | Operational details useful during development or field diagnosis.                                     | Provider count, device discovery, XPath expression, pointer coords |
| `trace` | Hot-path per-item iteration — extremely verbose; normally only enabled to debug specific subsystems.  | Per-node AT-SPI resolution, per-app enumeration                |

### Rules of Thumb

- **Never** log at `error` for something already returned as an error to the caller — the caller decides what to do.
- `info` messages should be rare and read like milestones in a lifecycle log.
- If a message would fire once per UI node or once per event-loop tick, use `trace`.
- When in doubt between `debug` and `trace`, ask: "Would this be noisy with 500 nodes?" If yes → `trace`.

## 3. Subscriber Setup (Binary Crates Only)

Library crates **never** initialize a subscriber — they only emit events. Subscriber initialization lives in the two binary entry points: `crates/cli/src/lib.rs` and `apps/inspector/src/lib.rs`.

The subscriber uses:
- `tracing_subscriber::fmt()` with `env_filter`
- `with_target(true)` — shows the emitting module path
- `with_writer(std::io::stderr)` — keeps diagnostics off stdout (stdout is reserved for command output)

### Log Level Priority Chain (highest wins)

1. **`RUST_LOG`** environment variable — fine-grained per-crate filtering (e.g., `RUST_LOG=platynui_runtime=debug,platynui_xpath=trace`).
2. **`--log-level`** CLI argument (values: `error`, `warn`, `info`, `debug`, `trace`).
3. **`PLATYNUI_LOG_LEVEL`** environment variable — simple single-level override.
4. **Default: `warn`**.

Reference implementation:

```rust
fn init_tracing(cli_level: Option<LogLevel>) {
    use tracing_subscriber::EnvFilter;

    let filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        let directive = if let Some(level) = cli_level {
            log_level_directive(level)
        } else if let Ok(val) = std::env::var("PLATYNUI_LOG_LEVEL") {
            val
        } else {
            "warn".to_string()
        };
        EnvFilter::new(directive)
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_writer(std::io::stderr)
        .init();
}
```

The `--log-level` argument is defined via `clap`:

```rust
#[arg(long = "log-level", value_enum, global = true)]
log_level: Option<LogLevel>,
```

## 4. Emitting Events

### Import Style

For crates that use many tracing macros across multiple functions, prefer importing the macros:

```rust
use tracing::{debug, info, warn, error, trace};
```

For crates where tracing calls are sparse, use the fully qualified path:

```rust
tracing::debug!(count = entries.len(), "discovered provider factories");
```

Both forms are acceptable; stay consistent within each module.

### Structured Fields

Always use structured key-value fields rather than format strings when possible:

```rust
// Good — structured, filterable
tracing::debug!(xpath, cached = options.cache().is_some(), "xpath evaluate");
tracing::info!(providers = count, "Runtime initialized");
tracing::warn!(pid, "no X11 window found for PID");

// Avoid — unstructured, harder to query
tracing::debug!("xpath evaluate: {} (cached={})", xpath, cached);
```

### Field Formatting

- **Direct value**: `field = value` — works for types implementing `tracing::Value` (integers, bools, `&str`).
- **Display**: `field = %value` — uses the `Display` trait.
- **Debug**: `field = ?value` — uses the `Debug` trait.

```rust
tracing::debug!(display = %disp, screen = screen_num, root, "X11 connection established");
tracing::debug!(button = ?target_button, clicks, target = ?target, "pointer click");
tracing::error!(%err, "platform module initialization failed");
```

### Naming Pitfall

Do **not** name a local variable `display` when using it with the `%` format specifier — the tracing macro interprets `%display` as a call to `tracing::field::display()`, causing a conflict. Rename the local, for example to `disp`:

```rust
// BAD — conflicts with tracing::field::display()
let display = get_display();
tracing::info!(%display, "connected");

// GOOD
let disp = get_display();
tracing::info!(display = %disp, "connected");
```

## 5. Per-Layer Instrumentation Guide

When adding tracing to a new crate or module, follow these patterns:

### Runtime (`crates/runtime`)
- `info!` for `Runtime::new()` completion and `shutdown()`.
- `debug!` for provider discovery count, device selection, platform module init.
- `debug!` for pointer actions (move_to, click, scroll, drag) with coordinates.
- `debug!` for keyboard execute with mode and segment count.
- `warn!` for fallbacks (no DesktopInfoProvider, stuck keyboard keys, position-ensure failures).
- `error!` for provider failures that are silently skipped (e.g., `get_nodes` in `DesktopNode::children`).

### XPath Pipeline (`crates/runtime/src/xpath.rs`)
- `debug!` for `evaluate` and `evaluate_iter` with the XPath expression and cache status.

### Platform Crates (`crates/platform-*`)
- `info!` on successful initialization (once per platform module lifetime).
- `debug!` for extension availability probes (XTEST, RANDR, DPI awareness).
- `warn!` for missing optional extensions or connect failures in background threads.
- `error!` for unrecoverable conditions (unsupported image depth, SendInput failure).

### Provider Crates (`crates/provider-*`)
- `info!` for bus/connection established (once).
- `debug!` for XID resolution, window actions (activate, close).
- `warn!` for lookup failures (no window for PID, walker creation failed).
- `trace!` for per-node and per-app iteration details.

### New Crates
When creating a new crate:
1. Add `tracing = { version = "0.1", default-features = false, features = ["std"] }` to the crate's `Cargo.toml`.
2. Add at least `info!` for lifecycle events and `warn!`/`error!` for failure paths.
3. Review the level table above to select appropriate levels.
4. **Do not** initialize a subscriber — that is the responsibility of binary crates.
