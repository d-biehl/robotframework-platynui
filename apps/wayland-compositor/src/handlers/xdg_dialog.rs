//! `xdg-dialog-v1` handler — modal dialog support.
//!
//! Clients signal that a toplevel surface is modal to another, allowing
//! the compositor to enforce correct stacking.  GTK4 and Qt use this
//! protocol increasingly.
//!
//! **Modal enforcement:** When a toplevel becomes modal, it is raised above
//! its parent.  Attempting to focus/raise the parent while a modal child
//! exists redirects focus to the modal child instead (see
//! [`focus_and_raise`](crate::input::focus_and_raise) and
//! [`State::find_modal_child`](crate::state::State::find_modal_child)).

use smithay::reexports::wayland_server::Resource;
use smithay::wayland::shell::xdg::{ToplevelSurface, dialog::XdgDialogHandler};

use crate::state::State;

impl XdgDialogHandler for State {
    fn modal_changed(&mut self, toplevel: ToplevelSurface, is_modal: bool) {
        tracing::debug!(is_modal, "xdg dialog modal state changed");

        if is_modal {
            // Raise the modal dialog above its parent so it's always visible.
            // Clone the matching window first to end the immutable borrow on space.
            let window = self
                .space
                .elements()
                .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == toplevel.wl_surface()))
                .cloned();

            if let Some(window) = window {
                self.space.raise_element(&window, true);

                // Also focus the modal dialog.
                let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                let keyboard = self.keyboard();
                keyboard.set_focus(self, Some(crate::focus::KeyboardFocusTarget::Window(window)), serial);
            } else {
                tracing::warn!(
                    surface = ?toplevel.wl_surface().id(),
                    "modal dialog surface not found in space",
                );
            }
        }
    }
}
