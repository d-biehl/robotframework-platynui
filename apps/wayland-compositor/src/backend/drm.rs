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
//! ## Render scheduling
//!
//! Rendering is VBlank-driven: a calloop [`Ping`](smithay::reexports::calloop::ping::Ping)
//! is used to request a frame render.  Protocol handlers (e.g. `wl_surface.commit`),
//! `VBlank` completions, and output configuration changes all trigger the ping via
//! [`schedule_render()`](crate::state::State::schedule_render).  This avoids
//! busy-loop rendering and only draws when there is actual work to present.
//!
//! For environments without a dedicated GPU, set `LIBGL_ALWAYS_SOFTWARE=1` to
//! use Mesa's software renderer (llvmpipe).
//!
//! Requires the `backend-drm` Cargo feature.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

/// Linux `ENODEV` errno value, returned when an input device cannot be opened.
const ENODEV: i32 = 19;

use smithay::{
    backend::{
        allocator::{
            Format as DrmFormat, Fourcc as DrmFourcc, Modifier as DrmModifier,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        drm::{
            DrmDevice, DrmDeviceFd, DrmEvent, DrmEventTime, DrmNode, DrmSurface,
            compositor::{DrmCompositor, FrameFlags},
            exporter::gbm::GbmFramebufferExporter,
        },
        libinput::LibinputInputBackend,
        renderer::glow::GlowRenderer,
        session::{Event as SessionEvent, Session, libseat::LibSeatSession},
        udev::{UdevBackend, UdevEvent},
    },
    desktop::utils::OutputPresentationFeedback,
    output::{Mode as OutputMode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{EventLoop, ping},
        drm::control::{self, Device as ControlDevice, connector, crtc},
        input::Libinput,
        wayland_protocols::wp::presentation_time::server::wp_presentation_feedback,
        wayland_server::Display,
    },
    utils::{Buffer as BufferCoords, Physical, Size},
    wayland::presentation::Refresh,
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
    /// Presentation feedback collected during `render_frame()`, delivered on `VBlank`.
    pub(crate) pending_presentation_feedback: Option<OutputPresentationFeedback>,
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
    /// Udev device ID (`dev_t`) — used to correlate hotplug events with this GPU.
    pub(crate) device_id: u64,
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
    /// libinput context — retained for `suspend()`/`resume()` on VT switch.
    pub(crate) libinput: Libinput,
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
        output_state.compositor = Some(ActiveDrmCompositor {
            crtc,
            drm_compositor,
            pending_frame: false,
            pending_presentation_feedback: None,
        });

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

    /// Set up a newly connected connector: create output, modes, and DRM compositor.
    ///
    /// Returns `Some(DrmOutputState)` on success, `None` if the connector has no
    /// usable modes.  The caller is responsible for mapping the output into the
    /// space and registering it in `state.outputs`.
    #[allow(clippy::too_many_lines)]
    pub fn setup_connector(
        &mut self,
        conn_handle: connector::Handle,
        display_handle: &smithay::reexports::wayland_server::DisplayHandle,
    ) -> Result<Option<DrmOutputState>, Box<dyn std::error::Error>> {
        let conn_info = self.drm_device.get_connector(conn_handle, false).map_err(|e| format!("get_connector: {e}"))?;
        let iface = conn_info.interface();
        let output_name = format!("{}-{}", iface.as_str(), conn_info.interface_id());

        if conn_info.state() != connector::State::Connected {
            return Ok(None);
        }

        let modes = conn_info.modes();
        if modes.is_empty() {
            tracing::warn!(name = output_name, "connected connector has no modes");
            return Ok(None);
        }

        let mode = modes
            .iter()
            .find(|m| m.mode_type().contains(control::ModeTypeFlags::PREFERRED))
            .or_else(|| modes.first())
            .copied()
            .ok_or("no mode available")?;

        let subpixel = match conn_info.subpixel() {
            connector::SubPixel::HorizontalRgb => Subpixel::HorizontalRgb,
            connector::SubPixel::HorizontalBgr => Subpixel::HorizontalBgr,
            connector::SubPixel::VerticalRgb => Subpixel::VerticalRgb,
            connector::SubPixel::VerticalBgr => Subpixel::VerticalBgr,
            connector::SubPixel::None => Subpixel::None,
            _ => Subpixel::Unknown,
        };

        let edid_info = read_edid_info(&self.drm_device, conn_handle);
        let (make, model_name) = match edid_info {
            Some(ref info) => (info.make.clone(), info.model.clone()),
            None => ("Unknown".to_string(), "Unknown".to_string()),
        };

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
        output.create_global::<State>(display_handle);

        let res_handles = self.drm_device.resource_handles().map_err(|e| format!("resource_handles: {e}"))?;
        let crtc_handle = find_crtc_for_connector(&self.drm_device, &res_handles, &conn_info, &self.outputs);

        let renderer_formats: Vec<DrmFormat> = [DrmFourcc::Argb8888, DrmFourcc::Xrgb8888]
            .iter()
            .map(|code| DrmFormat { code: *code, modifier: DrmModifier::Linear })
            .collect();

        let compositor = if let Some(crtc) = crtc_handle {
            let surface: DrmSurface = self.drm_device.create_surface(crtc, mode, &[conn_handle])?;
            let allocator =
                GbmAllocator::new(self.gbm_device.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
            let exporter = GbmFramebufferExporter::new(self.gbm_device.clone(), None);
            let color_formats = [DrmFourcc::Argb8888, DrmFourcc::Xrgb8888];

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

            Some(ActiveDrmCompositor {
                crtc,
                drm_compositor,
                pending_frame: false,
                pending_presentation_feedback: None,
            })
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

        Ok(Some(DrmOutputState { output, compositor }))
    }

    /// Re-enumerate connectors after a udev change event (monitor hotplug).
    ///
    /// Detects newly connected and disconnected connectors.  Returns the list
    /// of added connector handles (caller maps them into the space) and removed
    /// outputs (caller relocates windows and notifies clients).
    pub fn handle_hotplug(
        &mut self,
        display_handle: &smithay::reexports::wayland_server::DisplayHandle,
    ) -> (Vec<connector::Handle>, Vec<Output>) {
        let mut added = Vec::new();
        let mut removed = Vec::new();

        let Ok(res_handles) = self.drm_device.resource_handles() else {
            tracing::warn!("failed to query DRM resource handles during hotplug");
            return (added, removed);
        };

        // Detect disconnected connectors (were in our map, now gone or disconnected).
        let known_conns: Vec<connector::Handle> = self.outputs.keys().copied().collect();
        for conn in &known_conns {
            let still_connected = self
                .drm_device
                .get_connector(*conn, true)
                .is_ok_and(|info| info.state() == connector::State::Connected);

            if !still_connected && let Some(output_state) = self.outputs.remove(conn) {
                tracing::info!(output = output_state.output.name(), "connector disconnected (hotplug)",);
                removed.push(output_state.output);
            }
        }

        // Detect newly connected connectors.
        for conn_handle in res_handles.connectors() {
            if self.outputs.contains_key(conn_handle) {
                continue; // already tracked
            }

            match self.setup_connector(*conn_handle, display_handle) {
                Ok(Some(output_state)) => {
                    tracing::info!(
                        output = output_state.output.name(),
                        active = output_state.compositor.is_some(),
                        "connector connected (hotplug)",
                    );
                    self.outputs.insert(*conn_handle, output_state);
                    added.push(*conn_handle);
                }
                Ok(None) => {} // not connected or no modes
                Err(err) => {
                    tracing::warn!(%err, "failed to set up hotplugged connector");
                }
            }
        }

        (added, removed)
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
    state.init_dmabuf_with_fallback_formats();

    // Register Wayland display + listening socket + set WAYLAND_DISPLAY
    super::register_wayland_sources(&event_loop.handle(), display, listening_socket, &socket_name)?;

    // Initialize libinput for keyboard/mouse/touch input.
    // Clone the context before passing to `LibinputInputBackend` so we retain
    // a handle for `suspend()`/`resume()` during VT switching.
    let mut libinput_context = Libinput::new_with_udev(LibseatInterface(session.clone()));
    libinput_context.udev_assign_seat(&session.seat()).map_err(|()| "failed to assign libinput seat")?;

    let libinput_backend = LibinputInputBackend::new(libinput_context.clone());
    event_loop.handle().insert_source(libinput_backend, |event, (), state| {
        crate::input::process_input_event(state, event);
    })?;

    // Register session notifier for VT switching.
    // On pause: suspend libinput + DRM device (release DRM master).
    // On resume: activate DRM device (re-acquire DRM master) + resume libinput,
    //            then schedule a render to repaint all outputs.
    event_loop.handle().insert_source(notifier, |event, (), state| match event {
        SessionEvent::PauseSession => {
            tracing::info!("session paused (VT switch away)");
            if let Some(ref mut backend) = state.drm_backend {
                backend.libinput.suspend();
                backend.drm_device.pause();
                backend.session_active = false;
            }
        }
        SessionEvent::ActivateSession => {
            tracing::info!("session activated (VT switch back)");
            if let Some(ref mut backend) = state.drm_backend {
                if let Err(err) = backend.drm_device.activate(false) {
                    tracing::error!(%err, "failed to activate DRM device after VT switch");
                }
                if backend.libinput.resume().is_err() {
                    tracing::error!("failed to resume libinput after VT switch");
                }
                backend.session_active = true;
            }
            state.schedule_render();
        }
    })?;

    // Discover GPUs via udev
    let udev_backend = UdevBackend::new(session.seat())?;

    // Process initially connected devices (use the first usable GPU)
    for (device_id, path) in udev_backend.device_list() {
        tracing::info!(?device_id, ?path, "discovered GPU device");
        if state.drm_backend.is_none() {
            match initialize_drm_device(device_id, &mut session, &event_loop, &state, path, libinput_context.clone()) {
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

    // Watch for hotplug events (monitor plug/unplug and GPU add/remove).
    event_loop.handle().insert_source(udev_backend, |event, (), state| match event {
        UdevEvent::Added { device_id, path } => {
            handle_gpu_added(state, device_id, &path);
        }
        UdevEvent::Changed { device_id } => {
            tracing::debug!(?device_id, "GPU device changed — re-enumerating connectors");
            handle_connector_hotplug(state);
        }
        UdevEvent::Removed { device_id } => {
            handle_gpu_removed(state, device_id);
        }
    })?;

    // Register signal handlers, watchdog, XWayland, control socket, readiness
    let shutdown = super::setup_services(&event_loop.handle(), &mut state, args, timeout)?;

    // VBlank-driven render scheduling: a calloop Ping triggers a single
    // render pass.  VBlank completions, client commits, and output config
    // changes all request a render via schedule_render().
    let (render_ping, render_source) = ping::make_ping()?;
    state.render_ping = Some(render_ping.clone());

    event_loop.handle().insert_source(render_source, |(), (), state| {
        // Temporarily take the backend to avoid double mutable borrow
        // (render_drm_outputs needs &mut state for the space).
        let frame_queued = if let Some(mut backend) = state.drm_backend.take() {
            let queued = if backend.session_active { render_drm_outputs(&mut backend, state) } else { false };
            state.drm_backend = Some(backend);
            queued
        } else {
            false
        };
        // When no frame was queued (all outputs empty or pending), no VBlank
        // will fire, so send frame callbacks here as a fallback to prevent
        // clients from stalling.
        if !frame_queued {
            state.send_frame_callbacks(Duration::ZERO);
        }
        state.flush_and_refresh();
    })?;

    // Kick off the initial render.
    render_ping.ping();

    tracing::info!(backend = "drm", socket = %socket_name, "event loop starting");

    event_loop.run(None, &mut state, |state| {
        // Handle output configuration changes from wlr-output-management.
        if state.output_config_changed {
            state.output_config_changed = false;
            crate::handlers::output_management::notify_output_config_changed(state);
            state.reconfigure_windows_for_outputs();
            state.schedule_render();
        }

        if !state.running || shutdown.is_set() {
            state.loop_signal.stop();
        }
    })?;

    tracing::info!("compositor shutting down");
    Ok(())
}

/// Render one frame on each active DRM output.
///
/// For each output with an active `DrmCompositor`, renders all compositor
/// elements (windows + decorations), then queues the frame for scanout.
/// Frames are skipped when a previous frame is still pending (`VBlank`).
///
/// After a successful render, collects presentation feedback and updates
/// per-surface scanout/DMA-BUF state.  The feedback is stored on the
/// `ActiveDrmCompositor` and delivered when the `VBlank` event arrives.
///
/// Returns `true` if at least one frame was queued (a `VBlank` will follow).
fn render_drm_outputs(backend: &mut DrmBackendState, state: &mut State) -> bool {
    let conn_handles: Vec<connector::Handle> = backend.outputs.keys().copied().collect();
    let mut any_frame_queued = false;

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
                // Update per-surface scanout tracking and collect feedback
                // before submitting, while we still have the render states.
                let render_element_states = result.states;

                state.update_primary_scanout_output(&output, &render_element_states);
                state.send_dmabuf_feedback(&output, &render_element_states);

                // Collect presentation feedback — will be delivered on VBlank
                // with the real hardware timestamp.
                let mut feedback = OutputPresentationFeedback::new(&output);
                state.take_presentation_feedback(&output, &render_element_states, &mut feedback);

                if !result.is_empty {
                    if let Err(err) = active.drm_compositor.queue_frame(()) {
                        tracing::warn!(%err, crtc = ?active.crtc, "failed to queue DRM frame");
                    } else {
                        active.pending_frame = true;
                        active.pending_presentation_feedback = Some(feedback);
                        any_frame_queued = true;
                    }
                }
            }
            Err(err) => {
                tracing::warn!(?err, crtc = ?active.crtc, "DRM render_frame failed");
            }
        }
    }

    any_frame_queued
}

/// Handle a udev "changed" event by re-enumerating DRM connectors.
///
/// Detects newly connected monitors (maps them into the space to the right of
/// or below existing outputs) and disconnected monitors (unmaps them and
/// relocates their windows to a remaining output).  Notifies
/// `wlr-output-management` clients of the change.
fn handle_connector_hotplug(state: &mut State) {
    // Temporarily take the backend to avoid overlapping borrows.
    let Some(mut backend) = state.drm_backend.take() else {
        return;
    };

    let (added, removed) = backend.handle_hotplug(&state.display_handle);

    if added.is_empty() && removed.is_empty() {
        state.drm_backend = Some(backend);
        return;
    }

    // Remove disconnected outputs from the space and state.outputs.
    for output in &removed {
        state.space.unmap_output(output);
        state.outputs.retain(|o| o.name() != output.name());
    }

    // Map newly connected outputs at the end of the current layout.
    for conn in &added {
        let Some(output_state) = backend.outputs.get(conn) else {
            continue;
        };

        state.outputs.push(output_state.output.clone());

        // Only map active outputs (with a CRTC) into the space.
        if output_state.compositor.is_none() {
            continue;
        }

        // Compute position: place after the rightmost existing output.
        let next_pos = compute_next_output_position(state);
        output_state.output.change_current_state(None, None, None, Some(next_pos.into()));
        state.space.map_output(&output_state.output, next_pos);
    }

    // Put the backend back before further state operations.
    state.drm_backend = Some(backend);

    // If the primary output was removed, pick a new one.
    if removed.iter().any(|o| o.name() == state.output.name())
        && let Some(first) = state.space.outputs().next().cloned()
    {
        state.output = first;
    }

    // Relocate windows from removed outputs to remaining outputs.
    if !removed.is_empty() {
        state.reconfigure_windows_for_outputs();
    }

    // Notify wlr-output-management clients about the topology change.
    state.output_config_changed = true;
    state.schedule_render();

    tracing::info!(
        added = added.len(),
        removed = removed.len(),
        total = state.outputs.len(),
        "connector hotplug handled",
    );
}

/// Compute the position for the next output in the layout.
///
/// Places the new output to the right of all existing outputs (horizontal
/// layout is the default for DRM — the initial layout direction from CLI
/// args is not stored, so we always append horizontally).
fn compute_next_output_position(state: &State) -> (i32, i32) {
    let mut max_x = 0i32;
    for output in state.space.outputs() {
        if let Some(geo) = state.space.output_geometry(output) {
            max_x = max_x.max(geo.loc.x + geo.size.w);
        }
    }
    (max_x, 0)
}

/// Handle a new GPU device appearing (e.g. eGPU plugged in).
///
/// Currently only the primary GPU is supported.  If no GPU is active yet this
/// logs a hint; otherwise it reports that multi-GPU is not implemented.
fn handle_gpu_added(state: &mut State, device_id: u64, path: &Path) {
    if state.drm_backend.is_some() {
        tracing::info!(?device_id, ?path, "additional GPU detected — multi-GPU not yet supported, ignoring",);
    } else {
        // No GPU is active (e.g. the primary was removed earlier).  We cannot
        // initialize a new GPU here because `initialize_drm_device` needs the
        // calloop `EventLoop` handle which is not available in this callback.
        // A full implementation would store it in `State`.
        tracing::warn!(
            ?device_id,
            ?path,
            "GPU added but late initialization not yet supported — restart the compositor",
        );
    }
}

/// Handle the removal of a GPU device (e.g. eGPU unplugged).
///
/// If the removed device matches the active GPU, tears down all its outputs,
/// relocates windows, and clears the backend state.  The compositor continues
/// running (e.g. for a reconnect or graceful shutdown).
fn handle_gpu_removed(state: &mut State, device_id: u64) {
    let is_our_gpu = state.drm_backend.as_ref().is_some_and(|b| b.device_id == device_id);

    if !is_our_gpu {
        tracing::debug!(?device_id, "unknown GPU removed — ignoring");
        return;
    }

    tracing::warn!(?device_id, "active GPU removed — tearing down all outputs");

    // Take the backend to drop all DRM/GBM resources.
    let backend = state.drm_backend.take().expect("checked above");

    // Unmap every output that belonged to this GPU from the space.
    for output_state in backend.outputs.values() {
        state.space.unmap_output(&output_state.output);
        state.outputs.retain(|o| o.name() != output_state.output.name());
    }
    // The backend (DrmDevice, GbmDevice, renderer, compositors) is dropped here.
    drop(backend);

    // Pick a new primary output if any remain (from another backend, unlikely
    // today but future-proof).
    if let Some(first) = state.space.outputs().next().cloned() {
        state.output = first;
    }

    state.reconfigure_windows_for_outputs();
    state.output_config_changed = true;
    // No schedule_render — there is no GPU to render on.

    tracing::info!("GPU teardown complete; compositor still running (no rendering)");
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
#[allow(clippy::too_many_lines, clippy::similar_names)]
fn initialize_drm_device(
    device_id: u64,
    session: &mut LibSeatSession,
    event_loop: &EventLoop<'static, State>,
    state: &State,
    path: &Path,
    libinput: Libinput,
) -> Result<DrmBackendState, Box<dyn std::error::Error>> {
    use smithay::reexports::rustix::fs::OFlags;

    let fd = session.open(path, OFlags::RDWR | OFlags::CLOEXEC)?;
    let device_fd = DrmDeviceFd::new(smithay::utils::DeviceFd::from(fd));

    let node = DrmNode::from_file(&device_fd)?;

    let (drm_device, drm_notifier) = DrmDevice::new(device_fd.clone(), true)?;

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
    // On VBlank: confirm frame, deliver presentation feedback with hardware
    // timestamps, send frame callbacks, then schedule the next render.
    event_loop.handle().insert_source(drm_notifier, |event, metadata, state| match event {
        DrmEvent::VBlank(crtc) => {
            tracing::trace!(?crtc, "VBlank");
            let drm_metadata = metadata.take();

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

                    // Deliver presentation feedback with the real hardware timestamp.
                    if let Some(mut feedback) = active.pending_presentation_feedback.take() {
                        let output = &output_state.output;
                        let refresh = output.current_mode().map_or(Refresh::Unknown, |mode| {
                            Refresh::Fixed(Duration::from_secs_f64(1_000.0 / f64::from(mode.refresh)))
                        });

                        let (clock, flags) = match drm_metadata.as_ref().map(|m| &m.time) {
                            Some(DrmEventTime::Monotonic(tp)) => (
                                (*tp).into(),
                                wp_presentation_feedback::Kind::Vsync
                                    | wp_presentation_feedback::Kind::HwClock
                                    | wp_presentation_feedback::Kind::HwCompletion,
                            ),
                            _ => (
                                state.clock.now(),
                                wp_presentation_feedback::Kind::Vsync | wp_presentation_feedback::Kind::HwCompletion,
                            ),
                        };

                        let sequence = drm_metadata.as_ref().map_or(0, |m| u64::from(m.sequence));
                        feedback.presented::<_, smithay::utils::Monotonic>(clock, refresh, sequence, flags);
                    }
                }
            }

            // Send frame callbacks on VBlank — clients learn about presentation
            // at the correct time rather than at render time.
            state.send_frame_callbacks(Duration::ZERO);

            // Schedule the next render now that this output is ready.
            state.schedule_render();
        }
        DrmEvent::Error(err) => {
            tracing::error!(%err, "DRM device error");
        }
    })?;

    // Enumerate connectors and create an output for each connected one.
    // Uses the same `setup_connector()` method that hotplug uses later.
    let res_handles = drm_device.resource_handles().map_err(|e| format!("resource_handles: {e}"))?;
    let cursor_size: Size<u32, BufferCoords> = drm_device.cursor_size();
    let outputs = HashMap::new();

    let mut backend_state = DrmBackendState {
        device_id,
        drm_device,
        gbm_device,
        cursor_size,
        outputs,
        renderer,
        libinput,
        session: session.clone(),
        session_active: true,
    };

    for conn_handle in res_handles.connectors() {
        match backend_state.setup_connector(*conn_handle, &state.display_handle) {
            Ok(Some(output_state)) => {
                backend_state.outputs.insert(*conn_handle, output_state);
            }
            Ok(None) => {} // not connected or no modes
            Err(err) => {
                tracing::warn!(%err, "failed to set up connector during init");
            }
        }
    }

    if backend_state.outputs.is_empty() {
        return Err("no connected DRM outputs found".into());
    }

    let active_count = backend_state.outputs.values().filter(|o| o.compositor.is_some()).count();
    tracing::info!(
        ?path,
        ?node,
        total = backend_state.outputs.len(),
        active = active_count,
        disabled = backend_state.outputs.len() - active_count,
        "DRM device initialized",
    );

    Ok(backend_state)
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
