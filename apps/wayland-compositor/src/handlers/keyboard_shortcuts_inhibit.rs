//! `keyboard-shortcuts-inhibit-v1` handler.
//!
//! Allows clients (e.g., VNC/RDP viewers, remote desktop apps) to request
//! that compositor keyboard shortcuts be disabled, so all key events are
//! forwarded to the client.

use smithay::{
    reexports::wayland_server::Resource,
    wayland::keyboard_shortcuts_inhibit::{
        KeyboardShortcutsInhibitHandler, KeyboardShortcutsInhibitState, KeyboardShortcutsInhibitor,
    },
};

use crate::state::State;

impl KeyboardShortcutsInhibitHandler for State {
    fn keyboard_shortcuts_inhibit_state(&mut self) -> &mut KeyboardShortcutsInhibitState {
        &mut self.keyboard_shortcuts_inhibit_state
    }

    fn new_inhibitor(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        // Accept and activate all inhibitors in the test compositor so the
        // client receives the `active` event and knows shortcuts are forwarded.
        tracing::debug!(
            surface = ?inhibitor.wl_surface().id(),
            "activating keyboard shortcuts inhibitor",
        );
        inhibitor.activate();
    }

    fn inhibitor_destroyed(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        tracing::debug!(
            surface = ?inhibitor.wl_surface().id(),
            "keyboard shortcuts inhibitor destroyed",
        );
    }
}
