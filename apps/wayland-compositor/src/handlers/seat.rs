//! `wl_seat` handler — keyboard, pointer, touch.

use smithay::input::{Seat, SeatHandler, SeatState, pointer::CursorImageStatus};

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

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&Self::KeyboardFocus>) {
        // Focus change tracking — nothing special needed for the test compositor
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.cursor_status = image;
    }
}
