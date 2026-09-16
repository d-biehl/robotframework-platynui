## Why

A compositor started with `--xwayland` holds back its readiness announcement and its child program until XWayland reports that it is ready. When XWayland never gets there, nothing releases them: the binary is missing or not executable, the server exits before it is ready, it stays alive without ever reporting, or the X11 window manager fails to start. The compositor then runs indefinitely without announcing readiness or starting the child. A CI step using `--exit-with-child` or `--ready-fd` hangs until the job timeout, and with `--timeout` it ends after the timeout without any usable signal. At the scripts' default log level, the only trace is a warning nobody sees.

No lane passes `--xwayland` today, so CI is not affected yet. The Java provider work on Linux (`java-provider-linux`, Swing through XWayland) will be the first consumer. Commit ae5f614 established that a session must report its result instead of idling or reading as success, and this is the last startup path that still breaks that rule.

## What Changes

- **Fatal XWayland startup failure**: when `--xwayland` is requested and XWayland does not become usable, the compositor logs the failure at ERROR level, announces no readiness, starts no child program, and exits with code `1`. This applies whether or not `--exit-with-child` is given. It covers these cases:
  - the binary is missing or not executable;
  - the server exits before it is ready;
  - the server does not report readiness within the startup timeout;
  - the X11 window manager fails to start;
  - the session is ended from outside before XWayland became usable.

  Making a window-manager start failure fatal also removes a panic that is reachable today: `wm` stays unset while X11 clients can still reach the window-manager hooks.
- **Startup timeout**: a new `--xwayland-timeout <secs>` option (default `10`, the same bound `startxsession.sh` uses for an X server) limits how long the compositor waits for XWayland.
- **Readiness means usable**: with `--xwayland`, readiness and the child program follow only after XWayland is ready *and* its window manager runs, so `DISPLAY` is never handed out for a server that cannot serve X11 clients. Readiness and child launch happen in one place, after the control socket and EIS are set up, on both the XWayland and the plain path.
- **Visible cause**: XWayland's own output is forwarded into the compositor log instead of being discarded, and the fatal startup error includes the last lines of it.
- **XWayland terminating mid-session**: today nothing notices. With this change the compositor logs it at ERROR level, removes the dead server's X11 windows (mapped and minimized), reports XWayland as inactive, and keeps serving Wayland clients.
- **Status reports usability**: the IPC `status` field `xwayland` (shown by `platynui-wayland-compositor-ctl status`) is `true` only while XWayland is ready and its window manager runs. Today it is already `true` before readiness and stays `true` after failures.
- **Documentation**: add the startup-failure row to the exit-code table in the compositor usage docs and document the new option. Fix the drift found along the way: `--control-socket` should be `--no-control-socket`, and the `xwayland` cargo feature mentioned in the help text and module docs does not exist.
- **Not breaking**: callers of `--xwayland` whose XWayland fails to start hung until now, so no working setup depends on the old behavior. The IPC field keeps its name and type; only its meaning becomes precise (it no longer reports `true` before readiness or after a failure).

## Capabilities

### New Capabilities

- `compositor-session-lifecycle`: how a PlatynUI compositor session starts and ends. This covers when readiness is announced and what it carries, when the child program starts, how the session's exit code reports the child's result (the contract from ae5f614, which so far lives only in docs and tests), XWayland as a requested startup prerequisite, and how XWayland's state is reported during the session.

### Modified Capabilities

None. The existing compositor capabilities (`compositor-modifier-state`, `compositor-popup-geometry`) cover unrelated features.

## Impact

- **Rust, `apps/wayland-compositor` only**:
  - `src/xwayland.rs`: startup watch, window-manager-before-readiness, termination handling, output forwarding.
  - `src/backend/mod.rs`: one startup finalization after control socket and EIS.
  - `src/state.rs`: startup-failure exit code and XWayland status.
  - `src/lib.rs`: the `--xwayland-timeout` option and help text.
  - `src/control.rs`: the meaning of the status field.
  - `src/ready.rs`: unchanged interface.
- **Tests**:
  - A new integration test binary drives the real compositor binary with a fake `Xwayland` on `PATH`; no real XWayland is needed. The failing cases run in `just test` and CI.
  - A happy-path test and a mid-session termination test need a real `Xwayland` binary. CI does not install one, so they are ignored by default and run locally.
  - The existing `child_exit_tests` and `ipc_tests` stay green as regression anchors.
- **Tools and docs**:
  - `apps/wayland-compositor-ctl` needs no code change; its "XWayland: yes/no" line becomes accurate.
  - Docs: `apps/wayland-compositor/docs/usage.md`, `apps/wayland-compositor/docs/ipc-protocol.md`, `apps/wayland-compositor/README.md`.
- **Scripts and lanes**:
  - `scripts/startcompositor.sh` needs no change. Its `--xwayland` passthrough gets the fatal behavior, and the exit code reaches the lane result through the wrapper chain.
  - The acceptance lanes do not use `--xwayland`, so their results are unchanged.
- **No native Python rebuild**: nothing in `packages/native` or the Robot Framework library changes.
- **Platforms**: Linux only (the compositor builds only there). This applies to all three backends (headless, winit, DRM), since the startup path is shared.
