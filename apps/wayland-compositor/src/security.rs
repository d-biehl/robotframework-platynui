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
use smithay::wayland::compositor;
use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;

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
    /// Uses the client's PID (from credentials) to look up the process name
    /// in `/proc/{pid}/comm` and checks it against the policy whitelist.
    /// If the policy is `AllowAll`, returns `true` immediately.
    /// Under a restrictive policy, denies access by default (fail-closed).
    #[must_use]
    pub fn is_client_allowed(
        &self,
        client: &smithay::reexports::wayland_server::Client,
        display_handle: &smithay::reexports::wayland_server::DisplayHandle,
    ) -> bool {
        if !self.is_restrictive() {
            return true;
        }

        // Under a restrictive policy, we deny by default.
        // Look up the client's PID → process name as a best-effort app_id check.
        if let Ok(creds) = client.get_credentials(display_handle)
            && let Ok(cmdline) = std::fs::read_to_string(format!("/proc/{}/comm", creds.pid))
        {
            let process_name = cmdline.trim();
            if self.is_allowed(process_name) {
                return true;
            }
        }

        tracing::debug!("client denied by security policy (no matching app_id)");
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
            if let Some(app_id) = get_window_app_id(window.as_ref())
                && self.is_allowed(&app_id)
            {
                return true;
            }
        }

        false
    }
}

/// Extract the `app_id` from a window's toplevel surface data.
fn get_window_app_id(window: &Window) -> Option<String> {
    window.toplevel().and_then(|t| {
        compositor::with_states(t.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .and_then(|data| data.lock().ok())
                .and_then(|data| data.app_id.clone())
        })
    })
}
