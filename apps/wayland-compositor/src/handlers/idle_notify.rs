//! `ext-idle-notify-v1` handler — idle detection.
//!
//! Allows clients to be notified when the user has been idle for a
//! configurable duration. Used for screensaver activation and power management.

use smithay::wayland::idle_notify::{IdleNotifierHandler, IdleNotifierState};

use crate::state::State;

impl IdleNotifierHandler for State {
    fn idle_notifier_state(&mut self) -> &mut IdleNotifierState<Self> {
        &mut self.idle_notify_state
    }
}
