//! Per-client state for Wayland clients.

use smithay::wayland::compositor::CompositorClientState;

/// Per-client data stored in each `wayland_server::Client`.
#[derive(Default, Debug)]
pub struct ClientState {
    /// Compositor-specific per-client state.
    pub compositor_state: CompositorClientState,
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
