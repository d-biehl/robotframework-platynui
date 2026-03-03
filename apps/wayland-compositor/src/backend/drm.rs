//! DRM backend — direct hardware rendering on a TTY (no display server).
//!
//! Uses libseat for session management and privilege escalation, DRM/KMS for
//! display output, and libinput for input devices.
//! This is the production backend for running on bare metal (no nested compositor).
//!
//! ## Pipeline
//!
//! 1. Open a session via `LibSeatSession` (handles seatd/logind privilege escalation)
//! 2. Discover GPUs via `UdevBackend` (device hotplug events)
//! 3. For each GPU: enumerate connectors, find connected ones, pick the best mode
//! 4. Create `DrmSurface` → `GbmAllocator` → `DrmCompositor` per output
//! 5. Render via `GlowRenderer` (EGL on GBM) into GPU-backed buffers
//! 6. `DrmCompositor::render_frame()` → `queue_frame()` → `VBlank` → `frame_submitted()`
//! 7. VT-switching pauses/resumes the DRM device and input
//!
//! For environments without a dedicated GPU, set `LIBGL_ALWAYS_SOFTWARE=1` to
//! use Mesa's software renderer (llvmpipe).
//!
//! Requires the `backend-drm` Cargo feature.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

/// Event loop dispatch timeout — one frame period at ~60 FPS.
const FRAME_DISPATCH_TIMEOUT: Duration = Duration::from_millis(16);
/// Linux `ENODEV` errno value, returned when an input device cannot be opened.
const ENODEV: i32 = 19;

use smithay::{
    backend::{
        allocator::{
            Format as DrmFormat, Fourcc as DrmFourcc, Modifier as DrmModifier,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        drm::{
            DrmDevice, DrmDeviceFd, DrmEvent, DrmNode, DrmSurface,
            compositor::{DrmCompositor, FrameFlags},
            exporter::gbm::GbmFramebufferExporter,
        },
        libinput::LibinputInputBackend,
        renderer::glow::GlowRenderer,
        session::{Event as SessionEvent, Session, libseat::LibSeatSession},
        udev::{UdevBackend, UdevEvent},
    },
    output::{Mode as OutputMode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::EventLoop,
        drm::control::{self, Device as ControlDevice, connector, crtc},
        input::Libinput,
        wayland_server::Display,
    },
    utils::{Buffer as BufferCoords, Physical, Size},
};

use crate::{CompositorArgs, config::CompositorConfig, state::State};

/// Per-output rendering state for a DRM connector.
pub struct DrmOutputState {
    /// The DRM compositor that manages scanout for this output.
    pub(crate) drm_compositor:
        DrmCompositor<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>,
    /// The Smithay output object for this connector.
    pub(crate) output: Output,
    /// Whether a frame has been queued and we're waiting for `VBlank`.
    pub(crate) pending_frame: bool,
}

/// Per-GPU rendering state.
pub struct DrmBackendState {
    /// DRM device (retained for lifetime + VT switching).
    pub(crate) _drm_device: DrmDevice,
    /// Per-output rendering state, keyed by CRTC handle.
    pub(crate) outputs: HashMap<crtc::Handle, DrmOutputState>,
    /// `GlowRenderer` (EGL on GBM — GPU-accelerated or Mesa llvmpipe).
    pub(crate) renderer: GlowRenderer,
    /// Whether the session is currently active (false when VT-switched away).
    pub(crate) session_active: bool,
}

/// Run the compositor on real hardware using DRM/KMS.
///
/// # Errors
///
/// Returns an error if session, device, or event loop initialization fails.
#[allow(clippy::too_many_lines)]
pub fn run(args: &CompositorArgs, config: CompositorConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new()?;
    let display: Display<State> = Display::new()?;

    // Initialize the session (libseat handles privilege escalation for DRM/input).
    // This requires either a running seatd daemon or logind/elogind, and must
    // be invoked from a real TTY (not from within another graphical session).
    let (mut session, notifier) = LibSeatSession::new().map_err(|err| {
        format!(
            "Failed to open session: {err}\n\n\
             The DRM backend requires a seat manager (logind, elogind, or seatd) and \
             must be started from a real TTY (e.g. Ctrl+Alt+F2), not from within \
             another graphical session.\n\
             Hints:\n\
             - Switch to a TTY with Ctrl+Alt+F2 and run from there\n\
             - Ensure systemd-logind, elogind, or seatd is running\n\
             - For nested testing, use --backend winit instead"
        )
    })?;
    tracing::info!(seat = session.seat(), "libseat session opened");

    let (listening_socket, socket_name) = super::create_listening_socket(args)?;

    // Use a default output size — actual size comes from connected monitors
    let default_size: Size<i32, Physical> = (args.width.cast_signed(), args.height.cast_signed()).into();
    let timeout = if args.timeout > 0 { Some(Duration::from_secs(args.timeout)) } else { None };

    let mut state = State::new(
        display.handle(),
        event_loop.handle(),
        event_loop.get_signal(),
        socket_name.clone(),
        default_size,
        timeout,
        crate::resolve_xkb_config(args),
        args.outputs,
        args.output_layout,
        args.scale,
        crate::security::SecurityPolicy::from_args(args.restrict_protocols.as_deref()),
        config,
    );
    state.backend_name = "drm";

    // Register Wayland display + listening socket + set WAYLAND_DISPLAY
    super::register_wayland_sources(&event_loop.handle(), display, listening_socket, &socket_name)?;

    // Initialize libinput for keyboard/mouse/touch input
    let mut libinput_context = Libinput::new_with_udev(LibseatInterface(session.clone()));
    libinput_context.udev_assign_seat(&session.seat()).map_err(|()| "failed to assign libinput seat")?;

    let libinput_backend = LibinputInputBackend::new(libinput_context);
    event_loop.handle().insert_source(libinput_backend, |event, (), state| {
        crate::input::process_input_event(state, event);
    })?;

    // Register session notifier for VT switching
    event_loop.handle().insert_source(notifier, |event, (), state| match event {
        SessionEvent::PauseSession => {
            tracing::info!("session paused (VT switch away)");
            if let Some(ref mut backend) = state.drm_backend {
                backend.session_active = false;
            }
        }
        SessionEvent::ActivateSession => {
            tracing::info!("session activated (VT switch back)");
            if let Some(ref mut backend) = state.drm_backend {
                backend.session_active = true;
            }
        }
    })?;

    // Discover GPUs via udev
    let udev_backend = UdevBackend::new(session.seat())?;

    // Process initially connected devices (use the first usable GPU)
    for (device_id, path) in udev_backend.device_list() {
        tracing::info!(?device_id, ?path, "discovered GPU device");
        if state.drm_backend.is_none() {
            match initialize_drm_device(&mut session, &event_loop, &state, path) {
                Ok(backend_state) => {
                    // Map DRM-discovered outputs side-by-side (horizontal layout).
                    // Each output is placed to the right of the previous one.
                    let mut next_x: i32 = 0;
                    for output_state in backend_state.outputs.values() {
                        // Set the protocol-level position so clients know the
                        // output location in global coordinate space.
                        output_state.output.change_current_state(None, None, None, Some((next_x, 0).into()));
                        state.space.map_output(&output_state.output, (next_x, 0));
                        state.outputs.push(output_state.output.clone());

                        // Advance the x position by this output's logical width.
                        let mode = output_state.output.current_mode().unwrap_or(smithay::output::Mode {
                            size: (1920, 1080).into(),
                            refresh: crate::state::DEFAULT_REFRESH_MHTZ,
                        });
                        let scale = output_state.output.current_scale().fractional_scale();
                        #[allow(clippy::cast_possible_truncation)]
                        let logical_w = (f64::from(mode.size.w) / scale).round() as i32;
                        next_x += logical_w;
                    }
                    if let Some(first) = backend_state.outputs.values().next() {
                        state.output = first.output.clone();
                    }
                    // Pre-initialize the screenshot renderer with a shared EGL
                    // context so screenshots see the main renderer's GL objects.
                    match super::create_shared_glow_renderer(&backend_state.renderer) {
                        Ok(r) => state.screenshot_renderer = Some(r),
                        Err(err) => tracing::warn!(%err, "failed to create shared screenshot renderer"),
                    }
                    state.drm_backend = Some(backend_state);
                }
                Err(err) => {
                    tracing::warn!(%err, ?path, "failed to initialize DRM device");
                }
            }
        }
    }

    // Watch for hotplug events
    event_loop.handle().insert_source(udev_backend, |event, (), _state| match event {
        UdevEvent::Added { device_id, path } => {
            tracing::info!(?device_id, ?path, "GPU device added (hotplug)");
        }
        UdevEvent::Changed { device_id } => {
            tracing::debug!(?device_id, "GPU device changed");
        }
        UdevEvent::Removed { device_id } => {
            tracing::info!(?device_id, "GPU device removed");
        }
    })?;

    // Register signal handlers, watchdog, XWayland, control socket, readiness
    let shutdown = super::setup_services(&event_loop.handle(), &mut state, args, timeout)?;

    tracing::info!(backend = "drm", socket = %socket_name, "event loop starting");

    // Main event loop
    while state.running && !shutdown.is_set() {
        event_loop.dispatch(Some(FRAME_DISPATCH_TIMEOUT), &mut state)?;

        // Handle output configuration changes from wlr-output-management.
        // Reconfigure maximized/fullscreen windows for new logical dimensions.
        // NOTE: DRM mode changes would need hardware reconfiguration — not yet
        // implemented; only scale/position changes take effect immediately.
        if state.output_config_changed {
            state.output_config_changed = false;

            // Notify output management clients (e.g. kanshi) about the change.
            crate::handlers::output_management::notify_output_config_changed(&mut state);

            state.reconfigure_windows_for_outputs();
        }

        // Render on each DRM output (only when the session is active).
        // We temporarily take the backend out of state to avoid a double
        // mutable borrow (render_drm_outputs needs &mut state for the space).
        if let Some(mut backend) = state.drm_backend.take() {
            if backend.session_active {
                render_drm_outputs(&mut backend, &mut state);
            }
            state.drm_backend = Some(backend);
        }

        state.send_frame_callbacks();
        state.flush_and_refresh();
    }

    tracing::info!("compositor shutting down");
    Ok(())
}

/// Render one frame on each DRM output.
///
/// For each CRTC with a `DrmCompositor`, renders all compositor elements
/// (windows + decorations), then queues the frame for scanout.  Frames are
/// skipped when a previous frame is still pending (waiting for `VBlank`).
fn render_drm_outputs(backend: &mut DrmBackendState, state: &mut State) {
    let crtc_handles: Vec<crtc::Handle> = backend.outputs.keys().copied().collect();

    // Lazy-init the glow-based titlebar painter on the first frame.
    if !state.titlebar_renderer.is_glow_initialized() {
        state.titlebar_renderer.init_glow(&mut backend.renderer);
    }

    for crtc in crtc_handles {
        let Some(output_state) = backend.outputs.get_mut(&crtc) else {
            continue;
        };

        // Skip if we're still waiting for VBlank on a previous frame
        if output_state.pending_frame {
            continue;
        }

        let output = output_state.output.clone();
        let render_elements = crate::render::collect_render_elements(&mut backend.renderer, state, &output, true);

        match output_state.drm_compositor.render_frame::<_, _>(
            &mut backend.renderer,
            &render_elements,
            crate::state::BACKGROUND_COLOR,
            FrameFlags::DEFAULT,
        ) {
            Ok(result) => {
                if !result.is_empty {
                    if let Err(err) = output_state.drm_compositor.queue_frame(()) {
                        tracing::warn!(%err, ?crtc, "failed to queue DRM frame");
                    } else {
                        output_state.pending_frame = true;
                    }
                }
            }
            Err(err) => {
                tracing::warn!(?err, ?crtc, "DRM render_frame failed");
            }
        }
    }
}

/// Wrapper for libinput to use libseat for device access.
struct LibseatInterface(LibSeatSession);

impl ::smithay::reexports::input::LibinputInterface for LibseatInterface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<std::os::unix::io::OwnedFd, i32> {
        use smithay::reexports::rustix::fs::OFlags;

        let oflags = OFlags::from_bits_truncate(flags.unsigned_abs());
        self.0.open(path, oflags).map_err(|err| {
            tracing::warn!(%err, ?path, "failed to open input device");
            ENODEV
        })
    }

    fn close_restricted(&mut self, fd: std::os::unix::io::OwnedFd) {
        let _ = self.0.close(fd);
    }
}

/// Initialize a DRM device: open the GPU, enumerate connectors, create outputs.
///
/// For each connected connector, creates a [`DrmSurface`], [`GbmAllocator`],
/// [`GbmFramebufferExporter`], and [`DrmCompositor`].  Returns the per-GPU
/// backend state containing all per-output compositors.
#[allow(clippy::too_many_lines)]
fn initialize_drm_device(
    session: &mut LibSeatSession,
    event_loop: &EventLoop<'static, State>,
    state: &State,
    path: &Path,
) -> Result<DrmBackendState, Box<dyn std::error::Error>> {
    use smithay::reexports::rustix::fs::OFlags;

    let fd = session.open(path, OFlags::RDWR | OFlags::CLOEXEC)?;
    let device_fd = DrmDeviceFd::new(smithay::utils::DeviceFd::from(fd));

    let node = DrmNode::from_file(&device_fd)?;

    let (mut drm_device, drm_notifier) = DrmDevice::new(device_fd.clone(), true)?;

    let gbm_device = GbmDevice::new(device_fd)?;
    let renderer = {
        use smithay::backend::egl::{EGLContext, EGLDisplay};
        #[allow(unsafe_code)]
        // SAFETY: GbmDevice implements EGLNativeDisplay; creating the EGL
        // display from it is the standard Smithay pattern for DRM backends.
        let egl_display = unsafe { EGLDisplay::new(gbm_device.clone())? };
        let egl_context = EGLContext::new(&egl_display)?;
        #[allow(unsafe_code)]
        // SAFETY: The EGLContext is valid and current.  GlowRenderer wraps
        // GlesRenderer which manages GL state internally.
        unsafe {
            GlowRenderer::new(egl_context)?
        }
    };

    // Register DRM device events (VBlank, page flip completion).
    // On VBlank, mark the corresponding output as ready for a new frame.
    event_loop.handle().insert_source(drm_notifier, |event, _metadata, state| match event {
        DrmEvent::VBlank(crtc) => {
            tracing::trace!(?crtc, "VBlank");
            if let Some(ref mut backend) = state.drm_backend
                && let Some(output_state) = backend.outputs.get_mut(&crtc)
            {
                if let Err(err) = output_state.drm_compositor.frame_submitted() {
                    tracing::warn!(?crtc, %err, "frame_submitted failed");
                }
                output_state.pending_frame = false;
            }
        }
        DrmEvent::Error(err) => {
            tracing::error!(%err, "DRM device error");
        }
    })?;

    // Enumerate connectors and create an output for each connected one
    let res_handles = drm_device.resource_handles().map_err(|e| format!("resource_handles: {e}"))?;
    let cursor_size: Size<u32, BufferCoords> = drm_device.cursor_size();
    let mut outputs = HashMap::new();

    // Build renderer format list for the DRM compositor intersection
    let renderer_formats: Vec<DrmFormat> = [DrmFourcc::Argb8888, DrmFourcc::Xrgb8888]
        .iter()
        .map(|code| DrmFormat { code: *code, modifier: DrmModifier::Linear })
        .collect();

    for conn_handle in res_handles.connectors() {
        let conn_info = drm_device.get_connector(*conn_handle, false).map_err(|e| format!("get_connector: {e}"))?;

        if conn_info.state() != connector::State::Connected {
            tracing::debug!(?conn_handle, state = ?conn_info.state(), "skipping disconnected connector");
            continue;
        }

        let modes = conn_info.modes();
        if modes.is_empty() {
            tracing::warn!(?conn_handle, "connected connector has no modes");
            continue;
        }

        // Pick the preferred mode, or the first mode if none is preferred
        let mode = modes
            .iter()
            .find(|m| m.mode_type().contains(control::ModeTypeFlags::PREFERRED))
            .or_else(|| modes.first())
            .copied()
            .ok_or("no mode available")?;

        // Find a suitable CRTC for this connector
        let crtc_handle = find_crtc_for_connector(&drm_device, &res_handles, &conn_info, &outputs);
        let Some(crtc) = crtc_handle else {
            tracing::warn!(?conn_handle, "no available CRTC for connector");
            continue;
        };

        // Create the DRM surface for this CRTC + connector + mode
        let surface: DrmSurface = drm_device.create_surface(crtc, mode, &[*conn_handle])?;

        // Create the GBM allocator and framebuffer exporter
        let allocator = GbmAllocator::new(gbm_device.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
        let exporter = GbmFramebufferExporter::new(gbm_device.clone(), None);

        // Build the Smithay output
        let iface = conn_info.interface();
        let output_name = format!("{}-{}", iface.as_str(), conn_info.interface_id());
        let phys_size = conn_info.size().unwrap_or((0, 0));
        let output = Output::new(
            output_name.clone(),
            PhysicalProperties {
                #[allow(clippy::cast_possible_truncation)]
                size: (phys_size.0.min(i32::MAX as u32).cast_signed(), phys_size.1.min(i32::MAX as u32).cast_signed())
                    .into(),
                subpixel: Subpixel::Unknown,
                make: "PlatynUI".to_string(),
                model: "DRM Output".to_string(),
            },
        );

        let output_mode = OutputMode {
            size: (i32::from(mode.size().0), i32::from(mode.size().1)).into(),
            refresh: mode.vrefresh().min(i32::MAX as u32).cast_signed() * 1000,
        };
        output.change_current_state(Some(output_mode), None, None, None);
        output.set_preferred(output_mode);
        output.create_global::<State>(&state.display_handle);

        // Create the DRM compositor for this output
        let color_formats = [DrmFourcc::Argb8888, DrmFourcc::Xrgb8888];
        let drm_compositor = DrmCompositor::new(
            &output,
            surface,
            None,
            allocator,
            exporter,
            color_formats,
            renderer_formats.clone(),
            cursor_size,
            Some(gbm_device.clone()),
        )
        .map_err(|e| format!("DrmCompositor::new for {output_name}: {e}"))?;

        tracing::info!(
            name = output_name,
            ?crtc,
            mode_w = mode.size().0,
            mode_h = mode.size().1,
            refresh = mode.vrefresh(),
            "DRM output initialized",
        );

        outputs.insert(crtc, DrmOutputState { drm_compositor, output, pending_frame: false });
    }

    if outputs.is_empty() {
        return Err("no connected DRM outputs found".into());
    }

    tracing::info!(?path, ?node, outputs = outputs.len(), "DRM device initialized");

    Ok(DrmBackendState { _drm_device: drm_device, outputs, renderer, session_active: true })
}

/// Find an available CRTC for a connector that isn't already claimed.
///
/// Tries the CRTC currently associated with the connector's encoder first.
/// If that's taken, probes all encoders the connector supports and picks
/// the first unclaimed CRTC.
fn find_crtc_for_connector(
    device: &DrmDevice,
    res_handles: &control::ResourceHandles,
    conn_info: &connector::Info,
    used_crtcs: &HashMap<crtc::Handle, DrmOutputState>,
) -> Option<crtc::Handle> {
    // Try the CRTC already associated with the current encoder
    if let Some(enc_handle) = conn_info.current_encoder()
        && let Ok(enc) = device.get_encoder(enc_handle)
        && let Some(crtc) = enc.crtc()
        && !used_crtcs.contains_key(&crtc)
    {
        return Some(crtc);
    }

    // Probe all encoders this connector supports for an unclaimed CRTC
    for enc_handle in conn_info.encoders() {
        if let Ok(enc) = device.get_encoder(*enc_handle) {
            for crtc in res_handles.filter_crtcs(enc.possible_crtcs()) {
                if !used_crtcs.contains_key(&crtc) {
                    return Some(crtc);
                }
            }
        }
    }
    None
}
