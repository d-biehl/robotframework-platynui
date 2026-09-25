# Design

## Context

See proposal.md — Why for the motivation, and `specs/native-logging/spec.md` for the required behavior. This section records the current state and the constraints the approach has to work within. Everything marked *verified* was read in the working tree while this change was drafted; *inferred* marks conclusions drawn from verified facts but not exercised.

**The extension logs nowhere (verified).**

- `packages/native/src/lib.rs:12-21` registers types and nothing else.
- No crate in the extension's dependency tree installs a subscriber: there is no `set_global_default`, `tracing_subscriber`, `pyo3-log`, `tracing-log` or `env_logger` in `packages/native/src` or the library crates.
- `tracing-subscriber` is not in the extension's normal dependency tree. It is a normal dependency only of the binaries (`crates/cli/Cargo.toml:39`, `apps/inspector/Cargo.toml:33`, the compositor and the test apps) and a dev-dependency of three library crates.
- `tracing`'s `log` feature is off. The `log` crate reaches the tree only through `chrono`'s `iana-time-zone` and one calloop path in `reis`, so `log` records are not a concern.

**Calls hold the interpreter and a runtime lock (verified).**

- Nothing in `packages/native/src` calls `detach`, `allow_threads` or `attach`, so every Python-facing call holds the GIL for its whole duration.
- `PyRuntime` wraps `Mutex<runtime_rs::Runtime>` (`packages/native/src/runtime.rs:703-713`). Every runtime method takes that non-reentrant lock through one accessor, `PyRuntime::runtime()` (`:711`).
- Node and pattern methods (`runtime.rs:52-570`) call `self.inner` directly. There is no common accessor.
- pyo3 is 0.29 (`packages/native/Cargo.toml:22`, `Cargo.lock`), whose API is `Python::attach` / `try_attach` / `detach`.

**Native threads log (verified).**

- `wayland-events` (`crates/platform-linux-wayland/src/connection.rs:348-351`) logs from its dispatch loop and handlers. It is joined by `stop_global`.
- `atspi-popup-watch` (`crates/provider-atspi/src/popups.rs:213-216`) logs, including a `debug!` just before it exits (`:283`). It is joined by `PopupWatcher::stop()` from Drop, which runtime shutdown reaches (`crates/runtime/src/runtime/mod.rs:245-269`).
- zbus runs an executor thread per connection and traces there.
- The X11 highlight overlay thread (`crates/platform-linux-x11/src/highlight.rs:88`) warns when its connect fails.
- The JAB pump `platynui-jab-pump` (`crates/provider-java-jab/src/pump.rs:118-120`) logs on Windows.
- *Inferred:* a subscriber that took the GIL on the emitting thread would deadlock a runtime shutdown. The caller holds the GIL and joins `atspi-popup-watch`, which blocks on the GIL in its last `debug!`.

**Robot Framework drops foreign-thread messages (verified, RF 7.5 in `.venv`).**

- `robot/output/librarylogger.py:32,45` writes a message only when the current thread is `MainThread` or `RobotFrameworkTimeoutThread`. Everything else is discarded silently.
- During a run, RF puts `RobotHandler` on the root logger (`robot/output/pyloggingconf.py:32-49`). It maps levels (`:83-92`: ≥ERROR → error, ≥WARNING → warn, ≥INFO → info, ≥DEBUG → debug, else trace) and sets the root level from `--loglevel`, with `TRACE` → `NOTSET` (`:23-29,52-57`). `Set Log Level` updates it too (`robot/output/output.py:195-198`).
- `RobotHandler.emit` passes only the formatted message (`:66-75`). The record's `created` time is not used, so a message is stamped with its delivery time.
- A suite-scoped library's listener gets `close()` when its suite ends (`robot/running/libraryscopes.py:101-102`); a global one at the end of the run (`:84-87`).

**Where PlatynUI keywords run (verified).**

- Both RF libraries derive from `OurDynamicCore` (`src/PlatynUI/_our_libcore.py:8-25`). Every keyword is dispatched through `DynamicCore.run_keyword` (`.venv/.../robotlibcore/core/dynamic.py:24-25`), which `OurDynamicCore` does not override yet.
- `HybridCore` picks up `ROBOT_LIBRARY_LISTENER` (`robotlibcore/core/hybrid.py:33,67-81`).
- `PlatynUI.BareMetal` is suite-scoped (`src/PlatynUI/BareMetal/__init__.py:371-376`), so several instances, each with its own runtime, can coexist in one process (`tests/BareMetal/library_instance_isolation.robot`, `tests/acceptance/egui/coexisting_runtimes.robot`).
- The Python side already logs under `platynui.*`: `platynui.devices` (`src/PlatynUI/core/adapter_devices.py:18`) and `platynui.ui.application` (`src/PlatynUI/ui/application.py:22`).

**Existing conventions (verified).**

- The CLI's precedence is `RUST_LOG` > `--log-level` > `PLATYNUI_LOG_LEVEL` > `warn`, with `PLATYNUI_LOG_LEVEL` passed to `EnvFilter` verbatim (`crates/cli/src/lib.rs:33-59`). The Inspector copies it (`apps/inspector/src/lib.rs:548-579`).
- `.github/instructions/tracing.instructions.md:46` allows subscribers only in the two binaries, and `:12` forbids workspace dependencies for `tracing` crates (maturin).
- `EnvFilter` matches a directive's target as a prefix of the event's target (`tracing-subscriber-0.3.23/src/filter/env/directive.rs:246`), so the directive `platynui=debug` covers every `platynui_*` crate.

## Goals / Non-Goals

**Goals:**

- One subscriber in the extension that never calls Python from the emitting thread.
- Delivery on the calling Python thread at the points that matter for attribution:
  - after a runtime call returns;
  - before and after every PlatynUI keyword;
  - on demand.
- The CLI's level vocabulary and precedence, plus one RF-native knob.
- A path that stays correct if a later change releases the GIL during native calls.

**Non-Goals:**

- Releasing the GIL during native calls. Useful, but orthogonal, and the queue design does not need it.
- Attributing records to a runtime or library instance. The subscriber is process-global and first-party code uses no spans. See Risks.
- HTML formatting or console-only output in the RF log.
- Changing how the CLI, the Inspector or the other binaries log. Their duplicated `init_tracing` stays as it is.
- Deciding the levels of individual existing messages beyond what the warning review (decision 9) needs.

## Decisions

### 1. A queueing subscriber in the extension, installed at module import

At module init the extension installs a process-global dispatcher: `tracing_subscriber::registry()` with a reloadable `EnvFilter` and a queueing layer. The layer does the following for each enabled event, and nothing else:

- formats the message and fields into a record: level, target, message, fields, file and line, thread id and name, `SystemTime`;
- pushes the record onto a bounded, mutex-protected queue.

It holds no Python object and never takes the GIL. The mutex protects only the push and the swap-out (decision 3), so no thread ever waits on Python while holding it.

Installation happens once. If a global dispatcher is already set, the extension keeps it and records a single warning into its own queue on a best-effort basis. That can only happen if something else in the same binary set one, because each extension links its own copy of `tracing`.

Alternatives rejected:

- *`pyo3-log` (with `tracing`'s `log` feature).* It is built for the `log` crate and calls into Python synchronously from the emitting thread. That thread is a background thread in exactly the cases we care about: RF drops the message (`librarylogger.py:45`), and the GIL acquisition deadlocks a shutdown (Context, inferred).
- *A custom layer that calls `Python::attach` in `on_event`.* Same two defects.
- *A Rust-side pump thread that holds the GIL and emits into `logging`.* No deadlock, but every record would come from a non-main Python thread, which RF drops. It would serve plain Python users only.
- *Writing to stderr like the CLI.* RF does not capture the extension's stderr into the log, so the RF log would still show nothing.

### 2. Delivery points: after runtime calls, around keywords, on demand

Queued records are delivered to Python `logging` on the thread that delivers them. There are three delivery points:

1. **After every runtime call.** `PyRuntime::runtime()` returns a guard that releases the runtime lock first and then delivers. Every runtime method already goes through this accessor (`runtime.rs:711`), so all of them are covered without editing each method, and delivery never runs under the runtime lock. That rules out a deadlock if a Python log handler calls back into the same runtime.
   - Node and pattern methods have no common accessor and do not deliver themselves. Their records are delivered at the next delivery point. The queue is FIFO, so order is preserved.
   - Adding a guard to each of the roughly 30 node and pattern methods was rejected: it is churn, and every new method would be a place to forget it.
2. **Around every PlatynUI keyword.** `OurDynamicCore.run_keyword` calls the extension's flush before and after (in `finally`) dispatching the keyword. It runs inside the keyword on RF's runner thread, so RF records the messages in that keyword's log. The flush before the keyword attributes records that arrived between keywords (background threads) to the keyword that follows, instead of losing them. This covers `PlatynUI.BareMetal` and the high-level `PlatynUI` library alike.
   - A library listener's `start_keyword`/`end_keyword` was rejected. It fires for every keyword of every library and would scatter PlatynUI records into `Sleep` and `Log`. Where RF files messages logged from listener methods is also less obvious than inside the keyword body.
3. **On demand.** `platynui_native.flush_logs()` delivers explicitly. Plain Python users call it after node or pattern calls, and the RF library's listener `close()` calls it once when its suite ends.

### 3. How a record is delivered

Delivery works in five steps:

1. Swap the queue out under its mutex, then release the mutex.
2. For each record, get the Python logger `platynui.native.<module>`.
3. Check `isEnabledFor(levelno)`.
4. Build a `LogRecord` with `makeRecord`. Set `created`, `msecs`, `threadName` and `thread` to the emitting thread's values, set `pathname`/`lineno` from the event's metadata, and attach the structured fields as `record.native_fields`.
5. Call `logger.handle(record)`.

Consequences:

- Plain Python handlers see the true time and thread.
- RF, which uses only the formatted message (`pyloggingconf.py:66-75`), sees what the message text carries (decision 4).

A thread-local flag prevents re-entrant delivery. If a handler calls PlatynUI and that call reaches a delivery point, the nested point returns immediately, and the outer loop picks up what the nested call queued.

The outer loop makes a bounded number of passes, the records present plus one more swap. A handler that logs through PlatynUI on every record therefore cannot spin forever; anything left waits for the next delivery point.

An exception raised by a handler is contained per record. It never propagates into the PlatynUI call that happened to deliver it, and it goes through `logging`'s own error path (`Handler.handleError`), which RF silences with `raiseExceptions = False` (`pyloggingconf.py:42`).

Delivery uses `Python::try_attach` where it can run during interpreter finalization, so an exiting interpreter drops records instead of crashing.

### 4. Record naming and message text

**Logger name.** The logger is `platynui.native.` plus the event's target with `::` replaced by `.` and a leading `platynui_` removed:

| Event target | Logger |
|---|---|
| `platynui_provider_atspi::extents` | `platynui.native.provider_atspi.extents` |
| `zbus::connection` | `platynui.native.zbus.connection` |

The prefix nests native records under the hierarchy the Python side already uses (Context), so `logging.getLogger("platynui")` controls both. The `native` level separates them from the Python loggers `platynui.devices` and `platynui.ui.application`.

A single flat logger for everything was rejected: per-subsystem filtering in Python would be impossible.

**Message text.** The text carries everything RF can show:

- the short module name in brackets;
- the message;
- the fields as `key=value`;
- for records emitted on a thread other than the delivering one, the emitting thread and the emission time as a suffix.

For example: `[provider_atspi.extents] window manager cannot answer for this top-level window; … window=Frame "Sidecar App" call=resolve_window error=… (thread atspi-popup-watch, 12:34:56.789)`.

Records emitted on the delivering thread get no suffix, because they are delivered when the call that emitted them returns and their RF timestamp is accurate enough.

### 5. Level mapping

| Native | Python |
|---|---|
| `error` | `ERROR` |
| `warn` | `WARNING` |
| `info` | `INFO` |
| `debug` | `DEBUG` |
| `trace` | 5 |

RF maps anything below `DEBUG` to its `TRACE` and passes it only under `--loglevel TRACE` (`pyloggingconf.py:24,91-92`). Level 5 is given the name `TRACE` with `logging.addLevelName`, but only if nothing has named it yet, so a user's own naming wins.

A native `WARN` becomes an RF `WARN`, with everything that implies (console, *Test Execution Errors*) — maintainer decision. That is why decision 9 exists.

### 6. The level: a fixed `WARN` floor, lowered explicitly

The filter is built from the first of these that is present, the CLI's precedence (`crates/cli/src/lib.rs:33-59`):

| Source | Filter |
|---|---|
| `RUST_LOG` | used verbatim |
| the requested level (decision 7) | `warn,platynui=<level>` |
| `PLATYNUI_LOG_LEVEL` | used verbatim |
| none of them | `warn` |

The requested level uses the prefix directive `platynui` (Context: `EnvFilter` matches by prefix), which lowers every PlatynUI crate and leaves third-party crates at `warn`. The environment variables keep the CLI's full directive syntax, including third-party crates.

The filter is rebuilt through the reload handle whenever the requested level changes. The environment is read at each rebuild, not cached, so a test or a process that sets it before its first request is honored. An environment value that does not parse is skipped as though absent, and a warning naming the variable and value is queued.

RF's own `--loglevel` is deliberately **not** an input (maintainer decision). A run at `--loglevel DEBUG` does not make native code produce debug records, so neither costs nor volume depend on a switch users flip for their own keywords. To see native debug output in RF, both are needed: `native_log_level=debug` and an RF log level that keeps `DEBUG`.

Alternative rejected: following RF's log level. It is simpler, but the default `INFO` would put native lifecycle messages into every log, and `TRACE` would switch on per-node tracing and D-Bus message tracing for anyone debugging their own keywords.

*Performance:* disabled callsites are filtered by `tracing`'s interest cache before any field is evaluated. Under the default `warn` floor, per-node trace sites and field expressions such as `node.name()` in `crates/platform-linux-x11/src/window_manager.rs:373-381` cost what they cost today.

### 7. `native_log_level`: per-instance requests, process-wide effect

`PlatynUI.BareMetal` gains the keyword-only import argument `native_log_level: str | None = None`.

- A valid value registers a request with a small Python-side registry, which calls the extension's `set_log_level` with the most verbose live request.
- An invalid value raises from `__init__`, so the RF import fails naming the argument and the accepted values.
- The library's listener `close()` withdraws the request and flushes. RF calls it when the suite ends (`libraryscopes.py:101-102`).
- The registry lives in Python because requests belong to Python objects with RF-managed lifetimes. The extension only knows the effective level.

**Name.** The argument is `native_log_level` rather than `log_level`, so it cannot be mistaken for RF's own log level, which it deliberately does not follow (decision 6).

Alternatives rejected:

- *Last request wins.* A later suite's import would silently change an earlier suite's level while both are alive.
- *Sticky most-verbose-ever.* One suite's debug request would leak into every later suite.

### 8. The queue: bounded, drop newest, count

Capacity is 10 000 records.

When the queue is full, a new record is dropped and a counter increments. The next delivery emits one `WARNING` stating how many records were dropped since the last delivery, then resets the counter.

Dropping the newest keeps the beginning of a burst, which is usually the cause, and preserves order. A ring buffer that drops the oldest was rejected: it keeps the tail of a flood and loses what started it.

The capacity only matters while nothing delivers — between keywords, or for plain Python users who never flush. Under the default `warn` floor it is not reached in practice.

### 9. Review the warnings healthy lanes produce

Native warnings now reach RF's console, so a warning that is not actionable in a healthy session becomes noise that trains users to ignore the real ones.

The implementation runs the mock, X11 and compositor lanes, and on a Windows machine the Windows lane, and collects every native `WARN`/`ERROR` in their RF logs. For each one:

- it indicates a real defect → fix the defect or keep the warning;
- it describes an expected, non-actionable situation → lower it to `info` or `debug` in its crate, following `.github/instructions/tracing.instructions.md`'s level table;
- it is a known flake signal, such as the AT-SPI "D-Bus call timed out" noted in `dev-docs/python-library-design.md:6336-6340` → keep it and record the decision.

The level table gains a line: `warn` reaches the RF console.

### 10. Tests without a display, and one RF run

**Rust unit tests** in `packages/native` cover the layer and queue without Python:

- formatting and fields;
- the thread and time suffix decision;
- capacity, drop counting and FIFO order;
- filter construction and precedence from injected environment values;
- that emitting from a thread never blocks while another thread holds the queue's consumer side.

**A hidden test emitter.** `_native._emit_log_for_tests(level, message, *, on_background_thread)` emits an event with two fields, either on the calling thread or on a Rust thread that the call joins *while holding the GIL*. The join is the shutdown deadlock of the Context, reproduced deterministically. The emitter is compiled only with the `mock-provider` feature, which the Python test build already uses (`justfile:309-310`), so release wheels do not carry it.

**pytest** (`packages/native/tests`) uses `caplog` and the hidden emitter:

- delivery to `logging` with logger names, levels, fields and thread suffix;
- the background-thread join returns and its record arrives afterwards;
- a handler that calls back into the runtime completes;
- overflow reporting;
- `fn:trace()` through `Runtime.evaluate` on the mock at debug level;
- precedence with `RUST_LOG`/`PLATYNUI_LOG_LEVEL` set in a subprocess.

**The RF-level check.** A small suite under `tests/PlatynUI/` uses `PlatynUI.BareMetal` with `use_mock=${True}` and `native_log_level=debug`, and calls `Query` with `trace(...)` plus the hidden emitter via `Evaluate`. pytest runs it with `robot.run` and reads `output.xml` with `robot.api.ExecutionResult`. It asserts:

- the trace record is inside the `Query` keyword at `DEBUG`;
- the warning is at `WARN` and listed among the errors;
- the background record names its thread;
- a second run without `native_log_level` has no debug record.

The suite lives outside `tests/BareMetal`, so the regular mock lane does not pick it up without the assertions.

**Real-provider checks** are lane reviews (decision 9), plus one end-to-end check reusing `scripts/wayland-sidecar-harness.sh`: a Robot run in the sibling namespace with a dead control socket must show the bounds-substitution warning from `wayland-sidecar-capabilities` in its RF log.

## Risks / Trade-offs

- **[Records cannot be attributed to a library instance or runtime]** → The subscriber is process-global and no first-party code uses spans. A background record from runtime A can appear in a keyword of instance B. Mitigation: the thread and time suffix. Adding a span per runtime would fix attribution and is left for later.
- **[RF stamps the delivery time, not the emission time]** → Calling-thread records are delivered when the call returns, so the error is the call's duration at most. Other-thread records carry their true time in the text.
- **[Records emitted after the last PlatynUI keyword of a suite are delivered only by `close()`, and records emitted after that are lost]** → They belong to teardown of the runtime and the interpreter. Accepted. `flush_logs()` is available to plain Python users.
- **[Native warnings become RF console warnings]** → Deliberate. Decision 9 keeps healthy runs quiet. A deliberate warning (the bounds substitution) still reaches the console, which is the point.
- **[A more verbose native level changes costs]** → At `debug`/`trace`, field expressions with provider calls run and records cross into Python. Opt-in only, and documented next to the argument.
- **[`RUST_LOG` set in a developer's shell now affects RF runs]** → Consistent with the CLI and documented. `RUST_LOG` without a default directive disables unmatched targets, exactly as it does for the CLI.
- **[The hidden test emitter ships in mock builds]** → Mock builds are for tests. The emitter's leading underscore and absence from the stubs mark it private.
- **[A second global dispatcher in the same binary]** → Only possible if something in the same extension sets one. The extension keeps the existing one and says so; records then do not reach Python.

## Migration Plan

- **Additive, with one behavioral effect:** RF logs and consoles start showing native warnings and errors. No keyword, argument default or return value changes.
- **Needs a native rebuild.** An unrebuilt `platynui_native` keeps dropping every event. The new `native_log_level` argument then works only as a no-op request, because the Python side calls a `set_log_level` that does not exist. The library must therefore check for the function and fail the import with a message naming the missing rebuild, rather than silently ignoring the argument.
- **Sequence:**
  1. Extension subscriber, queue and delivery with their Rust and pytest tests.
  2. Python registry, `native_log_level` and the `run_keyword` delivery, with the RF-level check.
  3. The warning review.
  4. Docs and rules.
- **Rollback:** reverting the extension commit restores today's silence. The Python side must tolerate an extension without `flush_logs`/`set_log_level` only in the sense above; reverting both together is the clean path. Lowered warning levels from the review are independent commits and can stay.

## Open Questions

- Should the high-level `PlatynUI` library also offer `native_log_level` once it leaves placeholder state? Deferrable: it would reuse the registry and needs no change to the extension or the spec's requirements.
- Should `platynui_native` expose a context manager that flushes on exit, for plain Python users who work mostly with nodes? Deferrable: `flush_logs()` covers it. A context manager is sugar on top.

## Follow-ups

- **Windows lane warning review — pending, needs a Windows machine.** Decision 9's review covered the mock, X11 and compositor lanes on Linux. The Windows lane (`just test-acceptance-windows`: UIA, the JAB pump, the Java agent) emits its own native warnings, and only a Windows host can collect them. Until that review has run, a healthy Windows run may show native warnings that are not actionable.
