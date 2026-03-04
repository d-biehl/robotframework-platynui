//! Virtual pointer protocol handler (`zwlr-virtual-pointer-unstable-v1`).
//!
//! Allows external tools to inject pointer events into the compositor's
//! input stack.  Unlike virtual keyboards, smithay has no built-in support
//! for this protocol, so we implement [`GlobalDispatch`] and [`Dispatch`]
//! manually.
//!
//! The manager global (`zwlr_virtual_pointer_manager_v1`) creates per-client
//! virtual pointer objects (`zwlr_virtual_pointer_v1`).  Each request on the
//! pointer object maps to the corresponding smithay `PointerHandle` method
//! (motion, button, axis, frame).

use std::sync::Mutex;

use smithay::{
    backend::input::{Axis, AxisSource},
    input::pointer::{AxisFrame, MotionEvent},
    output::Output,
    reexports::{
        wayland_protocols_wlr::virtual_pointer::v1::server::{
            zwlr_virtual_pointer_manager_v1::{self, ZwlrVirtualPointerManagerV1},
            zwlr_virtual_pointer_v1::{self, ZwlrVirtualPointerV1},
        },
        wayland_server::{
            Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, WEnum,
            backend::{ClientId, GlobalId},
            protocol::wl_pointer,
        },
    },
    utils::SERIAL_COUNTER,
};

use crate::state::State;

// ---------------------------------------------------------------------------
// Manager global
// ---------------------------------------------------------------------------

/// Data attached to the `zwlr_virtual_pointer_manager_v1` global.
pub struct VirtualPointerManagerGlobalData {
    filter: Box<dyn Fn(&Client) -> bool + Send + Sync>,
}

impl GlobalDispatch<ZwlrVirtualPointerManagerV1, VirtualPointerManagerGlobalData> for State {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrVirtualPointerManagerV1>,
        _global_data: &VirtualPointerManagerGlobalData,
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }

    fn can_view(client: Client, global_data: &VirtualPointerManagerGlobalData) -> bool {
        (global_data.filter)(&client)
    }
}

// ---------------------------------------------------------------------------
// Manager dispatch
// ---------------------------------------------------------------------------

impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &ZwlrVirtualPointerManagerV1,
        request: zwlr_virtual_pointer_manager_v1::Request,
        _data: &(),
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwlr_virtual_pointer_manager_v1::Request::CreateVirtualPointer { seat: _, id } => {
                data_init.init(id, VirtualPointerUserData::default());
            }
            zwlr_virtual_pointer_manager_v1::Request::CreateVirtualPointerWithOutput { seat: _, output, id } => {
                // Resolve the wl_output to our internal Output so that
                // motion_absolute maps coordinates to that specific output
                // rather than the full combined geometry.  This is critical
                // for tools like wayvnc that capture a single output.
                let bound_output = output.as_ref().and_then(Output::from_resource);
                if bound_output.is_some() {
                    tracing::debug!(?bound_output, "virtual pointer bound to output");
                }
                data_init.init(id, VirtualPointerUserData::new(bound_output));
            }
            zwlr_virtual_pointer_manager_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("unhandled virtual pointer manager request");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Per-pointer user data — accumulates axis state between frames
// ---------------------------------------------------------------------------

/// Per-pointer user data — stores the (optional) bound output and
/// accumulates axis state between frames.
pub struct VirtualPointerUserData {
    /// The output this virtual pointer is bound to (set via
    /// `CreateVirtualPointerWithOutput`).  When present,
    /// `motion_absolute` maps coordinates to this output's geometry
    /// instead of the full combined geometry.
    bound_output: Option<Output>,
    /// Pending axis frame (built up by `axis`, `axis_source`, `axis_stop`,
    /// `axis_discrete` and flushed on `frame`).
    pending_axis: Mutex<Option<PendingAxisFrame>>,
}

impl VirtualPointerUserData {
    fn new(bound_output: Option<Output>) -> Self {
        Self { bound_output, pending_axis: Mutex::new(None) }
    }
}

impl Default for VirtualPointerUserData {
    fn default() -> Self {
        Self::new(None)
    }
}

/// Intermediate axis-frame builder before the `frame` request flushes it.
#[derive(Default)]
struct PendingAxisFrame {
    time: u32,
    source: Option<AxisSource>,
    horizontal: Option<f64>,
    vertical: Option<f64>,
    horizontal_discrete: Option<i32>,
    vertical_discrete: Option<i32>,
    stop_horizontal: bool,
    stop_vertical: bool,
}

// ---------------------------------------------------------------------------
// Virtual pointer dispatch
// ---------------------------------------------------------------------------

impl Dispatch<ZwlrVirtualPointerV1, VirtualPointerUserData> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &ZwlrVirtualPointerV1,
        request: zwlr_virtual_pointer_v1::Request,
        data: &VirtualPointerUserData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwlr_virtual_pointer_v1::Request::Motion { time, dx, dy } => {
                handle_motion(state, time, dx, dy);
            }
            zwlr_virtual_pointer_v1::Request::MotionAbsolute { time, x, y, x_extent, y_extent } => {
                handle_motion_absolute(state, data, time, x, y, x_extent, y_extent);
            }
            zwlr_virtual_pointer_v1::Request::Button { time, button, state: btn_state } => {
                handle_button(state, time, button, btn_state);
            }
            zwlr_virtual_pointer_v1::Request::Axis { time, axis, value } => {
                handle_axis(data, time, axis, value);
            }
            zwlr_virtual_pointer_v1::Request::Frame => {
                handle_frame(state, data);
            }
            zwlr_virtual_pointer_v1::Request::AxisSource { axis_source } => {
                handle_axis_source(data, axis_source);
            }
            zwlr_virtual_pointer_v1::Request::AxisStop { time, axis } => {
                handle_axis_stop(data, time, axis);
            }
            zwlr_virtual_pointer_v1::Request::AxisDiscrete { time, axis, value, discrete } => {
                handle_axis_discrete(data, time, axis, value, discrete);
            }
            zwlr_virtual_pointer_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("unhandled virtual pointer request");
            }
        }
    }

    fn destroyed(
        _state: &mut Self,
        _client: ClientId,
        _resource: &ZwlrVirtualPointerV1,
        _data: &VirtualPointerUserData,
    ) {
        // Nothing to clean up — axis state is dropped automatically.
    }
}

// ---------------------------------------------------------------------------
// Request handlers
// ---------------------------------------------------------------------------

/// Relative pointer motion — delta applied to current pointer location.
fn handle_motion(state: &mut State, time: u32, dx: f64, dy: f64) {
    tracing::trace!(time, dx, dy, "virtual pointer: motion");
    let serial = SERIAL_COUNTER.next_serial();
    state.pointer_location += (dx, dy).into();
    crate::input::clamp_pointer_location(state);
    crate::input::update_cursor_shape(state);

    let under = crate::input::surface_under(state);
    let pointer = state.pointer();
    pointer.motion(state, under, &MotionEvent { location: state.pointer_location, serial, time });
    pointer.frame(state);
}

/// Absolute pointer motion — coordinates normalised by extent.
///
/// When the virtual pointer is bound to a specific output (via
/// `CreateVirtualPointerWithOutput`), coordinates are mapped to that
/// output's geometry.  Otherwise they map to the full combined geometry
/// of all outputs.
#[allow(clippy::cast_possible_truncation)]
fn handle_motion_absolute(
    state: &mut State,
    data: &VirtualPointerUserData,
    time: u32,
    x: u32,
    y: u32,
    x_extent: u32,
    y_extent: u32,
) {
    if x_extent == 0 || y_extent == 0 {
        tracing::warn!(x_extent, y_extent, "virtual pointer: ignoring motion with zero extent");
        return;
    }
    tracing::trace!(time, x, y, x_extent, y_extent, "virtual pointer: motion absolute");

    let serial = SERIAL_COUNTER.next_serial();

    // Use the bound output's geometry when available, falling back to the
    // full combined geometry for unbound virtual pointers.
    let geo = data
        .bound_output
        .as_ref()
        .and_then(|o| state.space.output_geometry(o))
        .unwrap_or_else(|| state.combined_output_geometry());

    // Map [0, extent) → [0, output_size)
    let abs_x = f64::from(x) / f64::from(x_extent) * f64::from(geo.size.w);
    let abs_y = f64::from(y) / f64::from(y_extent) * f64::from(geo.size.h);

    state.pointer_location = (abs_x + f64::from(geo.loc.x), abs_y + f64::from(geo.loc.y)).into();

    crate::input::update_cursor_shape(state);

    let under = crate::input::surface_under(state);
    let pointer = state.pointer();
    pointer.motion(state, under, &MotionEvent { location: state.pointer_location, serial, time });
    pointer.frame(state);
}

/// Button press or release — delegates to the shared button handler so that
/// SSD titlebar clicks, window raising, and focus changes work identically
/// for virtual pointer clients (e.g. wayvnc) and direct backend input.
fn handle_button(state: &mut State, time: u32, button: u32, btn_state: WEnum<wl_pointer::ButtonState>) {
    use smithay::backend::input::ButtonState;

    let button_state = match btn_state {
        WEnum::Value(wl_pointer::ButtonState::Pressed) => ButtonState::Pressed,
        _ => ButtonState::Released,
    };
    tracing::debug!(time, button, ?button_state, "virtual pointer: button");

    crate::input::process_pointer_button(state, button, button_state, time);
}

/// Accumulate axis value into the pending frame.
fn handle_axis(data: &VirtualPointerUserData, time: u32, axis: WEnum<wl_pointer::Axis>, value: f64) {
    let Some(axis) = wl_axis_to_smithay(axis) else {
        return;
    };
    tracing::trace!(time, ?axis, value, "virtual pointer: axis");
    let mut pending = data.pending_axis.lock().expect("mutex poisoned");
    let frame = pending.get_or_insert_with(|| PendingAxisFrame { time, ..Default::default() });
    frame.time = time;

    match axis {
        Axis::Horizontal => frame.horizontal = Some(value),
        Axis::Vertical => frame.vertical = Some(value),
    }
}

/// Set the axis source for the pending frame.
fn handle_axis_source(data: &VirtualPointerUserData, axis_source: WEnum<wl_pointer::AxisSource>) {
    let Some(source) = wl_axis_source_to_smithay(axis_source) else {
        return;
    };
    tracing::trace!(?source, "virtual pointer: axis source");
    let mut pending = data.pending_axis.lock().expect("mutex poisoned");
    let frame = pending.get_or_insert_with(PendingAxisFrame::default);
    frame.source = Some(source);
}

/// Mark an axis as stopped in the pending frame.
fn handle_axis_stop(data: &VirtualPointerUserData, time: u32, axis: WEnum<wl_pointer::Axis>) {
    let Some(axis) = wl_axis_to_smithay(axis) else {
        return;
    };
    tracing::trace!(time, ?axis, "virtual pointer: axis stop");
    let mut pending = data.pending_axis.lock().expect("mutex poisoned");
    let frame = pending.get_or_insert_with(|| PendingAxisFrame { time, ..Default::default() });
    frame.time = time;

    match axis {
        Axis::Horizontal => frame.stop_horizontal = true,
        Axis::Vertical => frame.stop_vertical = true,
    }
}

/// Accumulate discrete axis step into the pending frame.
fn handle_axis_discrete(
    data: &VirtualPointerUserData,
    time: u32,
    axis: WEnum<wl_pointer::Axis>,
    value: f64,
    discrete: i32,
) {
    let Some(axis) = wl_axis_to_smithay(axis) else {
        return;
    };
    tracing::trace!(time, ?axis, value, discrete, "virtual pointer: axis discrete");
    let mut pending = data.pending_axis.lock().expect("mutex poisoned");
    let frame = pending.get_or_insert_with(|| PendingAxisFrame { time, ..Default::default() });
    frame.time = time;

    match axis {
        Axis::Horizontal => {
            frame.horizontal = Some(value);
            frame.horizontal_discrete = Some(discrete);
        }
        Axis::Vertical => {
            frame.vertical = Some(value);
            frame.vertical_discrete = Some(discrete);
        }
    }
}

/// Flush the accumulated axis state as a single `AxisFrame`.
fn handle_frame(state: &mut State, data: &VirtualPointerUserData) {
    if let Some(pending) = data.pending_axis.lock().expect("mutex poisoned").take() {
        let mut frame = AxisFrame::new(pending.time);

        if let Some(source) = pending.source {
            frame = frame.source(source);
        }

        if let Some(val) = pending.horizontal {
            frame = frame.value(Axis::Horizontal, val);
        }
        if let Some(val) = pending.vertical {
            frame = frame.value(Axis::Vertical, val);
        }

        if let Some(v120) = pending.horizontal_discrete {
            frame = frame.v120(Axis::Horizontal, v120);
        }
        if let Some(v120) = pending.vertical_discrete {
            frame = frame.v120(Axis::Vertical, v120);
        }

        if pending.stop_horizontal {
            frame = frame.stop(Axis::Horizontal);
        }
        if pending.stop_vertical {
            frame = frame.stop(Axis::Vertical);
        }

        let pointer = state.pointer();
        pointer.axis(state, frame);
    }

    // Always send a frame event so motion/button events are properly delimited.
    let pointer = state.pointer();
    pointer.frame(state);
}

// ---------------------------------------------------------------------------
// Protocol ↔ smithay type helpers
// ---------------------------------------------------------------------------

/// Convert `wl_pointer::Axis` (protocol enum) to `smithay::backend::input::Axis`.
fn wl_axis_to_smithay(axis: WEnum<wl_pointer::Axis>) -> Option<Axis> {
    match axis {
        WEnum::Value(wl_pointer::Axis::HorizontalScroll) => Some(Axis::Horizontal),
        WEnum::Value(wl_pointer::Axis::VerticalScroll) => Some(Axis::Vertical),
        _ => None,
    }
}

/// Convert `wl_pointer::AxisSource` (protocol enum) to `smithay::backend::input::AxisSource`.
fn wl_axis_source_to_smithay(source: WEnum<wl_pointer::AxisSource>) -> Option<AxisSource> {
    match source {
        WEnum::Value(wl_pointer::AxisSource::Wheel) => Some(AxisSource::Wheel),
        WEnum::Value(wl_pointer::AxisSource::Finger) => Some(AxisSource::Finger),
        WEnum::Value(wl_pointer::AxisSource::Continuous) => Some(AxisSource::Continuous),
        WEnum::Value(wl_pointer::AxisSource::WheelTilt) => Some(AxisSource::WheelTilt),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Global registration helper
// ---------------------------------------------------------------------------

/// Register the `zwlr_virtual_pointer_manager_v1` global.
///
/// The `filter` closure controls visibility: return `true` to advertise the
/// global to the client, `false` to hide it (used by the security policy).
pub fn init_virtual_pointer_manager(
    dh: &DisplayHandle,
    filter: impl Fn(&Client) -> bool + Send + Sync + 'static,
) -> GlobalId {
    dh.create_global::<State, ZwlrVirtualPointerManagerV1, _>(
        2,
        VirtualPointerManagerGlobalData { filter: Box::new(filter) },
    )
}
