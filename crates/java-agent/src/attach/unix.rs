//! The Unix leg of the attach protocol: a trigger file, `SIGQUIT`, and the
//! JVM's own Unix domain socket.
//!
//! No `unsafe` anywhere — the whole protocol is a file, a signal and a socket.
//! That asymmetry with the Windows leg is why the design keeps "vendor
//! `jattach`'s Windows leg only" as the escape hatch rather than replacing this
//! side too.
//!
//! Sequence, as HotSpot's attach listener expects it:
//!
//! 1. If `/tmp/.java_pid<pid>` already exists, the listener is up — connect.
//! 2. Otherwise create `/tmp/.attach_pid<pid>` and send `SIGQUIT`. HotSpot
//!    treats that pair as "start your attach listener" rather than as the
//!    thread dump `SIGQUIT` normally triggers.
//! 3. Wait for the socket to appear, then connect and remove the trigger file.

use crate::error::AgentError;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How often to look for the socket while the JVM starts its listener.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

pub fn execute(pid: u32, command: &str, args: &[&str], timeout: Duration) -> Result<String, AgentError> {
    let socket = ensure_attach_listener(pid, timeout)?;
    let mut stream = UnixStream::connect(&socket).map_err(|e| AgentError::AttachFailed {
        pid,
        details: format!("cannot connect to {}: {e}", socket.display()),
    })?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(|e| AgentError::AttachFailed { pid, details: e.to_string() })?;

    stream
        .write_all(&request_bytes(command, args))
        .map_err(|e| AgentError::AttachFailed { pid, details: format!("cannot send the attach request: {e}") })?;

    let mut reply = Vec::new();
    stream
        .read_to_end(&mut reply)
        .map_err(|e| AgentError::AttachFailed { pid, details: format!("cannot read the attach reply: {e}") })?;
    Ok(String::from_utf8_lossy(&reply).into_owned())
}

/// The attach request: a protocol version, then NUL-terminated strings.
///
/// Exactly three argument slots, always — the listener reads a fixed count and
/// would block waiting for the rest.
fn request_bytes(command: &str, args: &[&str]) -> Vec<u8> {
    let mut request = Vec::new();
    request.extend_from_slice(b"1\0");
    request.extend_from_slice(command.as_bytes());
    request.push(0);
    for index in 0..3 {
        request.extend_from_slice(args.get(index).copied().unwrap_or("").as_bytes());
        request.push(0);
    }
    request
}

/// Returns the path of a socket the target JVM is listening on, starting the
/// listener if it is not up yet.
fn ensure_attach_listener(pid: u32, timeout: Duration) -> Result<PathBuf, AgentError> {
    if let Some(socket) = existing_socket(pid) {
        return Ok(socket);
    }

    let trigger = trigger_path(pid);
    std::fs::File::create(&trigger).map_err(|e| AgentError::AttachFailed {
        pid,
        details: format!("cannot create the attach trigger {}: {e}", trigger.display()),
    })?;

    let signalled = send_quit(pid);
    let deadline = Instant::now() + timeout;
    let mut found = None;
    if signalled.is_ok() {
        while Instant::now() < deadline {
            if let Some(socket) = existing_socket(pid) {
                found = Some(socket);
                break;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }
    // The JVM removes the trigger file itself once it has read it, but a JVM
    // that never answered leaves it behind, and a stale trigger changes how the
    // next SIGQUIT is interpreted.
    let _ = std::fs::remove_file(&trigger);

    match (signalled, found) {
        (Err(details), _) => Err(AgentError::AttachFailed { pid, details }),
        (Ok(()), Some(socket)) => Ok(socket),
        (Ok(()), None) => Err(AgentError::AttachFailed {
            pid,
            details: format!(
                "the JVM did not start its attach listener within {timeout:?} \
                 (is it running with -XX:+DisableAttachMechanism?)"
            ),
        }),
    }
}

/// An existing attach socket for `pid`, if it is one we may use.
fn existing_socket(pid: u32) -> Option<PathBuf> {
    socket_candidates(pid).into_iter().find(|path| is_our_socket(path))
}

/// The socket has to belong to us. A foreign-owned file at that path is either
/// a different user's JVM or an attempt to be handed our attach commands.
fn is_our_socket(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    metadata.uid() == rustix::process::getuid().as_raw()
}

/// HotSpot puts both files in `/tmp`, not in `$TMPDIR`.
///
/// On Linux the target may live in a mount namespace of its own (a container),
/// where its `/tmp` is reachable through `/proc/<pid>/root/tmp` — checked first,
/// because when it differs it is the one the JVM actually wrote to.
fn socket_candidates(pid: u32) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    #[cfg(target_os = "linux")]
    candidates.push(PathBuf::from(format!("/proc/{pid}/root/tmp/.java_pid{pid}")));
    candidates.push(PathBuf::from(format!("/tmp/.java_pid{pid}")));
    candidates
}

/// Where to place the "please start your attach listener" trigger file.
fn trigger_path(pid: u32) -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let namespaced = PathBuf::from(format!("/proc/{pid}/root/tmp"));
        if namespaced.is_dir() {
            return namespaced.join(format!(".attach_pid{pid}"));
        }
    }
    PathBuf::from(format!("/tmp/.attach_pid{pid}"))
}

fn send_quit(pid: u32) -> Result<(), String> {
    let raw = i32::try_from(pid).map_err(|_| format!("{pid} is not a valid process id"))?;
    let target = rustix::process::Pid::from_raw(raw).ok_or_else(|| format!("{pid} is not a valid process id"))?;
    rustix::process::kill_process(target, rustix::process::Signal::QUIT)
        .map_err(|e| format!("cannot signal process {pid}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_is_version_command_and_exactly_three_nul_terminated_args() {
        // Fewer arguments than slots must still fill all three, or the listener
        // blocks waiting for the rest.
        assert_eq!(request_bytes("load", &["instrument"]), b"1\0load\0instrument\0\0\0".to_vec());
        assert_eq!(
            request_bytes("load", &["instrument", "false", "/tmp/a.jar"]),
            b"1\0load\0instrument\0false\0/tmp/a.jar\0".to_vec()
        );
    }

    #[test]
    fn extra_arguments_beyond_the_three_slots_are_not_sent() {
        assert_eq!(request_bytes("x", &["a", "b", "c", "d"]), b"1\0x\0a\0b\0c\0".to_vec());
    }

    #[test]
    fn the_socket_lives_in_tmp_not_in_tmpdir() {
        let candidates = socket_candidates(4711);
        assert!(candidates.iter().any(|path| path == Path::new("/tmp/.java_pid4711")));
    }
}
