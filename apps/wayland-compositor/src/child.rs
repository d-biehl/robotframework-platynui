//! Child program management — spawn a program after compositor readiness.
//!
//! Like Weston, Sway, and other compositors, trailing arguments after `--` are
//! interpreted as a program with arguments, spawned once the compositor is ready
//! (Wayland socket created, all protocols registered, optionally `XWayland` ready).
//!
//! With `--exit-with-child`, the compositor shuts down when the child exits and
//! reports the child's result as its own exit code — essential for CI pipelines,
//! where a failing test run inside the session must fail the whole command (see
//! [`State::exit_code`]).

use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command, ExitStatus};

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
/// Without `XWayland` (`inherit_display == false`), `DISPLAY` is removed
/// from the child environment: any value still present is the *host* X
/// server leaking through the compositor process (which may legitimately
/// need it for its own nested window). A session component binding that
/// display would observe the wrong seat — e.g. the Inspector's X11 modifier
/// reader watching the host keyboard instead of this compositor's.
///
/// Returns the [`Child`] handle if the program was spawned, or `None` if no
/// child command was specified.
///
/// # Errors
///
/// Returns the spawn error if the program could not be started.
pub fn spawn_child(command: &[String], inherit_display: bool) -> io::Result<Option<Child>> {
    let Some((program, args)) = command.split_first() else {
        return Ok(None);
    };
    tracing::info!(program, ?args, inherit_display, "spawning child program");

    let mut cmd = Command::new(program);
    cmd.args(args);
    if !inherit_display {
        cmd.env_remove("DISPLAY");
    }

    let child = cmd.spawn().inspect_err(|err| tracing::error!(program, %err, "failed to spawn child program"))?;
    tracing::info!(pid = child.id(), program, "child program started");
    Ok(Some(child))
}

/// Map the child's exit status to the exit code the compositor reports.
///
/// A normal exit keeps its code. A child terminated by signal `n` maps to
/// `128 + n`, the shell convention, so a crashed or killed child never reads
/// as success.
fn exit_code(status: ExitStatus) -> u8 {
    status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1)
}

/// Map a spawn error to the exit code the compositor reports, following the
/// shell convention: `127` if the program does not exist, `126` if it cannot
/// be executed, `1` otherwise.
pub(crate) fn spawn_error_exit_code(err: &io::Error) -> u8 {
    match err.kind() {
        io::ErrorKind::NotFound => 127,
        io::ErrorKind::PermissionDenied => 126,
        _ => 1,
    }
}

/// Check whether the monitored child (`state.child`) has exited.
///
/// Once it has, its exit code (see [`exit_code`]) is recorded in
/// `state.child_exit_code` and `state.running` is set to `false`, so the event
/// loop terminates gracefully and the compositor exits with that code.
///
/// Returns `true` once there is nothing left to monitor.
pub(crate) fn poll_child_exit(state: &mut State) -> bool {
    let Some(child) = state.child.as_mut() else {
        return true;
    };
    let code = match child.try_wait() {
        Ok(None) => return false,
        Ok(Some(status)) => {
            let code = exit_code(status);
            if status.success() {
                tracing::info!(code, "child program exited successfully");
            } else {
                tracing::warn!(code, %status, "child program exited with error");
            }
            code
        }
        Err(err) => {
            tracing::error!(%err, "failed to check child process status");
            // The child's outcome is unknown — never report it as success.
            1
        }
    };
    state.child = None;
    state.child_exit_code = Some(code);
    state.running = false;
    true
}

/// Register a calloop timer that periodically checks whether the monitored
/// child (`state.child`) has exited (see `poll_child_exit`).
///
/// # Errors
///
/// Returns an error if the timer source cannot be registered with the event loop.
pub fn monitor_child_exit(loop_handle: &LoopHandle<'static, State>) -> Result<(), Box<dyn std::error::Error>> {
    // Check periodically — fast enough for CI, negligible overhead.
    let timer = Timer::from_duration(CHILD_POLL_INTERVAL);

    loop_handle
        .insert_source(timer, |_deadline, (), state| {
            if poll_child_exit(state) { TimeoutAction::Drop } else { TimeoutAction::ToDuration(CHILD_POLL_INTERVAL) }
        })
        .map_err(|err| format!("failed to register child monitor timer: {err}"))?;

    tracing::debug!("child exit monitor registered");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_keeps_a_normal_exit_code() {
        // Raw wait status: the exit code sits in the second byte.
        assert_eq!(exit_code(ExitStatus::from_raw(0)), 0);
        assert_eq!(exit_code(ExitStatus::from_raw(3 << 8)), 3);
        assert_eq!(exit_code(ExitStatus::from_raw(255 << 8)), 255);
    }

    #[test]
    fn exit_code_maps_a_signal_to_128_plus_signal() {
        // Raw wait status: a terminating signal sits in the low bits (9 = SIGKILL, 15 = SIGTERM).
        assert_eq!(exit_code(ExitStatus::from_raw(9)), 137);
        assert_eq!(exit_code(ExitStatus::from_raw(15)), 143);
    }

    #[test]
    fn spawn_error_exit_code_follows_the_shell_convention() {
        assert_eq!(spawn_error_exit_code(&io::Error::from(io::ErrorKind::NotFound)), 127);
        assert_eq!(spawn_error_exit_code(&io::Error::from(io::ErrorKind::PermissionDenied)), 126);
        assert_eq!(spawn_error_exit_code(&io::Error::from(io::ErrorKind::OutOfMemory)), 1);
    }
}
