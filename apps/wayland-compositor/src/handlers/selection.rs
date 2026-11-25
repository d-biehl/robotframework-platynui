//! Data device (clipboard, drag and drop) + primary selection handlers.
//!
//! Drag and drop runs through smithay's generic `DnDGrab`: a Wayland client's
//! `start_drag` arrives as [`WaylandDndGrabHandler::dnd_requested`], which
//! installs the grab on the pointer or touch that started it. Drop targets are
//! the pointer focus targets ([`PointerFocusTarget`](crate::focus::PointerFocusTarget)
//! implements `DndFocus`), so drags also cross between Wayland and X11 clients.

use smithay::{
    input::{
        Seat,
        dnd::{DnDGrab, DndGrabHandler, GrabType, Source},
        pointer::Focus,
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::Serial,
    wayland::selection::{
        SelectionHandler,
        data_device::{DataDeviceHandler, DataDeviceState, WaylandDndGrabHandler},
        primary_selection::{PrimarySelectionHandler, PrimarySelectionState},
    },
};

use crate::state::State;

impl SelectionHandler for State {
    type SelectionUserData = ();
}

impl DataDeviceHandler for State {
    fn data_device_state(&mut self) -> &mut DataDeviceState {
        &mut self.data_device_state
    }
}

impl DndGrabHandler for State {}

impl WaylandDndGrabHandler for State {
    fn dnd_requested<S: Source>(
        &mut self,
        source: S,
        _icon: Option<WlSurface>,
        seat: Seat<Self>,
        serial: Serial,
        type_: GrabType,
    ) {
        let dh = self.display_handle.clone();
        match type_ {
            GrabType::Pointer => {
                let Some(pointer) = seat.get_pointer() else { return source.cancel() };
                let Some(start_data) = pointer.grab_start_data() else { return source.cancel() };
                pointer.set_grab(self, DnDGrab::new_pointer(&dh, start_data, source, seat), serial, Focus::Keep);
            }
            GrabType::Touch => {
                let Some(touch) = seat.get_touch() else { return source.cancel() };
                let Some(start_data) = touch.grab_start_data() else { return source.cancel() };
                touch.set_grab(self, DnDGrab::new_touch(&dh, start_data, source, seat), serial);
            }
        }
    }
}

impl PrimarySelectionHandler for State {
    fn primary_selection_state(&mut self) -> &mut PrimarySelectionState {
        &mut self.primary_selection_state
    }
}
