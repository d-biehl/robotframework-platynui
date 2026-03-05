//! `wl_seat` handler — keyboard, pointer, touch.

use smithay::input::{Seat, SeatHandler, SeatState, pointer::CursorImageStatus};
use smithay::utils::IsAlive;

use crate::{
    focus::{KeyboardFocusTarget, PointerFocusTarget},
    state::State,
};

impl SeatHandler for State {
    type KeyboardFocus = KeyboardFocusTarget;
    type PointerFocus = PointerFocusTarget;
    type TouchFocus = PointerFocusTarget;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, focused: Option<&Self::KeyboardFocus>) {
        tracing::debug!(?focused, "keyboard focus changed");

        // Notify foreign-toplevel clients about activated state changes so
        // that taskbars (ironbar) can correctly distinguish between focused
        // and background windows and choose the right action (activate vs.
        // minimize) on click.
        //
        // For X11 surfaces we also call `set_activated` because they don't
        // receive XDG configure events.

        // Deactivate the previously focused window (if it's a Window).
        if let Some(prev) = self.last_focused_window.take()
            && prev.alive()
        {
            if let Some(x11) = prev.x11_surface()
                && let Err(err) = x11.set_activated(false)
            {
                tracing::warn!(%err, "failed to deactivate X11 window");
            }
            // Use explicit activated=false because XDG current_state() is
            // stale until the client acks the configure.
            crate::handlers::foreign_toplevel::send_foreign_toplevel_state_activated(self, &prev, false);
        }

        // Activate the newly focused window (if it's a Window).
        if let Some(KeyboardFocusTarget::Window(window)) = focused {
            if let Some(x11) = window.x11_surface()
                && let Err(err) = x11.set_activated(true)
            {
                tracing::warn!(%err, "failed to activate X11 window");
            }
            // Use explicit activated=true for the same reason.
            crate::handlers::foreign_toplevel::send_foreign_toplevel_state_activated(self, window, true);
            self.last_focused_window = Some(window.clone());
        }
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.cursor_status = image;
    }
}
