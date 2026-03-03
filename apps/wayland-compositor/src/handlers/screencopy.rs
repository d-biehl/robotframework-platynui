//! Screencopy protocol handler (`ext-image-copy-capture-v1` + `ext-image-capture-source-v1`).
//!
//! Implements the standard Wayland screencopy protocol so that tools like
//! `grim`, `wayvnc`, and `wl-mirror` can capture output or toplevel content.
//!
//! Since smithay 0.7 has no built-in support, we implement [`GlobalDispatch`]
//! and [`Dispatch`] manually for all protocol interfaces.
//!
//! ## Protocol flow
//!
//! 1. Client creates a capture source via
//!    `ext_output_image_capture_source_manager_v1.create_source(output)` or
//!    `ext_foreign_toplevel_image_capture_source_manager_v1.create_source(handle)`.
//! 2. Client creates a session via
//!    `ext_image_copy_capture_manager_v1.create_session(source, options)`.
//! 3. Compositor sends buffer constraint events (`buffer_size`, `shm_format`,
//!    `done`) on the session.
//! 4. Client creates a frame via `session.create_frame()`.
//! 5. Client attaches a `wl_buffer`, optionally marks damage, calls `capture`.
//! 6. Compositor renders the source into an offscreen texture, copies the
//!    pixels into the client's shm buffer, and sends `transform`, `damage`,
//!    `presentation_time`, `ready` (or `failed`).
//!
//! ## Supported features
//!
//! - Output capture (full monitor)
//! - Toplevel capture (individual window via ext-foreign-toplevel handle)
//! - `wl_shm` buffers (ARGB8888 / XRGB8888)
//! - `paint_cursors` option (composites software cursor onto captured frame)
//! - Cursor sessions (sends cursor image separately for VNC Cursor Pseudo-Encoding)

use std::sync::atomic::{AtomicBool, Ordering};

use smithay::{
    desktop::Window,
    output::Output,
    reexports::{
        wayland_protocols::ext::{
            image_capture_source::v1::server::{
                ext_foreign_toplevel_image_capture_source_manager_v1::{
                    self, ExtForeignToplevelImageCaptureSourceManagerV1,
                },
                ext_image_capture_source_v1::ExtImageCaptureSourceV1,
                ext_output_image_capture_source_manager_v1::{self, ExtOutputImageCaptureSourceManagerV1},
            },
            image_copy_capture::v1::server::{
                ext_image_copy_capture_cursor_session_v1::{self, ExtImageCopyCaptureCursorSessionV1},
                ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1},
                ext_image_copy_capture_manager_v1::{self, ExtImageCopyCaptureManagerV1},
                ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1},
            },
        },
        wayland_server::{
            Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, Weak,
            backend::GlobalId,
            protocol::{wl_buffer::WlBuffer, wl_shm},
        },
    },
    utils::{Physical, Rectangle, Size, Transform},
    wayland::shm::{BufferData, with_buffer_contents_mut},
};

use crate::state::State;

// ---------------------------------------------------------------------------
// Capture source — what to capture
// ---------------------------------------------------------------------------

/// What the capture session is targeting.
#[derive(Clone, Debug)]
pub enum CaptureSource {
    /// Full output (monitor) capture.
    Output(Output),
    /// Individual toplevel (window) capture.
    Toplevel(Window),
    /// Cursor image capture — renders only the cursor shape.
    ///
    /// Created by cursor sessions so that clients like wayvnc can obtain
    /// the cursor image separately (for VNC Cursor Pseudo-Encoding).
    Cursor,
}

/// Data attached to `ext_image_capture_source_v1` objects.
pub struct SourceData {
    pub source: CaptureSource,
}

// ---------------------------------------------------------------------------
// Source manager globals
// ---------------------------------------------------------------------------

/// Data attached to the output source manager global (for `can_view` filtering).
pub struct OutputSourceManagerGlobalData {
    filter: Box<dyn Fn(&Client) -> bool + Send + Sync>,
}

/// Data attached to the toplevel source manager global.
pub struct ToplevelSourceManagerGlobalData {
    filter: Box<dyn Fn(&Client) -> bool + Send + Sync>,
}

/// Data attached to the copy-capture manager global.
pub struct CaptureManagerGlobalData {
    filter: Box<dyn Fn(&Client) -> bool + Send + Sync>,
}

// ---------------------------------------------------------------------------
// Session data
// ---------------------------------------------------------------------------

/// Per-session state.
pub struct SessionData {
    /// The capture source for this session.
    pub source: CaptureSource,
    /// Whether cursors should be painted onto captured frames.
    pub paint_cursors: bool,
    /// Buffer size (physical pixels) advertised to the client.
    pub buffer_size: Size<i32, Physical>,
    /// Whether the session has an active (undestroyed) frame.
    ///
    /// Uses `AtomicBool` for interior mutability because smithay's `Dispatch`
    /// only provides `&SessionData` (shared reference) and `DataInit` requires `Sync`.
    pub has_active_frame: AtomicBool,
    /// Back-reference to the parent cursor session (cursor captures only).
    ///
    /// Used to send `hotspot` events when a cursor frame is captured.
    pub cursor_session: Option<Weak<ExtImageCopyCaptureCursorSessionV1>>,
}

// ---------------------------------------------------------------------------
// Frame data
// ---------------------------------------------------------------------------

/// Per-frame state — tracks a single capture request.
pub struct FrameData {
    /// Source (inherited from session).
    pub source: CaptureSource,
    /// Whether to paint cursors.
    pub paint_cursors: bool,
    /// Attached client buffer (set by `attach_buffer`).
    pub buffer: Option<WlBuffer>,
    /// Accumulated damage regions (set by `damage_buffer`).
    pub damage: Vec<Rectangle<i32, Physical>>,
    /// Whether `capture` has been sent already.
    pub captured: bool,
    /// Expected buffer size.
    pub buffer_size: Size<i32, Physical>,
    /// Weak reference back to the owning session (to clear `has_active_frame`).
    pub session: Weak<ExtImageCopyCaptureSessionV1>,
    /// Back-reference to cursor session for sending cursor events (cursor captures only).
    pub cursor_session: Option<Weak<ExtImageCopyCaptureCursorSessionV1>>,
}

// ---------------------------------------------------------------------------
// Cursor session data (stub)
// ---------------------------------------------------------------------------

/// Cursor session data — tracks cursor shape capture state.
///
/// A cursor session lets clients obtain the cursor image separately from
/// the desktop frame, enabling protocols like VNC's Cursor Pseudo-Encoding
/// where the client renders the cursor locally.
pub struct CursorSessionData {
    /// The capture source the cursor is associated with (output or toplevel).
    pub source: CaptureSource,
    /// Whether `get_capture_session` has been called.
    ///
    /// Uses `AtomicBool` for interior mutability because smithay's `Dispatch`
    /// only provides `&CursorSessionData` (shared reference) and `DataInit` requires `Sync`.
    pub session_created: AtomicBool,
    /// Cursor image buffer size (width × height in physical pixels).
    pub cursor_size: Size<i32, Physical>,
}

// ---------------------------------------------------------------------------
// Output source manager — GlobalDispatch + Dispatch
// ---------------------------------------------------------------------------

impl GlobalDispatch<ExtOutputImageCaptureSourceManagerV1, OutputSourceManagerGlobalData> for State {
    fn bind(
        _state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ExtOutputImageCaptureSourceManagerV1>,
        _global_data: &OutputSourceManagerGlobalData,
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }

    fn can_view(client: Client, global_data: &OutputSourceManagerGlobalData) -> bool {
        (global_data.filter)(&client)
    }
}

impl Dispatch<ExtOutputImageCaptureSourceManagerV1, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &ExtOutputImageCaptureSourceManagerV1,
        request: ext_output_image_capture_source_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let ext_output_image_capture_source_manager_v1::Request::CreateSource { source, output } = request {
            // Resolve the wl_output to our Output object.
            let Some(output_obj) = output_from_wl_output(state, &output) else {
                tracing::warn!("screencopy: create_source for unknown wl_output");
                // Create a dead source — session creation will eventually fail.
                data_init.init(source, SourceData { source: CaptureSource::Output(state.output.clone()) });
                return;
            };
            tracing::debug!(output = %output_obj.name(), "screencopy: output source created");
            data_init.init(source, SourceData { source: CaptureSource::Output(output_obj) });
        }
    }
}

// ---------------------------------------------------------------------------
// Toplevel source manager — GlobalDispatch + Dispatch
// ---------------------------------------------------------------------------

impl GlobalDispatch<ExtForeignToplevelImageCaptureSourceManagerV1, ToplevelSourceManagerGlobalData> for State {
    fn bind(
        _state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ExtForeignToplevelImageCaptureSourceManagerV1>,
        _global_data: &ToplevelSourceManagerGlobalData,
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }

    fn can_view(client: Client, global_data: &ToplevelSourceManagerGlobalData) -> bool {
        (global_data.filter)(&client)
    }
}

impl Dispatch<ExtForeignToplevelImageCaptureSourceManagerV1, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &ExtForeignToplevelImageCaptureSourceManagerV1,
        request: ext_foreign_toplevel_image_capture_source_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let ext_foreign_toplevel_image_capture_source_manager_v1::Request::CreateSource { source, toplevel_handle } =
            request
        {
            // Look up the Window by its ext-foreign-toplevel handle.
            // Get the ForeignToplevelHandle from the resource's user data,
            // then find the matching entry by checking resource identity.
            let window = state
                .ext_toplevel_handles
                .iter()
                .find(|(_, h)| h.resources().iter().any(|r| r.id() == toplevel_handle.id()))
                .map(|(w, _)| w.clone());

            if let Some(window) = window {
                tracing::debug!("screencopy: toplevel source created");
                data_init.init(source, SourceData { source: CaptureSource::Toplevel(window) });
            } else {
                tracing::warn!("screencopy: create_source for unknown toplevel handle");
                // Create a source pointing at primary output as fallback.
                data_init.init(source, SourceData { source: CaptureSource::Output(state.output.clone()) });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Capture source — Dispatch (only has destroy request)
// ---------------------------------------------------------------------------

impl Dispatch<ExtImageCaptureSourceV1, SourceData> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &ExtImageCaptureSourceV1,
        _request: <ExtImageCaptureSourceV1 as Resource>::Request,
        _data: &SourceData,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        // Only request is `destroy` (destructor) — handled automatically.
    }
}

// ---------------------------------------------------------------------------
// Copy-capture manager — GlobalDispatch + Dispatch
// ---------------------------------------------------------------------------

impl GlobalDispatch<ExtImageCopyCaptureManagerV1, CaptureManagerGlobalData> for State {
    fn bind(
        _state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ExtImageCopyCaptureManagerV1>,
        _global_data: &CaptureManagerGlobalData,
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }

    fn can_view(client: Client, global_data: &CaptureManagerGlobalData) -> bool {
        (global_data.filter)(&client)
    }
}

impl Dispatch<ExtImageCopyCaptureManagerV1, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &ExtImageCopyCaptureManagerV1,
        request: ext_image_copy_capture_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            ext_image_copy_capture_manager_v1::Request::CreateSession { session, source, options } => {
                let source_data: &SourceData = source.data().expect("source missing data");
                let capture_source = source_data.source.clone();

                // Parse options bitfield.
                let paint_cursors = options
                    .into_result()
                    .is_ok_and(|opts| opts.contains(ext_image_copy_capture_manager_v1::Options::PaintCursors));

                // Compute buffer size from the source.
                let buffer_size = source_buffer_size(state, &capture_source);

                let session_obj = data_init.init(
                    session,
                    SessionData {
                        source: capture_source,
                        paint_cursors,
                        buffer_size,
                        has_active_frame: AtomicBool::new(false),
                        cursor_session: None,
                    },
                );

                // Send buffer constraints to the client.
                send_session_constraints(&session_obj, buffer_size);

                tracing::debug!(
                    width = buffer_size.w,
                    height = buffer_size.h,
                    paint_cursors,
                    "screencopy: session created"
                );
            }
            ext_image_copy_capture_manager_v1::Request::CreatePointerCursorSession { session, source, pointer: _ } => {
                let source_data: &SourceData = source.data().expect("source missing data");
                let capture_source = source_data.source.clone();

                // Determine cursor image dimensions from the xcursor theme.
                let (cw, ch) = state.cursor_theme.default_cursor_dimensions();
                let cursor_size: Size<i32, Physical> = (cw.cast_signed(), ch.cast_signed()).into();

                let cursor_session = data_init.init(
                    session,
                    CursorSessionData { source: capture_source, session_created: AtomicBool::new(false), cursor_size },
                );

                // Send enter event — the cursor is present on the output.
                cursor_session.enter();

                // Send initial hotspot from the current cursor icon.
                let time = state.start_time.elapsed();
                if let Some(data) =
                    state.cursor_theme.get_cursor_data(smithay::input::pointer::CursorIcon::Default, 1, time)
                {
                    #[allow(clippy::cast_possible_wrap)]
                    cursor_session.hotspot(data.xhot as i32, data.yhot as i32);
                }

                tracing::debug!(width = cursor_size.w, height = cursor_size.h, "screencopy: cursor session created");
            }
            ext_image_copy_capture_manager_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("unhandled screencopy manager request");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Capture session — Dispatch
// ---------------------------------------------------------------------------

impl Dispatch<ExtImageCopyCaptureSessionV1, SessionData> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        resource: &ExtImageCopyCaptureSessionV1,
        request: ext_image_copy_capture_session_v1::Request,
        data: &SessionData,
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let ext_image_copy_capture_session_v1::Request::CreateFrame { frame } = request {
            if data.has_active_frame.load(Ordering::Relaxed) {
                resource.post_error(
                    ext_image_copy_capture_session_v1::Error::DuplicateFrame,
                    "a frame already exists for this session",
                );
                return;
            }

            // Mark that the session has an active frame.
            data.has_active_frame.store(true, Ordering::Relaxed);

            data_init.init(
                frame,
                FrameData {
                    source: data.source.clone(),
                    paint_cursors: data.paint_cursors,
                    buffer: None,
                    damage: Vec::new(),
                    captured: false,
                    buffer_size: data.buffer_size,
                    session: resource.downgrade(),
                    cursor_session: data.cursor_session.clone(),
                },
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Capture frame — Dispatch (the core capture logic)
// ---------------------------------------------------------------------------

impl Dispatch<ExtImageCopyCaptureFrameV1, FrameData> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &ExtImageCopyCaptureFrameV1,
        request: ext_image_copy_capture_frame_v1::Request,
        data: &FrameData,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            ext_image_copy_capture_frame_v1::Request::AttachBuffer { buffer } => {
                if data.captured {
                    resource.post_error(
                        ext_image_copy_capture_frame_v1::Error::AlreadyCaptured,
                        "cannot attach buffer after capture",
                    );
                    return;
                }
                // Store the buffer.  Since we have `&FrameData` we use a
                // workaround: we'll re-read the buffer from `resource.data()`
                // in the capture handler.  For now, store it via an interior
                // mutability escape hatch (see capture handler).
                //
                // Actually, the Dispatch trait gives us &FrameData (immutable).
                // We'll handle attach/damage/capture by deferring the actual
                // work to a separate mutable function called from `capture`.
                // For `attach_buffer` and `damage_buffer` we accumulate state
                // in a side-channel on State (keyed by the frame resource id).
                let frame_id = resource.id();
                let entry = state
                    .pending_captures
                    .entry(frame_id)
                    .or_insert_with(|| PendingCapture { buffer: None, damage: Vec::new() });
                entry.buffer = Some(buffer);
            }
            ext_image_copy_capture_frame_v1::Request::DamageBuffer { x, y, width, height } => {
                if data.captured {
                    resource.post_error(
                        ext_image_copy_capture_frame_v1::Error::AlreadyCaptured,
                        "cannot add damage after capture",
                    );
                    return;
                }
                if x < 0 || y < 0 || width <= 0 || height <= 0 {
                    resource.post_error(
                        ext_image_copy_capture_frame_v1::Error::InvalidBufferDamage,
                        "invalid damage coordinates",
                    );
                    return;
                }
                let frame_id = resource.id();
                let entry = state
                    .pending_captures
                    .entry(frame_id)
                    .or_insert_with(|| PendingCapture { buffer: None, damage: Vec::new() });
                entry.damage.push(Rectangle::new((x, y).into(), (width, height).into()));
            }
            ext_image_copy_capture_frame_v1::Request::Capture => {
                if data.captured {
                    resource
                        .post_error(ext_image_copy_capture_frame_v1::Error::AlreadyCaptured, "capture already sent");
                    return;
                }
                let frame_id = resource.id();
                let pending = state.pending_captures.remove(&frame_id);
                let Some(pending) = pending else {
                    resource.post_error(ext_image_copy_capture_frame_v1::Error::NoBuffer, "no buffer attached");
                    return;
                };
                let Some(buffer) = pending.buffer else {
                    resource.post_error(ext_image_copy_capture_frame_v1::Error::NoBuffer, "no buffer attached");
                    return;
                };

                // Perform the capture.
                perform_capture(
                    state,
                    resource,
                    &data.source,
                    data.paint_cursors,
                    data.buffer_size,
                    &buffer,
                    data.cursor_session.as_ref(),
                );
            }
            ext_image_copy_capture_frame_v1::Request::Destroy => {
                // Clean up any pending state and release the session's frame slot.
                let frame_id = resource.id();
                state.pending_captures.remove(&frame_id);
                clear_session_active_frame(&data.session);
            }
            _ => {
                tracing::debug!("unhandled screencopy frame request");
            }
        }
    }

    fn destroyed(
        state: &mut Self,
        _client_id: smithay::reexports::wayland_server::backend::ClientId,
        resource: &ExtImageCopyCaptureFrameV1,
        data: &FrameData,
    ) {
        // Clean up pending captures when the client disconnects without
        // sending an explicit Destroy request (e.g. client crash).
        let frame_id = resource.id();
        state.pending_captures.remove(&frame_id);
        clear_session_active_frame(&data.session);
    }
}

// ---------------------------------------------------------------------------
// Cursor session — Dispatch
// ---------------------------------------------------------------------------

impl Dispatch<ExtImageCopyCaptureCursorSessionV1, CursorSessionData> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        resource: &ExtImageCopyCaptureCursorSessionV1,
        request: ext_image_copy_capture_cursor_session_v1::Request,
        data: &CursorSessionData,
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let ext_image_copy_capture_cursor_session_v1::Request::GetCaptureSession { session } = request {
            if data.session_created.load(Ordering::Relaxed) {
                resource.post_error(
                    ext_image_copy_capture_cursor_session_v1::Error::DuplicateSession,
                    "capture session already created",
                );
                return;
            }

            // Mark that a capture session has been created.
            data.session_created.store(true, Ordering::Relaxed);

            // The cursor session's capture session renders only the cursor
            // image — the client captures it to obtain cursor shape data
            // (used by wayvnc for VNC Cursor Pseudo-Encoding).
            let cursor_size = data.cursor_size;
            let session_obj = data_init.init(
                session,
                SessionData {
                    source: CaptureSource::Cursor,
                    paint_cursors: false,
                    buffer_size: cursor_size,
                    has_active_frame: AtomicBool::new(false),
                    cursor_session: Some(resource.downgrade()),
                },
            );
            send_session_constraints(&session_obj, cursor_size);

            tracing::debug!(
                width = cursor_size.w,
                height = cursor_size.h,
                "screencopy: cursor capture session created"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Pending capture accumulator (side-channel for frame state)
// ---------------------------------------------------------------------------

/// Accumulated state for an in-progress frame capture.
///
/// Since `Dispatch` gives us only `&FrameData` (immutable), we store mutable
/// per-frame state here, keyed by the frame's object ID in `State::pending_captures`.
pub struct PendingCapture {
    pub buffer: Option<WlBuffer>,
    pub damage: Vec<Rectangle<i32, Physical>>,
}

// ---------------------------------------------------------------------------
// Initialization — register globals
// ---------------------------------------------------------------------------

/// Register all screencopy-related Wayland globals.
///
/// Returns the global IDs for the three manager objects (for potential
/// later destruction).
pub fn init_screencopy(
    dh: &DisplayHandle,
    filter: impl Fn(&Client) -> bool + Send + Sync + Clone + 'static,
) -> (GlobalId, GlobalId, GlobalId) {
    let output_source_global = dh.create_global::<State, ExtOutputImageCaptureSourceManagerV1, _>(
        1,
        OutputSourceManagerGlobalData { filter: Box::new(filter.clone()) },
    );
    let toplevel_source_global = dh.create_global::<State, ExtForeignToplevelImageCaptureSourceManagerV1, _>(
        1,
        ToplevelSourceManagerGlobalData { filter: Box::new(filter.clone()) },
    );
    let capture_manager_global = dh.create_global::<State, ExtImageCopyCaptureManagerV1, _>(
        1,
        CaptureManagerGlobalData { filter: Box::new(filter) },
    );

    (output_source_global, toplevel_source_global, capture_manager_global)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Clear the `has_active_frame` flag on a session when its frame is destroyed.
fn clear_session_active_frame(session: &Weak<ExtImageCopyCaptureSessionV1>) {
    if let Ok(session) = session.upgrade()
        && let Some(data) = session.data::<SessionData>()
    {
        data.has_active_frame.store(false, Ordering::Relaxed);
    }
}

/// Resolve a `wl_output` resource to our internal `Output` object.
fn output_from_wl_output(
    state: &State,
    wl_output: &smithay::reexports::wayland_server::protocol::wl_output::WlOutput,
) -> Option<Output> {
    state.outputs.iter().find(|o| o.owns(wl_output)).cloned()
}

/// Compute the buffer size (in physical pixels) for a capture source.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn source_buffer_size(state: &State, source: &CaptureSource) -> Size<i32, Physical> {
    match source {
        CaptureSource::Output(output) => {
            let mode = output.current_mode().unwrap_or(smithay::output::Mode {
                size: (
                    state.output.current_mode().map_or(1920, |m| m.size.w),
                    state.output.current_mode().map_or(1080, |m| m.size.h),
                )
                    .into(),
                refresh: crate::state::DEFAULT_REFRESH_MHTZ,
            });
            // The mode size is already in physical pixels.
            mode.size
        }
        CaptureSource::Toplevel(window) => {
            let geo = window.geometry();
            let scale = state.outputs.first().map_or(1.0, |o| o.current_scale().fractional_scale());
            let w = (f64::from(geo.size.w) * scale).ceil() as i32;
            let h = (f64::from(geo.size.h) * scale).ceil() as i32;
            (w.max(1), h.max(1)).into()
        }
        CaptureSource::Cursor => {
            // Cursor image size is determined at cursor session creation.
            // This code path should not be reached since cursor sessions
            // set buffer_size directly. Fall back to the nominal theme size.
            let size = state.cursor_theme.nominal_size().cast_signed();
            (size, size).into()
        }
    }
}

/// Send buffer constraint events on a newly created session.
fn send_session_constraints(session: &ExtImageCopyCaptureSessionV1, size: Size<i32, Physical>) {
    // Advertise the buffer size.
    session.buffer_size(u32::try_from(size.w).unwrap_or(1), u32::try_from(size.h).unwrap_or(1));

    // Advertise supported shm formats.
    // ARGB8888 and XRGB8888 are universally supported.
    session.shm_format(wl_shm::Format::Argb8888);
    session.shm_format(wl_shm::Format::Xrgb8888);

    // End the constraint batch.
    session.done();
}

/// Perform the actual frame capture: render → copy to shm buffer → send events.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn perform_capture(
    state: &mut State,
    frame: &ExtImageCopyCaptureFrameV1,
    source: &CaptureSource,
    paint_cursors: bool,
    expected_size: Size<i32, Physical>,
    buffer: &WlBuffer,
    cursor_session: Option<&Weak<ExtImageCopyCaptureCursorSessionV1>>,
) {
    // Cursor source captures use a fast path that copies xcursor pixel data
    // directly into the SHM buffer without the GL render pipeline.
    if matches!(source, CaptureSource::Cursor) {
        perform_cursor_capture(state, frame, expected_size, buffer, cursor_session);
        return;
    }

    // Validate the shm buffer and its dimensions.
    let buf_info = match validate_shm_buffer(buffer, expected_size) {
        Ok(info) => info,
        Err(reason) => {
            tracing::debug!(reason, "screencopy: capture failed (buffer validation)");
            frame.failed(ext_image_copy_capture_frame_v1::FailureReason::BufferConstraints);
            return;
        }
    };

    // Render the source into an offscreen GL texture.
    let pixel_data = match render_source_to_pixels(state, source, expected_size, paint_cursors) {
        Ok(pixels) => pixels,
        Err(err) => {
            tracing::warn!(err, "screencopy: capture failed (render)");
            frame.failed(ext_image_copy_capture_frame_v1::FailureReason::Unknown);
            return;
        }
    };

    // Copy rendered pixels into the client's shm buffer.
    if let Err(err) = copy_pixels_to_shm(buffer, &buf_info, &pixel_data, expected_size) {
        tracing::warn!(err, "screencopy: capture failed (shm copy)");
        frame.failed(ext_image_copy_capture_frame_v1::FailureReason::Unknown);
        return;
    }

    // Send frame metadata events.
    send_frame_metadata(state, frame, expected_size);
}

/// Perform a cursor-only capture — copies the current cursor image into the
/// client's SHM buffer.
///
/// This bypasses the full GL render pipeline: the xcursor pixel data is
/// already in ARGB8888 format and can be written directly into the SHM buffer
/// with only a size-fitting blit.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn perform_cursor_capture(
    state: &mut State,
    frame: &ExtImageCopyCaptureFrameV1,
    expected_size: Size<i32, Physical>,
    buffer: &WlBuffer,
    cursor_session: Option<&Weak<ExtImageCopyCaptureCursorSessionV1>>,
) {
    use smithay::input::pointer::CursorImageStatus;

    // Validate the SHM buffer.
    let buf_info = match validate_shm_buffer(buffer, expected_size) {
        Ok(info) => info,
        Err(reason) => {
            tracing::debug!(reason, "screencopy: cursor capture failed (buffer validation)");
            frame.failed(ext_image_copy_capture_frame_v1::FailureReason::BufferConstraints);
            return;
        }
    };

    // Get the current cursor icon.
    let (icon, time) = match &state.cursor_status {
        CursorImageStatus::Named(icon) => (*icon, state.start_time.elapsed()),
        CursorImageStatus::Surface(_) | CursorImageStatus::Hidden => {
            // Surface cursors and hidden cursors: fill with transparent pixels.
            if let Err(err) = fill_shm_transparent(buffer, &buf_info, expected_size) {
                tracing::warn!(err, "screencopy: cursor capture failed (transparent fill)");
                frame.failed(ext_image_copy_capture_frame_v1::FailureReason::Unknown);
                return;
            }
            // For hidden cursor, send leave on the cursor session.
            if matches!(state.cursor_status, CursorImageStatus::Hidden)
                && let Some(cs) = cursor_session.and_then(|w| w.upgrade().ok())
            {
                cs.leave();
            }
            send_frame_metadata(state, frame, expected_size);
            return;
        }
    };

    // Get cursor image data from the xcursor theme.
    let Some(cursor_data) = state.cursor_theme.get_cursor_data(icon, 1, time) else {
        tracing::debug!(?icon, "screencopy: cursor icon not available");
        if let Err(err) = fill_shm_transparent(buffer, &buf_info, expected_size) {
            tracing::warn!(err, "screencopy: cursor capture failed (transparent fill)");
            frame.failed(ext_image_copy_capture_frame_v1::FailureReason::Unknown);
            return;
        }
        send_frame_metadata(state, frame, expected_size);
        return;
    };

    // Copy cursor pixels into the SHM buffer, centering or clipping as needed.
    if let Err(err) = copy_cursor_to_shm(buffer, &buf_info, &cursor_data, expected_size) {
        tracing::warn!(err, "screencopy: cursor capture failed (shm copy)");
        frame.failed(ext_image_copy_capture_frame_v1::FailureReason::Unknown);
        return;
    }

    // Send hotspot event on the cursor session before marking the frame ready.
    if let Some(cs) = cursor_session.and_then(|w| w.upgrade().ok()) {
        cs.hotspot(cursor_data.xhot.cast_signed(), cursor_data.yhot.cast_signed());
    }

    send_frame_metadata(state, frame, expected_size);
    tracing::trace!(width = cursor_data.width, height = cursor_data.height, "screencopy: cursor frame captured");
}

/// Send common frame metadata (transform, damage, presentation time, ready).
#[allow(clippy::cast_possible_truncation)]
fn send_frame_metadata(state: &State, frame: &ExtImageCopyCaptureFrameV1, size: Size<i32, Physical>) {
    frame.transform(Transform::Normal.into());

    // Full damage for simplicity (entire buffer has been updated).
    frame.damage(0, 0, size.w, size.h);

    // Presentation time — monotonic clock approximation.
    let duration = state.start_time.elapsed();
    let total_secs = duration.as_secs();
    let nsec_part = duration.subsec_nanos();
    let sec_hi = ((total_secs >> 32) & 0xFFFF_FFFF) as u32;
    let sec_lo = (total_secs & 0xFFFF_FFFF) as u32;
    frame.presentation_time(sec_hi, sec_lo, nsec_part);

    frame.ready();
}

/// Validate that the shm buffer matches expected constraints.
///
/// Returns buffer metadata on success, or an error reason string on failure.
fn validate_shm_buffer(buffer: &WlBuffer, expected_size: Size<i32, Physical>) -> Result<BufferData, &'static str> {
    // Try to read buffer info.  `with_buffer_contents_mut` verifies it's an shm buffer.
    let info = with_buffer_contents_mut(buffer, |_ptr, _len, data| data).map_err(|_| "not a valid shm buffer")?;

    if info.width != expected_size.w || info.height != expected_size.h {
        return Err("buffer dimensions do not match session constraints");
    }

    // Check format — we only support ARGB8888 and XRGB8888.
    if info.format != wl_shm::Format::Argb8888 && info.format != wl_shm::Format::Xrgb8888 {
        return Err("unsupported shm format");
    }

    Ok(info)
}

/// Render a capture source into a pixel buffer (RGBA8888 byte order).
///
/// Uses the shared offscreen rendering pipeline from [`crate::render::render_to_pixels`].
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn render_source_to_pixels(
    state: &mut State,
    source: &CaptureSource,
    size: Size<i32, Physical>,
    paint_cursors: bool,
) -> Result<Vec<u8>, String> {
    // Resolve scale and output from the capture source.
    let scale = match source {
        CaptureSource::Output(output) => output.current_scale().fractional_scale(),
        CaptureSource::Toplevel(_) => state.outputs.first().map_or(1.0, |o| o.current_scale().fractional_scale()),
        CaptureSource::Cursor => {
            return Err("cursor captures should use perform_cursor_capture, not the GL pipeline".to_string());
        }
    };
    let output = match source {
        CaptureSource::Output(o) => o.clone(),
        CaptureSource::Toplevel(_) => state.output.clone(),
        CaptureSource::Cursor => {
            return Err("cursor captures should use perform_cursor_capture, not the GL pipeline".to_string());
        }
    };

    // Lazily initialize the screenshot renderer.
    if state.screenshot_renderer.is_none() {
        state.screenshot_renderer = Some(
            crate::backend::create_offscreen_glow_renderer()
                .map_err(|e| format!("failed to create offscreen renderer: {e}"))?,
        );
    }

    // Take the renderer to avoid borrow conflicts.
    let mut renderer = state.screenshot_renderer.take().expect("screenshot renderer was just initialized above");
    let result = crate::render::render_to_pixels(&mut renderer, state, &output, size, scale, paint_cursors);
    state.screenshot_renderer = Some(renderer);

    result
}

/// Copy rendered pixels (ABGR8888) into a client's shm buffer (ARGB8888 or XRGB8888).
///
/// The GL readback produces ABGR8888 (R, G, B, A byte order in memory).
/// Wayland's ARGB8888 is B, G, R, A in memory (little-endian 0xAARRGGBB).
/// We need to swizzle: GL(R,G,B,A) → WL(B,G,R,A).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn copy_pixels_to_shm(
    buffer: &WlBuffer,
    buf_info: &BufferData,
    pixel_data: &[u8],
    size: Size<i32, Physical>,
) -> Result<(), String> {
    let src_stride = size.w as usize * 4; // ABGR8888 = 4 bytes per pixel
    let dst_stride = buf_info.stride as usize;
    let height = size.h as usize;
    let width = size.w as usize;

    with_buffer_contents_mut(buffer, |ptr, pool_len, data| {
        let offset = data.offset as usize;
        let needed = offset + height * dst_stride;
        if needed > pool_len {
            return Err("shm pool too small for buffer".to_string());
        }

        // SAFETY: `ptr` is valid for `pool_len` bytes (guaranteed by smithay's
        // `with_buffer_contents_mut`), and we verified `needed <= pool_len`.
        #[allow(unsafe_code)]
        let dst = unsafe { std::slice::from_raw_parts_mut(ptr, pool_len) };

        for y in 0..height {
            let src_row = &pixel_data[y * src_stride..y * src_stride + width * 4];
            let dst_start = offset + y * dst_stride;
            for x in 0..width {
                let si = x * 4;
                let di = dst_start + x * 4;
                // GL ABGR8888: bytes are [R, G, B, A]
                // WL ARGB8888: bytes are [B, G, R, A] (little-endian 0xAARRGGBB)
                dst[di] = src_row[si + 2];
                dst[di + 1] = src_row[si + 1];
                dst[di + 2] = src_row[si];
                dst[di + 3] = src_row[si + 3];
            }
        }
        Ok(())
    })
    .map_err(|e| format!("shm buffer access error: {e:?}"))?
}

/// Copy cursor image data into a client's SHM buffer.
///
/// The xcursor pixel data is already in ARGB8888 format (B, G, R, A byte order
/// on little-endian), matching Wayland's `wl_shm::Format::Argb8888` directly.
/// If the cursor image is smaller than the buffer, the remaining pixels are
/// filled with transparent black. If larger, it is clipped.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn copy_cursor_to_shm(
    buffer: &WlBuffer,
    buf_info: &BufferData,
    cursor: &crate::cursor::CursorImageData,
    size: Size<i32, Physical>,
) -> Result<(), String> {
    let dst_stride = buf_info.stride as usize;
    let buf_w = size.w as usize;
    let buf_h = size.h as usize;
    let cur_w = cursor.width as usize;
    let cur_h = cursor.height as usize;
    let copy_w = buf_w.min(cur_w);
    let copy_h = buf_h.min(cur_h);

    with_buffer_contents_mut(buffer, |ptr, pool_len, data| {
        let offset = data.offset as usize;
        let needed = offset + buf_h * dst_stride;
        if needed > pool_len {
            return Err("shm pool too small for buffer".to_string());
        }

        // SAFETY: `ptr` is valid for `pool_len` bytes (guaranteed by smithay's
        // `with_buffer_contents_mut`), and we verified `needed <= pool_len`.
        #[allow(unsafe_code)]
        let dst = unsafe { std::slice::from_raw_parts_mut(ptr, pool_len) };

        // Clear the entire buffer to transparent first.
        dst[offset..offset + buf_h * dst_stride].fill(0);

        // Copy cursor pixels (already ARGB8888, no swizzle needed).
        let src_stride = cur_w * 4;
        for y in 0..copy_h {
            let src_start = y * src_stride;
            let dst_start = offset + y * dst_stride;
            let copy_bytes = copy_w * 4;
            dst[dst_start..dst_start + copy_bytes].copy_from_slice(&cursor.pixels[src_start..src_start + copy_bytes]);
        }
        Ok(())
    })
    .map_err(|e| format!("shm buffer access error: {e:?}"))?
}

/// Fill an SHM buffer with fully transparent pixels.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn fill_shm_transparent(buffer: &WlBuffer, buf_info: &BufferData, size: Size<i32, Physical>) -> Result<(), String> {
    let dst_stride = buf_info.stride as usize;
    let height = size.h as usize;

    with_buffer_contents_mut(buffer, |ptr, pool_len, data| {
        let offset = data.offset as usize;
        let needed = offset + height * dst_stride;
        if needed > pool_len {
            return Err("shm pool too small for buffer".to_string());
        }

        // SAFETY: `ptr` is valid for `pool_len` bytes (guaranteed by smithay's
        // `with_buffer_contents_mut`), and we verified `needed <= pool_len`.
        #[allow(unsafe_code)]
        let dst = unsafe { std::slice::from_raw_parts_mut(ptr, pool_len) };

        dst[offset..offset + height * dst_stride].fill(0);
        Ok(())
    })
    .map_err(|e| format!("shm buffer access error: {e:?}"))?
}
