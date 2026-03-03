//! `ext-session-lock-v1` handler — screen locking.
//!
//! Allows lock screen applications (e.g., swaylock) to acquire a session lock,
//! covering all outputs while the session is locked.
//!
//! When `--restrict-protocols` is active, the lock request is rejected unless
//! the compositor is in permissive mode.

use smithay::{
    reexports::wayland_server::protocol::wl_output::WlOutput,
    wayland::session_lock::{LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker},
};

use crate::state::State;

impl SessionLockHandler for State {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.session_lock_state
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        // Under a restrictive policy, reject lock requests from unknown clients.
        // Screen locking is a privileged operation.
        if self.security_policy.is_restrictive() {
            tracing::warn!("session lock rejected: restrictive security policy active");
            // Don't confirm — the client will see the lock fail.
            return;
        }

        // Accept the lock immediately in the test compositor
        confirmation.lock();
        tracing::info!("session locked");
    }

    fn unlock(&mut self) {
        tracing::info!("session unlocked");
    }

    fn new_surface(&mut self, _surface: LockSurface, _output: WlOutput) {
        // No-op: per-output lock surface tracking not yet implemented.
    }
}
