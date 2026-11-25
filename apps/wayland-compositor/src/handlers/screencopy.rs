//! Screen capture (`ext-image-capture-source-v1` + `ext-image-copy-capture-v1`).
//!
//! Lets tools like `grim`, `wayvnc` and `wl-mirror` capture an output, a
//! single toplevel, or the cursor image. Smithay implements both protocols —
//! sessions, frames, buffer validation against the announced constraints, the
//! protocol errors. This module supplies what smithay leaves to the
//! compositor:
//!
//! - **Sources.** An output source remembers its output; a toplevel source
//!   remembers its `ext-foreign-toplevel-list` handle and is resolved to the
//!   window at capture time.
//! - **Constraints.** `wl_shm` ARGB8888/XRGB8888 at the output's mode size, or
//!   at the toplevel's window geometry in physical pixels. No dmabuf.
//! - **Session lifetime.** Sessions are kept here; the sessions of a toplevel
//!   or output are stopped when it goes away ([`stop_sessions_for_toplevel`],
//!   [`stop_sessions_for_output`]).
//! - **Rendering.** An output frame is the output's scene (with the cursor if
//!   the session asked for it); a toplevel frame is only that window's own
//!   surfaces, with its geometry at the buffer origin. A cursor session gets
//!   the cursor image and its hotspot.
//!
//! The three globals are hidden from every client under
//! `--restrict-protocols` (filter passed to [`ScreencopyState::new`]).

use smithay::{
    backend::renderer::{element::AsRenderElements, element::surface::WaylandSurfaceRenderElement, glow::GlowRenderer},
    desktop::Window,
    output::{Output, WeakOutput},
    reexports::wayland_server::{Client, DisplayHandle, protocol::wl_buffer::WlBuffer, protocol::wl_shm},
    utils::{Physical, Point, Rectangle, Size, Transform},
    wayland::{
        foreign_toplevel_list::ForeignToplevelHandle,
        image_capture_source::{
            ImageCaptureSource, ImageCaptureSourceHandler, OutputCaptureSourceHandler, OutputCaptureSourceState,
            ToplevelCaptureSourceHandler, ToplevelCaptureSourceState,
        },
        image_copy_capture::{
            BufferConstraints, CaptureFailureReason, CursorSession, CursorSessionRef, Frame, ImageCopyCaptureHandler,
            ImageCopyCaptureState, Session, SessionRef,
        },
        shm::{BufferData, with_buffer_contents_mut},
    },
};

use crate::state::State;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// The capture protocol states and the live sessions.
pub struct ScreencopyState {
    pub output_sources: OutputCaptureSourceState,
    pub toplevel_sources: ToplevelCaptureSourceState,
    pub copy_capture: ImageCopyCaptureState,
    /// Live sessions; a session sends `stopped` when it is dropped.
    sessions: Vec<Session>,
    cursor_sessions: Vec<CursorSession>,
}

impl ScreencopyState {
    /// Create the three capture globals, visible to the clients `filter` accepts.
    pub fn new(dh: &DisplayHandle, filter: impl Fn(&Client) -> bool + Send + Sync + Clone + 'static) -> Self {
        Self {
            output_sources: OutputCaptureSourceState::new_with_filter::<State, _>(dh, filter.clone()),
            toplevel_sources: ToplevelCaptureSourceState::new_with_filter::<State, _>(dh, filter.clone()),
            copy_capture: ImageCopyCaptureState::new_with_filter::<State, _>(dh, filter),
            sessions: Vec::new(),
            cursor_sessions: Vec::new(),
        }
    }
}

/// What a capture source shows, stored in the source's user data.
#[derive(Clone, Debug)]
enum CaptureTarget {
    Output(WeakOutput),
    Toplevel(ForeignToplevelHandle),
}

/// A capture target resolved to what is live right now.
enum Resolved {
    Output(Output),
    Toplevel(Window),
}

fn target(source: &ImageCaptureSource) -> Option<CaptureTarget> {
    source.user_data().get::<CaptureTarget>().cloned()
}

fn same_toplevel(a: &ForeignToplevelHandle, b: &ForeignToplevelHandle) -> bool {
    a.identifier() == b.identifier()
}

fn resolve(state: &State, target: &CaptureTarget) -> Option<Resolved> {
    match target {
        CaptureTarget::Output(output) => output.upgrade().map(Resolved::Output),
        CaptureTarget::Toplevel(handle) => state
            .ext_toplevel_handles
            .iter()
            .find(|(_, h)| same_toplevel(h, handle))
            .map(|(window, _)| Resolved::Toplevel(window.clone())),
    }
}

/// The buffer size a target is captured at, in physical pixels.
#[allow(clippy::cast_possible_truncation)]
fn capture_size(state: &State, target: &Resolved) -> Size<i32, Physical> {
    match target {
        Resolved::Output(output) => output.current_mode().map_or_else(|| (1, 1).into(), |mode| mode.size),
        Resolved::Toplevel(window) => {
            let geo = window.geometry();
            let scale = toplevel_scale(state);
            let w = (f64::from(geo.size.w) * scale).ceil() as i32;
            let h = (f64::from(geo.size.h) * scale).ceil() as i32;
            (w.max(1), h.max(1)).into()
        }
    }
}

fn toplevel_scale(state: &State) -> f64 {
    state.outputs.first().map_or(1.0, |o| o.current_scale().fractional_scale())
}

fn shm_constraints(size: Size<i32, Physical>) -> BufferConstraints {
    BufferConstraints {
        size: (size.w, size.h).into(),
        shm: vec![wl_shm::Format::Argb8888, wl_shm::Format::Xrgb8888],
        dma: None,
    }
}

/// Stop every session (and cursor session) whose source matches `pred`.
fn stop_sessions_where(state: &mut State, pred: impl Fn(&CaptureTarget) -> bool) {
    let matches = |source: &ImageCaptureSource| target(source).is_some_and(|t| pred(&t));
    let screencopy = &mut state.screencopy;
    let (stopped, kept): (Vec<_>, Vec<_>) =
        std::mem::take(&mut screencopy.sessions).into_iter().partition(|s| matches(&s.source()));
    screencopy.sessions = kept;
    let (stopped_cursors, kept_cursors): (Vec<_>, Vec<_>) =
        std::mem::take(&mut screencopy.cursor_sessions).into_iter().partition(|s| matches(&s.source()));
    screencopy.cursor_sessions = kept_cursors;
    if !stopped.is_empty() || !stopped_cursors.is_empty() {
        tracing::debug!(
            sessions = stopped.len(),
            cursor_sessions = stopped_cursors.len(),
            "screencopy: source gone, stopping sessions"
        );
    }
    stopped.into_iter().for_each(Session::stop);
    stopped_cursors.into_iter().for_each(CursorSession::stop);
}

/// Stop the capture sessions of a toplevel that is going away.
pub fn stop_sessions_for_toplevel(state: &mut State, handle: &ForeignToplevelHandle) {
    stop_sessions_where(state, |t| matches!(t, CaptureTarget::Toplevel(h) if same_toplevel(h, handle)));
}

/// Stop the capture sessions of an output that is going away.
pub fn stop_sessions_for_output(state: &mut State, output: &Output) {
    stop_sessions_where(state, |t| matches!(t, CaptureTarget::Output(o) if o.upgrade().as_ref() == Some(output)));
}

// ---------------------------------------------------------------------------
// Protocol handlers
// ---------------------------------------------------------------------------

impl ImageCaptureSourceHandler for State {}

impl OutputCaptureSourceHandler for State {
    fn output_capture_source_state(&mut self) -> &mut OutputCaptureSourceState {
        &mut self.screencopy.output_sources
    }

    fn output_source_created(&mut self, source: ImageCaptureSource, output: &Output) {
        source.user_data().insert_if_missing(|| CaptureTarget::Output(output.downgrade()));
    }
}

impl ToplevelCaptureSourceHandler for State {
    fn toplevel_capture_source_state(&mut self) -> &mut ToplevelCaptureSourceState {
        &mut self.screencopy.toplevel_sources
    }

    fn toplevel_source_created(&mut self, source: ImageCaptureSource, toplevel: ForeignToplevelHandle) {
        source.user_data().insert_if_missing(|| CaptureTarget::Toplevel(toplevel));
    }
}

impl ImageCopyCaptureHandler for State {
    fn image_copy_capture_state(&mut self) -> &mut ImageCopyCaptureState {
        &mut self.screencopy.copy_capture
    }

    fn capture_constraints(&mut self, source: &ImageCaptureSource) -> Option<BufferConstraints> {
        let resolved = resolve(self, &target(source)?)?;
        Some(shm_constraints(capture_size(self, &resolved)))
    }

    fn cursor_capture_constraints(
        &mut self,
        source: &ImageCaptureSource,
        _pointer: &smithay::reexports::wayland_server::protocol::wl_pointer::WlPointer,
    ) -> Option<BufferConstraints> {
        resolve(self, &target(source)?)?;
        let (w, h) = self.cursor_theme.default_cursor_dimensions();
        Some(shm_constraints((w.cast_signed(), h.cast_signed()).into()))
    }

    fn new_session(&mut self, session: Session) {
        tracing::debug!(paint_cursors = session.draw_cursor(), "screencopy: session created");
        self.screencopy.sessions.push(session);
    }

    fn new_cursor_session(&mut self, session: CursorSession) {
        // Announce the cursor right away: its hotspot, and its position on the source.
        let hotspot = default_cursor_hotspot(self);
        session.set_cursor_hotspot(hotspot);
        let position = cursor_position_on(self, &session.source());
        session.set_cursor_pos(position);
        tracing::debug!(?position, "screencopy: cursor session created");
        self.screencopy.cursor_sessions.push(session);
    }

    fn frame(&mut self, session: &SessionRef, frame: Frame) {
        capture_frame(self, session, frame);
    }

    fn cursor_frame(&mut self, session: &CursorSessionRef, frame: Frame) {
        capture_cursor_frame(self, session, frame);
    }

    fn session_destroyed(&mut self, session: SessionRef) {
        self.screencopy.sessions.retain(|s| *s != session);
    }

    fn cursor_session_destroyed(&mut self, session: CursorSessionRef) {
        self.screencopy.cursor_sessions.retain(|s| *s != session);
    }
}

// ---------------------------------------------------------------------------
// Capture
// ---------------------------------------------------------------------------

/// Damage covering the whole frame: every capture redraws the full buffer.
fn full_damage(size: Size<i32, Physical>) -> Vec<Rectangle<i32, smithay::utils::Buffer>> {
    vec![Rectangle::from_size((size.w, size.h).into())]
}

/// The shm layout of a frame's buffer (smithay has already checked it against
/// the session's constraints).
fn shm_info(buffer: &WlBuffer) -> Option<BufferData> {
    with_buffer_contents_mut(buffer, |_ptr, _len, data| data).ok()
}

fn capture_frame(state: &mut State, session: &SessionRef, frame: Frame) {
    let Some(resolved) = target(&session.source()).and_then(|t| resolve(state, &t)) else {
        frame.fail(CaptureFailureReason::Stopped);
        return;
    };
    let size = capture_size(state, &resolved);
    let buffer = frame.buffer();
    let Some(info) = shm_info(&buffer) else {
        frame.fail(CaptureFailureReason::BufferConstraints);
        return;
    };
    if info.width != size.w || info.height != size.h {
        // The source changed size since the constraints were sent.
        session.update_constraints(shm_constraints(size));
        frame.fail(CaptureFailureReason::BufferConstraints);
        return;
    }

    let pixels = match &resolved {
        Resolved::Output(output) => render_output_to_pixels(state, output, size, session.draw_cursor()),
        Resolved::Toplevel(window) => render_window_to_pixels(state, window, size),
    };
    let result = pixels.and_then(|pixels| copy_pixels_to_shm(&buffer, &info, &pixels, size));
    match result {
        Ok(()) => frame.success(Transform::Normal, full_damage(size), state.frame_clock_now()),
        Err(err) => {
            tracing::warn!(err, "screencopy: capture failed");
            frame.fail(CaptureFailureReason::Unknown);
        }
    }
}

fn with_offscreen_renderer<T>(
    state: &mut State,
    f: impl FnOnce(&mut GlowRenderer, &mut State) -> Result<T, String>,
) -> Result<T, String> {
    if state.screenshot_renderer.is_none() {
        state.screenshot_renderer = Some(
            crate::backend::create_offscreen_glow_renderer()
                .map_err(|e| format!("failed to create offscreen renderer: {e}"))?,
        );
    }
    let mut renderer = state.screenshot_renderer.take().expect("screenshot renderer was just initialized above");
    let result = f(&mut renderer, state);
    state.screenshot_renderer = Some(renderer);
    result
}

/// Render an output's scene (RGBA8888 byte order).
fn render_output_to_pixels(
    state: &mut State,
    output: &Output,
    size: Size<i32, Physical>,
    paint_cursors: bool,
) -> Result<Vec<u8>, String> {
    let scale = output.current_scale().fractional_scale();
    let output = output.clone();
    with_offscreen_renderer(state, |renderer, state| {
        crate::render::render_to_pixels(renderer, state, &output, size, scale, paint_cursors)
    })
}

/// Render only `window`'s own surfaces, with its geometry at the buffer
/// origin (RGBA8888 byte order). The server-side titlebar and every other
/// window are left out.
fn render_window_to_pixels(state: &mut State, window: &Window, size: Size<i32, Physical>) -> Result<Vec<u8>, String> {
    use smithay::backend::allocator::Fourcc as DrmFourcc;
    use smithay::backend::renderer::damage::OutputDamageTracker;
    use smithay::backend::renderer::gles::GlesTexture;
    use smithay::backend::renderer::{Bind, ExportMem, Offscreen};

    let scale = toplevel_scale(state);
    let window = window.clone();
    with_offscreen_renderer(state, |renderer, _state| {
        let buffer_size: Size<i32, smithay::utils::Buffer> = (size.w, size.h).into();
        let mut texture: GlesTexture =
            renderer.create_buffer(DrmFourcc::Abgr8888, buffer_size).map_err(|e| format!("create_buffer: {e}"))?;
        let mut framebuffer = renderer.bind(&mut texture).map_err(|e| format!("bind: {e}"))?;

        let geometry_loc = window.geometry().loc;
        let origin: Point<i32, Physical> =
            Point::<i32, smithay::utils::Logical>::from((-geometry_loc.x, -geometry_loc.y))
                .to_physical_precise_round(scale);
        let surfaces: Vec<WaylandSurfaceRenderElement<GlowRenderer>> =
            window.render_elements(renderer, origin, scale.into(), 1.0);
        let elements: Vec<crate::render::CompositorRenderElement> =
            surfaces.into_iter().map(crate::render::CompositorRenderElement::Surface).collect();

        let mut damage_tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
        damage_tracker
            .render_output(renderer, &mut framebuffer, 0, &elements, [0.0, 0.0, 0.0, 0.0])
            .map_err(|e| format!("render_output: {e}"))?;

        let mapping = renderer
            .copy_framebuffer(&framebuffer, Rectangle::from_size(buffer_size), DrmFourcc::Abgr8888)
            .map_err(|e| format!("copy_framebuffer: {e}"))?;
        let pixels = renderer.map_texture(&mapping).map_err(|e| format!("map_texture: {e}"))?;
        Ok(pixels.to_vec())
    })
}

// ---------------------------------------------------------------------------
// Cursor capture
// ---------------------------------------------------------------------------

fn default_cursor_hotspot(state: &mut State) -> Point<i32, smithay::utils::Buffer> {
    let time = state.start_time.elapsed();
    state
        .cursor_theme
        .get_cursor_data(smithay::input::pointer::CursorIcon::Default, 1, time)
        .map_or_else(|| (0, 0).into(), |data| (data.xhot.cast_signed(), data.yhot.cast_signed()).into())
}

/// The pointer position relative to a source, if the pointer is on it.
#[allow(clippy::cast_possible_truncation)]
fn cursor_position_on(state: &State, source: &ImageCaptureSource) -> Option<Point<i32, smithay::utils::Buffer>> {
    let pointer = state.pointer_location;
    let area = match resolve(state, &target(source)?)? {
        Resolved::Output(output) => state.space.output_geometry(&output)?,
        Resolved::Toplevel(window) => {
            let loc = state.space.element_location(&window)?;
            Rectangle::new(loc, window.geometry().size)
        }
    };
    let inside = area.to_f64().contains(pointer);
    inside.then(|| ((pointer.x - f64::from(area.loc.x)) as i32, (pointer.y - f64::from(area.loc.y)) as i32).into())
}

/// Capture the cursor image into a cursor session's frame.
fn capture_cursor_frame(state: &mut State, session: &CursorSessionRef, frame: Frame) {
    use smithay::input::pointer::CursorImageStatus;

    let Some(constraints) = session.current_constraints() else {
        frame.fail(CaptureFailureReason::Stopped);
        return;
    };
    let size: Size<i32, Physical> = (constraints.size.w, constraints.size.h).into();
    let buffer = frame.buffer();
    let Some(info) = shm_info(&buffer) else {
        frame.fail(CaptureFailureReason::BufferConstraints);
        return;
    };
    session.set_cursor_pos(cursor_position_on(state, &session.source()));

    // The compositor overrides the client cursor for SSD interactions (resize
    // borders, move grab), as the main render pipeline does.
    let compositor_override = crate::render::compositor_cursor_icon(state.compositor_cursor_shape);
    let icon = match (compositor_override, &state.cursor_status) {
        (Some(icon), _) => icon,
        (None, CursorImageStatus::Named(icon)) => *icon,
        (None, CursorImageStatus::Surface(surface)) => {
            // Surface cursors (e.g. set by X11 clients through XWayland):
            // render the cursor surface itself.
            use smithay::input::pointer::CursorImageSurfaceData;
            let surface = surface.clone();
            let hotspot = smithay::wayland::compositor::with_states(&surface, |states| {
                states
                    .data_map
                    .get::<CursorImageSurfaceData>()
                    .map_or_default(|d| d.lock().expect("mutex poisoned").hotspot)
            });
            let result = match render_cursor_surface_to_pixels(state, &surface, size) {
                Ok(pixels) => copy_pixels_to_shm(&buffer, &info, &pixels, size),
                Err(err) => {
                    tracing::debug!(err, "screencopy: cursor surface render failed, filling transparent");
                    fill_shm_transparent(&buffer, &info, size)
                }
            };
            session.set_cursor_hotspot((hotspot.x, hotspot.y));
            return finish(state, frame, size, result);
        }
        (None, CursorImageStatus::Hidden) => {
            session.set_cursor_pos(None);
            let result = fill_shm_transparent(&buffer, &info, size);
            return finish(state, frame, size, result);
        }
    };

    let time = state.start_time.elapsed();
    let result = if let Some(cursor) = state.cursor_theme.get_cursor_data(icon, 1, time) {
        session.set_cursor_hotspot((cursor.xhot.cast_signed(), cursor.yhot.cast_signed()));
        copy_cursor_to_shm(&buffer, &info, &cursor, size)
    } else {
        tracing::debug!(?icon, "screencopy: cursor icon not available");
        fill_shm_transparent(&buffer, &info, size)
    };
    finish(state, frame, size, result);
}

fn finish(state: &State, frame: Frame, size: Size<i32, Physical>, result: Result<(), String>) {
    match result {
        Ok(()) => frame.success(Transform::Normal, full_damage(size), state.frame_clock_now()),
        Err(err) => {
            tracing::warn!(err, "screencopy: cursor capture failed");
            frame.fail(CaptureFailureReason::Unknown);
        }
    }
}

/// Render a cursor surface (e.g. from `XWayland` clients) into a pixel buffer.
///
/// Uses the shared offscreen GL renderer to composit the surface tree into a
/// small texture, then reads back the pixels in ABGR8888 format (same as
/// [`render_source_to_pixels`]).
fn render_cursor_surface_to_pixels(
    state: &mut State,
    surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    size: Size<i32, Physical>,
) -> Result<Vec<u8>, String> {
    use smithay::backend::allocator::Fourcc as DrmFourcc;
    use smithay::backend::renderer::damage::OutputDamageTracker;
    use smithay::backend::renderer::element::surface::{
        WaylandSurfaceRenderElement, render_elements_from_surface_tree,
    };
    use smithay::backend::renderer::gles::GlesTexture;
    use smithay::backend::renderer::glow::GlowRenderer;
    use smithay::backend::renderer::{Bind, ExportMem, Offscreen};
    use smithay::utils::Scale;

    // Lazily initialize the offscreen renderer (shared with screenshot capture).
    if state.screenshot_renderer.is_none() {
        state.screenshot_renderer = Some(
            crate::backend::create_offscreen_glow_renderer()
                .map_err(|e| format!("failed to create offscreen renderer: {e}"))?,
        );
    }

    let mut renderer = state.screenshot_renderer.take().expect("screenshot renderer was just initialized above");

    let buffer_size: Size<i32, smithay::utils::Buffer> = (size.w, size.h).into();

    let mut texture: GlesTexture =
        renderer.create_buffer(DrmFourcc::Abgr8888, buffer_size).map_err(|e| format!("create_buffer: {e}"))?;
    let mut framebuffer = renderer.bind(&mut texture).map_err(|e| format!("bind: {e}"))?;

    // Render the cursor surface at origin with scale 1.
    let scale = Scale::from(1.0);
    let cursor_elements: Vec<WaylandSurfaceRenderElement<GlowRenderer>> = render_elements_from_surface_tree(
        &mut renderer,
        surface,
        (0, 0),
        scale,
        1.0,
        smithay::backend::renderer::element::Kind::Cursor,
    );

    let elements: Vec<crate::render::CompositorRenderElement> =
        cursor_elements.into_iter().map(crate::render::CompositorRenderElement::Surface).collect();

    // Render into the offscreen texture with transparent clear colour.
    let mut damage_tracker = OutputDamageTracker::new(size, 1.0, Transform::Normal);
    damage_tracker
        .render_output(&mut renderer, &mut framebuffer, 0, &elements, [0.0, 0.0, 0.0, 0.0])
        .map_err(|e| format!("render_output: {e}"))?;

    // Read back rendered pixels.
    let region = Rectangle::from_size(buffer_size);
    let mapping = renderer
        .copy_framebuffer(&framebuffer, region, DrmFourcc::Abgr8888)
        .map_err(|e| format!("copy_framebuffer: {e}"))?;
    let pixel_data = renderer.map_texture(&mapping).map_err(|e| format!("map_texture: {e}"))?;

    state.screenshot_renderer = Some(renderer);
    Ok(pixel_data.to_vec())
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
