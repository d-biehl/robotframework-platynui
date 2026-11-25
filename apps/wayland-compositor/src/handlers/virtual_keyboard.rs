//! Virtual keyboard protocol handler (`zwp-virtual-keyboard-v1`).
//!
//! Smithay delivers a virtual keyboard as a regular input device: its keys,
//! modifier changes and keymap uploads arrive here as
//! `InputEvent<VirtualKeyboardBackend>`. The compositor owns what smithay
//! leaves to it:
//!
//! - **Keymap.** A virtual keyboard's key codes mean something only under the
//!   keymap it uploaded, so that keymap is made active before its keys and
//!   modifiers; input from every other source switches back to the
//!   compositor's keymap ([`crate::input::activate_keymap`]).
//! - **Held input.** Keys go through the device's own keyboard source, so
//!   removing the device releases exactly the keys it still holds.
//! - **Modifiers.** The masks a device set with its `modifiers` request are
//!   tracked and cleared from the seat when the device goes away; masks other
//!   devices set stay.
//!
//! The global is registered in `state.rs` with the restrictive-policy filter.

use smithay::{
    backend::input::{Event, InputEvent, KeyboardKeyEvent},
    input::keyboard::FilterResult,
    utils::SERIAL_COUNTER,
    wayland::virtual_keyboard::{
        VirtualKeyboardBackend, VirtualKeyboardDevice, VirtualKeyboardHandler, VirtualKeyboardSpecialEvent,
    },
};

use crate::state::State;

/// Raw xkb modifier masks a virtual keyboard set: depressed, latched, locked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Masks {
    pub depressed: u32,
    pub latched: u32,
    pub locked: u32,
}

impl VirtualKeyboardHandler for State {
    fn process_virtual_keyboard_event(&mut self, event: InputEvent<VirtualKeyboardBackend>) {
        match event {
            InputEvent::Keyboard { event } => {
                let device = event.device();
                activate_device_keymap(self, &device);
                let keyboard = self.keyboard();
                let serial = SERIAL_COUNTER.next_serial();
                keyboard.input_from_source::<(), _>(
                    device.source(),
                    self,
                    event.key_code(),
                    event.state(),
                    serial,
                    Event::time(&event),
                    |_, _, _| FilterResult::Forward,
                );
            }
            InputEvent::Special(VirtualKeyboardSpecialEvent::Modifiers {
                device,
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
            }) => {
                // The masks are relative to the device's keymap.
                activate_device_keymap(self, &device);
                let keyboard = self.keyboard();
                keyboard.with_xkb_state(self, |mut context| {
                    context.set_modifier_mask(mods_depressed, mods_latched, mods_locked, group);
                });
                let masks = Masks { depressed: mods_depressed, latched: mods_latched, locked: mods_locked };
                if masks == Masks::default() {
                    self.virtual_keyboard_mods.remove(&device);
                } else {
                    self.virtual_keyboard_mods.insert(device, masks);
                }
            }
            InputEvent::DeviceRemoved { device } => {
                tracing::debug!("virtual keyboard removed — releasing what it holds");
                let keyboard = self.keyboard();
                keyboard.release_source(self, device.source());
                if let Some(set) = self.virtual_keyboard_mods.remove(&device) {
                    clear_masks(self, set);
                }
            }
            // A new or replaced keymap takes effect with the device's next key.
            _ => {}
        }
    }
}

fn activate_device_keymap(state: &mut State, device: &VirtualKeyboardDevice) {
    match device.keymap() {
        Some(keymap) => crate::input::activate_keymap(state, &keymap),
        None => crate::input::activate_compositor_keymap(state),
    }
}

/// Remove `set` from the seat's modifier state, keeping the bits other
/// devices still hold.
fn clear_masks(state: &mut State, set: Masks) {
    let still_set = state.virtual_keyboard_mods.values().fold(Masks::default(), |acc, m| Masks {
        depressed: acc.depressed | m.depressed,
        latched: acc.latched | m.latched,
        locked: acc.locked | m.locked,
    });
    let keyboard = state.keyboard();
    let current = keyboard.modifier_state().serialized;
    let keep = |current: u32, set: u32, others: u32| current & !(set & !others);
    keyboard.with_xkb_state(state, |mut context| {
        context.set_modifier_mask(
            keep(current.depressed, set.depressed, still_set.depressed),
            keep(current.latched, set.latched, still_set.latched),
            keep(current.locked, set.locked, still_set.locked),
            current.layout_effective,
        );
    });
}
