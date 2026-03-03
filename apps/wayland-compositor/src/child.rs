//! Child program management — spawn a program after compositor readiness.
//!
//! Like Weston, Sway, and other compositors, trailing arguments after `--` are
//! interpreted as a program with arguments, spawned once the compositor is ready
//! (Wayland socket created, all protocols registered, optionally `XWayland` ready).
//!
//! With `--exit-with-child`, the compositor shuts down when the child exits —
//! essential for CI pipelines.

use std::process::{Child, Command};

use calloop::{
    LoopHandle,
    timer::{TimeoutAction, Timer},
};

/// How often to check whether the child process has exited.
const CHILD_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

use crate::state::State;

/// Spawn the child program with the compositor's environment.
///
/// The child inherits `WAYLAND_DISPLAY`, `DISPLAY` (if `XWayland`), and
/// `XDG_RUNTIME_DIR` from the compositor process environment.
///
/// Returns the [`Child`] handle if the program was spawned successfully,
/// or `None` if no child command was specified.
///
/// # Panics
///
/// Panics if `command` is non-empty but `split_first()` fails (unreachable).
pub fn spawn_child(command: &[String]) -> Option<Child> {
    if command.is_empty() {
        return None;
    }

    let (program, args) = command.split_first().expect("command is non-empty");
    tracing::info!(program, ?args, "spawning child program");

    match Command::new(program).args(args).spawn() {
        Ok(child) => {
            tracing::info!(pid = child.id(), program, "child program started");
            Some(child)
        }
        Err(err) => {
            tracing::error!(program, %err, "failed to spawn child program");
            None
        }
    }
}

/// Register a calloop timer that periodically checks if the child process has exited.
///
/// When the child exits, `state.running` is set to `false` so the event loop
/// terminates gracefully. The child's exit code is logged.
///
/// # Errors
///
/// Returns an error if the timer source cannot be registered with the event loop.
pub fn monitor_child_exit(
    loop_handle: &LoopHandle<'static, State>,
    child: Child,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check periodically — fast enough for CI, negligible overhead.
    let timer = Timer::from_duration(CHILD_POLL_INTERVAL);

    loop_handle
        .insert_source(timer, {
            let mut child = child;
            move |_deadline, (), state| match child.try_wait() {
                Ok(Some(status)) => {
                    if status.success() {
                        tracing::info!(code = 0, "child program exited successfully");
                    } else {
                        let code = status.code().unwrap_or(-1);
                        tracing::warn!(code, "child program exited with error");
                    }
                    state.running = false;
                    TimeoutAction::Drop
                }
                Ok(None) => TimeoutAction::ToDuration(CHILD_POLL_INTERVAL),
                Err(err) => {
                    tracing::error!(%err, "failed to check child process status");
                    state.running = false;
                    TimeoutAction::Drop
                }
            }
        })
        .map_err(|err| format!("failed to register child monitor timer: {err}"))?;

    tracing::debug!("child exit monitor registered");
    Ok(())
}
