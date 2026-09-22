//! Per-client state for Wayland clients, and the identity of a connection's peer.
//!
//! Which process a client belongs to is established **once**, when the
//! compositor accepts the connection and still owns the socket. Afterwards the
//! file descriptor belongs to `wayland-backend`, whose way back to the peer
//! credentials cannot express a peer it is unable to represent and panics
//! instead — which is why that accessor is banned in `clippy.toml`.

use std::os::unix::net::UnixStream;

use nix::sys::socket::{getsockopt, sockopt};
use smithay::wayland::compositor::CompositorClientState;

/// Why the compositor could not tell which process is on the other end of a
/// connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unidentified {
    /// The kernel reported a process id of `0`: the peer lives in a PID
    /// namespace this compositor cannot see, so the peer has no number here.
    /// The normal answer in a sidecar deployment, not a malfunction.
    PidNotVisible,
    /// The `SO_PEERCRED` read itself failed.
    ReadFailed,
}

impl std::fmt::Display for Unidentified {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PidNotVisible => {
                formatter.write_str("the peer has no process id in this PID namespace (SO_PEERCRED reported 0)")
            }
            Self::ReadFailed => formatter.write_str("the SO_PEERCRED read failed"),
        }
    }
}

/// The process behind an accepted connection, from its peer credentials.
///
/// `SO_PEERCRED` is a snapshot the kernel takes when the peer connects, so
/// reading it at accept time is as fresh as the answer ever gets — and accept
/// time is the only moment the compositor still holds the socket.
pub fn peer_process(stream: &UnixStream) -> Result<u32, Unidentified> {
    identify_peer(getsockopt(stream, sockopt::PeerCredentials).ok().map(|credentials| credentials.pid()))
}

/// Map what `SO_PEERCRED` answered onto the identity the compositor reports.
///
/// `Some(pid)` is the kernel's answer, `None` a read that failed. A process id
/// of `0` means the peer has no number in this namespace and a negative one is
/// no process at all; both are *unknown*. Neither is ever reported as a PID —
/// `0` is a PID-shaped value that a consumer filtering by process would match
/// against, silently associating unrelated windows.
fn identify_peer(peer_pid: Option<i32>) -> Result<u32, Unidentified> {
    match peer_pid {
        None => Err(Unidentified::ReadFailed),
        Some(pid) => u32::try_from(pid).ok().filter(|pid| *pid > 0).ok_or(Unidentified::PidNotVisible),
    }
}

/// Per-client data stored in each `wayland_server::Client`.
#[derive(Default, Debug)]
pub struct ClientState {
    /// Compositor-specific per-client state.
    pub compositor_state: CompositorClientState,
    /// The client's process, as established when its connection was accepted.
    /// `None` means it could not be established — the compositor does not guess
    /// and reports no placeholder.
    pub peer_process: Option<u32>,
}

impl ClientState {
    /// Per-client data for a freshly accepted connection, carrying the identity
    /// of its peer.
    ///
    /// The decision is logged here and never again. A client that could not be
    /// identified warns, because the compositor's default log level is `warn`
    /// (see [`crate::run`]) and a session in which nothing can be identified has
    /// to be visible in an ordinary log; an identified client logs at `debug`.
    /// Logging this per request instead would drown that log — `list_windows`
    /// runs on every window operation.
    #[must_use]
    pub fn accepted(stream: &UnixStream) -> Self {
        let peer_process = match peer_process(stream) {
            Ok(pid) => {
                tracing::debug!(pid, "Wayland client process identified");
                Some(pid)
            }
            Err(reason) => {
                tracing::warn!(%reason, "could not identify the client's process");
                None
            }
        };
        Self { compositor_state: CompositorClientState::default(), peer_process }
    }
}

impl wayland_server::backend::ClientData for ClientState {
    fn initialized(&self, _client_id: wayland_server::backend::ClientId) {}

    fn disconnected(
        &self,
        _client_id: wayland_server::backend::ClientId,
        _reason: wayland_server::backend::DisconnectReason,
    ) {
    }
}

// Re-export for use from wayland_server
use smithay::reexports::wayland_server;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_positive_peer_pid_identifies_that_process() {
        assert_eq!(identify_peer(Some(4242)), Ok(4242));
    }

    #[test]
    fn a_peer_pid_of_zero_is_unknown() {
        // The kernel reports 0 for a peer whose process has no number in this
        // namespace — the sidecar topology this mapping exists for.
        assert_eq!(identify_peer(Some(0)), Err(Unidentified::PidNotVisible));
    }

    #[test]
    fn a_negative_peer_pid_is_unknown() {
        assert_eq!(identify_peer(Some(-1)), Err(Unidentified::PidNotVisible));
    }

    #[test]
    fn a_failed_credential_read_is_unknown() {
        assert_eq!(identify_peer(None), Err(Unidentified::ReadFailed));
    }
}
