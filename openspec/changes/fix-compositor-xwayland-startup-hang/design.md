## Context

See proposal.md for the motivation. This section covers the current startup path and the constraints the fix works within. All references are to the working tree at ae5f614 and to smithay 0.7.0 (`Cargo.lock`).

**Startup path today (verified in code):**
- `setup_services` (`apps/wayland-compositor/src/backend/mod.rs:224-283`) registers signals and the watchdog. With `--xwayland` it then copies `print_env`, `ready_fd`, `exit_with_child` and `child_command` into `State` and calls `start_xwayland` (`:242-255`). Next it sets up the control socket (`:259-264`) and EIS (`:267-272`), each of which exports its environment variable. Only without `--xwayland` does it announce readiness and spawn the child right away (`:275-280`).
- `start_xwayland` (`src/xwayland.rs:61-125`):
  - A spawn error only logs a warning and returns (`:72-75`).
  - On `XWaylandEvent::Ready` it exports `DISPLAY` (`:90`), prints it (`:94-96`), announces readiness (`:98`) and spawns the child (`:101`). Only after that does it call `X11Wm::start_wm` (`:106`), whose failure is only logged (`:113-115`).
  - `XWaylandEvent::Error` only logs and clears `state.xwayland` (`:118-121`).
  - A failure to register the event source only logs (`:123`).
  - Xwayland's stdout and stderr go to `Stdio::null()` (`:67-68`), although smithay passes `-verbose` (smithay `xwayland/xserver.rs:143`).
- smithay reports `XWaylandEvent::Error` only for a real read error on the display fd. A server that exits before reporting a display gives EOF, which `take_socket` returns as `Ok(None)` (`xserver.rs:352`). The source is level-triggered (`xserver.rs:193`), so it keeps firing without reporting anything, which most likely busy-loops the event loop. That busy loop was inferred from the code, not observed. A server that stays alive and silent produces no event at all.
- smithay spawns `Command::new("Xwayland")` with an environment cleared down to `PATH`, `XDG_RUNTIME_DIR` and `WAYLAND_SOCKET` (`xserver.rs:137-173`). A caller can therefore control which `Xwayland` is found through `PATH`. Dropping the `XWayland` source only disconnects its Wayland client (`xserver.rs:330-335`); smithay 0.7 offers no API to kill the process.
- `XwmHandler::xwm_state` panics when the window manager is missing (`src/xwayland.rs:158`), and smithay calls it from X11 event handling and the xwayland-shell commit hook. Today the reachable panic is on the WM-start failure path: `start_wm` attaches its `XwmId` to the XWayland client before later fallible steps, so a failure there leaves `wm` as `None` while the server keeps running. A mid-session termination cannot panic today, because smithay disables the source after Ready (`xserver.rs:305`) and `XwmHandler::disconnected` keeps smithay's no-op default (`xwm/mod.rs:396`), so the stored WM stays in place.
- smithay keeps Xwayland's `Child` private and reaps it on its own thread when the client disconnects, logging `Xwayland terminated: <status>` at ERROR (`xserver.rs:364-386`). The compositor has no access to the exit status. `XWayland::spawn` returns the XWayland `Client`, whose liveness can be checked through the display handle.
- The IPC `status` field is `state.xwayland.is_some()` (`src/control.rs:845`). `platynui-wayland-compositor-ctl` prints it as "XWayland: yes/no" (`apps/wayland-compositor-ctl/src/app.rs:348`, `:356`).
- `State::exit_code` (`src/state.rs:718-726`) already implements the child-result contract. Without `--exit-with-child` it falls through to success (`:724`), so a startup error in that mode would read as `0` unless the fix adds a startup-error state.

**Constraints:**
- Readiness must follow the control socket and EIS setup, or `--print-env` output and the child environment lack `PLATYNUI_CONTROL_SOCKET` and `LIBEI_SOCKET`.
- The fix must work the same on the headless, winit and DRM backends; all three return `state.exit_code()` after the loop.
- The existing X11 lane already applies this policy to its own X server: `scripts/startxsession.sh:122-134` waits 10 s for the display number and exits `1` with an ERROR message on timeout, EOF or a dead server.

## Goals / Non-Goals

**Goals:**
- Every way XWayland can fail to become usable ends the process promptly with `1`, a visible cause, no readiness and no child. With or without `--exit-with-child`, and also when the session is ended from outside before XWayland became usable.
- One place announces readiness and starts the child, after every requested service is usable.
- XWayland's state is reported accurately, and its termination mid-session stays contained: its windows are cleaned up, and the compositor keeps running without crashing.
- Every failure case is testable without a real XWayland.

**Non-Goals:**
- Restarting XWayland or lazily starting it on the first X11 client.
- An `optional` mode that degrades to a Wayland-only session. It is not needed now, and it could be added later as a value for the flag.
- Killing a hung Xwayland process on timeout. smithay 0.7 exposes no handle to do so (see Risks).
- Installing XWayland in CI.

## Decisions

**1. An XWayland that does not become usable is a fatal startup error, with exit code `1`.** A caller who passes `--xwayland` needs X11. Degrading to a Wayland-only session would move the failure into the tests ("cannot open display"), and suites without X11 would pass although the requested capability is missing. That breaks the lanes' fail-loud rule (`openspec/specs/acceptance-lane-selection` "Runtime-only prerequisites fail, they do not skip") and ae5f614's "an unknown result never reads as success". Code `1` matches `main.rs`'s error path, the "other start failures" row of the exit-code table, and `startxsession.sh`.
- *Rejected:* degrading like sway. sway only covers the synchronous create failure and does not hold its session start on XWayland.
- *Rejected:* a distinct code such as `125`. It still collides with Robot Framework's failed-test counts, and nothing in the repo would branch on it.
- *Rejected:* a `required|optional` switch. It adds CLI surface nobody has asked for.

**2. A single startup finalization, called once everything requested is usable.** `setup_services` always stores `print_env`, `ready_fd`, `exit_with_child` and `child_command` in `State`, then sets up the control socket and EIS as today. A new `State::finish_startup` method announces readiness (including the `DISPLAY` line when XWayland is usable) and then spawns the child. Without `--xwayland`, `setup_services` calls it at its end, as today. With `--xwayland`, the XWayland ready path calls it once the window manager runs. Because the event loop only starts after `setup_services` has returned, the control socket and EIS variables always exist by then.
- *Rejected:* announcing readiness in each failure branch separately. That is how the two diverging copies at `backend/mod.rs:242-246` and `:275-279` came about.

**3. The X11 window manager starts before readiness, and its failure is fatal.** In the Ready handler, `X11Wm::start_wm` runs first. Only on success are the WM stored, `DISPLAY` exported and startup finished. On failure, the compositor records the startup error and stops the loop from within the callback, before any X11 client can reach `xwm_state`. smithay's own comment says X11 clients are not accepted before the WM holds `WM_S0` (`xwm/mod.rs:782-798`). Handing out `DISPLAY` first, as today, gives the child a display that may not work.

**4. Startup watch: a 100 ms poll timer covers exit-before-ready and the timeout.** Next to the XWayland source, `start_xwayland` registers a timer, the same polling approach `child.rs` uses for the child program. The source's `RegistrationToken` is kept, not discarded. On each tick the timer checks three things:
- whether XWayland became usable, and if so removes itself;
- whether XWayland's Wayland client, which `XWayland::spawn` returns and `State` already keeps until Ready, has disconnected, which means the process exited. `display_handle.backend_handle().get_client_data(client.id())` fails once the client is gone; the serial in `ClientId` rules out a reused slot;
- whether `--xwayland-timeout` has elapsed.

On either failure it removes the XWayland source, which also ends a busy loop on the EOF-ed display fd within one tick, then records the startup error and stops the loop. The existing `XWaylandEvent::Error` arm and a failed source registration record the startup error too. A synchronous spawn error does the same before the loop starts.
- *Rejected:* a second event source on a duplicate of `XWayland::poll_fd()` (`xserver.rs:238`) that reacts to HUP. Its HUP can be processed before smithay reads a `READY` line still in the pipe, which would falsely fail a server that reported and then exited, and it needs extra fd plumbing.
- *Rejected:* relying on the timeout alone. The failure would arrive late and cost CPU in the meantime.
- calloop runs expired timers after the fd events of the same dispatch, and a server that reports a display writes to the display fd before its socket closes. The watch therefore cannot mistake a server that reported readiness and then exited for one that exited before readiness.
- *Assumed, verified by the "exits before ready" test:* that a server process exiting always disconnects its Wayland client, observable within a tick.

**5. `--xwayland-timeout <secs>`, default `10`, minimum `1`.** Ten seconds matches `startxsession.sh:124`. XWayland on software rendering normally becomes ready well within that (not measured here). The tests pass `1` for the never-ready case, so the option is also the test override.
- *Rejected:* a hidden environment variable. It would be undocumented behavior.
- *Rejected:* a TOML config key. Startup policy belongs with the other session flags.

**6. The startup error takes precedence in `State::exit_code`.** A `record_startup_error(cause)` helper does four things: it logs at ERROR, sets a startup-error field, and sets `state.running = false`. That is the mechanism all three backends already turn into a loop stop, and it also works before the loop starts, whereas a direct `loop_signal.stop()` there would be lost because calloop resets the stop flag when `run` begins. Every fatal path above calls the helper. `exit_code()` returns `1` when the field is set, before the child-result logic, which closes the gap where the mode without `--exit-with-child` would fall through to success (`state.rs:724`). `exit_code()` also treats a session that stops while a requested XWayland is still starting (watchdog, IPC `shutdown`, `SIGTERM`/`SIGINT`, window close) as a startup error: that session never became usable, so `0` would misreport it. The ERROR message is produced where the failure is detected, so each cause carries its own text:
- "Xwayland not found on PATH";
- "not executable";
- "exited before it was ready" (without a status, which smithay keeps private and logs itself);
- "did not become ready within N s";
- "X11 window manager failed to start".

**7. XWayland's output is forwarded into the log.** `XWayland::spawn` receives pipes for stdout and stderr instead of `Stdio::null()`. One reader thread per pipe logs each line under a `xwayland` tracing target at DEBUG and keeps the last few lines in a small shared buffer. The fatal startup error appends that tail to its ERROR message. When the process is known to have exited (client disconnected, or the WM failed after the server went away), the error first waits for both readers to reach EOF, bounded at about 500 ms, so the server's last lines are not lost to thread scheduling. On a timeout the pipes stay open, so the error takes the tail as it is. This keeps the cause visible at the scripts' default `error` log level (`scripts/startcompositor.sh:158`) without flooding normal runs with `-verbose` output.
- *Rejected:* inheriting the compositor's stderr. It would print Xwayland's verbose output into every lane console.

**8. XWayland terminating mid-session is reported and contained, not fatal.** `XwmHandler::disconnected` is overridden to do four things:
- log at ERROR with the compositor's own message, distinct from smithay's `Xwayland terminated` line;
- remove every X11 window of that server: the mapped ones from the space via the existing `remove_x11_window` (`src/xwayland.rs:33-40`), and the minimized ones from `minimized_windows`, closing their foreign-toplevel handles as `toplevel_destroyed` does for Wayland windows (`src/handlers/xdg_shell.rs:333-343`);
- mark XWayland inactive;
- keep the `X11Wm` value, so `xwm_state` has something to return if a queued smithay callback still arrives.

`xwm_state` stays as it is otherwise, because a panic there still indicates a real bug before startup completes. Ending the session was rejected: a child that needs X11 fails on its own terms, and the exit-code contract reports that result. A restart is a non-goal.

**9. The IPC status reflects usability through an explicit XWayland state.** `State` tracks XWayland as not requested, starting, running or terminated, instead of deriving it from `xwayland.is_some()`. The status field `xwayland` is `true` only while running. The field keeps its name and type, so `platynui-wayland-compositor-ctl` needs no change. `docs/ipc-protocol.md` defines "usable" precisely.

**10. Doc drift is fixed in the same change**, because this change rewrites the same help texts:
- `usage.md` lists `--control-socket`, but the flag is `--no-control-socket` (`src/lib.rs:132-136`).
- `src/lib.rs:128`, `src/xwayland.rs:3` and `:53` mention an `xwayland` cargo feature that the crate does not have.

## Risks / Trade-offs

- **A hung Xwayland process can outlive the compositor.** On timeout, dropping the source only disconnects the Wayland client (`xserver.rs:330-335`), and smithay 0.7 exposes no process handle. → A real Xwayland normally exits once its Wayland connection closes (assumed). The test fake used for the never-ready case exits by itself shortly after the test. Killing the process is left to a future smithay API.
- **Tests with a fake `Xwayland` take real X display slots.** smithay creates `/tmp/.X<N>-lock` and `/tmp/.X11-unix/X<N>` before exec. → Parallel test processes are safe, because smithay creates the lock with `create_new` and reclaims stale locks by PID. The tests document that they touch this global state.
- **The cause-specific assertions need working X11 socket preparation.** If `/tmp/.X11-unix` is missing or not writable, smithay fails before exec with "Could not find a free socket for the XServer" (`x11_sockets.rs:26-44`), and every fake case would report that instead of its own cause. → The test helper checks the precondition first and fails with a clear message rather than a misleading cause assertion.
- **smithay logs an ERROR of its own when Xwayland dies.** A log assertion on "terminated" would pass without this change. → The tests assert the compositor's own message (Decision 8) and the observable cleanup, not smithay's line.
- **Fake servers must not block the event loop.** `start_wm` runs synchronously in the Ready callback. A fake that reports a display and keeps the WM socket open without answering would hang it. → The "window manager cannot start" fake writes `1\n` to the display fd, since smithay needs the newline (`xserver.rs:353`), and then exits, so `start_wm` fails fast on EOF. That this failure is fast is assumed, and verified by the test.
- **Nothing in the tests may depend on `PATH`**, because the tests restrict `PATH` to the fake's directory to control the `Xwayland` lookup, and smithay passes that `PATH` on to the fake. → The child is `/bin/sh -c ': > <marker>'` with a shell redirection, not `touch`, so the "no marker" assertion cannot pass by accident. The fake scripts use absolute paths (`exec /bin/sleep 30`). The WM fake writes through `/proc/self/fd/<n>`, because `/bin/sh` is dash on CI and dash rejects redirections to fd numbers above 9.
- **Real-XWayland scenarios are not covered by CI.** CI installs no `xwayland` package. → Those tests are `#[ignore]` and run locally in the verification tasks; the failure paths, which caused the hang, run in CI.
- **Behavior change for the IPC `xwayland` field.** It no longer reports `true` while XWayland is still starting or after a failure. → No consumer in the repo relies on the old imprecise meaning; the ctl tool only displays it.

## Migration Plan

- The change is behavioral for `--xwayland` startup and additive for `--xwayland-timeout`. Sessions without `--xwayland` behave as before, including their readiness output and exit codes.
- Nothing outside `apps/wayland-compositor` changes, so there is no native Python rebuild. The next `cargo build` (and `startcompositor.sh`, which builds via `cargo run`) picks it up.
- CI needs no workflow change; the lanes do not pass `--xwayland`.
- Rollback: revert the change's commit(s). No persistent state, config format or protocol shape changes.
