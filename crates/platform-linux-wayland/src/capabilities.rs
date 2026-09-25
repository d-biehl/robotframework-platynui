//! Compositor identification.
//!
//! The Wayland backend decides once, at initialization, which compositor it is
//! talking to, and every capability gate reads that answer. The evidence, in
//! the order it decides:
//!
//! 1. **The control-socket handshake.** The `PlatynUI` compositor names itself in
//!    its `status` response. One round trip over the channel every gated
//!    capability uses, so an identification by handshake is also proof that
//!    the channel works. It needs no process visibility, so it holds when the
//!    runtime runs in another PID namespace than the compositor.
//! 2. **The session environment.** `XDG_CURRENT_DESKTOP=platynui` marks a
//!    `PlatynUI` session (`scripts/startcompositor.sh` sets it). A session
//!    identified this way whose control socket does not answer reports the
//!    socket's error from every capability that needs it.
//! 3. **The peer process.** `SO_PEERCRED` on the Wayland socket and
//!    `/proc/<pid>/exe` name the compositor binary — the only way to tell
//!    Mutter from `KWin` from sway without a desktop variable. Across a PID
//!    namespace boundary the peer is not visible, which decides nothing.
//! 4. **The environment heuristic** for foreign desktops (GNOME, KDE, …).
//!
//! The decision itself is `decide_compositor`, a pure function over that
//! evidence; the backend logs one identification record naming the result,
//! the evidence and what decided.

use std::fmt;
use std::path::{Path, PathBuf};

use nix::sys::socket::{getsockopt, sockopt};
use platynui_core::platform::PlatformError;
use tracing::{debug, info, warn};
use wayland_client::Connection;

use crate::control_ipc::{self, HandshakeOutcome};

/// Known compositor types relevant for backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompositorType {
    /// Our own test compositor (platynui-wayland-compositor).
    PlatynUi,
    /// GNOME's Mutter.
    Mutter,
    /// KDE's `KWin`.
    KWin,
    /// Hyprland.
    Hyprland,
    /// Sway / wlroots-based.
    Sway,
    /// Another wlroots-based compositor.
    Wlroots,
    /// Compositor we don't specifically recognise.
    Unknown,
}

impl std::fmt::Display for CompositorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlatynUi => f.write_str("PlatynUI"),
            Self::Mutter => f.write_str("Mutter"),
            Self::KWin => f.write_str("KWin"),
            Self::Hyprland => f.write_str("Hyprland"),
            Self::Sway => f.write_str("Sway"),
            Self::Wlroots => f.write_str("wlroots"),
            Self::Unknown => f.write_str("Unknown"),
        }
    }
}

/// The evidence that decided an identification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Basis {
    /// The control socket answered with the `PlatynUI` identity marker.
    Handshake,
    /// `XDG_CURRENT_DESKTOP`.
    Environment,
    /// The Wayland socket's peer process.
    PeerProcess,
    /// Nothing: the compositor is unrecognised.
    None,
}

impl fmt::Display for Basis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Handshake => "handshake",
            Self::Environment => "environment",
            Self::PeerProcess => "peer-process",
            Self::None => "none",
        })
    }
}

/// Decide the compositor from the handshake outcome, the value of
/// `XDG_CURRENT_DESKTOP` and the name of the Wayland socket's peer binary
/// (`None` when the peer is not visible), in the order of the module
/// documentation.
pub(crate) fn decide_compositor(
    handshake: &HandshakeOutcome,
    desktop: &str,
    peer_process: Option<&str>,
) -> (CompositorType, Basis) {
    if matches!(handshake, HandshakeOutcome::Identified { .. }) {
        return (CompositorType::PlatynUi, Basis::Handshake);
    }
    let from_desktop = classify_desktop(desktop);
    if from_desktop == CompositorType::PlatynUi {
        return (CompositorType::PlatynUi, Basis::Environment);
    }
    if let Some(from_peer) = peer_process.map(classify_binary_name).filter(|ct| *ct != CompositorType::Unknown) {
        return (from_peer, Basis::PeerProcess);
    }
    if from_desktop != CompositorType::Unknown {
        return (from_desktop, Basis::Environment);
    }
    (CompositorType::Unknown, Basis::None)
}

/// The session's compositor, what decided it, and the control socket the
/// handshake asked.
#[derive(Debug, Clone)]
pub(crate) struct Identification {
    pub(crate) compositor: CompositorType,
    pub(crate) basis: Basis,
    pub(crate) control_socket: Option<PathBuf>,
    pub(crate) handshake: HandshakeOutcome,
}

impl Identification {
    /// The control socket and its handshake outcome, for a refusal that has to
    /// be traceable to this identification.
    pub(crate) fn control_channel(&self) -> String {
        match &self.control_socket {
            Some(path) => format!("control socket {}: {}", path.display(), self.handshake),
            None => self.handshake.to_string(),
        }
    }
}

/// The refusal of a capability the identified compositor does not support. It
/// names the capability, the compositor and — so the refusal can be traced to
/// the identification record — the control socket the handshake asked.
pub(crate) fn unsupported_compositor(capability: &'static str, compositor: Option<CompositorType>) -> PlatformError {
    let which = compositor.map_or_else(|| "an undetected Wayland compositor".to_string(), |c| c.to_string());
    let channel = crate::connection::control_channel().map(|channel| format!(" ({channel})")).unwrap_or_default();
    PlatformError::CapabilityUnavailable {
        capability,
        details: Some(format!(
            "not implemented for {which}; only the PlatynUI compositor (control socket) is supported so far{channel}"
        )),
    }
}

/// Identify the compositor behind `conn`, and log the identification record.
pub(crate) fn detect_compositor(conn: &Connection) -> Identification {
    let control_socket = control_ipc::discover_control_socket_path();
    let handshake = control_ipc::handshake(control_socket.as_deref());
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let peer = peer_process(conn);
    let (compositor, basis) = decide_compositor(&handshake, &desktop, peer.binary_name());
    let identification = Identification { compositor, basis, control_socket, handshake };
    record(&identification, &desktop, &peer);
    if let HandshakeOutcome::Identified { socket_name: Some(socket_name) } = &identification.handshake {
        warn_on_socket_mismatch(socket_name, &identification);
    }
    identification
}

/// Log the one identification record: `info` when the handshake decided,
/// `warn` when the session is unrecognised or identified without a working
/// control channel.
fn record(identification: &Identification, desktop: &str, peer: &PeerProcess) {
    let Identification { compositor, basis, control_socket, handshake } = identification;
    let control_socket = control_socket.as_deref().map_or_else(|| "none".to_owned(), |path| path.display().to_string());
    macro_rules! record {
        ($level:ident, $message:literal) => {
            $level!(
                %compositor,
                %basis,
                control_socket,
                %handshake,
                XDG_CURRENT_DESKTOP = desktop,
                peer_credentials = %peer,
                $message
            )
        };
    }
    match (compositor, basis) {
        (_, Basis::Handshake) => record!(info, "Wayland compositor identification"),
        (CompositorType::Unknown, _) => record!(
            warn,
            "Wayland compositor identification: unrecognised; capabilities that need a supported compositor are unavailable"
        ),
        (CompositorType::PlatynUi, _) => record!(
            warn,
            "Wayland compositor identification: PlatynUI without a working control-socket handshake; capabilities that use the control socket will fail naming it"
        ),
        _ => record!(info, "Wayland compositor identification"),
    }
}

/// Warn when the compositor answering on the control socket names another
/// Wayland socket than the one this client connected to. Advisory only: in a
/// sidecar the two sides may well mount the same socket at different paths.
fn warn_on_socket_mismatch(socket_name: &str, identification: &Identification) {
    if std::env::var_os("WAYLAND_SOCKET").is_some() {
        return; // connected through an inherited file descriptor: nothing to compare
    }
    let Ok(wayland_display) = std::env::var("WAYLAND_DISPLAY") else {
        return;
    };
    if socket_names_disagree(socket_name, &wayland_display) {
        warn!(
            compositor_socket = socket_name,
            WAYLAND_DISPLAY = wayland_display,
            control_socket = %identification.control_channel(),
            "the compositor answering on the control socket reports a different Wayland socket than WAYLAND_DISPLAY; \
             the identification stands, but check that both name the same compositor"
        );
    }
}

/// Whether the compositor's socket name and the client's `WAYLAND_DISPLAY`
/// (a name under `XDG_RUNTIME_DIR`, or an absolute path) name different sockets.
fn socket_names_disagree(socket_name: &str, wayland_display: &str) -> bool {
    Path::new(wayland_display).file_name().is_some_and(|name| name != socket_name)
}

/// What `SO_PEERCRED` on the Wayland socket says about the compositor process.
enum PeerProcess {
    /// A process visible in our PID namespace, with its binary.
    Visible { pid: i32, binary: String },
    /// The process has no number in our PID namespace.
    NotVisible,
    /// The credentials or the binary could not be read.
    Unreadable(String),
}

impl PeerProcess {
    fn binary_name(&self) -> Option<&str> {
        match self {
            Self::Visible { binary, .. } => Path::new(binary).file_name().and_then(|name| name.to_str()),
            Self::NotVisible | Self::Unreadable(_) => None,
        }
    }
}

impl fmt::Display for PeerProcess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Visible { pid, binary } => write!(f, "pid {pid} ({binary})"),
            Self::NotVisible => f.write_str("peer process not visible in this PID namespace"),
            Self::Unreadable(reason) => write!(f, "unreadable: {reason}"),
        }
    }
}

/// Read the Wayland socket's peer process. The kernel reports a peer in a PID
/// namespace we cannot see as process id `0`, which is "not visible" — never
/// an identity, and never evidence against any compositor.
fn peer_process(conn: &Connection) -> PeerProcess {
    let backend = conn.backend();
    let fd = backend.poll_fd();
    let pid = match getsockopt(&fd, sockopt::PeerCredentials) {
        Ok(credentials) => credentials.pid(),
        Err(error) => return PeerProcess::Unreadable(format!("SO_PEERCRED: {error}")),
    };
    if pid <= 0 {
        return PeerProcess::NotVisible;
    }
    match std::fs::read_link(format!("/proc/{pid}/exe")) {
        Ok(path) => PeerProcess::Visible { pid, binary: path.display().to_string() },
        Err(error) => PeerProcess::Unreadable(format!("/proc/{pid}/exe: {error}")),
    }
}

/// Classify a compositor by its binary name.
fn classify_binary_name(name: &str) -> CompositorType {
    // Normalize: strip path, lowercase for matching.
    let lower = name.to_ascii_lowercase();
    if lower.contains("platynui") {
        CompositorType::PlatynUi
    } else if lower.contains("mutter") || lower.contains("gnome-shell") {
        CompositorType::Mutter
    } else if lower.contains("kwin") {
        CompositorType::KWin
    } else if lower == "hyprland" || lower.starts_with("hyprland") {
        CompositorType::Hyprland
    } else if lower == "sway" {
        CompositorType::Sway
    } else if lower.contains("wlroots") {
        CompositorType::Wlroots
    } else {
        debug!(binary = name, "unrecognised compositor binary — using Unknown");
        CompositorType::Unknown
    }
}

/// Classify a compositor by the session's `XDG_CURRENT_DESKTOP`.
fn classify_desktop(desktop: &str) -> CompositorType {
    let lower = desktop.to_ascii_lowercase();
    if lower.contains("platynui") {
        CompositorType::PlatynUi
    } else if lower.contains("gnome") {
        CompositorType::Mutter
    } else if lower.contains("kde") || lower.contains("plasma") {
        CompositorType::KWin
    } else if lower.contains("hyprland") {
        CompositorType::Hyprland
    } else if lower.contains("sway") {
        CompositorType::Sway
    } else {
        CompositorType::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_known_binaries() {
        assert_eq!(classify_binary_name("mutter"), CompositorType::Mutter);
        assert_eq!(classify_binary_name("gnome-shell"), CompositorType::Mutter);
        assert_eq!(classify_binary_name("kwin_wayland"), CompositorType::KWin);
        assert_eq!(classify_binary_name("Hyprland"), CompositorType::Hyprland);
        assert_eq!(classify_binary_name("sway"), CompositorType::Sway);
        assert_eq!(classify_binary_name("platynui-wayland-compositor"), CompositorType::PlatynUi);
        assert_eq!(classify_binary_name("cage"), CompositorType::Unknown);
    }

    // ── The decision (spec: *Compositor identification does not depend on
    //    process visibility*) ──────────────────────────────────────────────

    fn identified() -> HandshakeOutcome {
        HandshakeOutcome::Identified { socket_name: Some("wl-0".into()) }
    }

    /// Handshake outcomes that do not decide anything.
    fn undecided() -> [HandshakeOutcome; 4] {
        [
            HandshakeOutcome::NoMarker,
            HandshakeOutcome::NoSocketPath,
            HandshakeOutcome::ConnectFailed("connection refused".into()),
            HandshakeOutcome::ExchangeFailed("timed out".into()),
        ]
    }

    #[test]
    fn a_handshake_with_the_marker_identifies_platynui() {
        assert_eq!(decide_compositor(&identified(), "", None), (CompositorType::PlatynUi, Basis::Handshake));
    }

    #[test]
    fn a_handshake_that_does_not_decide_identifies_nothing_by_itself() {
        // An answer without the marker, no socket path, a refused connection, a
        // timeout: none of them is "PlatynUI", and none of them is "not PlatynUI".
        for outcome in undecided() {
            assert_eq!(decide_compositor(&outcome, "", None), (CompositorType::Unknown, Basis::None), "{outcome:?}");
        }
    }

    #[test]
    fn the_session_environment_identifies_platynui_when_the_handshake_does_not() {
        for outcome in undecided() {
            assert_eq!(
                decide_compositor(&outcome, "platynui", None),
                (CompositorType::PlatynUi, Basis::Environment),
                "{outcome:?}"
            );
        }
        assert_eq!(
            decide_compositor(&HandshakeOutcome::NoSocketPath, "PlatynUI", None),
            (CompositorType::PlatynUi, Basis::Environment)
        );
    }

    #[test]
    fn the_handshake_decides_even_where_the_environment_agrees() {
        assert_eq!(decide_compositor(&identified(), "platynui", None), (CompositorType::PlatynUi, Basis::Handshake));
    }

    #[test]
    fn a_marker_on_a_foreign_desktop_still_identifies_platynui() {
        assert_eq!(
            decide_compositor(&identified(), "GNOME", Some("gnome-shell")),
            (CompositorType::PlatynUi, Basis::Handshake)
        );
    }

    #[test]
    fn a_visible_foreign_peer_names_its_compositor() {
        let refused = HandshakeOutcome::ConnectFailed("no such file".into());
        assert_eq!(
            decide_compositor(&refused, "GNOME", Some("gnome-shell")),
            (CompositorType::Mutter, Basis::PeerProcess)
        );
        assert_eq!(decide_compositor(&refused, "", Some("kwin_wayland")), (CompositorType::KWin, Basis::PeerProcess));
    }

    #[test]
    fn a_visible_platynui_process_identifies_it_without_a_control_socket() {
        // `--no-control-socket` in the compositor's own namespace (design 1).
        assert_eq!(
            decide_compositor(&HandshakeOutcome::NoSocketPath, "", Some("platynui-wayland-compositor")),
            (CompositorType::PlatynUi, Basis::PeerProcess)
        );
    }

    #[test]
    fn a_foreign_desktop_without_a_visible_peer_is_classified_from_the_environment() {
        let none = HandshakeOutcome::NoSocketPath;
        assert_eq!(decide_compositor(&none, "KDE", None), (CompositorType::KWin, Basis::Environment));
        assert_eq!(decide_compositor(&none, "ubuntu:GNOME", None), (CompositorType::Mutter, Basis::Environment));
        assert_eq!(decide_compositor(&none, "sway", Some("cage")), (CompositorType::Sway, Basis::Environment));
    }

    #[test]
    fn a_session_with_no_evidence_stays_unrecognised() {
        assert_eq!(
            decide_compositor(&HandshakeOutcome::NoSocketPath, "", None),
            (CompositorType::Unknown, Basis::None)
        );
        assert_eq!(
            decide_compositor(&HandshakeOutcome::NoSocketPath, "", Some("cage")),
            (CompositorType::Unknown, Basis::None)
        );
    }

    #[test]
    fn the_compositor_socket_is_compared_by_name() {
        assert!(!socket_names_disagree("wl-0", "wl-0"));
        assert!(!socket_names_disagree("wl-0", "/run/pod/xdg/wl-0"), "an absolute WAYLAND_DISPLAY compares by name");
        assert!(socket_names_disagree("wl-0", "wayland-1"));
        assert!(socket_names_disagree("wl-0", "/run/pod/xdg/wayland-0"));
    }
}
