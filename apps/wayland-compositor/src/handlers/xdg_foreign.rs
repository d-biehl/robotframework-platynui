//! `xdg-foreign-v2` handler — cross-application window relationships.
//!
//! Allows applications to establish parent/child relationships between
//! windows belonging to different clients (e.g., portal dialogs as children
//! of the requesting application's window).

use smithay::wayland::xdg_foreign::{XdgForeignHandler, XdgForeignState};

use crate::state::State;

impl XdgForeignHandler for State {
    fn xdg_foreign_state(&mut self) -> &mut XdgForeignState {
        &mut self.xdg_foreign_state
    }
}
