//! `keyboard-shortcuts-inhibit-v1` handler.
//!
//! Allows clients (e.g., VNC/RDP viewers, remote desktop apps) to request
//! that compositor keyboard shortcuts be disabled, so all key events are
//! forwarded to the client.

use smithay::wayland::keyboard_shortcuts_inhibit::{
    KeyboardShortcutsInhibitHandler, KeyboardShortcutsInhibitState, KeyboardShortcutsInhibitor,
};

use crate::state::State;

impl KeyboardShortcutsInhibitHandler for State {
    fn keyboard_shortcuts_inhibit_state(&mut self) -> &mut KeyboardShortcutsInhibitState {
        &mut self.keyboard_shortcuts_inhibit_state
    }

    fn new_inhibitor(&mut self, _inhibitor: KeyboardShortcutsInhibitor) {
        // Accept all inhibitors in the test compositor
    }

    fn inhibitor_destroyed(&mut self, _inhibitor: KeyboardShortcutsInhibitor) {
        // Nothing to clean up
    }
}
