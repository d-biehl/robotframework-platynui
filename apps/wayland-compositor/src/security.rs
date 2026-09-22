//! Client permissions — configurable access control for privileged protocols.
//!
//! By default (test compositor), all clients are allowed to use all protocols.
//! When `--restrict-protocols` is passed, a whitelist-based filter is active:
//! only clients whose `app_id` or PID matches an entry are granted access to
//! privileged protocols (e.g. `zwlr_virtual_pointer`, `wlr-foreign-toplevel`,
//! `ext-image-copy-capture`, layer-shell).
//!
//! This is relevant for testing Flatpak/sandbox scenarios where protocol
//! access is restricted.
//!
//! ## Enforcement Points
//!
//! The policy is checked in:
//! - `SecurityContextHandler::context_created` — sandboxed client connections
//! - `SessionLockHandler::lock` — screen lock acquisition
//! - `InputMethodManagerState` global filter — input method access
//! - Any future privileged protocol handlers (layer-shell, virtual-pointer, etc.)

use std::collections::HashSet;

use smithay::desktop::Window;

use crate::handlers::foreign_toplevel;

/// Security policy for privileged protocol access.
#[derive(Debug)]
pub enum SecurityPolicy {
    /// All clients are allowed (default for a test compositor).
    AllowAll,
    /// Only whitelisted app IDs are allowed to use privileged protocols.
    Whitelist(WhitelistPolicy),
}

/// Whitelist-based security policy.
#[derive(Debug, Default)]
pub struct WhitelistPolicy {
    /// Allowed app IDs (matched against `xdg_toplevel.set_app_id`).
    pub allowed_app_ids: HashSet<String>,
}

impl SecurityPolicy {
    /// Create a permissive policy (all clients allowed).
    #[must_use]
    pub fn allow_all() -> Self {
        Self::AllowAll
    }

    /// Create a whitelist policy from a comma-separated list of app IDs.
    #[must_use]
    pub fn from_whitelist(app_ids: &str) -> Self {
        let allowed = app_ids.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();

        Self::Whitelist(WhitelistPolicy { allowed_app_ids: allowed })
    }

    /// Check whether an app ID is allowed to use privileged protocols.
    #[must_use]
    pub fn is_allowed(&self, app_id: &str) -> bool {
        match self {
            Self::AllowAll => true,
            Self::Whitelist(policy) => policy.allowed_app_ids.contains(app_id),
        }
    }

    /// Whether this is a restrictive policy (whitelist active).
    #[must_use]
    pub fn is_restrictive(&self) -> bool {
        matches!(self, Self::Whitelist(_))
    }

    /// Create a policy from the CLI `--restrict-protocols` argument.
    ///
    /// `None` → allow all; `Some(list)` → whitelist of app IDs.
    #[must_use]
    pub fn from_args(restrict: Option<&str>) -> Self {
        match restrict {
            Some(list) => Self::from_whitelist(list),
            None => Self::allow_all(),
        }
    }

    /// Check whether a Wayland client is allowed to use privileged protocols.
    ///
    /// Reads the identity the compositor established when it accepted the
    /// client's connection ([`crate::client::ClientState`]), so a client whose
    /// process cannot be identified reaches a decision instead of ending the
    /// session.
    #[must_use]
    pub fn is_client_allowed(&self, client: &smithay::reexports::wayland_server::Client) -> bool {
        self.allows_process(client.get_data::<crate::client::ClientState>().and_then(|data| data.peer_process))
    }

    /// Whether a client belonging to `pid` may use privileged protocols.
    ///
    /// Looks the process name up in `/proc/{pid}/comm` and checks it against the
    /// whitelist. A permissive policy allows everyone; a restrictive one denies
    /// by default (fail-closed), and therefore denies a client whose process
    /// could not be identified — there is no name to check.
    ///
    /// Split out from [`Self::is_client_allowed`] so the decision is testable
    /// without a Wayland client.
    #[must_use]
    pub fn allows_process(&self, pid: Option<u32>) -> bool {
        if !self.is_restrictive() {
            return true;
        }

        if let Some(pid) = pid
            && let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm"))
            && self.is_allowed(comm.trim())
        {
            return true;
        }

        tracing::debug!(?pid, "client denied by security policy (no matching app_id)");
        false
    }

    /// Check whether any window in the space belongs to a client with an allowed `app_id`.
    ///
    /// This is a fallback method for when we can't directly get credentials
    /// from the client object.
    #[must_use]
    pub fn has_allowed_window(&self, windows: impl Iterator<Item = impl AsRef<Window>>) -> bool {
        if !self.is_restrictive() {
            return true;
        }

        for window in windows {
            let app_id = foreign_toplevel::window_app_id(window.as_ref());
            if !app_id.is_empty() && self.is_allowed(&app_id) {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Under a restrictive policy there is no process name to check, so the
    /// fail-closed default applies — and it is reached without the compositor
    /// having to ask an accessor that cannot express "unidentifiable".
    #[test]
    fn a_restrictive_policy_denies_a_client_it_cannot_identify() {
        assert!(!SecurityPolicy::from_whitelist("allowed-app").allows_process(None));
    }

    /// A permissive policy judges nobody, identified or not.
    #[test]
    fn a_permissive_policy_allows_a_client_it_cannot_identify() {
        assert!(SecurityPolicy::allow_all().allows_process(None));
    }

    /// The identified branch, so the restrictive case is known to deny by name
    /// rather than to deny everything: this process is allowed exactly when the
    /// whitelist carries the name behind its own process id.
    #[test]
    fn an_identified_client_is_judged_by_the_name_behind_its_process_id() {
        let own = std::process::id();
        let comm = std::fs::read_to_string(format!("/proc/{own}/comm")).expect("Linux /proc is available in tests");

        assert!(SecurityPolicy::from_whitelist(comm.trim()).allows_process(Some(own)));
        assert!(!SecurityPolicy::from_whitelist("not-the-name-of-this-process").allows_process(Some(own)));
    }
}
