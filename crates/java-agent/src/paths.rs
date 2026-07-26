//! Where agents publish themselves.
//!
//! **This rule is a contract with the Java side** — `java/agent`'s
//! `platynui.agent.AgentPaths` computes the same path, and a change here has to
//! happen there in the same commit.
//!
//! | Platform | Directory |
//! |---|---|
//! | any | `$PLATYNUI_AGENT_DIR` when set — the test and dev override |
//! | Windows | `%LOCALAPPDATA%\PlatynUI\agents` |
//! | Unix, `XDG_RUNTIME_DIR` set | `$XDG_RUNTIME_DIR/platynui/agents` |
//! | Unix, otherwise | `$TMPDIR/platynui-<user>/agents` (`/tmp` if unset) |
//!
//! The environment variable rather than a platform temp API on both sides, so
//! the two agree even when the target JVM was launched with an overridden
//! `java.io.tmpdir`.

use std::path::PathBuf;

/// Environment variable that overrides the handshake directory.
pub const DIR_ENV: &str = "PLATYNUI_AGENT_DIR";

/// The handshake directory for the current user. Not created here — only the
/// agent creates it, and it creates it owner-only.
#[must_use]
pub fn handshake_dir() -> PathBuf {
    if let Some(override_dir) = std::env::var_os(DIR_ENV).filter(|value| !value.is_empty()) {
        return PathBuf::from(override_dir);
    }
    platform_handshake_dir()
}

#[cfg(windows)]
fn platform_handshake_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").filter(|value| !value.is_empty()).map_or_else(
        || {
            let home = std::env::var_os("USERPROFILE").unwrap_or_default();
            PathBuf::from(home).join("AppData").join("Local")
        },
        PathBuf::from,
    );
    base.join("PlatynUI").join("agents")
}

#[cfg(unix)]
fn platform_handshake_dir() -> PathBuf {
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR").filter(|value| !value.is_empty()) {
        return PathBuf::from(runtime_dir).join("platynui").join("agents");
    }
    // Fallback only: `/tmp` is world-writable, so the agent hardens the
    // directory on every start. The user name has to match what the JVM's
    // `user.name` property reports — in a desktop session both come from the
    // same passwd entry, but `XDG_RUNTIME_DIR` is the reliable path.
    let base = std::env::var_os("TMPDIR")
        .filter(|value| !value.is_empty())
        .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    let user = std::env::var("USER").or_else(|_| std::env::var("LOGNAME")).unwrap_or_else(|_| "unknown".to_owned());
    base.join(format!("platynui-{user}")).join("agents")
}

/// The handshake file a JVM with process id `pid` publishes.
#[must_use]
pub fn handshake_file(pid: u32) -> PathBuf {
    handshake_file_in(&handshake_dir(), pid)
}

/// The handshake file for `pid` inside a directory the caller names.
#[must_use]
pub fn handshake_file_in(directory: &std::path::Path, pid: u32) -> PathBuf {
    directory.join(format!("agent-{pid}"))
}

/// The process id encoded in a handshake file name, if it is one.
#[must_use]
pub fn pid_from_file_name(file_name: &str) -> Option<u32> {
    file_name.strip_prefix("agent-").and_then(|pid| pid.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_file_names_encode_the_pid() {
        assert_eq!(pid_from_file_name("agent-4711"), Some(4711));
        assert_eq!(pid_from_file_name("agent-0"), Some(0));
    }

    #[test]
    fn non_handshake_file_names_are_rejected() {
        // The agent writes its temp files into the same directory; a scan must
        // not mistake one for a published agent.
        assert_eq!(pid_from_file_name("agent-4711.tmp"), None);
        assert_eq!(pid_from_file_name("agent-"), None);
        assert_eq!(pid_from_file_name("agent-abc"), None);
        assert_eq!(pid_from_file_name("readme.txt"), None);
        assert_eq!(pid_from_file_name(""), None);
    }

    #[test]
    fn the_file_lives_in_the_directory() {
        let file = handshake_file(1234);
        assert_eq!(file.parent(), Some(handshake_dir().as_path()));
        assert_eq!(file.file_name().and_then(|name| name.to_str()), Some("agent-1234"));
    }
}
