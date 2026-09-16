## Purpose

Defines how a PlatynUI Wayland compositor session starts and ends. This covers when readiness is announced and what it carries, when the child program starts, how the process exit code reports the child program's result, and how a requested XWayland server takes part in startup and is reported during the session. CI scripts and the acceptance lanes rely on these signals to know when to start, and whether the command inside the session succeeded.

## ADDED Requirements

### Requirement: Readiness is announced once the session is usable
The compositor SHALL announce readiness exactly once. Readiness means `READY` on the `--ready-fd` descriptor, or on stderr when no descriptor is given, plus the `--print-env` output. It SHALL come after the Wayland socket is listening and after the control socket and EIS server have been set up, unless they are disabled. Their setup is best effort: a failure is logged and does not hold back readiness.

When `--xwayland` is given, readiness SHALL come only after XWayland is ready *and* its X11 window manager runs. The compositor SHALL NOT announce readiness if that never happens (see "A requested XWayland that cannot start is a startup error").

The `--print-env` output SHALL list `WAYLAND_DISPLAY`, then `DISPLAY` when XWayland runs, then the control socket and EIS paths exported to the session (`PLATYNUI_CONTROL_SOCKET`, `LIBEI_SOCKET`).

#### Scenario: Readiness without XWayland lists the session's sockets
- **GIVEN** a compositor started headless with `--print-env` and without `--xwayland`, in an environment without `PLATYNUI_CONTROL_SOCKET`, `LIBEI_SOCKET` or `DISPLAY`
- **WHEN** it becomes ready
- **THEN** stdout lists `WAYLAND_DISPLAY`, `PLATYNUI_CONTROL_SOCKET` and `LIBEI_SOCKET`, prints no `DISPLAY` line, and `READY` appears exactly once on stderr

#### Scenario: Readiness with XWayland waits for the X11 window manager
- **GIVEN** a compositor started headless with `--xwayland --print-env --exit-with-child -- /bin/true` and a working `Xwayland` binary
- **WHEN** the session starts
- **THEN** the compositor's log shows that the X11 window manager started before readiness was announced and before the child program was spawned, and stdout contains a `DISPLAY=` line (verifiable only with a real `Xwayland` binary)

### Requirement: The child program starts only after readiness
With a child program given after `--`, the compositor SHALL start it only after readiness has been announced. The child SHALL inherit the environment of the ready session. `DISPLAY` SHALL be present only when XWayland runs, and SHALL otherwise be removed, so a host display can never leak into the session. When the session never becomes ready, the child SHALL NOT be started at all.

#### Scenario: The child sees the ready session's environment
- **GIVEN** a compositor started headless, with a host `DISPLAY` in its own environment and without `--xwayland`
- **WHEN** it runs `--exit-with-child -- /bin/sh -c '[ -n "$WAYLAND_DISPLAY" ] && [ -n "$PLATYNUI_CONTROL_SOCKET" ] && [ -n "$LIBEI_SOCKET" ] && [ -z "$DISPLAY" ]'`
- **THEN** the compositor exits with `0`

#### Scenario: No child when the session never becomes ready
- **GIVEN** a compositor started with `--xwayland --exit-with-child -- /bin/sh -c ': > <marker>'` whose XWayland cannot start
- **WHEN** the compositor exits
- **THEN** the marker file does not exist

### Requirement: The session's exit code reports the child program's result
With `--exit-with-child`, the compositor SHALL shut down when the child program exits and SHALL exit with the child's result, following shell conventions:
- the child's own exit code when it exited;
- `128 + n` when signal `n` terminated it;
- `127` when the program does not exist, `126` when it is not executable, and `1` for any other failure to start it, in which case the session ends right away;
- `1` when the session ends while the child is still running (`--timeout`, the IPC `shutdown` command, `SIGTERM`/`SIGINT`, closing the window), because the child's result is unknown and SHALL never read as success.

Without `--exit-with-child`, or without a child program, a normal shutdown SHALL exit with `0`. A startup error (see "A requested XWayland that cannot start is a startup error") SHALL exit with `1` in every mode.

#### Scenario: A failing child fails the session
- **WHEN** the compositor runs headless with `--exit-with-child -- sh -c 'exit 3'`
- **THEN** the compositor exits with `3`

#### Scenario: A child killed by a signal
- **WHEN** the compositor runs headless with `--exit-with-child -- sh -c 'kill -TERM $$'`
- **THEN** the compositor exits with `143`

#### Scenario: A child that cannot be started
- **WHEN** the compositor runs headless with `--exit-with-child -- /nonexistent/program`
- **THEN** the compositor exits with `127` promptly, without waiting for a timeout

#### Scenario: The session ends before the child does
- **WHEN** the compositor runs headless with `--timeout 1 --exit-with-child -- sleep 10`
- **THEN** the compositor exits with `1` after about one second

#### Scenario: No child program, normal shutdown
- **GIVEN** a compositor started headless without `--exit-with-child` and without a child program
- **WHEN** it receives the IPC `shutdown` command
- **THEN** it exits with `0`

### Requirement: A requested XWayland that cannot start is a startup error
When `--xwayland` is given and XWayland does not become usable, the compositor SHALL treat it as a startup error. Usable means the server has reported readiness and its X11 window manager is running. That includes a session that ends for any reason (`--timeout`, the IPC `shutdown` command, `SIGTERM`/`SIGINT`, closing the window) before XWayland became usable. On a startup error the compositor SHALL:
- log the failure at ERROR level, naming the cause and including the last lines of XWayland's own output when there is any;
- not announce readiness;
- not start the child program;
- end promptly;
- exit with `1`, with or without `--exit-with-child`.

The compositor SHALL wait for XWayland at most `--xwayland-timeout <secs>` seconds (default `10`, at least `1`). A server that has not become usable by then SHALL count as failed. The compositor SHALL detect a server that exits before becoming usable without waiting for the timeout.

These scenarios assume X11 display sockets can be created (a writable `/tmp/.X11-unix`); without them no XWayland can start, whatever the binary.

#### Scenario: Xwayland binary not found
- **GIVEN** a `PATH` that contains no `Xwayland` binary
- **WHEN** the compositor runs headless with `--xwayland --exit-with-child -- /bin/sh -c ': > <marker>'`
- **THEN** it exits with `1` within a few seconds, the log contains an ERROR naming the missing `Xwayland` binary, `READY` is never written, and the marker file does not exist

#### Scenario: Startup error without --exit-with-child
- **GIVEN** a `PATH` that contains no `Xwayland` binary
- **WHEN** the compositor runs headless with `--xwayland` and no child program
- **THEN** it exits with `1` within a few seconds instead of running without readiness

#### Scenario: Xwayland binary not executable
- **GIVEN** a `PATH` whose only `Xwayland` is a file without execute permission
- **WHEN** the compositor runs headless with `--xwayland --exit-with-child -- /bin/sh -c ': > <marker>'`
- **THEN** it exits with `1` within a few seconds, the ERROR log says the binary is not executable, no readiness is announced, and the marker file does not exist

#### Scenario: Xwayland exits before it is ready
- **GIVEN** an `Xwayland` on `PATH` that prints a diagnostic line to stderr and exits with a non-zero status without reporting a display
- **WHEN** the compositor runs headless with `--xwayland --xwayland-timeout 30 --exit-with-child -- /bin/sh -c ': > <marker>'`
- **THEN** it exits with `1` within a few seconds, well before the 30-second timeout, the ERROR log says XWayland exited before it was ready and includes the fake server's diagnostic line, no readiness is announced, and the marker file does not exist

#### Scenario: Xwayland never reports readiness
- **GIVEN** an `Xwayland` on `PATH` that stays alive without ever reporting a display
- **WHEN** the compositor runs headless with `--xwayland --xwayland-timeout 1 --exit-with-child -- /bin/sh -c ': > <marker>'`
- **THEN** it exits with `1` within a few seconds, the ERROR log names the startup timeout, no readiness is announced, and the marker file does not exist

#### Scenario: The X11 window manager cannot start
- **GIVEN** an `Xwayland` on `PATH` that reports a display and then exits, so no window manager connection can be established
- **WHEN** the compositor runs headless with `--xwayland --print-env --exit-with-child -- /bin/sh -c ': > <marker>'`
- **THEN** it exits with `1` within a few seconds, the ERROR log says the X11 window manager failed to start, no readiness is announced and no `DISPLAY=` line is printed, and the marker file does not exist

#### Scenario: The session ends while XWayland is still starting
- **GIVEN** an `Xwayland` on `PATH` that stays alive without ever reporting a display
- **WHEN** the compositor runs headless with `--xwayland --xwayland-timeout 30 --timeout 1` and no child program
- **THEN** it exits with `1` after about one second, and no readiness is announced

#### Scenario: An invalid startup timeout is rejected
- **WHEN** the compositor is started with `--xwayland --xwayland-timeout 0`
- **THEN** it exits with a non-zero status and a usage message naming `--xwayland-timeout`, without starting a session

#### Scenario: A working XWayland session succeeds
- **GIVEN** a working `Xwayland` binary on `PATH`
- **WHEN** the compositor runs headless with `--xwayland --print-env --exit-with-child -- /bin/sh -c 'test -n "$DISPLAY"'`
- **THEN** it exits with `0` and stdout contains a `DISPLAY=` line (verifiable only with a real `Xwayland` binary)

### Requirement: XWayland terminating during the session is reported, not fatal
When XWayland terminates after it became usable, the compositor SHALL:
- log it at ERROR level with its own message;
- remove all of that server's X11 windows, mapped and minimized alike, so they no longer appear in the window list or the foreign-toplevel list;
- report XWayland as inactive;
- keep serving Wayland clients without crashing.

It SHALL NOT restart XWayland, and SHALL NOT end the session because of it. A child program that depends on X11 fails on its own terms, and the exit-code rules report that result.

#### Scenario: Wayland clients keep working after XWayland dies
- **GIVEN** a ready compositor with a working XWayland, one mapped and one minimized X11 window, and one mapped Wayland window
- **WHEN** the Xwayland process is killed
- **THEN**:
  - the log contains the compositor's own ERROR message about XWayland terminating;
  - the IPC `list_windows` result contains neither X11 window, mapped or minimized, but still the Wayland window;
  - the IPC `status` result reports the `xwayland` field as `false`;
  - the compositor keeps answering IPC requests.

  This scenario is verifiable only with a real `Xwayland` binary.

### Requirement: Session status reports whether XWayland is usable
The IPC `status` result SHALL report the `xwayland` field as `true` only while XWayland is ready and its X11 window manager is running. It SHALL report `false` without `--xwayland`, while XWayland is still starting, and after XWayland has terminated. `platynui-wayland-compositor-ctl status` SHALL show the same state.

#### Scenario: Status without XWayland
- **GIVEN** a ready compositor started without `--xwayland`
- **WHEN** a client sends the IPC `status` command
- **THEN** the result's `xwayland` field is `false`

#### Scenario: Status while XWayland is still starting
- **GIVEN** a compositor started headless with the control socket enabled, `--xwayland --xwayland-timeout 10`, and an `Xwayland` on `PATH` that stays alive without reporting a display
- **WHEN** a client sends the IPC `status` command before the startup timeout
- **THEN** the result's `xwayland` field is `false`

#### Scenario: Status with a usable XWayland
- **GIVEN** a ready compositor started with `--xwayland` and a working `Xwayland` binary
- **WHEN** a client sends the IPC `status` command and `platynui-wayland-compositor-ctl status` runs against the same compositor
- **THEN** the result's `xwayland` field is `true` and the ctl output shows `yes` on its `XWayland:` line (verifiable only with a real `Xwayland` binary)
