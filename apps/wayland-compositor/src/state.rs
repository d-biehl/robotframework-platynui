//! Compositor state — central struct holding all protocol states and runtime data.

use smithay::{
    delegate_compositor, delegate_cursor_shape, delegate_data_device, delegate_dmabuf, delegate_fractional_scale,
    delegate_idle_notify, delegate_input_method_manager, delegate_keyboard_shortcuts_inhibit, delegate_output,
    delegate_pointer_constraints, delegate_presentation, delegate_primary_selection, delegate_relative_pointer,
    delegate_seat, delegate_security_context, delegate_session_lock, delegate_shm, delegate_single_pixel_buffer,
    delegate_text_input_manager, delegate_viewporter, delegate_xdg_activation, delegate_xdg_decoration,
    delegate_xdg_foreign, delegate_xdg_shell,
    desktop::{PopupManager, Space, Window},
    input::{Seat, SeatState, keyboard::XkbConfig, pointer::CursorImageStatus},
    output::{Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{LoopHandle, LoopSignal},
        wayland_server::DisplayHandle,
    },
    utils::{Clock, Logical, Monotonic, Physical, Point, Rectangle, Size},
    wayland::{
        compositor::CompositorState,
        cursor_shape::CursorShapeManagerState,
        dmabuf::DmabufState,
        fractional_scale::{FractionalScaleHandler, FractionalScaleManagerState},
        idle_notify::IdleNotifierState,
        input_method::InputMethodManagerState,
        keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitState,
        output::{OutputHandler, OutputManagerState},
        pointer_constraints::PointerConstraintsState,
        presentation::PresentationState,
        relative_pointer::RelativePointerManagerState,
        security_context::SecurityContextState,
        selection::{data_device::DataDeviceState, primary_selection::PrimarySelectionState},
        session_lock::SessionLockManagerState,
        shell::xdg::{XdgShellState, decoration::XdgDecorationState},
        shm::ShmState,
        single_pixel_buffer::SinglePixelBufferState,
        text_input::TextInputManagerState,
        viewporter::ViewporterState,
        xdg_activation::XdgActivationState,
        xdg_foreign::XdgForeignState,
    },
};

/// Saved state for a fullscreen window: (window, previous position, previous pending size).
type PreFullscreenState = (Window, Point<i32, Logical>, Option<Size<i32, Logical>>);

/// Central compositor state holding all Wayland protocol states and runtime data.
#[allow(dead_code, clippy::struct_field_names)]
pub struct State {
    // -- Core protocol states --
    pub compositor_state: CompositorState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Self>,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    pub dmabuf_state: DmabufState,

    // -- Application compatibility protocols --
    pub viewporter_state: ViewporterState,
    pub fractional_scale_state: FractionalScaleManagerState,
    pub xdg_activation_state: XdgActivationState,
    pub pointer_constraints_state: PointerConstraintsState,
    pub relative_pointer_state: RelativePointerManagerState,
    pub single_pixel_buffer_state: SinglePixelBufferState,
    pub presentation_state: PresentationState,
    pub keyboard_shortcuts_inhibit_state: KeyboardShortcutsInhibitState,
    pub text_input_state: TextInputManagerState,
    pub input_method_state: InputMethodManagerState,
    pub idle_notify_state: IdleNotifierState<Self>,
    pub session_lock_state: SessionLockManagerState,
    pub xdg_foreign_state: XdgForeignState,
    pub security_context_state: SecurityContextState,
    pub cursor_shape_state: CursorShapeManagerState,

    // -- Window management --
    pub space: Space<Window>,
    pub popup_manager: PopupManager,

    // -- Display / event loop --
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, Self>,
    pub loop_signal: LoopSignal,

    // -- Seat --
    pub seat: Seat<Self>,

    // -- Input state --
    pub cursor_status: CursorImageStatus,
    pub pointer_location: Point<f64, Logical>,

    /// Compositor-driven cursor shape (for SSD resize borders, title bar).
    pub compositor_cursor_shape: crate::decorations::CursorShape,

    /// Pre-maximize window positions — restored when unmaximizing.
    pub pre_maximize_positions: Vec<(Window, Point<i32, Logical>)>,

    /// Pre-fullscreen window states — restored when leaving fullscreen.
    pub pre_fullscreen_states: Vec<PreFullscreenState>,

    /// Minimized windows — removed from the space but kept alive.
    /// Stores the window and its last position for restoring.
    pub minimized_windows: Vec<(Window, Point<i32, Logical>)>,

    /// Last titlebar click time — used to detect double-clicks for maximize toggle.
    pub last_titlebar_click: Option<std::time::Instant>,

    /// Open right-click context menu on an SSD titlebar (if any).
    pub context_menu: Option<crate::decorations::TitlebarContextMenu>,

    // -- Output --
    pub output: Output,
    /// All outputs (the first is also stored in `output` for backwards compatibility).
    pub outputs: Vec<Output>,

    // -- Timing --
    pub clock: Clock<Monotonic>,

    // -- Readiness --
    pub socket_name: String,
    /// Whether `--print-env` was requested (deferred until `XWayland` ready if applicable).
    pub print_env: bool,
    /// File descriptor for readiness notification (deferred until `XWayland` ready if applicable).
    pub ready_fd: Option<i32>,

    // -- Lifecycle --
    pub start_time: std::time::Instant,
    pub timeout: Option<std::time::Duration>,
    pub running: bool,
    /// Active backend name ("headless", "winit", or "drm") — for status reporting.
    pub backend_name: &'static str,

    // -- Security policy --
    pub security_policy: crate::security::SecurityPolicy,

    // -- Configuration --
    pub config: crate::config::CompositorConfig,

    // -- Child program --
    pub exit_with_child: bool,
    /// Child command to spawn after readiness (deferred if `XWayland` is pending).
    pub child_command: Vec<String>,

    // -- XWayland --
    pub xwayland: Option<crate::xwayland::XWaylandState>,
    pub xwayland_shell_state: Option<smithay::wayland::xwayland_shell::XWaylandShellState>,

    // -- UI rendering --
    pub titlebar_renderer: crate::ui::TitlebarRenderer,

    /// Separate [`TitlebarRenderer`](crate::ui::TitlebarRenderer) whose egui
    /// painter (and VAO) lives on the screenshot renderer's GL context.
    ///
    /// VAOs are per-context in OpenGL — they are NOT shared even when the
    /// parent and child EGL contexts share textures via `EGLContext::new_shared`.
    /// Using the main titlebar renderer with the screenshot renderer would cause
    /// `GL_INVALID_OPERATION in glBindVertexArray(non-gen name)`.
    pub screenshot_titlebar_renderer: crate::ui::TitlebarRenderer,

    /// Offscreen `GlowRenderer` for screenshots.
    ///
    /// For winit/DRM backends this is pre-initialized with a shared EGL context
    /// so that GL objects (textures, sync objects) from the main renderer are
    /// accessible.  For headless it is lazily created as a standalone renderer.
    ///
    /// Uses the `Option::take()` pattern to avoid borrow conflicts with `&mut State`.
    pub screenshot_renderer: Option<smithay::backend::renderer::glow::GlowRenderer>,

    // -- DRM backend --
    pub drm_backend: Option<crate::backend::drm::DrmBackendState>,
}

impl State {
    /// Create a new compositor state, registering all Wayland globals.
    ///
    /// The `Display` is managed as a calloop event source; we receive its
    /// `DisplayHandle` here to register globals and store for later use.
    ///
    /// # Panics
    ///
    /// Panics if the keyboard cannot be added to the seat.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    #[must_use]
    pub fn new(
        dh: DisplayHandle,
        loop_handle: LoopHandle<'static, Self>,
        loop_signal: LoopSignal,
        socket_name: String,
        output_size: Size<i32, Physical>,
        timeout: Option<std::time::Duration>,
        xkb_config: XkbConfig<'_>,
        output_count: u32,
        output_layout: crate::multi_output::OutputLayout,
        security_policy: crate::security::SecurityPolicy,
        config: crate::config::CompositorConfig,
    ) -> Self {
        let clock = Clock::new();

        // Core protocols
        let compositor_state = CompositorState::new::<Self>(&dh);
        let shm_state = {
            use smithay::reexports::wayland_server::protocol::wl_shm::Format as ShmFormat;
            ShmState::new::<Self>(
                &dh,
                vec![ShmFormat::Abgr8888, ShmFormat::Xbgr8888, ShmFormat::Abgr16161616f, ShmFormat::Xbgr16161616f],
            )
        };
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<Self>(&dh);
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&dh);
        let viewporter_state = ViewporterState::new::<Self>(&dh);
        let fractional_scale_state = FractionalScaleManagerState::new::<Self>(&dh);

        // DMA-BUF — advertise common formats with Linear modifier.
        // Real compositors query the GPU; these four are universally supported.
        let mut dmabuf_state = DmabufState::new();
        let dmabuf_formats = {
            use smithay::backend::allocator::{Format, Fourcc, Modifier};
            vec![
                Format { code: Fourcc::Argb8888, modifier: Modifier::Linear },
                Format { code: Fourcc::Xrgb8888, modifier: Modifier::Linear },
                Format { code: Fourcc::Abgr8888, modifier: Modifier::Linear },
                Format { code: Fourcc::Xbgr8888, modifier: Modifier::Linear },
            ]
        };
        let _dmabuf_global = dmabuf_state.create_global::<Self>(&dh, dmabuf_formats);

        let xdg_activation_state = XdgActivationState::new::<Self>(&dh);
        let pointer_constraints_state = PointerConstraintsState::new::<Self>(&dh);
        let relative_pointer_state = RelativePointerManagerState::new::<Self>(&dh);
        let single_pixel_buffer_state = SinglePixelBufferState::new::<Self>(&dh);
        // Clock ID 1 = CLOCK_MONOTONIC (matching our Clock<Monotonic>)
        let presentation_state = PresentationState::new::<Self>(&dh, 1);
        let keyboard_shortcuts_inhibit_state = KeyboardShortcutsInhibitState::new::<Self>(&dh);
        let text_input_state = TextInputManagerState::new::<Self>(&dh);
        // Input method access is gated by the security policy when restrictive.
        let policy_is_restrictive = security_policy.is_restrictive();
        let input_method_state = InputMethodManagerState::new::<Self, _>(&dh, move |_| !policy_is_restrictive);
        let idle_notify_state = IdleNotifierState::<Self>::new(&dh, loop_handle.clone());
        // Session lock global filter: advertise to all clients (handler-level
        // enforcement in SessionLockHandler::lock checks the policy).
        let session_lock_state = SessionLockManagerState::new::<Self, _>(&dh, |_| true);
        let xdg_foreign_state = XdgForeignState::new::<Self>(&dh);
        // Security context global filter: always advertise (handler-level
        // enforcement in context_created checks the app_id against the policy).
        let security_context_state = SecurityContextState::new::<Self, _>(&dh, |_| true);
        let cursor_shape_state = CursorShapeManagerState::new::<Self>(&dh);

        // Create a single seat
        let mut seat = seat_state.new_wl_seat(&dh, "seat0");
        seat.add_keyboard(xkb_config, 200, 25).expect("Failed to add keyboard to seat");
        seat.add_pointer();

        // Create the output(s)
        let mut space = Space::default();

        // Priority: [[output]] config sections > --outputs/--width/--height CLI flags
        let (output, outputs) = if !config.output.is_empty() {
            // Config-file output definitions override CLI flags
            let configs: Vec<crate::multi_output::OutputConfig> = config
                .output
                .iter()
                .enumerate()
                .map(|(i, cfg)| crate::multi_output::OutputConfig {
                    name: format!("PLATYNUI-{}", i + 1),
                    size: (cfg.width.cast_signed(), cfg.height.cast_signed()).into(),
                    refresh: 60_000,
                    position: (cfg.x, cfg.y),
                })
                .collect();
            let outputs = crate::multi_output::create_outputs(&configs, &dh, &mut space);
            let primary = outputs[0].clone();
            (primary, outputs)
        } else if output_count <= 1 {
            // Single output (original path)
            let output = Output::new(
                "PLATYNUI-1".to_string(),
                PhysicalProperties {
                    size: (0, 0).into(),
                    subpixel: Subpixel::Unknown,
                    make: "PlatynUI".to_string(),
                    model: "Wayland Compositor".to_string(),
                },
            );

            let mode = smithay::output::Mode { size: output_size, refresh: 60_000 };
            output.change_current_state(Some(mode), None, None, Some((0, 0).into()));
            output.set_preferred(mode);
            output.create_global::<Self>(&dh);
            space.map_output(&output, (0, 0));

            let outputs = vec![output.clone()];
            (output, outputs)
        } else {
            // Multi-monitor
            let configs = crate::multi_output::create_output_configs(
                output_count,
                output_size.w.unsigned_abs(),
                output_size.h.unsigned_abs(),
                output_layout,
            );
            let outputs = crate::multi_output::create_outputs(&configs, &dh, &mut space);
            let primary = outputs[0].clone();
            (primary, outputs)
        };

        Self {
            compositor_state,
            shm_state,
            output_manager_state,
            seat_state,
            xdg_shell_state,
            xdg_decoration_state,
            data_device_state,
            primary_selection_state,
            dmabuf_state,
            viewporter_state,
            fractional_scale_state,
            xdg_activation_state,
            pointer_constraints_state,
            relative_pointer_state,
            single_pixel_buffer_state,
            presentation_state,
            keyboard_shortcuts_inhibit_state,
            text_input_state,
            input_method_state,
            idle_notify_state,
            session_lock_state,
            xdg_foreign_state,
            security_context_state,
            cursor_shape_state,
            space,
            popup_manager: PopupManager::default(),
            display_handle: dh,
            loop_handle,
            loop_signal,
            seat,
            cursor_status: CursorImageStatus::default_named(),
            pointer_location: (0.0, 0.0).into(),
            compositor_cursor_shape: crate::decorations::CursorShape::Default,
            pre_maximize_positions: Vec::new(),
            pre_fullscreen_states: Vec::new(),
            minimized_windows: Vec::new(),
            last_titlebar_click: None,
            context_menu: None,
            output,
            outputs,
            clock,
            socket_name,
            start_time: std::time::Instant::now(),
            timeout,
            running: true,
            backend_name: "unknown",
            print_env: false,
            ready_fd: None,
            security_policy,
            titlebar_renderer: crate::ui::TitlebarRenderer::new(&config.font.family, config.font.size),
            screenshot_titlebar_renderer: crate::ui::TitlebarRenderer::new(&config.font.family, config.font.size),
            screenshot_renderer: None,
            config,
            exit_with_child: false,
            child_command: Vec::new(),
            xwayland: None,
            xwayland_shell_state: None,
            drm_backend: None,
        }
    }

    /// Spawn the child program (if configured) and optionally monitor its exit.
    ///
    /// Call this after compositor readiness is established (Wayland socket ready,
    /// optionally `XWayland` ready). The child inherits `WAYLAND_DISPLAY`, `DISPLAY`,
    /// and `XDG_RUNTIME_DIR` from the compositor environment.
    pub fn spawn_child_if_requested(&self) {
        if let Some(child) = crate::child::spawn_child(&self.child_command)
            && self.exit_with_child
            && let Err(err) = crate::child::monitor_child_exit(&self.loop_handle, child)
        {
            tracing::error!(%err, "failed to register child exit monitor");
        }
    }

    /// Compute the combined bounding box of all outputs in the compositor space.
    ///
    /// Returns the smallest axis-aligned rectangle that contains every output.
    /// Falls back to the primary output geometry if no outputs are registered.
    #[must_use]
    pub fn combined_output_geometry(&self) -> Rectangle<i32, Logical> {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for output in &self.outputs {
            if let Some(geo) = self.space.output_geometry(output) {
                min_x = min_x.min(geo.loc.x);
                min_y = min_y.min(geo.loc.y);
                max_x = max_x.max(geo.loc.x + geo.size.w);
                max_y = max_y.max(geo.loc.y + geo.size.h);
            }
        }

        if min_x <= max_x && min_y <= max_y {
            Rectangle::new((min_x, min_y).into(), (max_x - min_x, max_y - min_y).into())
        } else {
            self.space.output_geometry(&self.output).unwrap_or_default()
        }
    }

    /// Find the output whose geometry contains the given point.
    ///
    /// Falls back to the primary output if no output contains the point.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn output_at_point(&self, point: Point<f64, Logical>) -> &Output {
        let ix = point.x as i32;
        let iy = point.y as i32;

        for output in &self.outputs {
            if let Some(geo) = self.space.output_geometry(output)
                && ix >= geo.loc.x
                && ix < geo.loc.x + geo.size.w
                && iy >= geo.loc.y
                && iy < geo.loc.y + geo.size.h
            {
                return output;
            }
        }

        &self.output
    }

    /// Return the output geometry that best contains the given window.
    ///
    /// Uses the window's center point to determine which output it belongs to.
    /// Falls back to the primary output if the window is not mapped.
    #[must_use]
    pub fn output_geometry_for_window(&self, window: &Window) -> Rectangle<i32, Logical> {
        let output = if let Some(loc) = self.space.element_location(window) {
            let size = window.geometry().size;
            let center = Point::from((f64::from(loc.x + size.w / 2), f64::from(loc.y + size.h / 2)));
            self.output_at_point(center)
        } else {
            &self.output
        };

        self.space.output_geometry(output).unwrap_or_default()
    }

    /// Return the current time for frame callbacks.
    #[must_use]
    pub fn frame_clock_now(&self) -> std::time::Duration {
        self.clock.now().into()
    }
}

// -- Handler trait implementations --

impl OutputHandler for State {
    fn output_bound(
        &mut self,
        _output: Output,
        _wl_output: smithay::reexports::wayland_server::protocol::wl_output::WlOutput,
    ) {
        // Default: nothing special needed
    }
}

impl FractionalScaleHandler for State {
    fn new_fractional_scale(&mut self, _surface: smithay::reexports::wayland_server::protocol::wl_surface::WlSurface) {
        // Default: nothing special needed
    }
}

// -- Protocol delegation macros --

delegate_compositor!(State);
delegate_shm!(State);
delegate_output!(State);
delegate_seat!(State);
delegate_xdg_shell!(State);
delegate_xdg_decoration!(State);
delegate_data_device!(State);
delegate_primary_selection!(State);
delegate_dmabuf!(State);
delegate_viewporter!(State);
delegate_fractional_scale!(State);
delegate_presentation!(State);
delegate_xdg_activation!(State);
delegate_pointer_constraints!(State);
delegate_relative_pointer!(State);
delegate_single_pixel_buffer!(State);
delegate_keyboard_shortcuts_inhibit!(State);
delegate_text_input_manager!(State);
delegate_input_method_manager!(State);
delegate_idle_notify!(State);
delegate_session_lock!(State);
delegate_xdg_foreign!(State);
delegate_security_context!(State);
delegate_cursor_shape!(State);
