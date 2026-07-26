//! Reading the handshake files agents publish.
//!
//! One mechanism answers four questions at once (design decision 1a): which
//! loopback port the agent bound, what token authenticates a client, which
//! toolkits are live in that JVM, and — by merely existing for a running pid —
//! whether that JVM has an agent at all.
//!
//! Nothing here scans on its own. Quiescence is a property of the caller: with
//! agent support inactive the provider never calls into this module, so no
//! directory is read and no file is touched.

use crate::error::AgentError;
use crate::jvm;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What an agent publishes about itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeInfo {
    /// Wire-format version of the handshake file and the RPC on top of it.
    pub protocol: u32,
    /// The JVM's process id.
    pub pid: u32,
    /// The loopback port the agent bound.
    pub port: u16,
    /// The token a client must present.
    pub token: String,
    /// The toolkits live in that JVM; empty while none has loaded yet.
    pub toolkits: Vec<String>,
    /// The agent's version, compared for exact equality on connect.
    #[serde(rename = "agentVersion")]
    pub agent_version: String,
}

/// The protocol version this client speaks.
pub const SUPPORTED_PROTOCOL: u32 = 1;

/// Reads and parses one handshake file.
///
/// # Errors
///
/// [`AgentError::Transport`] when the file cannot be read and
/// [`AgentError::Protocol`] when its content is malformed or announces a
/// protocol this client does not speak.
pub fn read_file(path: &Path) -> Result<HandshakeInfo, AgentError> {
    let content = std::fs::read_to_string(path).map_err(|e| AgentError::Transport {
        details: format!("cannot read the handshake file {}: {e}", path.display()),
    })?;
    let info: HandshakeInfo = serde_json::from_str(&content)
        .map_err(|e| AgentError::Protocol { details: format!("malformed handshake file {}: {e}", path.display()) })?;
    if info.protocol != SUPPORTED_PROTOCOL {
        return Err(AgentError::Protocol {
            details: format!(
                "handshake file {} declares protocol {}, this client speaks {SUPPORTED_PROTOCOL}",
                path.display(),
                info.protocol
            ),
        });
    }
    Ok(info)
}

/// The agent published by process `pid`, if there is a live one.
///
/// `Ok(None)` covers both "no agent here" and "the file is stale" — a file
/// whose process no longer runs must never lead to a connection attempt,
/// because the port may since have been taken by something else entirely.
///
/// # Errors
///
/// The parse errors of [`read_file`], and [`AgentError::Protocol`] when the
/// file's own pid disagrees with its name.
pub fn for_pid(pid: u32) -> Result<Option<HandshakeInfo>, AgentError> {
    for_pid_in(&paths::handshake_dir(), pid)
}

/// [`for_pid`], against a directory the caller names.
///
/// The lookup is a single `is_file` on one exact path — it never lists the
/// directory. That is what makes the quiescence promise cheap to keep: asking
/// "does this JVM have an agent?" about a process that has none costs one failed
/// stat, whether or not any agent exists anywhere.
///
/// # Errors
///
/// The same as [`for_pid`].
pub fn for_pid_in(directory: &Path, pid: u32) -> Result<Option<HandshakeInfo>, AgentError> {
    let path = paths::handshake_file_in(directory, pid);
    if !path.is_file() {
        return Ok(None);
    }
    let info = read_file(&path)?;
    if info.pid != pid {
        return Err(AgentError::Protocol {
            details: format!("handshake file {} reports pid {} instead of {pid}", path.display(), info.pid),
        });
    }
    if !jvm::process_is_alive(pid) {
        tracing::debug!(pid, path = %path.display(), "ignoring a stale handshake file");
        return Ok(None);
    }
    Ok(Some(info))
}

/// Whether process `pid` carries a PlatynUI agent.
///
/// The cheap form of the signal — a file existence check plus a liveness probe,
/// which is what the Java-app classification consumes. It costs nothing when
/// there is no agent, and it deliberately does **not** connect: consuming the
/// signal must never itself instrument anything.
#[must_use]
pub fn agent_present(pid: u32) -> bool {
    matches!(for_pid(pid), Ok(Some(_)))
}

/// Every live agent this user can see.
///
/// For diagnostics and tests; the normal path is [`for_pid`], because PlatynUI
/// arrives at a JVM through the window that owns it, not by browsing.
pub fn discover() -> Vec<HandshakeInfo> {
    let directory = paths::handshake_dir();
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return Vec::new();
    };
    let mut agents = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(paths::pid_from_file_name) else {
            continue;
        };
        match for_pid(pid) {
            Ok(Some(info)) => agents.push(info),
            Ok(None) => {}
            Err(e) => tracing::debug!(pid, error = %e, "skipping an unreadable handshake file"),
        }
    }
    agents.sort_by_key(|info| info.pid);
    agents
}

/// Deletes handshake files whose process is gone, and reports what it removed.
///
/// A killed JVM never runs its shutdown hook, so stale files accumulate;
/// nothing depends on this running, since [`for_pid`] already ignores them.
pub fn remove_stale() -> Vec<PathBuf> {
    let directory = paths::handshake_dir();
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return Vec::new();
    };
    let mut removed = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(paths::pid_from_file_name) else {
            continue;
        };
        if jvm::process_is_alive(pid) {
            continue;
        }
        let path = entry.path();
        if std::fs::remove_file(&path).is_ok() {
            removed.push(path);
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_handshake(dir: &Path, pid: u32, body: &str) -> PathBuf {
        let path = dir.join(format!("agent-{pid}"));
        std::fs::write(&path, body).expect("write handshake file");
        path
    }

    fn sample(pid: u32) -> String {
        format!(
            r#"{{"protocol":1,"pid":{pid},"port":51234,"token":"abc","toolkits":["swing"],"agentVersion":"9.9.9"}}"#
        )
    }

    #[test]
    fn a_well_formed_file_parses() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write_handshake(dir.path(), 4711, &sample(4711));
        let info = read_file(&path).expect("parse");
        assert_eq!(info.pid, 4711);
        assert_eq!(info.port, 51234);
        assert_eq!(info.token, "abc");
        assert_eq!(info.toolkits, vec!["swing".to_owned()]);
        assert_eq!(info.agent_version, "9.9.9");
    }

    #[test]
    fn an_empty_toolkit_list_is_valid() {
        // A JVM injected at launch has loaded no toolkit class yet; that is a
        // normal answer, not a malformed file.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write_handshake(
            dir.path(),
            7,
            r#"{"protocol":1,"pid":7,"port":1,"token":"t","toolkits":[],"agentVersion":"1.0"}"#,
        );
        assert!(read_file(&path).expect("parse").toolkits.is_empty());
    }

    #[test]
    fn a_future_protocol_is_refused_rather_than_guessed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write_handshake(
            dir.path(),
            8,
            r#"{"protocol":99,"pid":8,"port":1,"token":"t","toolkits":[],"agentVersion":"1.0"}"#,
        );
        assert!(matches!(read_file(&path), Err(AgentError::Protocol { .. })));
    }

    #[test]
    fn malformed_content_is_a_protocol_error_not_a_panic() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = write_handshake(dir.path(), 9, "{ not json");
        assert!(matches!(read_file(&path), Err(AgentError::Protocol { .. })));
    }

    #[test]
    fn a_missing_file_is_a_transport_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("agent-1");
        assert!(matches!(read_file(&path), Err(AgentError::Transport { .. })));
    }

    /// Quiescence: asking about a JVM with no agent must touch nothing. No
    /// directory listing, no directory creation — one failed stat and done.
    #[test]
    fn asking_about_an_agentless_process_creates_and_scans_nothing() {
        let workspace = tempfile::tempdir().expect("temp dir");
        let absent = workspace.path().join("never-created");

        assert!(for_pid_in(&absent, 4711).expect("lookup").is_none());
        assert!(!absent.exists(), "a lookup must never create the handshake directory");

        // Even an existing directory full of other agents is not listed: the
        // path is computed from the pid, so an unrelated file is invisible.
        let populated = workspace.path().join("agents");
        std::fs::create_dir(&populated).expect("create");
        write_handshake(&populated, 1234, &sample(1234));
        assert!(for_pid_in(&populated, 4711).expect("lookup").is_none());
    }

    /// A file whose process is gone must never lead to a connection: by then
    /// the port it names may belong to something else entirely.
    #[test]
    fn a_file_for_a_dead_process_is_not_offered_as_an_agent() {
        let dir = tempfile::tempdir().expect("temp dir");
        // A pid that cannot be running; the file is well-formed regardless.
        let dead = 0xFFFF_FFF0_u32;
        write_handshake(dir.path(), dead, &sample(dead));
        assert!(for_pid_in(dir.path(), dead).expect("lookup").is_none());
    }

    /// A file naming a different pid than its own name is not something to
    /// guess about — it is either corrupt or somebody else's.
    #[test]
    fn a_file_whose_pid_disagrees_with_its_name_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");
        write_handshake(dir.path(), 4711, &sample(9999));
        assert!(matches!(for_pid_in(dir.path(), 4711), Err(AgentError::Protocol { .. })));
    }
}
