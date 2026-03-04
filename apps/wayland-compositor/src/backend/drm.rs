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

/// Active scanout state for a DRM output that has a CRTC assigned.
pub struct ActiveDrmCompositor {
    /// The CRTC driving this output.
    pub(crate) crtc: crtc::Handle,
    /// The DRM compositor that manages scanout for this output.
    pub(crate) drm_compositor:
        DrmCompositor<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>,
    /// Whether a frame has been queued and we're waiting for `VBlank`.
    pub(crate) pending_frame: bool,
}

/// Per-output state for a DRM connector.
///
/// Every connected connector gets a `DrmOutputState` and a Smithay [`Output`]
/// object (visible to wlr-randr).  When the GPU has fewer CRTCs than
/// connected monitors, some outputs start with `compositor: None` (disabled)
/// and can be activated later by freeing a CRTC from another output.
pub struct DrmOutputState {
    /// The Smithay output object for this connector.
    pub(crate) output: Output,
    /// Active compositor — `None` when the output is disabled (no CRTC).
    pub(crate) compositor: Option<ActiveDrmCompositor>,
}

/// Per-GPU rendering state.
pub struct DrmBackendState {
    /// DRM device — needed for creating surfaces when activating outputs.
    pub(crate) drm_device: DrmDevice,
    /// GBM device — needed for allocators when activating outputs.
    pub(crate) gbm_device: GbmDevice<DrmDeviceFd>,
    /// Hardware cursor size (from DRM device capabilities).
    pub(crate) cursor_size: Size<u32, BufferCoords>,
    /// All connected outputs, keyed by connector handle.
    pub(crate) outputs: HashMap<connector::Handle, DrmOutputState>,
    /// `GlowRenderer` (EGL on GBM — GPU-accelerated or Mesa llvmpipe).
    pub(crate) renderer: GlowRenderer,
    /// libseat session — used for VT switching (`Ctrl+Alt+F<n>`).
    pub(crate) session: LibSeatSession,
    /// Whether the session is currently active (false when VT-switched away).
    pub(crate) session_active: bool,
}

impl DrmBackendState {
    /// Activate a disabled output by assigning it a free CRTC.
    ///
    /// Returns `Ok(())` if the output was successfully activated or was already
    /// active.  Returns an error if no CRTC is available or hardware setup fails.
    pub fn activate_output(&mut self, conn: connector::Handle) -> Result<(), Box<dyn std::error::Error>> {
        // Check if already active.
        {
            let output_state = self.outputs.get(&conn).ok_or("unknown connector")?;
            if output_state.compositor.is_some() {
                return Ok(()); // already active
            }
        }

        // Re-query connector info for DRM modes.
        let conn_info = self.drm_device.get_connector(conn, false).map_err(|e| format!("get_connector: {e}"))?;
        let res_handles = self.drm_device.resource_handles().map_err(|e| format!("resource_handles: {e}"))?;

        // Find a free CRTC.
        let crtc = find_crtc_for_connector(&self.drm_device, &res_handles, &conn_info, &self.outputs)
            .ok_or("no available CRTC — disable another output first")?;

        // Match the output's current Smithay mode to a DRM mode.
        let output = self.outputs.get(&conn).expect("checked above").output.clone();
        let smithay_mode = output.current_mode().ok_or("output has no current mode")?;

        let drm_modes = conn_info.modes();
        let drm_mode = drm_modes
            .iter()
            .find(|m| {
                i32::from(m.size().0) == smithay_mode.size.w
                    && i32::from(m.size().1) == smithay_mode.size.h
                    && m.vrefresh().min(i32::MAX as u32).cast_signed() * 1000 == smithay_mode.refresh
            })
            // Fallback: match size only (refresh may differ due to rounding).
            .or_else(|| {
                drm_modes.iter().find(|m| {
                    i32::from(m.size().0) == smithay_mode.size.w && i32::from(m.size().1) == smithay_mode.size.h
                })
            })
            .or_else(|| drm_modes.first())
            .copied()
            .ok_or("no DRM mode available")?;

        // Create hardware resources.
        let surface = self.drm_device.create_surface(crtc, drm_mode, &[conn])?;
        let allocator = GbmAllocator::new(self.gbm_device.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
        let exporter = GbmFramebufferExporter::new(self.gbm_device.clone(), None);
        let color_formats = [DrmFourcc::Argb8888, DrmFourcc::Xrgb8888];
        let renderer_formats: Vec<DrmFormat> = [DrmFourcc::Argb8888, DrmFourcc::Xrgb8888]
            .iter()
            .map(|code| DrmFormat { code: *code, modifier: DrmModifier::Linear })
            .collect();

        let drm_compositor = DrmCompositor::new(
            &output,
            surface,
            None,
            allocator,
            exporter,
            color_formats,
            renderer_formats,
            self.cursor_size,
            Some(self.gbm_device.clone()),
        )
        .map_err(|e| format!("DrmCompositor::new: {e}"))?;

        let output_state = self.outputs.get_mut(&conn).expect("checked above");
        output_state.compositor = Some(ActiveDrmCompositor { crtc, drm_compositor, pending_frame: false });

        tracing::info!(output = output.name(), ?crtc, "DRM output activated");
        Ok(())
    }

    /// Deactivate an output, releasing its CRTC for use by another output.
    pub fn deactivate_output(&mut self, conn: connector::Handle) {
        if let Some(output_state) = self.outputs.get_mut(&conn)
            && let Some(active) = output_state.compositor.take()
        {
            tracing::info!(
                output = output_state.output.name(),
                crtc = ?active.crtc,
                "DRM output deactivated — CRTC released",
            );
        }
    }

    /// Find the connector handle for an output by matching names.
    pub fn connector_for_output(&self, output: &Output) -> Option<connector::Handle> {
        let name = output.name();
        self.outputs.iter().find(|(_, o)| o.output.name() == name).map(|(conn, _)| *conn)
    }
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
        0, // DRM backend discovers real hardware outputs
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
                    // Apply the CLI --scale to DRM outputs if specified.
                    let cli_scale = if args.scale > 0.0 && (args.scale - 1.0).abs() > f64::EPSILON {
                        Some(smithay::output::Scale::Fractional(args.scale))
                    } else {
                        None
                    };

                    // Map DRM-discovered outputs according to the --output-layout.
                    // Default: horizontal (side by side, left to right).
                    // Only active outputs (with a CRTC) are mapped into the space;
                    // disabled outputs are still added to state.outputs so wlr-randr
                    // can see and potentially enable them.
                    let mut next_pos: i32 = 0;
                    for output_state in backend_state.outputs.values() {
                        // Always register the output so wlr-randr sees it.
                        state.outputs.push(output_state.output.clone());

                        // Only map active (CRTC-assigned) outputs into the space.
                        if output_state.compositor.is_none() {
                            continue;
                        }

                        let position = match args.output_layout {
                            crate::multi_output::OutputLayout::Horizontal => (next_pos, 0),
                            crate::multi_output::OutputLayout::Vertical => (0, next_pos),
                        };

                        output_state.output.change_current_state(None, None, cli_scale, Some(position.into()));
                        state.space.map_output(&output_state.output, position);

                        // Advance the position by this output's logical extent.
                        let mode = output_state.output.current_mode().unwrap_or(smithay::output::Mode {
                            size: (1920, 1080).into(),
                            refresh: crate::state::DEFAULT_REFRESH_MHTZ,
                        });
                        let scale = output_state.output.current_scale().fractional_scale();
                        #[allow(clippy::cast_possible_truncation)]
                        let extent = match args.output_layout {
                            crate::multi_output::OutputLayout::Horizontal => {
                                (f64::from(mode.size.w) / scale).round() as i32
                            }
                            crate::multi_output::OutputLayout::Vertical => {
                                (f64::from(mode.size.h) / scale).round() as i32
                            }
                        };
                        next_pos += extent;
                    }
                    // Use the first active output as the primary output.
                    if let Some(first_active) = backend_state.outputs.values().find(|o| o.compositor.is_some()) {
                        state.output = first_active.output.clone();
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

/// Render one frame on each active DRM output.
///
/// For each output with an active `DrmCompositor`, renders all compositor
/// elements (windows + decorations), then queues the frame for scanout.
/// Frames are skipped when a previous frame is still pending (`VBlank`).
fn render_drm_outputs(backend: &mut DrmBackendState, state: &mut State) {
    // Lazy-init the glow-based titlebar painter on the first frame.
    if !state.titlebar_renderer.is_glow_initialized() {
        state.titlebar_renderer.init_glow(&mut backend.renderer);
    }

    let conn_handles: Vec<connector::Handle> = backend.outputs.keys().copied().collect();

    for conn in conn_handles {
        let Some(output_state) = backend.outputs.get_mut(&conn) else {
            continue;
        };

        let Some(ref mut active) = output_state.compositor else {
            continue; // disabled output — no CRTC
        };

        // Skip if we're still waiting for VBlank on a previous frame
        if active.pending_frame {
            continue;
        }

        // Skip outputs that were unmapped from the space.
        let output = output_state.output.clone();
        if state.space.output_geometry(&output).is_none() {
            continue;
        }

        let render_elements = crate::render::collect_render_elements(&mut backend.renderer, state, &output, true);

        match active.drm_compositor.render_frame::<_, _>(
            &mut backend.renderer,
            &render_elements,
            crate::state::BACKGROUND_COLOR,
            FrameFlags::DEFAULT,
        ) {
            Ok(result) => {
                if !result.is_empty {
                    if let Err(err) = active.drm_compositor.queue_frame(()) {
                        tracing::warn!(%err, crtc = ?active.crtc, "failed to queue DRM frame");
                    } else {
                        active.pending_frame = true;
                    }
                }
            }
            Err(err) => {
                tracing::warn!(?err, crtc = ?active.crtc, "DRM render_frame failed");
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
            if let Some(ref mut backend) = state.drm_backend {
                let output_state =
                    backend.outputs.values_mut().find(|o| o.compositor.as_ref().is_some_and(|a| a.crtc == crtc));
                if let Some(output_state) = output_state
                    && let Some(ref mut active) = output_state.compositor
                {
                    if let Err(err) = active.drm_compositor.frame_submitted() {
                        tracing::warn!(?crtc, %err, "frame_submitted failed");
                    }
                    active.pending_frame = false;
                }
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
        let iface = conn_info.interface();
        let output_name = format!("{}-{}", iface.as_str(), conn_info.interface_id());

        if conn_info.state() != connector::State::Connected {
            tracing::debug!(name = output_name, state = ?conn_info.state(), "skipping disconnected connector");
            continue;
        }

        let modes = conn_info.modes();
        if modes.is_empty() {
            tracing::warn!(name = output_name, "connected connector has no modes");
            continue;
        }

        // Pick the preferred mode, or the first mode if none is preferred
        let mode = modes
            .iter()
            .find(|m| m.mode_type().contains(control::ModeTypeFlags::PREFERRED))
            .or_else(|| modes.first())
            .copied()
            .ok_or("no mode available")?;

        // Map DRM subpixel geometry to Smithay's enum.
        let subpixel = match conn_info.subpixel() {
            connector::SubPixel::HorizontalRgb => Subpixel::HorizontalRgb,
            connector::SubPixel::HorizontalBgr => Subpixel::HorizontalBgr,
            connector::SubPixel::VerticalRgb => Subpixel::VerticalRgb,
            connector::SubPixel::VerticalBgr => Subpixel::VerticalBgr,
            connector::SubPixel::None => Subpixel::None,
            _ => Subpixel::Unknown,
        };

        // Read EDID for manufacturer + model name (falls back to "Unknown").
        let edid_info = read_edid_info(&drm_device, *conn_handle);
        let (make, model_name) = match edid_info {
            Some(ref info) => (info.make.clone(), info.model.clone()),
            None => ("Unknown".to_string(), "Unknown".to_string()),
        };

        // Build the Smithay output with the real connector name.
        let phys_size = conn_info.size().unwrap_or((0, 0));
        let output = Output::new(
            output_name.clone(),
            PhysicalProperties {
                #[allow(clippy::cast_possible_truncation)]
                size: (phys_size.0.min(i32::MAX as u32).cast_signed(), phys_size.1.min(i32::MAX as u32).cast_signed())
                    .into(),
                subpixel,
                make,
                model: model_name,
            },
        );

        // Register all modes from the connector so wlr-randr can list them.
        let preferred_mode = OutputMode {
            size: (i32::from(mode.size().0), i32::from(mode.size().1)).into(),
            refresh: mode.vrefresh().min(i32::MAX as u32).cast_signed() * 1000,
        };
        for drm_mode in modes {
            let output_mode = OutputMode {
                size: (i32::from(drm_mode.size().0), i32::from(drm_mode.size().1)).into(),
                refresh: drm_mode.vrefresh().min(i32::MAX as u32).cast_signed() * 1000,
            };
            output.add_mode(output_mode);
        }
        output.change_current_state(Some(preferred_mode), None, None, None);
        output.set_preferred(preferred_mode);
        output.create_global::<State>(&state.display_handle);

        // Find a suitable CRTC for this connector.
        // If no CRTC is available (more monitors than GPU CRTCs), the output is
        // still registered as a Wayland global (visible in wlr-randr) but starts
        // disabled — it can be activated later by freeing a CRTC from another output.
        let crtc_handle = find_crtc_for_connector(&drm_device, &res_handles, &conn_info, &outputs);

        let compositor = if let Some(crtc) = crtc_handle {
            // Create the DRM surface for this CRTC + connector + mode
            let surface: DrmSurface = drm_device.create_surface(crtc, mode, &[*conn_handle])?;

            let allocator = GbmAllocator::new(gbm_device.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
            let exporter = GbmFramebufferExporter::new(gbm_device.clone(), None);
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
                make = edid_info.as_ref().map_or("Unknown", |i| &i.make),
                model = edid_info.as_ref().map_or("Unknown", |i| &i.model),
                mode_w = mode.size().0,
                mode_h = mode.size().1,
                refresh = mode.vrefresh(),
                "DRM output initialized (active)",
            );

            Some(ActiveDrmCompositor { crtc, drm_compositor, pending_frame: false })
        } else {
            tracing::info!(
                name = output_name,
                make = edid_info.as_ref().map_or("Unknown", |i| &i.make),
                model = edid_info.as_ref().map_or("Unknown", |i| &i.model),
                mode_w = mode.size().0,
                mode_h = mode.size().1,
                refresh = mode.vrefresh(),
                "DRM output detected (disabled — no CRTC available)",
            );
            None
        };

        outputs.insert(*conn_handle, DrmOutputState { output, compositor });
    }

    if outputs.is_empty() {
        return Err("no connected DRM outputs found".into());
    }

    let active_count = outputs.values().filter(|o| o.compositor.is_some()).count();
    tracing::info!(
        ?path,
        ?node,
        total = outputs.len(),
        active = active_count,
        disabled = outputs.len() - active_count,
        "DRM device initialized",
    );

    Ok(DrmBackendState {
        drm_device,
        gbm_device,
        cursor_size,
        outputs,
        renderer,
        session: session.clone(),
        session_active: true,
    })
}

/// EDID-derived monitor identification.
struct EdidInfo {
    /// 3-letter PNP manufacturer ID (e.g. "DEL" for Dell, "SAM" for Samsung).
    make: String,
    /// Human-readable monitor name from EDID descriptor (e.g. "DELL U2723QE").
    model: String,
}

/// Read and parse the EDID blob from a DRM connector to extract manufacturer
/// and model name.
///
/// Returns `None` if the EDID property is missing, the blob is too short, or
/// the header signature is invalid.
fn read_edid_info(device: &DrmDevice, conn_handle: connector::Handle) -> Option<EdidInfo> {
    let props = device.get_properties(conn_handle).ok()?;
    let (handles, values) = props.as_props_and_values();

    // Find the "EDID" property.
    let edid_blob_id = handles.iter().zip(values.iter()).find_map(|(handle, value)| {
        let info = device.get_property(*handle).ok()?;
        if info.name().to_str() == Ok("EDID") { Some(*value) } else { None }
    })?;

    if edid_blob_id == 0 {
        return None;
    }

    let edid = device.get_property_blob(edid_blob_id).ok()?;
    parse_edid(&edid)
}

/// Parse raw EDID bytes into manufacturer + model name.
fn parse_edid(edid: &[u8]) -> Option<EdidInfo> {
    // Minimum EDID block is 128 bytes.
    if edid.len() < 128 {
        return None;
    }

    // Validate EDID header: 00 FF FF FF FF FF FF 00
    if edid[0..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
        return None;
    }

    // Manufacturer ID: bytes 8-9, three 5-bit letters (A=1 .. Z=26).
    let mfg_raw = u16::from_be_bytes([edid[8], edid[9]]);
    let c1 = ((mfg_raw >> 10) & 0x1F) as u8;
    let c2 = ((mfg_raw >> 5) & 0x1F) as u8;
    let c3 = (mfg_raw & 0x1F) as u8;
    let make = if (1..=26).contains(&c1) && (1..=26).contains(&c2) && (1..=26).contains(&c3) {
        let s: String = [c1 + b'A' - 1, c2 + b'A' - 1, c3 + b'A' - 1].iter().map(|&b| b as char).collect();
        s
    } else {
        "Unknown".to_string()
    };

    // Scan the four 18-byte descriptor blocks (starting at byte 54) for a
    // Monitor Name descriptor (tag 0xFC in byte 3).
    let mut model = String::new();
    for i in 0..4 {
        let base = 54 + i * 18;
        if base + 18 > edid.len() {
            break;
        }
        // Descriptor blocks that are not detailed timing have bytes 0-1 == 0.
        if edid[base] != 0 || edid[base + 1] != 0 {
            continue;
        }
        // Byte 3 is the tag: 0xFC = Monitor Name.
        if edid[base + 3] == 0xFC {
            // Name is in bytes 5..18, padded with 0x0A (newline) / spaces.
            model = edid[base + 5..base + 18]
                .iter()
                .take_while(|&&b| b != 0x0A && b != 0x00)
                .map(|&b| b as char)
                .collect::<String>()
                .trim()
                .to_string();
            break;
        }
    }

    if model.is_empty() {
        // Fallback: use the product code from bytes 10-11.
        let product = u16::from_le_bytes([edid[10], edid[11]]);
        model = format!("0x{product:04X}");
    }

    Some(EdidInfo { make, model })
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
    used_outputs: &HashMap<connector::Handle, DrmOutputState>,
) -> Option<crtc::Handle> {
    // Collect CRTCs that are already assigned to active outputs.
    let used_crtcs: Vec<crtc::Handle> =
        used_outputs.values().filter_map(|o| o.compositor.as_ref().map(|a| a.crtc)).collect();

    // Try the CRTC already associated with the current encoder
    if let Some(enc_handle) = conn_info.current_encoder()
        && let Ok(enc) = device.get_encoder(enc_handle)
        && let Some(crtc) = enc.crtc()
        && !used_crtcs.contains(&crtc)
    {
        return Some(crtc);
    }

    // Probe all encoders this connector supports for an unclaimed CRTC
    for enc_handle in conn_info.encoders() {
        if let Ok(enc) = device.get_encoder(*enc_handle) {
            for crtc in res_handles.filter_crtcs(enc.possible_crtcs()) {
                if !used_crtcs.contains(&crtc) {
                    return Some(crtc);
                }
            }
        }
    }
    None
}
