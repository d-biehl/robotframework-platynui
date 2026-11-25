//! EIS (Emulated Input Server) — accepts libei sender clients for input
//! emulation.
//!
//! Listens on `$XDG_RUNTIME_DIR/eis-platynui` (exported as `LIBEI_SOCKET`) and
//! serves every connecting client through smithay's libei backend, each with
//! its own devices; a new client never displaces another. Receiver contexts
//! are refused by smithay.
//!
//! Each client gets a seat `default` with a keyboard (the compositor's
//! configured keymap), a relative pointer, an absolute pointer and touch (one
//! region per output, in global logical coordinates), plus buttons and scroll.
//! No text device is created. EI events take the compositor's generic input
//! path; absolute pointer and touch positions are used as the global
//! positions they are.
//!
//! Keys go through a keyboard source of their own per client, and the buttons
//! a client holds are tracked, so a client that disconnects without releasing
//! leaves nothing pressed — keys and buttons held by other devices stay.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use calloop::LoopHandle;
use reis::calloop::EisListenerSource;
use reis::eis;
use smithay::{
    backend::{
        input::{
            AbsolutePositionEvent, ButtonState, Event, InputEvent, KeyboardKeyEvent, PointerButtonEvent, TouchEvent,
        },
        libei::{EiInput, EiInputEvent, EiInputSeat, EiRegion},
    },
    input::keyboard::{FilterResult, KeyboardSource},
    utils::{Logical, Point, SERIAL_COUNTER},
};

use crate::input;
use crate::state::State;

/// What the compositor keeps per connected EI client.
#[derive(Debug)]
pub struct EisClient {
    seat: Option<EiInputSeat>,
    keyboard_source: KeyboardSource,
    buttons: HashSet<u32>,
}

/// The connected EI clients, by connection number.
pub type EisClients = HashMap<u64, EisClient>;

/// Create the EIS listener and register it with the event loop.
///
/// Returns the socket path on success (for setting `LIBEI_SOCKET`).
pub fn setup_eis_server(handle: &LoopHandle<'static, State>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = eis_socket_path("eis-platynui");

    // Remove a stale socket if one exists.
    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    let listener = eis::Listener::bind(&path)?;
    tracing::info!(path = %path.display(), "EIS server listening");

    let loop_handle = handle.clone();
    let mut next_id: u64 = 0;
    handle.insert_source(EisListenerSource::new(listener), move |context, (), state| {
        next_id += 1;
        let id = next_id;
        state.eis_clients.insert(
            id,
            EisClient { seat: None, keyboard_source: KeyboardSource::new_auxiliary(), buttons: HashSet::new() },
        );
        let inserted = loop_handle.insert_source(EiInput::new(context), move |event, connection, state| {
            handle_event(state, id, event, connection);
        });
        if let Err(err) = inserted {
            tracing::warn!(%err, "failed to register an EIS client");
            state.eis_clients.remove(&id);
        } else {
            tracing::info!(client = id, "EIS client connected (handshake in progress)");
        }
        Ok(calloop::PostAction::Continue)
    })?;

    Ok(path)
}

/// Compute the EIS socket path in `$XDG_RUNTIME_DIR`.
fn eis_socket_path(name: &str) -> PathBuf {
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    runtime_dir.join(name)
}

/// One region per output, in global logical coordinates.
fn output_regions(state: &State) -> Vec<EiRegion> {
    state
        .outputs
        .iter()
        .filter_map(|output| {
            let rect = state.space.output_geometry(output)?;
            #[allow(clippy::cast_possible_truncation)]
            let scale = output.current_scale().fractional_scale() as f32;
            Some(EiRegion { rect, scale, mapping_id: Some(output.name()) })
        })
        .collect()
}

/// Send the current output regions to every connected client.
pub fn outputs_changed(state: &State) {
    let regions = output_regions(state);
    for client in state.eis_clients.values() {
        if let Some(seat) = &client.seat {
            seat.update_regions(&regions);
        }
    }
}

fn handle_event(
    state: &mut State,
    id: u64,
    event: EiInputEvent,
    connection: &mut smithay::backend::libei::EiInputConnection,
) {
    match event {
        EiInputEvent::Connected => {
            tracing::info!(
                client = id,
                name = connection.name().unwrap_or("<unknown>"),
                "EIS client handshake complete"
            );
            let seat = connection.add_seat("default");
            if let Err(err) = seat.add_keyboard("keyboard", state.xkb_names.config()) {
                tracing::warn!(?err, "EIS: cannot build the keyboard keymap");
            }
            seat.add_pointer("pointer");
            let regions = output_regions(state);
            seat.add_pointer_absolute("absolute pointer", &regions);
            seat.add_touch("touch", &regions);
            if let Some(client) = state.eis_clients.get_mut(&id) {
                client.seat = Some(seat);
            }
        }
        EiInputEvent::Disconnected => disconnected(state, id),
        EiInputEvent::Event(event) => handle_input(state, id, event),
    }
}

fn handle_input(state: &mut State, id: u64, event: InputEvent<EiInput>) {
    match event {
        InputEvent::Keyboard { event } => {
            let Some(source) = state.eis_clients.get(&id).map(|c| c.keyboard_source) else { return };
            input::activate_compositor_keymap(state);
            let keyboard = state.keyboard();
            let serial = SERIAL_COUNTER.next_serial();
            keyboard.input_from_source::<(), _>(
                source,
                state,
                event.key_code(),
                event.state(),
                serial,
                Event::time(&event),
                |_, _, _| FilterResult::Forward,
            );
        }
        InputEvent::PointerMotionAbsolute { event } => {
            // EI absolute positions are global logical coordinates.
            let location = state.clamp_to_outputs(Point::<f64, Logical>::from((event.x(), event.y())));
            input::move_pointer_to(state, location, event.time());
        }
        InputEvent::PointerButton { event } => {
            if let Some(client) = state.eis_clients.get_mut(&id) {
                match event.state() {
                    ButtonState::Pressed => client.buttons.insert(event.button_code()),
                    ButtonState::Released => client.buttons.remove(&event.button_code()),
                };
            }
            input::process_pointer_button(state, event.button_code(), event.state(), event.time());
        }
        InputEvent::TouchDown { event } => {
            input::process_touch_down(state, (event.x(), event.y()).into(), event.slot(), event.time());
        }
        InputEvent::TouchMotion { event } => {
            input::process_touch_motion(state, (event.x(), event.y()).into(), event.slot(), event.time());
        }
        InputEvent::TouchUp { event } => input::process_touch_up(state, event.slot(), event.time()),
        // No text device is created, so no text events arrive.
        InputEvent::Special(_) | InputEvent::DeviceAdded { .. } | InputEvent::DeviceRemoved { .. } => {}
        other => input::process_input_event(state, other),
    }
}

/// Release what a departing client still holds and forget it.
fn disconnected(state: &mut State, id: u64) {
    let Some(client) = state.eis_clients.remove(&id) else { return };
    tracing::info!(client = id, "EIS client disconnected");
    let keyboard = state.keyboard();
    keyboard.release_source(state, client.keyboard_source);
    for button in client.buttons {
        input::process_pointer_button(state, button, ButtonState::Released, smithay::backend::input::InputTime::now());
    }
}
