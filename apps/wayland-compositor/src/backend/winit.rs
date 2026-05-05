//! Winit backend — nested compositor running in a window for development.
//!
//! Uses `smithay::backend::winit` with a GL renderer to display client surfaces
//! in a desktop window. Useful for interactive testing and debugging.
//!
//! The event loop is driven by two calloop ping sources:
//! - **event ping** — pumps winit events (`dispatch_new_events`). On
//!   `PumpStatus::Continue` it re-pings itself to keep pumping.
//! - **render ping** — renders one frame and submits it. Triggered on demand
//!   by visual-change events (resize, input, surface commit, redraw request),
//!   NOT every calloop iteration.
//!
//! Between pings the loop sleeps on the Wayland display fd, processing client
//! messages at full speed. This decouples protocol roundtrips from rendering,
//! which is critical for responsive resize/maximize of DMA-BUF clients.

use std::{cell::RefCell, rc::Rc, time::Duration};

use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, element::RenderElementStates, glow::GlowRenderer},
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    desktop::utils::OutputPresentationFeedback,
    reexports::{
        calloop::{EventLoop, ping},
        wayland_protocols::wp::presentation_time::server::wp_presentation_feedback,
        wayland_server::Display,
        winit::{
            platform::wayland::WindowAttributesExtWayland,
            window::{CursorIcon, Icon, Theme, WindowAttributes},
        },
    },
    utils::{Physical, Size},
    wayland::presentation::Refresh,
};

use crate::{CompositorArgs, config::CompositorConfig, state::State};

/// GL renders Y-up, Wayland is Y-down. `Flipped180` compensates.
const WINIT_RENDER_TRANSFORM: smithay::utils::Transform = smithay::utils::Transform::Flipped180;

/// Disable vsync by setting EGL swap interval to 0.
///
/// Smithay selects an EGL config that supports interval 0 (`vsync: false`)
/// but never actually calls `eglSwapInterval`. With the default interval of 1,
/// every `eglSwapBuffers` blocks for ~16 ms (one vsync period), which stalls
/// the calloop and delays client protocol roundtrips (e.g. the ~24
/// `wl_display.sync` calls Mesa Vulkan WSI performs during swapchain creation).
///
/// `eglSwapInterval` operates on the *current draw surface*, so we must first
/// make the context current with the backend's EGL surface. Mesa stores the
/// interval per-surface, so this call persists across later `eglMakeCurrent`.
#[allow(unsafe_code)]
fn disable_vsync(backend: &mut WinitGraphicsBackend<GlowRenderer>) {
    use smithay::backend::egl::ffi::egl;

    // Extract raw EGL handles sequentially to avoid simultaneous borrows.
    let display = backend.renderer().egl_context().display().get_display_handle();
    let ctx = backend.renderer().egl_context().get_context_handle();
    let surface = backend.egl_surface().get_surface_handle();

    // SAFETY: All three handles are owned by the backend and remain valid for
    // its lifetime. We are on the main thread (single-threaded calloop), so no
    // concurrent EGL access is possible.
    unsafe {
        // Make the surface current — eglSwapInterval requires a current draw surface.
        // Without this, Mesa returns EGL_BAD_SURFACE because GlesRenderer init
        // leaves the context current but without a draw surface.
        egl::MakeCurrent(**display, surface, surface, ctx);
        let result = egl::SwapInterval(**display, 0);
        if result == egl::TRUE {
            tracing::info!("eglSwapInterval set to 0 (vsync disabled)");
        } else {
            tracing::warn!("failed to set eglSwapInterval to 0, swap may block on vsync");
        }
    }
}

/// Shared mutable state accessed by both ping callbacks.
struct WinitData {
    backend: WinitGraphicsBackend<GlowRenderer>,
    damage_tracker: OutputDamageTracker,
}

/// Run the compositor in a winit window.
///
/// # Errors
///
/// Returns an error if winit initialization, socket creation, or runtime fails.
#[allow(clippy::too_many_lines)]
pub fn run(args: &CompositorArgs, config: CompositorConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new()?;
    let display: Display<State> = Display::new()?;

    let attributes = WindowAttributes::default()
        .with_title("PlatynUI Wayland Compositor")
        .with_name("org.platynui.compositor", "platynui-wayland-compositor")
        .with_theme(detect_system_theme())
        .with_window_icon(load_icon());
    let (mut backend, winit_evt): (WinitGraphicsBackend<GlowRenderer>, _) = winit::init_from_attributes(attributes)?;

    // Make eglSwapBuffers non-blocking so the calloop can process client
    // protocol roundtrips (e.g. Mesa Vulkan WSI sync requests) at full speed.
    disable_vsync(&mut backend);

    let (listening_socket, socket_name) = super::create_listening_socket(args)?;

    let output_size: Size<i32, Physical> = (args.width.cast_signed(), args.height.cast_signed()).into();
    let timeout = if args.timeout > 0 { Some(Duration::from_secs(args.timeout)) } else { None };

    let mut state = State::new(
        display.handle(),
        event_loop.handle(),
        event_loop.get_signal(),
        socket_name.clone(),
        output_size,
        timeout,
        crate::resolve_xkb_config(args),
        args.outputs,
        args.output_layout,
        args.scale,
        crate::security::SecurityPolicy::from_args(args.restrict_protocols.as_deref()),
        config,
    );
    state.backend_name = "winit";
    state.window_scale = args.window_scale;
    state.software_cursor = args.software_cursor;

    state.init_dmabuf_from_renderer(backend.renderer());

    match super::create_shared_glow_renderer(backend.renderer()) {
        Ok(renderer) => state.screenshot_renderer = Some(renderer),
        Err(err) => tracing::warn!(%err, "failed to create shared screenshot renderer, will use standalone"),
    }

    super::register_wayland_sources(&event_loop.handle(), display, listening_socket, &socket_name)?;
    let shutdown = super::setup_services(&event_loop.handle(), &mut state, args, timeout)?;

    let tracker_size = state.render_size();

    if tracker_size.w != output_size.w || tracker_size.h != output_size.h {
        use smithay::reexports::winit::dpi::PhysicalSize;
        let _ = backend
            .window()
            .request_inner_size(PhysicalSize::new(tracker_size.w.unsigned_abs(), tracker_size.h.unsigned_abs()));
        tracing::info!(
            width = tracker_size.w,
            height = tracker_size.h,
            window_scale = state.window_scale,
            outputs = state.outputs.len(),
            "resized winit window for multi-output/window-scale",
        );
    }

    let render_scale = state.max_output_scale() * state.window_scale;
    let damage_tracker = OutputDamageTracker::new(tracker_size, render_scale, WINIT_RENDER_TRANSFORM);

    let winit_data = Rc::new(RefCell::new(WinitData { backend, damage_tracker }));

    // Register the render ping in State so protocol handlers (e.g.
    // wl_surface.commit) can schedule a frame render on demand.
    let (render_ping, render_source) = ping::make_ping()?;
    state.render_ping = Some(render_ping.clone());

    // Render ping: render one frame.
    let winit_render = Rc::clone(&winit_data);
    event_loop.handle().insert_source(render_source, move |(), (), state| {
        let mut wd = winit_render.borrow_mut();
        render_frame(&mut wd, state);
    })?;

    // Register winit's event loop directly as a calloop source.  Smithay's
    // `WinitEventLoop` implements `EventSource` and exposes its internal
    // wake fd via `register`/`reregister`; calloop will only invoke the
    // callback when winit actually has events to deliver, so the loop
    // sleeps on epoll between events instead of busy-pumping
    // `pump_app_events(Duration::ZERO)` on every iteration.
    let winit_events = Rc::clone(&winit_data);
    let render_ping_for_events = render_ping.clone();
    event_loop.handle().insert_source(winit_evt, move |event, (), state| {
        let mut wd = winit_events.borrow_mut();
        process_winit_event(&mut wd, state, event, &render_ping_for_events);
    })?;

    tracing::info!(backend = "winit", socket = %socket_name, "event loop starting");

    // Per-iteration housekeeping: refresh the space, drain expired popups,
    // apply pending output reconfigurations, and flush queued client
    // protocol messages.  Runs after every event-loop iteration (after
    // any source has been dispatched) and before the loop goes back to
    // sleep on epoll.
    let winit_idle = Rc::clone(&winit_data);
    let render_ping_for_idle = render_ping.clone();
    event_loop.run(None, &mut state, move |state| {
        state.space.refresh();
        state.popup_manager.cleanup();

        if state.output_config_changed {
            let mut wd = winit_idle.borrow_mut();
            handle_output_config_change(&mut wd, state);
            render_ping_for_idle.ping();
        }

        if !state.running || shutdown.is_set() {
            state.loop_signal.stop();
            return;
        }
        let _ = state.display_handle.flush_clients();
    })?;

    tracing::info!("compositor shutting down");
    Ok(())
}

/// Process a single winit event.
fn process_winit_event(wd: &mut WinitData, state: &mut State, event: WinitEvent, render_ping: &ping::Ping) {
    match event {
        WinitEvent::Resized { size, .. } => {
            handle_resize(wd, state, size);
            render_ping.ping();
        }
        WinitEvent::Focus(focused) => {
            if !focused {
                crate::input::release_all_pressed_inputs(state);
            }
        }
        WinitEvent::Redraw => {
            render_ping.ping();
        }
        WinitEvent::Input(input) => {
            crate::input::process_input_event(state, input);
            render_ping.ping();
        }
        WinitEvent::CloseRequested => {
            state.running = false;
        }
    }
}

/// Handle a window resize event.
fn handle_resize(wd: &mut WinitData, state: &mut State, size: Size<i32, Physical>) {
    if state.outputs.len() > 1 {
        state.resize_edge_outputs(size);
    } else {
        let ws = state.window_scale.max(f64::EPSILON);
        #[allow(clippy::cast_possible_truncation)]
        let mode_size: Size<i32, Physical> =
            ((f64::from(size.w) / ws).round() as i32, (f64::from(size.h) / ws).round() as i32).into();
        let mode = smithay::output::Mode { size: mode_size, refresh: crate::state::DEFAULT_REFRESH_MHTZ };
        for old in state.output.modes() {
            if old != mode {
                state.output.delete_mode(old);
            }
        }
        state.output.change_current_state(Some(mode), None, None, None);
        state.output.set_preferred(mode);
    }
    state.reconfigure_windows_for_outputs();
    // Flush immediately so the configure reaches the client without waiting
    // for the next idle callback — reduces roundtrip latency during resize.
    let _ = state.display_handle.flush_clients();
    rebuild_damage_tracker(wd, state);
}

/// Handle wlr-output-management configuration changes.
fn handle_output_config_change(wd: &mut WinitData, state: &mut State) {
    state.output_config_changed = false;
    crate::handlers::output_management::notify_output_config_changed(state);
    state.reconfigure_windows_for_outputs();
    let _ = state.display_handle.flush_clients();

    let new_render = state.render_size();
    {
        use smithay::reexports::winit::dpi::PhysicalSize;
        let _ = wd
            .backend
            .window()
            .request_inner_size(PhysicalSize::new(new_render.w.unsigned_abs(), new_render.h.unsigned_abs()));
    }
    rebuild_damage_tracker(wd, state);

    tracing::debug!(w = new_render.w, h = new_render.h, "output configuration changed",);
}

/// Rebuild the damage tracker for the current render size and scale.
fn rebuild_damage_tracker(wd: &mut WinitData, state: &State) {
    let tracker_size = state.render_size();
    let render_scale = state.max_output_scale() * state.window_scale;
    wd.damage_tracker = OutputDamageTracker::new(tracker_size, render_scale, smithay::utils::Transform::Flipped180);
}

/// Make the EGL context current with the winit backend's surface.
///
/// `buffer_age()` requires the surface to be the current EGL draw surface.
/// Between frames, DMA-BUF import operations may call `make_current()` without
/// a surface, which unbinds ours. This re-establishes it.
#[allow(unsafe_code)]
fn make_egl_surface_current(backend: &mut WinitGraphicsBackend<GlowRenderer>) {
    // SAFETY: We need to work around the borrow checker — `renderer()` borrows
    // the backend mutably, but `egl_surface()` borrows it immutably, and we need
    // both at the same time. The EGL context and surface are valid for the
    // backend's lifetime and we are single-threaded.
    let display = backend.renderer().egl_context().display().get_display_handle();
    let ctx = backend.renderer().egl_context().get_context_handle();
    let surface_ptr = backend.egl_surface().get_surface_handle();
    unsafe {
        smithay::backend::egl::ffi::egl::MakeCurrent(**display, surface_ptr, surface_ptr, ctx);
    }
}

/// Render one frame into the winit window.
fn render_frame(wd: &mut WinitData, state: &mut State) {
    let output = state.output.clone();

    // Ensure the EGL surface is resized before querying buffer age. bind()
    // handles the resize but does not call eglMakeCurrent itself (that happens
    // inside render_output). We need the surface current so buffer_age()
    // doesn't fail with BAD_SURFACE.
    if wd.backend.bind().is_err() {
        tracing::warn!("failed to bind winit backend for rendering");
        return;
    }
    make_egl_surface_current(&mut wd.backend);
    let age = wd.backend.buffer_age().unwrap_or(0);

    let (damage, render_element_states) = {
        let Ok((renderer, mut framebuffer)) = wd.backend.bind() else {
            tracing::warn!("failed to bind winit backend for rendering");
            return;
        };

        let render_elements = crate::render::collect_render_elements(renderer, state, &output, state.software_cursor);

        match wd.damage_tracker.render_output(
            renderer,
            &mut framebuffer,
            age,
            &render_elements,
            crate::state::BACKGROUND_COLOR,
        ) {
            Ok(result) => (result.damage.cloned(), result.states),
            Err(err) => {
                tracing::warn!(%err, "render_output failed");
                (None, RenderElementStates::default())
            }
        }
    };

    if damage.is_some()
        && let Err(err) = wd.backend.submit(damage.as_deref())
    {
        tracing::warn!(%err, "failed to submit frame to winit backend");
    }

    state.update_primary_scanout_output(&output, &render_element_states);
    state.send_frame_callbacks(Duration::ZERO);

    if damage.is_some() {
        let mut feedback = OutputPresentationFeedback::new(&output);
        state.take_presentation_feedback(&output, &render_element_states, &mut feedback);
        let refresh = output.current_mode().map_or(Refresh::Unknown, |mode| {
            Refresh::Fixed(Duration::from_secs_f64(1_000.0 / f64::from(mode.refresh)))
        });
        feedback.presented::<_, smithay::utils::Monotonic>(
            state.clock.now(),
            refresh,
            0,
            wp_presentation_feedback::Kind::Vsync,
        );
    }

    state.send_dmabuf_feedback(&output, &render_element_states);
    update_cursor(&mut wd.backend, state);
}

/// Update the host cursor shape based on compositor and client state.
fn update_cursor(backend: &mut WinitGraphicsBackend<GlowRenderer>, state: &State) {
    let compositor_cursor = state.compositor_cursor_shape;
    if state.software_cursor {
        backend.window().set_cursor_visible(false);
    } else if compositor_cursor == crate::decorations::CursorShape::Default {
        use smithay::input::pointer::CursorImageStatus;
        match &state.cursor_status {
            CursorImageStatus::Named(icon) => {
                backend.window().set_cursor_visible(true);
                backend.window().set_cursor(*icon);
            }
            CursorImageStatus::Hidden | CursorImageStatus::Surface(_) => {
                backend.window().set_cursor_visible(false);
            }
        }
    } else {
        let icon = match compositor_cursor {
            crate::decorations::CursorShape::Default | crate::decorations::CursorShape::Move => CursorIcon::Default,
            crate::decorations::CursorShape::ResizeN => CursorIcon::NResize,
            crate::decorations::CursorShape::ResizeS => CursorIcon::SResize,
            crate::decorations::CursorShape::ResizeW => CursorIcon::WResize,
            crate::decorations::CursorShape::ResizeE => CursorIcon::EResize,
            crate::decorations::CursorShape::ResizeNw => CursorIcon::NwResize,
            crate::decorations::CursorShape::ResizeNe => CursorIcon::NeResize,
            crate::decorations::CursorShape::ResizeSw => CursorIcon::SwResize,
            crate::decorations::CursorShape::ResizeSe => CursorIcon::SeResize,
        };
        backend.window().set_cursor_visible(true);
        backend.window().set_cursor(icon);
    }
}

/// Load the embedded application icon as a winit [`Icon`].
fn load_icon() -> Option<Icon> {
    let png_bytes = include_bytes!("../../assets/icon.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    let mut reader = decoder.read_info().ok()?;
    let buf_size = reader.output_buffer_size()?;
    let mut buf = vec![0u8; buf_size];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    Icon::from_rgba(buf, info.width, info.height).ok()
}

/// Detect the system color scheme via the XDG Desktop Portal.
fn detect_system_theme() -> Option<Theme> {
    let connection = zbus::blocking::Connection::session().ok()?;
    let reply = connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "Read",
            &("org.freedesktop.appearance", "color-scheme"),
        )
        .ok()?;

    let body = reply.body();
    let outer: zbus::zvariant::Value<'_> = body.deserialize().ok()?;
    let inner: zbus::zvariant::Value<'_> = outer.downcast_ref().ok()?;
    let scheme: u32 = inner.downcast_ref().ok()?;

    // XDG Portal color-scheme: 0 = no preference, 1 = dark, 2 = light
    match scheme {
        1 => Some(Theme::Dark),
        2 => Some(Theme::Light),
        _ => None,
    }
}
