//! Virtual pointer protocol handler (`zwlr-virtual-pointer-v1`).
//!
//! Lets external tools (e.g. `wayvnc`, `wlrctl`) move the pointer, click and
//! scroll. Smithay exposes each virtual pointer as a regular input device; its
//! events go through the compositor's generic input path
//! ([`crate::input::process_input_event`]), so a virtual click focuses and
//! raises windows, hits server-side decorations and reaches X11 clients
//! exactly like a physical one.
//!
//! What this module adds on top:
//!
//! - **Absolute motion** of a pointer created for an output maps onto that
//!   output's geometry; otherwise onto the union of all outputs.
//! - **Held buttons** are tracked per device and released when the device goes
//!   away, so a client that disconnects mid-drag leaves no button pressed.
//!
//! The global is hidden under `--restrict-protocols` (filter passed to
//! [`init_virtual_pointer_manager`]).

use std::collections::HashSet;

use smithay::{
    backend::input::{AbsolutePositionEvent, ButtonState, Event, InputEvent, InputTime, PointerButtonEvent},
    reexports::wayland_server::{Client, DisplayHandle},
    utils::{Logical, Point},
    wayland::virtual_pointer::{VirtualPointerBackend, VirtualPointerHandler, VirtualPointerManagerState},
};

use crate::state::State;

/// Register the `zwlr_virtual_pointer_manager_v1` global for the clients
/// `filter` accepts.
pub fn init_virtual_pointer_manager(
    dh: &DisplayHandle,
    filter: impl Fn(&Client) -> bool + Send + Sync + 'static,
) -> VirtualPointerManagerState {
    VirtualPointerManagerState::new::<State, _>(dh, filter)
}

impl VirtualPointerHandler for State {
    fn process_virtual_pointer_event(&mut self, event: InputEvent<VirtualPointerBackend>) {
        match event {
            InputEvent::PointerMotionAbsolute { event } => {
                let geo = event
                    .device()
                    .output()
                    .and_then(|output| self.space.output_geometry(&output))
                    .unwrap_or_else(|| self.combined_output_geometry());
                let location: Point<f64, Logical> = (
                    f64::from(geo.loc.x) + event.x_transformed(geo.size.w),
                    f64::from(geo.loc.y) + event.y_transformed(geo.size.h),
                )
                    .into();
                crate::input::move_pointer_to(self, location, event.time());
            }
            InputEvent::PointerButton { event } => {
                let held = self.virtual_pointer_buttons.entry(event.device()).or_default();
                match event.state() {
                    ButtonState::Pressed => held.insert(event.button_code()),
                    ButtonState::Released => held.remove(&event.button_code()),
                };
                crate::input::process_input_event::<VirtualPointerBackend>(self, InputEvent::PointerButton { event });
            }
            InputEvent::DeviceRemoved { device } => {
                let held: HashSet<u32> = self.virtual_pointer_buttons.remove(&device).unwrap_or_default();
                if !held.is_empty() {
                    tracing::debug!(count = held.len(), "virtual pointer removed — releasing its buttons");
                }
                for button in held {
                    crate::input::process_pointer_button(self, button, ButtonState::Released, InputTime::now());
                }
            }
            other => crate::input::process_input_event(self, other),
        }
    }
}
