## 1. Tests first

- [ ] 1.1 Create `apps/wayland-compositor/tests/xwayland_startup_tests.rs` with a harness modeled on `child_exit_tests.rs`:
  - start the compositor headless with a unique `--socket-name` and `--log-level info`;
  - pass the watchdog `--timeout 60`, deliberately above the 30 s `try_wait` deadline, so an unfixed hang shows up as a deadline failure and not as a watchdog exit;
  - set `LIBGL_ALWAYS_SOFTWARE=1`, write the log to `CARGO_TARGET_TMPDIR`, and include the log in every assertion message;
  - return exit status, elapsed time, log text, and whether a marker file exists.

  A `with_fake_xwayland` helper sets `PATH` to a per-test temp dir holding an optional `Xwayland` script. The fake scripts use only absolute paths (`/bin/sleep`, `/bin/cat`), and the WM fake writes through `/proc/self/fd/<n>`, because `/bin/sh` may be dash. The child is `/bin/sh -c ': > <marker>'`. The helper first checks that `/tmp/.X11-unix` exists and is writable, and fails with a message naming that precondition. Verify that the file compiles with `just test-crate platynui-wayland-compositor`.
- [ ] 1.2 Add regression anchors for the non-XWayland path that task 2.1 rewrites. Run them and confirm they **pass on the current code**:
  - "Readiness without XWayland lists the session's sockets": `--print-env` with the control socket and EIS enabled, and `PLATYNUI_CONTROL_SOCKET`, `LIBEI_SOCKET` and `DISPLAY` removed from the compositor's environment. Expect stdout with `WAYLAND_DISPLAY`, `PLATYNUI_CONTROL_SOCKET` and `LIBEI_SOCKET`, no `DISPLAY` line, and exactly one `READY` line on stderr.
  - "The child sees the ready session's environment": the compositor gets a host `DISPLAY=:99`, runs the spec's `/bin/sh` check as `--exit-with-child`, and exits with `0`.
  - "No child program, normal shutdown": already covered by `ipc_tests.rs` `ipc_shutdown`, which sends IPC `shutdown` without a child and asserts a successful exit. Confirm it still exists and passes; no new test is needed.
- [ ] 1.3 Add the startup-error scenarios as tests. Each asserts:
  - exit `1`;
  - elapsed time below 5 s, except "session ends while starting", which waits for the 1 s watchdog;
  - no marker file;
  - no `READY` line in the log;
  - the cause-specific ERROR text from the spec.

  The cases:
  - binary not found (empty fake dir), with and without `--exit-with-child`;
  - binary not executable (a mode-0644 `Xwayland`);
  - exits before ready (the fake prints a diagnostic to stderr and exits `1`, with `--xwayland-timeout 30`; also assert that the diagnostic is in the ERROR message);
  - never ready (the fake runs `exec /bin/sleep 30`, with `--xwayland-timeout 1`);
  - window manager cannot start (the fake writes `1\n` to the fd given after `-displayfd` and exits; with `--print-env`, also assert there is no `DISPLAY=` line);
  - session ends while starting (never-ready fake, `--xwayland-timeout 30 --timeout 1`, no child);
  - invalid timeout (`--xwayland --xwayland-timeout 0` exits non-zero with a usage message naming the option);
  - "Status while XWayland is still starting" (never-ready fake, control socket enabled, `--xwayland-timeout 10`; query `status` over the control socket and parse the JSON: `xwayland` is `false`).

  Confirm that each test fails on the current code, and record how. Cases without the new flag hang until the 30 s deadline, or exit `0` without `--exit-with-child`. Cases that pass `--xwayland-timeout` fail with clap's exit `2` until task 2.2. The status test reports `true`. After 2.2, re-run the `--xwayland-timeout` cases and confirm they fail for the real reason.
- [ ] 1.4 Add `#[ignore]`d tests that need a real `Xwayland` on `PATH`, with a module doc comment showing how to run them (`cargo nextest run -p platynui-wayland-compositor --run-ignored ignored-only -E 'binary(xwayland_startup_tests)'`):
  - "A working XWayland session succeeds": exit `0` and a `DISPLAY=` line on stdout.
  - "Readiness with XWayland waits for the X11 window manager": in the info log, the window-manager-started line (added in 3.3) comes before `compositor ready` and before `spawning child program`. On the current code this order is reversed.
  - "Status with a usable XWayland": parse the `status` JSON and expect `xwayland` to be `true`. Also run `target/debug/platynui-wayland-compositor-ctl status` against the same socket and match its `XWayland:` line whitespace-tolerantly for `yes`.

  Verify that the tests are listed as ignored, and that the order test fails locally on the current code.
- [ ] 1.5 Add a unit test for exit-code precedence in `state.rs`. A recorded startup error must yield `1` in all three modes: without `--exit-with-child`, with it, and with a child exit code already recorded. A requested XWayland still in the starting state when `exit_code()` runs must also yield `1`. Confirm the test fails until 2.4 lands.

## 2. Startup path

- [ ] 2.1 `src/backend/mod.rs`: always store `print_env`, `ready_fd`, `exit_with_child` and `child_command` in `State`, dropping the two diverging copies at `:242-246` and `:275-279`. Add `State::finish_startup`, which announces readiness (the `DISPLAY` line only when XWayland is usable) and then spawns the child. Call it at the end of `setup_services` when `--xwayland` is not requested. Verify that the 1.2 anchors, `child_exit_tests` and `ipc_tests` stay green.
- [ ] 2.2 `src/lib.rs`: add `--xwayland-timeout <secs>` (default `10`, minimum `1`, validated by a clap range) with help text. Rewrite the `--xwayland` help to describe the fatal startup behavior and drop the nonexistent `xwayland` feature. Verify with the "invalid timeout" test and `target/debug/platynui-wayland-compositor --help`.
- [ ] 2.3 `src/state.rs`: add the explicit XWayland state (not requested, starting, running, terminated), a startup-error field, and `record_startup_error(cause)`. The helper logs at ERROR, sets the field and sets `running = false`; it does not use `loop_signal.stop()`, which calloop resets at `run` start. Verify with the 1.5 unit test once 2.4 lands.
- [ ] 2.4 `State::exit_code`: return `1` when a startup error is recorded, or when a requested XWayland is still starting, before the child-result logic. Verify that the 1.5 unit test, the "session ends while starting" test and `child_exit_tests` pass.

## 3. XWayland startup and termination

- [ ] 3.1 `src/xwayland.rs` `start_xwayland`: turn the spawn error into `record_startup_error`, naming the cause from the error kind (not found, not executable, or other with the error text). Verify that the "not found" (both variants) and "not executable" tests pass.
- [ ] 3.2 Keep the source's `RegistrationToken` and add the 100 ms startup-watch timer. On each tick it:
  - removes itself once XWayland is running;
  - detects that XWayland's Wayland client has disconnected (`display_handle.backend_handle().get_client_data(client.id())` fails);
  - detects that `--xwayland-timeout` has elapsed.

  On failure it removes the XWayland source and records the startup error ("exited before it was ready" / "did not become ready within N s"). A failed source registration and the `XWaylandEvent::Error` arm also record a startup error. Verify that the "exits before ready", "never ready" and "session ends while starting" tests pass within their time bounds.
- [ ] 3.3 Reorder the Ready handler so `X11Wm::start_wm` runs first. On success: store the WM, log a distinct info line (e.g. `X11 window manager started`), set the state to running, export `DISPLAY` and call `finish_startup`. On failure: `record_startup_error("X11 window manager failed to start: …")` and return without exporting `DISPLAY`. Verify that the "window manager cannot start" and "Status while XWayland is still starting" tests pass, and that the ignored order and happy-path tests pass locally with a real `Xwayland`.
- [ ] 3.4 Forward Xwayland's stdout and stderr: pass pipes to `XWayland::spawn`, read each on a thread, log lines under the `xwayland` target at DEBUG, and keep the last lines in a shared buffer. When the process is known to have exited, `record_startup_error` waits up to about 500 ms for both readers to reach EOF before it appends the tail; on a timeout it appends the tail as it is. Verify that the "exits before ready" test sees the diagnostic line, and that it passes 20 runs in a row under parallel load (`cargo nextest run -p platynui-wayland-compositor -E 'binary(xwayland_startup_tests)'` in a loop).
- [ ] 3.5 Override `XwmHandler::disconnected` to:
  - log the compositor's own ERROR (`XWayland terminated during the session`);
  - remove the server's mapped X11 windows via `remove_x11_window`;
  - remove its minimized X11 windows from `minimized_windows` and close their foreign-toplevel handles;
  - set the state to terminated, keeping the `X11Wm`.

  Add an `#[ignore]`d real-XWayland test for "Wayland clients keep working after XWayland dies":
  - Setup: map one X11 window and minimize a second (via IPC), and open a Wayland window using the raw Wayland client fixture from `ipc_tests.rs`.
  - Action: kill the `Xwayland` process.
  - Expect: the compositor's own ERROR text in the log; `list_windows` contains neither X11 window (mapped or `minimized`) but still the Wayland window; `status` reports `xwayland` as `false`; IPC keeps answering.

  Run it locally.

## 4. Status and documentation

- [ ] 4.1 `src/control.rs:845`: report `xwayland` from the explicit state (`true` only while running). Add an `ipc_tests` case "Status without XWayland" that parses the `status` JSON and checks that `xwayland` is `false`. Verify with `just test-crate platynui-wayland-compositor`, including the 1.3 status test.
- [ ] 4.2 `apps/wayland-compositor/docs/usage.md`:
  - add the startup-error row to the exit-code table ("a requested XWayland cannot become usable, including a session ended while it is starting", exit `1`, no readiness, no child);
  - document `--xwayland-timeout` and what readiness means with `--xwayland`;
  - fix `--control-socket` to `--no-control-socket` in the option table.

  Verify by comparing the table row by row with the spec requirements.
- [ ] 4.3 Update the remaining docs:
  - `apps/wayland-compositor/docs/ipc-protocol.md`: define the `xwayland` status field as "XWayland is ready and its window manager runs".
  - `apps/wayland-compositor/README.md`: say that `--xwayland` needs an `Xwayland` binary and fails the start without one.
  - `apps/wayland-compositor/src/xwayland.rs:3` and `:53` ("if the feature is enabled") and `src/lib.rs:128`: drop the feature wording.

  Verify with `grep -n -i 'feature' apps/wayland-compositor/src/xwayland.rs apps/wayland-compositor/src/lib.rs`: no line may mention an `xwayland` feature (the `backend-drm` mention in `lib.rs` stays).

## 5. Verification

- [ ] 5.1 Run `just test-crate platynui-wayland-compositor` and confirm that all section 1 tests pass, together with `child_exit_tests`, `ipc_tests` and the unit tests. Then run the ignored real-XWayland tests (1.4, 3.5) locally with `/usr/bin/Xwayland` on `PATH` and confirm they pass.
- [ ] 5.2 Run `just check` (fmt, clippy `-D warnings`, ruff, mypy) and `just doc`; both must finish without warnings.
- [ ] 5.3 Manual end-to-end check through the script, since that is the future consumer's path. Put a fake `Xwayland` that prints a line and exits `1` at the front of `PATH` (`PATH=<fake dir>:$PATH`; `cargo` and the other tools still resolve). Run `PLATYNUI_BACKEND=headless scripts/startcompositor.sh --xwayland -- /bin/true`. Expect a prompt exit with `1`, and an ERROR message that includes the fake's line and is visible at the script's default `error` log level.
- [ ] 5.4 Regression on the real lane, which does not use `--xwayland`: `just headless=true test-acceptance-compositor` passes (judged via `uv run --no-sync robotcode results summary --failed`) and the lane exits with `0`.
