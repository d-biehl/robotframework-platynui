# Tasks

## 1. Failing tests first — extension (Rust)

- [x] 1.1 Add Rust unit tests in `packages/native` for the not-yet-existing logging module (design decisions 1, 4, 5, 6, 8), one per behavior:
  - an event becomes a record with level, target, message, every field and emitting thread;
  - the logger name is derived from the target (`platynui_provider_atspi::extents` → `platynui.native.provider_atspi.extents`, `zbus::connection` → `platynui.native.zbus.connection`);
  - the thread-and-time suffix appears only for a record from another thread;
  - the queue keeps FIFO order, drops the newest beyond capacity and counts the drops;
  - the filter is built with the CLI precedence from injected values (`RUST_LOG`, then the requested level as `warn,platynui=<level>`, then `PLATYNUI_LOG_LEVEL`, then `warn`), and an unparsable environment value is skipped with a warning naming it;
  - pushing from one thread never waits while another thread holds the consumer side.

  Verify the module does not compile yet with `just test-crate platynui_native` — that is the failing state.

## 2. Failing tests first — delivery into Python (pytest)

- [x] 2.1 Add `packages/native/tests/test_native_logging.py`. Use `caplog` and the hidden emitter `_native._emit_log_for_tests(level, message, *, on_background_thread)` from design decision 10. Cover:
  - a warning with two fields arrives once, as `WARNING`, under `platynui.native…`, with module name and fields in the message;
  - one event per level maps to `ERROR`/`WARNING`/`INFO`/`DEBUG`/5 once `set_log_level("trace")` is in effect;
  - without a requested level, a debug event is not delivered;
  - `fn:trace()` through `Runtime.new_with_mock().evaluate(...)` produces a debug record carrying label and value;
  - an emitter call that joins a logging background thread while holding the GIL returns, and the record then arrives naming its thread;
  - a handler that calls back into the runtime completes, and every record arrives exactly once;
  - overflowing the queue delivers the fitting records in order plus one warning with the drop count;
  - in a subprocess, `RUST_LOG`/`PLATYNUI_LOG_LEVEL` override as specified, and an invalid `PLATYNUI_LOG_LEVEL` keeps the default and is reported by name.

  Verify the file fails today with `just test-python` (missing API, no records).

## 3. Failing tests first — the Robot Framework log

- [x] 3.1 Add a fixture suite `tests/PlatynUI/robot/native_logging.robot`, outside `tests/BareMetal` so the mock lane does not pick it up without its assertions. It imports `PlatynUI.BareMetal` with `use_mock=${True}` and `native_log_level=debug` and runs:
  - `Query` with an XPath `trace(...)`;
  - `Evaluate` of the hidden emitter for a warning on the calling thread and one on a background thread;
  - a `Set Log Level` step.

  Follow the `robot-test-style` skill. First check that `Query` returns a non-node `trace()` result on the mock; if it does not, use a node-valued `trace()`.
- [x] 3.2 Add `tests/PlatynUI/test_native_logging_rf.py`. It runs the fixture with `robot.run` into a temporary directory and reads `output.xml` with `robot.api.ExecutionResult`. Assert:
  - the trace record sits inside the `Query` keyword at `DEBUG`;
  - the calling-thread warning is at `WARN` and among the execution errors;
  - the background warning names its thread and time;
  - under `--loglevel INFO` the debug record is absent until `Set Log Level    DEBUG`;
  - a second run without `native_log_level` (at `--loglevel DEBUG`) has warnings and no native debug record;
  - an import with `native_log_level=verbose` fails naming the argument and the accepted values.

  Verify it fails today with `just test-python`.

## 4. Extension: subscriber, queue and delivery

- [x] 4.1 Add `tracing-subscriber` (`default-features = false`, features `registry`, `env-filter`, `std`) to `packages/native/Cargo.toml`, declared per crate as `.github/instructions/tracing.instructions.md:12` requires. Verify `cargo tree -p platynui_native -e normal -i tracing-subscriber` lists it and `just check` stays clean.
- [x] 4.2 Implement the logging module (design decisions 1, 4, 5, 8):
  - the queueing layer, which holds no Python object and never takes the GIL;
  - record formatting, including the thread-and-time suffix;
  - the bounded queue with drop counting;
  - logger-name derivation.

  Verify the 1.1 tests for these pass.
- [x] 4.3 Implement the reloadable filter with the CLI precedence (decision 6). The environment is read at every rebuild, and an unparsable value is skipped and queued as a warning. Install the dispatcher once at module init in `packages/native/src/lib.rs`, keeping an existing global dispatcher if one is set. Verify the 1.1 precedence tests pass.
- [x] 4.4 Implement delivery into Python `logging` (decision 3):
  - swap out the queue under the mutex;
  - check `isEnabledFor`, build the record with `makeRecord` (`created`/`msecs`/`threadName`/`thread`/`pathname`/`lineno`, fields as `native_fields`) and call `handle`;
  - name level 5 `TRACE` only if it is unnamed;
  - use a thread-local re-entrancy guard, a bounded number of passes and per-record containment of handler exceptions;
  - use `Python::try_attach` where finalization can reach it.

  Expose it as `platynui_native.flush_logs()`, and expose `platynui_native.set_log_level(level: str | None)` validating `error|warn|info|debug|trace` case-insensitively. Add both to `_native.pyi` and `platynui_native/__init__.py`. Verify with `just test-crate platynui_native`.
- [x] 4.5 Make `PyRuntime::runtime()` (`packages/native/src/runtime.rs:711`) return a guard that releases the runtime lock and then delivers (decision 2). Verify that no runtime method needed editing beyond the accessor, and that the runtime-callback and background-join cases of 2.1 pass.
- [x] 4.6 Add the hidden `_emit_log_for_tests` behind the `mock-provider` feature (decision 10). It emits a warning with two fields on the calling thread, or on a Rust thread that the call joins while holding the GIL. Verify it is absent from a build without `mock-provider` (`just build-native`, then `hasattr` is false) and present in the mock build.
- [x] 4.7 Rebuild the mock native module and verify all of 2.1 passes with `just test-python`.

## 5. Python library: level requests and keyword delivery

- [x] 5.1 Add a small registry of level requests under `src/PlatynUI/core/` (decision 7). Each library instance registers at most one request, the effective level is the most verbose live request, and the registry calls `platynui_native.set_log_level` on every change. If the extension lacks `set_log_level`/`flush_logs`, the registry raises an error that names the missing native rebuild. Verify with pytest in `tests/PlatynUI/`:
  - add and withdraw requests;
  - most verbose wins;
  - withdrawal restores the previous level;
  - a missing function produces the named error (monkeypatched).
- [x] 5.2 Override `run_keyword` in `OurDynamicCore` (`src/PlatynUI/_our_libcore.py`) to call `platynui_native.flush_logs()` before dispatching and in a `finally` after it. Verify with a pytest double that both flushes run for a passing and a failing keyword, and that the keyword's result or exception is unchanged.
- [x] 5.3 Add the keyword-only import argument `native_log_level: str | None = None` to `PlatynUI.BareMetal.__init__`:
  - validate it, raising with the argument name and accepted values;
  - register a request;
  - add a library listener (API version 3) whose `close()` withdraws the request and flushes;
  - document the argument in the import-argument table (`src/PlatynUI/BareMetal/__init__.py:929-935`) in the user-facing voice of the other entries: default `WARN` floor, PlatynUI modules only, the environment variables, and that RF's own log level must also keep the records.

  Verify with 3.1/3.2 now passing under `just test-python`, and with the existing mock suites unchanged (`just test-baremetal`).

## 6. Review native warnings in healthy lanes

- [x] 6.1 Run the mock lane (`just test-baremetal`) and the real lanes (`just headless=true test-acceptance-x11`, `just headless=true test-acceptance-compositor`) with the bridge in place. Collect every native `WARN`/`ERROR` in their `output.xml`. For each, decide per design decision 9: fix the cause, lower it in its crate, or keep it with a recorded reason. Verify with a rerun of the three lanes: every remaining native warning is on the recorded keep list, and all lanes stay green.
  Result on Linux: the mock lane produced no native warning. The X11 and compositor lanes produced one kind, `[provider_atspi.node] children: get_children failed or timed out`, during `Wait Until Exists` in the Inspector-picker suites: a node the picker's own tree update removed between enumeration and the children call. That is expected in a live tree, and a real timeout already warns in `block_on_timeout`, so it is lowered to `debug`. Keep list: empty.
- [x] 6.2 Record the Windows lane review as pending for a Windows machine (`just test-acceptance-windows`): the JAB pump and UIA warnings cannot be reviewed on Linux. Verify the pending item is written down where the change's follow-ups are tracked, not silently skipped.

## 7. Documentation and rules

- [x] 7.1 Update `.github/instructions/tracing.instructions.md`:
  - the Python extension is the third place allowed to install a subscriber, and it queues and never writes to stderr;
  - `tracing-subscriber` features for the extension;
  - the level table notes that `warn` reaches the Robot Framework console.

  Verify the file no longer says subscribers live only in the two binaries.
- [x] 7.2 Add a logging section to `dev-docs/python-bindings.md`. Explain in prose:
  - where records go and when they are delivered;
  - the logger hierarchy;
  - the level sources and their precedence;
  - why delivery happens on the calling thread (RF drops other threads, GIL-held joins);
  - `flush_logs()` for plain Python users.

  Point to the code rather than transcribing types. Verify the section names the `native_log_level` argument and both environment variables.

## 8. Verification

- [x] 8.1 Run `just check`, `just test` and `just test-python`, then `just build-native` (the Python tests switch the native module to the mock build). Verify everything is green, including the 1.1, 2.1, 3.2 and 5.x tests.
- [x] 8.2 Run `just test-baremetal`, `just headless=true test-acceptance-x11` and `just headless=true test-acceptance-compositor`, and summarize with `just test-summary`. Verify every lane is green and the native warnings in their logs match the 6.1 keep list.
- [x] 8.3 End-to-end on a real provider: with `scripts/wayland-sidecar-harness.sh --with-app --env XDG_CURRENT_DESKTOP=platynui --env PLATYNUI_CONTROL_SOCKET=<a dead path>`, run a minimal Robot suite that reads a Frame's `@Bounds` through `PlatynUI.BareMetal`. Verify its `output.xml` contains exactly one `WARN` from `platynui.native.provider_atspi.extents` naming the window and the control-socket path, inside the keyword that read the bounds.

## 9. Commit (only when the user asks)

- [x] 9.1 Commit the extension bridge as `feat(native): forward native tracing to Python logging` (≤ 72 characters) with its Rust and pytest tests. The pre-commit hooks gate the whole project, so the tree must be lint-clean at this commit.
- [x] 9.2 Commit the library side as `feat(baremetal): show native diagnostics in the Robot Framework log`, with the registry, the `run_keyword` delivery, the `native_log_level` argument, the RF-level check and the docs.
- [x] 9.3 Commit each crate's level adjustments from 6.1 separately under that crate's scope (for example `fix(provider-atspi): lower non-actionable warnings to debug`), so each can be reverted on its own.
