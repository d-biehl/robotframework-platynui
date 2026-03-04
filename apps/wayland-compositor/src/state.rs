//! Compositor state — central struct holding all protocol states and runtime data.

use std::collections::HashMap;

use smithay::{
    delegate_compositor, delegate_content_type, delegate_cursor_shape, delegate_data_control, delegate_data_device,
    delegate_dmabuf, delegate_fractional_scale, delegate_idle_notify, delegate_input_method_manager,
    delegate_keyboard_shortcuts_inhibit, delegate_layer_shell, delegate_output, delegate_pointer_constraints,
    delegate_presentation, delegate_primary_selection, delegate_relative_pointer, delegate_seat,
    delegate_security_context, delegate_session_lock, delegate_shm, delegate_single_pixel_buffer,
    delegate_text_input_manager, delegate_viewporter, delegate_virtual_keyboard_manager, delegate_xdg_activation,
    delegate_xdg_decoration, delegate_xdg_foreign, delegate_xdg_shell,
    desktop::{PopupManager, Space, Window, layer_map_for_output},
    input::{Seat, SeatState, keyboard::XkbConfig, pointer::CursorImageStatus},
    output::{Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{LoopHandle, LoopSignal},
        wayland_server::DisplayHandle,
    },
    utils::{Clock, Logical, Monotonic, Physical, Point, Rectangle, Size},
    wayland::{
        compositor::CompositorState,
        content_type::ContentTypeState,
        cursor_shape::CursorShapeManagerState,
        dmabuf::DmabufState,
        foreign_toplevel_list::{ForeignToplevelHandle, ForeignToplevelListState},
        fractional_scale::{FractionalScaleHandler, FractionalScaleManagerState},
        idle_notify::IdleNotifierState,
        input_method::InputMethodManagerState,
        keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitState,
        output::{OutputHandler, OutputManagerState},
        pointer_constraints::PointerConstraintsState,
        presentation::PresentationState,
        relative_pointer::RelativePointerManagerState,
        security_context::SecurityContextState,
        selection::{
            data_device::DataDeviceState, primary_selection::PrimarySelectionState, wlr_data_control::DataControlState,
        },
        session_lock::SessionLockManagerState,
        shell::{
            wlr_layer::WlrLayerShellState,
            xdg::{XdgShellState, decoration::XdgDecorationState},
        },
        shm::ShmState,
        single_pixel_buffer::SinglePixelBufferState,
        text_input::TextInputManagerState,
        viewporter::ViewporterState,
        virtual_keyboard::VirtualKeyboardManagerState,
        xdg_activation::XdgActivationState,
        xdg_foreign::XdgForeignState,
    },
};

/// Saved state for a maximized window: (window, previous position, previous size).
type PreMaximizeState = (Window, Point<i32, Logical>, Option<Size<i32, Logical>>);
/// Saved state for a fullscreen window: (window, previous position, previous pending size).
type PreFullscreenState = (Window, Point<i32, Logical>, Option<Size<i32, Logical>>);

/// POSIX `CLOCK_MONOTONIC` clock ID for the Wayland presentation protocol.
const CLOCK_MONOTONIC: u32 = 1;
/// Initial delay before keyboard key repeat starts (milliseconds).
const KEY_REPEAT_DELAY_MS: i32 = 200;
/// Number of key repeats per second once repeat has started.
const KEY_REPEAT_RATE: i32 = 25;

/// Default output refresh rate in millihertz (60 Hz).
pub const DEFAULT_REFRESH_MHTZ: i32 = 60_000;

/// Default background clear color (dark grey, fully opaque).
pub const BACKGROUND_COLOR: [f32; 4] = [0.1, 0.1, 0.1, 1.0];

/// Central compositor state holding all Wayland protocol states and runtime data.
#[allow(clippy::struct_field_names, clippy::struct_excessive_bools)]
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

    // -- Phase 3: Automation protocols --
    pub layer_shell_state: WlrLayerShellState,
    pub data_control_state: DataControlState,
    pub content_type_state: ContentTypeState,
    pub virtual_keyboard_state: VirtualKeyboardManagerState,
    pub virtual_pointer_global: smithay::reexports::wayland_server::backend::GlobalId,
    pub output_management_global: smithay::reexports::wayland_server::backend::GlobalId,

    /// Weak references to all bound output management instances.
    ///
    /// Used to send `finished` events on old head/mode objects and re-send
    /// updated state when outputs are reconfigured (e.g. via `wlr-randr`).
    pub output_managers: Vec<smithay::reexports::wayland_server::Weak<
        smithay::reexports::wayland_protocols_wlr::output_management::v1::server::zwlr_output_manager_v1::ZwlrOutputManagerV1,
    >>,

    // -- Screencopy protocol --
    pub screencopy_globals: (
        smithay::reexports::wayland_server::backend::GlobalId,
        smithay::reexports::wayland_server::backend::GlobalId,
        smithay::reexports::wayland_server::backend::GlobalId,
    ),
    /// Pending frame capture state, keyed by frame object ID.
    pub pending_captures:
        HashMap<smithay::reexports::wayland_server::backend::ObjectId, crate::handlers::screencopy::PendingCapture>,

    // -- Foreign-toplevel protocols --
    pub wlr_foreign_toplevel_state: crate::handlers::foreign_toplevel::WlrForeignToplevelManagerState,
    pub ext_foreign_toplevel_list_state: smithay::wayland::foreign_toplevel_list::ForeignToplevelListState,
    /// Ext-foreign-toplevel handle per window (for title / `app_id` updates).
    pub ext_toplevel_handles: Vec<(Window, ForeignToplevelHandle)>,

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

    /// Currently physically pressed pointer buttons (tracked for focus-loss cleanup).
    pub pressed_buttons: Vec<u32>,

    /// Compositor-driven cursor shape (for SSD resize borders, title bar).
    pub compositor_cursor_shape: crate::decorations::CursorShape,

    /// Cursor theme state for rendering xcursor images (Named/Default cursors).
    pub cursor_theme: crate::cursor::CursorThemeState,

    /// Pre-maximize window states — restored when unmaximizing.
    pub pre_maximize_positions: Vec<PreMaximizeState>,

    /// Pre-fullscreen window states — restored when leaving fullscreen.
    pub pre_fullscreen_states: Vec<PreFullscreenState>,

    /// Minimized windows — removed from the space but kept alive.
    /// Stores the window and its last position for restoring.
    pub minimized_windows: Vec<(Window, Point<i32, Logical>)>,

    /// The last window that had keyboard focus.
    /// Used to send foreign-toplevel deactivation state updates on focus change.
    pub last_focused_window: Option<Window>,

    /// Last titlebar click time — used to detect double-clicks for maximize toggle.
    pub last_titlebar_click: Option<std::time::Instant>,

    /// Titlebar button that is currently pressed (deferred until release).
    ///
    /// Standard UI behaviour: button actions fire on mouse *release*, not
    /// press.  This lets the user cancel by moving the pointer away before
    /// releasing.  Only Close / Maximize / Minimize are deferred; `TitleBar`
    /// (move / double-click) still triggers on press.
    pub pressed_titlebar_button: Option<(Window, crate::decorations::DecorationClick)>,

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

    /// Flag set by output-management apply (wlr-randr) to signal that the
    /// winit event loop should rebuild its damage tracker and reconfigure
    /// windows for the new output dimensions.
    pub output_config_changed: bool,

    /// Preview scale for the winit backend window.
    ///
    /// Scales down both the window size and the rendering resolution so that
    /// large multi-output setups fit on screen.  Wayland clients still see
    /// the real output scale/mode.  Default `1.0` (no scaling).
    pub window_scale: f64,

    /// Whether to render the cursor as a software element in the frame buffer.
    ///
    /// When `true`, the xcursor theme image is composited into every frame.
    /// When `false`, Named cursors are delegated to the host windowing system.
    pub software_cursor: bool,
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
        output_scale: f64,
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
        let presentation_state = PresentationState::new::<Self>(&dh, CLOCK_MONOTONIC);
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

        // Phase 3: Automation protocols
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let data_control_state = DataControlState::new::<Self, _>(&dh, Some(&primary_selection_state), |_| true);
        let content_type_state = ContentTypeState::new::<Self>(&dh);
        let virtual_keyboard_state = {
            let restrict = security_policy.is_restrictive();
            VirtualKeyboardManagerState::new::<Self, _>(&dh, move |_| !restrict)
        };
        let virtual_pointer_global = {
            let restrict = security_policy.is_restrictive();
            crate::handlers::virtual_pointer::init_virtual_pointer_manager(&dh, move |_| !restrict)
        };
        let output_management_global = {
            let restrict = security_policy.is_restrictive();
            crate::handlers::output_management::init_output_management(&dh, move |_| !restrict)
        };
        let screencopy_globals = {
            let restrict = security_policy.is_restrictive();
            crate::handlers::screencopy::init_screencopy(&dh, move |_| !restrict)
        };

        // Foreign-toplevel protocols
        let wlr_foreign_toplevel_state =
            crate::handlers::foreign_toplevel::WlrForeignToplevelManagerState::new::<Self>(&dh);
        let ext_foreign_toplevel_list_state = ForeignToplevelListState::new::<Self>(&dh);

        let mut seat = seat_state.new_wl_seat(&dh, "seat0");
        seat.add_keyboard(xkb_config, KEY_REPEAT_DELAY_MS, KEY_REPEAT_RATE).expect("Failed to add keyboard to seat");
        seat.add_pointer();

        let mut space = Space::default();

        // Priority: [[output]] config sections > --outputs/--width/--height CLI flags.
        // output_count == 0 means the backend discovers its own outputs (e.g.
        // DRM for real hardware monitors) — skip virtual output creation.
        let (output, outputs) = if output_count == 0 {
            // Backend-managed outputs (e.g. DRM).  Create a dummy placeholder
            // for `state.output`; the backend will overwrite it and populate
            // `state.outputs` with real hardware outputs.
            let output = Output::new(
                "placeholder".to_string(),
                PhysicalProperties {
                    size: (0, 0).into(),
                    subpixel: Subpixel::Unknown,
                    make: "PlatynUI".to_string(),
                    model: "Placeholder".to_string(),
                },
            );
            let mode = smithay::output::Mode { size: output_size, refresh: DEFAULT_REFRESH_MHTZ };
            output.change_current_state(Some(mode), None, None, None);
            output.set_preferred(mode);
            // No Wayland global, no space mapping — the backend adds real outputs.
            (output, Vec::new())
        } else if !config.output.is_empty() {
            let configs: Vec<crate::multi_output::OutputConfig> = config
                .output
                .iter()
                .enumerate()
                .map(|(i, cfg)| crate::multi_output::OutputConfig {
                    name: format!("PLATYNUI-{}", i + 1),
                    size: (cfg.width.cast_signed(), cfg.height.cast_signed()).into(),
                    refresh: DEFAULT_REFRESH_MHTZ,
                    position: (cfg.x, cfg.y),
                    scale: if cfg.scale > 0.0 { cfg.scale } else { 1.0 },
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

            let mode = smithay::output::Mode { size: output_size, refresh: DEFAULT_REFRESH_MHTZ };
            let scale_opt = if output_scale > 0.0 && (output_scale - 1.0).abs() > f64::EPSILON {
                Some(smithay::output::Scale::Fractional(output_scale))
            } else {
                None
            };
            output.change_current_state(Some(mode), None, scale_opt, Some((0, 0).into()));
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
                output_scale,
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
            layer_shell_state,
            data_control_state,
            content_type_state,
            virtual_keyboard_state,
            virtual_pointer_global,
            output_management_global,
            output_managers: Vec::new(),
            screencopy_globals,
            pending_captures: HashMap::new(),
            wlr_foreign_toplevel_state,
            ext_foreign_toplevel_list_state,
            ext_toplevel_handles: Vec::new(),
            space,
            popup_manager: PopupManager::default(),
            display_handle: dh,
            loop_handle,
            loop_signal,
            seat,
            cursor_status: CursorImageStatus::default_named(),
            pointer_location: (0.0, 0.0).into(),
            pressed_buttons: Vec::new(),
            compositor_cursor_shape: crate::decorations::CursorShape::Default,
            cursor_theme: crate::cursor::CursorThemeState::new(),
            pre_maximize_positions: Vec::new(),
            pre_fullscreen_states: Vec::new(),
            minimized_windows: Vec::new(),
            last_focused_window: None,
            last_titlebar_click: None,
            pressed_titlebar_button: None,
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
            output_config_changed: false,
            window_scale: 1.0,
            software_cursor: false,
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

    /// Get the keyboard handle from the seat.
    ///
    /// # Panics
    ///
    /// Panics if the seat has no keyboard capability.  This should never
    /// happen because `State::new()` unconditionally adds a keyboard.
    #[must_use]
    pub fn keyboard(&self) -> smithay::input::keyboard::KeyboardHandle<Self> {
        self.seat.get_keyboard().expect("seat has no keyboard capability")
    }

    /// Get the pointer handle from the seat.
    ///
    /// # Panics
    ///
    /// Panics if the seat has no pointer capability.  This should never
    /// happen because `State::new()` unconditionally adds a pointer.
    #[must_use]
    pub fn pointer(&self) -> smithay::input::pointer::PointerHandle<Self> {
        self.seat.get_pointer().expect("seat has no pointer capability")
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

    /// Compute the combined physical pixel size needed for all outputs.
    ///
    /// For mixed-scale setups, iterates each output individually to compute its
    /// physical extent (logical position × scale + mode size) and returns the
    /// bounding box in physical pixels.  For uniform-scale setups, this reduces
    /// to `combined_logical × scale`.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn combined_physical_size(&self) -> Size<i32, Physical> {
        let max_scale = self.max_output_scale();
        let combined = self.combined_output_geometry();
        let w = (f64::from(combined.size.w) * max_scale).ceil() as i32;
        let h = (f64::from(combined.size.h) * max_scale).ceil() as i32;
        (w, h).into()
    }

    /// Return the maximum scale factor across all outputs.
    ///
    /// When rendering all outputs into a single framebuffer (winit backend),
    /// every element must use this scale so that the physical pixel positions
    /// match.  Using a per-output scale would create a non-linear mapping
    /// between the framebuffer and logical coordinates, breaking pointer
    /// hit-testing after the first window move.
    #[must_use]
    pub fn max_output_scale(&self) -> f64 {
        self.outputs.iter().map(|o| o.current_scale().fractional_scale()).fold(1.0_f64, f64::max)
    }

    /// Compute the render size for the winit preview window.
    ///
    /// Applies [`window_scale`](Self::window_scale) to the combined physical
    /// size.  When `window_scale == 1.0` this returns the same value as
    /// [`combined_physical_size`](Self::combined_physical_size).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn render_size(&self) -> Size<i32, Physical> {
        let full = self.combined_physical_size();
        if (self.window_scale - 1.0).abs() <= f64::EPSILON {
            return full;
        }
        let w = (f64::from(full.w) * self.window_scale).ceil() as i32;
        let h = (f64::from(full.h) * self.window_scale).ceil() as i32;
        (w, h).into()
    }

    /// Resize outputs that sit on the bounding-box edges to match a new window
    /// size, so that the combined layout fills the winit window exactly.
    ///
    /// Outputs touching the **right** edge of the current bounding box get their
    /// width adjusted; outputs touching the **bottom** edge get their height
    /// adjusted.  Corner outputs (both right and bottom) get both.  Interior
    /// outputs and those only on the left/top edges stay untouched.
    ///
    /// This keeps pointer mapping consistent: the logical layout always fills
    /// the winit window, so `position_transformed(logical_size)` is correct.
    #[allow(clippy::cast_possible_truncation)]
    pub fn resize_edge_outputs(&mut self, new_window_size: Size<i32, Physical>) {
        // Minimum logical size for an output (don't shrink below this).
        const MIN_LOGICAL: f64 = 100.0;

        let bbox = self.combined_output_geometry();
        if bbox.size.w <= 0 || bbox.size.h <= 0 {
            return;
        }

        let max_scale = self.max_output_scale();
        let ws = (self.window_scale * max_scale).max(f64::EPSILON);

        // Derive the new logical bounding box size from the window's physical pixels.
        let new_logical_w = f64::from(new_window_size.w) / ws;
        let new_logical_h = f64::from(new_window_size.h) / ws;
        let delta_w = new_logical_w - f64::from(bbox.size.w);
        let delta_h = new_logical_h - f64::from(bbox.size.h);

        // Nothing to do if the size didn't meaningfully change.
        if delta_w.abs() < 0.5 && delta_h.abs() < 0.5 {
            return;
        }

        let right_edge = bbox.loc.x + bbox.size.w;
        let bottom_edge = bbox.loc.y + bbox.size.h;

        for output in &self.outputs {
            let Some(geo) = self.space.output_geometry(output) else {
                continue;
            };
            let scale = output.current_scale().fractional_scale();
            let mode = output.current_mode().unwrap_or(smithay::output::Mode {
                size: (geo.size.w, geo.size.h).into(),
                refresh: DEFAULT_REFRESH_MHTZ,
            });

            let mut logical_w = f64::from(mode.size.w) / scale;
            let mut logical_h = f64::from(mode.size.h) / scale;

            let touches_right = geo.loc.x + geo.size.w == right_edge;
            let touches_bottom = geo.loc.y + geo.size.h == bottom_edge;

            if touches_right && delta_w.abs() >= 0.5 {
                logical_w = (logical_w + delta_w).max(MIN_LOGICAL);
            }
            if touches_bottom && delta_h.abs() >= 0.5 {
                logical_h = (logical_h + delta_h).max(MIN_LOGICAL);
            }

            if !touches_right && !touches_bottom {
                continue;
            }

            let new_phys_w = (logical_w * scale).round() as i32;
            let new_phys_h = (logical_h * scale).round() as i32;
            if new_phys_w == mode.size.w && new_phys_h == mode.size.h {
                continue;
            }

            let new_mode = smithay::output::Mode { size: (new_phys_w, new_phys_h).into(), refresh: mode.refresh };
            // Remove stale modes so wlr-randr doesn't accumulate one
            // entry per resize event.  Keep only the new current mode.
            for old in output.modes() {
                if old != new_mode {
                    output.delete_mode(old);
                }
            }
            output.change_current_state(Some(new_mode), None, None, None);
            output.set_preferred(new_mode);

            tracing::debug!(
                name = output.name(),
                old_w = mode.size.w,
                old_h = mode.size.h,
                new_w = new_phys_w,
                new_h = new_phys_h,
                "resized edge output for window resize",
            );
        }
    }

    /// Re-arrange all layer maps after output mode/scale changes.
    ///
    /// This recalculates the layout of every layer surface (panels, bars,
    /// docks) on every output and sends updated configure events.  Must be
    /// called whenever output modes or scales change so that layer surfaces
    /// get the correct dimensions and exclusive zones are updated.
    pub fn rearrange_layer_maps(&self) {
        for output in &self.outputs {
            let mut map = layer_map_for_output(output);
            map.arrange();
            // Send pending configure to every layer surface on this output
            for layer in map.layers() {
                layer.layer_surface().send_pending_configure();
            }
        }
    }

    /// Reconfigure windows for the current output dimensions.
    ///
    /// Called after output configuration changes (scale, mode, position) to
    /// adjust window sizes and positions to the new logical viewport.
    /// Also re-arranges layer maps so panels/bars adapt to the new sizes.
    ///
    /// - **Maximized / fullscreen** windows are resized to fill the new output.
    /// - **Normal (floating)** windows are clamped so that at least the
    ///   titlebar (or a minimum strip) remains visible, ensuring the user
    ///   can always grab and reposition the window.
    pub fn reconfigure_windows_for_outputs(&mut self) {
        use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

        // Re-arrange layer surfaces first so that usable_geometry (which
        // queries the layer map's non_exclusive_zone) is up to date.
        self.rearrange_layer_maps();

        let windows: Vec<Window> = self.space.elements().cloned().collect();

        for window in &windows {
            // --- Wayland toplevel ---
            if let Some(toplevel) = window.toplevel() {
                let (is_maximized, is_fullscreen) = toplevel.with_pending_state(|s| {
                    (
                        s.states.contains(xdg_toplevel::State::Maximized),
                        s.states.contains(xdg_toplevel::State::Fullscreen),
                    )
                });

                if is_fullscreen {
                    let output_geo = self.output_geometry_for_window(window);
                    toplevel.with_pending_state(|s| {
                        s.size = Some(output_geo.size);
                    });
                    self.space.map_element(window.clone(), output_geo.loc, true);
                    toplevel.send_configure();
                } else if is_maximized {
                    let usable_geo = self.usable_geometry_for_window(window);
                    let y_offset = if crate::decorations::window_has_ssd(window) {
                        crate::decorations::TITLEBAR_HEIGHT
                    } else {
                        0
                    };
                    toplevel.with_pending_state(|s| {
                        s.size =
                            Some((usable_geo.size.w, usable_geo.size.h - crate::decorations::TITLEBAR_HEIGHT).into());
                    });
                    self.space.map_element(window.clone(), (usable_geo.loc.x, usable_geo.loc.y + y_offset), true);
                    toplevel.send_configure();
                }
                continue;
            }

            // --- X11 (XWayland) ---
            if let Some(x11) = window.x11_surface() {
                if x11.is_fullscreen() {
                    let output_geo = self.output_geometry_for_window(window);
                    if let Err(err) = x11.configure(output_geo) {
                        tracing::warn!(%err, "failed to reconfigure fullscreen X11 window");
                    }
                    self.space.map_element(window.clone(), output_geo.loc, true);
                } else if x11.is_maximized() {
                    let usable_geo = self.usable_geometry_for_window(window);
                    let y_offset = if crate::decorations::window_has_ssd(window) {
                        crate::decorations::TITLEBAR_HEIGHT
                    } else {
                        0
                    };
                    let max_size = Size::from((usable_geo.size.w, usable_geo.size.h - y_offset));
                    if let Err(err) = x11.configure(Rectangle::new(
                        (usable_geo.loc.x, usable_geo.loc.y + y_offset).into(),
                        max_size,
                    )) {
                        tracing::warn!(%err, "failed to reconfigure maximized X11 window");
                    }
                    self.space
                        .map_element(window.clone(), (usable_geo.loc.x, usable_geo.loc.y + y_offset), true);
                }
            }
        }

        // --- Clamp normal (floating) windows into the visible area ---
        //
        // After maximized/fullscreen windows have been resized, ensure that
        // every remaining window has at least its titlebar (or a minimum
        // strip) within the combined output bounds so the user can still
        // reach it.  Windows are repositioned but never resized.
        self.clamp_floating_windows_to_outputs();
    }

    /// Move floating windows so they remain reachable after output changes.
    ///
    /// Maximized and fullscreen windows are skipped (already handled).
    /// For every other window the position is clamped so that at least the
    /// titlebar stays visible within the combined output bounding box.
    fn clamp_floating_windows_to_outputs(&mut self) {
        use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

        let bbox = self.combined_output_geometry();
        if bbox.size.w <= 0 || bbox.size.h <= 0 {
            return;
        }

        // Minimum number of pixels that must remain visible on each axis.
        // Using the titlebar height ensures the user can always grab the
        // window to drag it back.
        let min_visible = crate::decorations::TITLEBAR_HEIGHT;

        let windows: Vec<Window> = self.space.elements().cloned().collect();

        for window in &windows {
            // Skip maximized / fullscreen Wayland toplevels.
            if let Some(toplevel) = window.toplevel() {
                let dominated = toplevel.with_pending_state(|s| {
                    s.states.contains(xdg_toplevel::State::Maximized)
                        || s.states.contains(xdg_toplevel::State::Fullscreen)
                });
                if dominated {
                    continue;
                }
            }

            // Skip maximized / fullscreen X11 windows.
            if let Some(x11) = window.x11_surface()
                && (x11.is_maximized() || x11.is_fullscreen())
            {
                continue;
            }

            let Some(loc) = self.space.element_location(window) else {
                continue;
            };

            let win_size = window.geometry().size;
            let has_ssd = crate::decorations::window_has_ssd(window);
            let titlebar_h = if has_ssd { crate::decorations::TITLEBAR_HEIGHT } else { 0 };

            // The visual top of the window includes the titlebar drawn above
            // the element location.
            let visual_w = win_size.w;
            let visual_h = win_size.h + titlebar_h;

            let mut new_x = loc.x;
            let mut new_y = loc.y;

            // Horizontal: ensure at least `min_visible` pixels of the window
            // are inside the bounding box.
            // If the window is too far to the right, pull it left.
            if new_x > bbox.loc.x + bbox.size.w - min_visible {
                new_x = bbox.loc.x + bbox.size.w - min_visible;
            }
            // If the window is too far to the left, pull it right.
            if new_x + visual_w < bbox.loc.x + min_visible {
                new_x = bbox.loc.x + min_visible - visual_w;
            }

            // Vertical: keep the titlebar (or top strip) visible.
            // Too far down — at least the top edge must be within bounds.
            let visual_new_top = new_y - titlebar_h;
            if visual_new_top > bbox.loc.y + bbox.size.h - min_visible {
                new_y = bbox.loc.y + bbox.size.h - min_visible + titlebar_h;
            }
            // Too far up — at least the titlebar must peek out from the top.
            if visual_new_top + visual_h < bbox.loc.y + min_visible {
                new_y = bbox.loc.y + min_visible - visual_h + titlebar_h;
            }

            if new_x != loc.x || new_y != loc.y {
                self.space.map_element(window.clone(), (new_x, new_y), false);

                // For X11 windows, also update the X11 surface geometry.
                if let Some(x11) = window.x11_surface() {
                    let _ = x11.configure(Rectangle::new((new_x, new_y).into(), win_size));
                }

                tracing::debug!(
                    old_x = loc.x,
                    old_y = loc.y,
                    new_x,
                    new_y,
                    "clamped floating window into visible area",
                );
            }
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

    /// Return the output that best contains the given window.
    ///
    /// Uses the window's center point to determine which output it belongs to.
    /// Falls back to the primary output if the window is not mapped.
    #[must_use]
    pub fn output_for_window(&self, window: &Window) -> &Output {
        if let Some(loc) = self.space.element_location(window) {
            let size = window.geometry().size;
            let center = Point::from((f64::from(loc.x + size.w / 2), f64::from(loc.y + size.h / 2)));
            self.output_at_point(center)
        } else {
            &self.output
        }
    }

    /// Return the full output geometry that best contains the given window.
    ///
    /// This is the **total** output area including regions reserved by layer
    /// surfaces (panels, bars).  Use [`usable_geometry_for_window`](Self::usable_geometry_for_window)
    /// for maximize / tiling where windows should avoid layer surface zones.
    #[must_use]
    pub fn output_geometry_for_window(&self, window: &Window) -> Rectangle<i32, Logical> {
        self.space.output_geometry(self.output_for_window(window)).unwrap_or_default()
    }

    /// Return the usable output area for the given window, accounting for
    /// exclusive zones claimed by layer surfaces (panels, bars, docks).
    ///
    /// The returned rectangle is in global logical coordinates.  Use this
    /// for maximize and window-tiling operations so windows don't overlap
    /// bars or panels.
    #[must_use]
    pub fn usable_geometry_for_window(&self, window: &Window) -> Rectangle<i32, Logical> {
        let output = self.output_for_window(window);
        self.usable_geometry_for_output(output)
    }

    /// Return the usable output area for the given output, accounting for
    /// exclusive zones claimed by layer surfaces.
    ///
    /// The returned rectangle is in global logical coordinates.
    #[must_use]
    pub fn usable_geometry_for_output(&self, output: &Output) -> Rectangle<i32, Logical> {
        let output_origin = self.space.output_geometry(output).map_or_else(Point::default, |g| g.loc);
        let map = layer_map_for_output(output);
        let mut zone = map.non_exclusive_zone();
        drop(map);
        // non_exclusive_zone is output-local; translate to global coords.
        zone.loc += output_origin;
        zone
    }

    /// Return the current time for frame callbacks.
    #[must_use]
    pub fn frame_clock_now(&self) -> std::time::Duration {
        self.clock.now().into()
    }

    /// Send frame callbacks to all mapped windows and layer surfaces.
    ///
    /// Each window receives a callback from the output its centre lies on.
    /// Layer surfaces receive callbacks from their associated output.
    /// Without frame callbacks, clients that block on the next frame
    /// (like GTK4 during popup creation) would hang indefinitely.
    pub fn send_frame_callbacks(&self) {
        let now = self.frame_clock_now();
        for window in self.space.elements() {
            let output = self
                .output_at_point({
                    let loc = self.space.element_location(window).unwrap_or_default();
                    let size = window.geometry().size;
                    (f64::from(loc.x + size.w / 2), f64::from(loc.y + size.h / 2)).into()
                })
                .clone();
            window.send_frame(&output, now, Some(std::time::Duration::ZERO), |_, _| Some(output.clone()));
        }

        for output in &self.outputs {
            let map = smithay::desktop::layer_map_for_output(output);
            for layer_surface in map.layers() {
                layer_surface.send_frame(output, now, Some(std::time::Duration::ZERO), |_, _| Some(output.clone()));
            }
        }
    }

    /// Refresh the space, clean up popups, and flush pending client events.
    ///
    /// Call at the end of each event loop iteration to ensure
    /// `wl_surface.enter` events are included in the same flush.
    pub fn flush_and_refresh(&mut self) {
        self.space.refresh();
        self.popup_manager.cleanup();

        if let Err(err) = self.display_handle.flush_clients() {
            tracing::warn!(%err, "failed to flush Wayland clients");
        }
    }

    /// Test whether a logical point lies inside any output.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn point_in_any_output(&self, point: Point<f64, Logical>) -> bool {
        let ix = point.x as i32;
        let iy = point.y as i32;
        self.outputs.iter().any(|o| {
            self.space
                .output_geometry(o)
                .is_some_and(|g| ix >= g.loc.x && ix < g.loc.x + g.size.w && iy >= g.loc.y && iy < g.loc.y + g.size.h)
        })
    }

    /// Clamp a point to the nearest location that is inside an output.
    ///
    /// If the point already lies within an output it is returned unchanged.
    /// Otherwise the nearest point on the closest output boundary is used.
    /// This prevents the pointer from sitting in dead zones between
    /// non-rectangular output layouts (e.g. L-shaped multi-monitor).
    #[must_use]
    pub fn clamp_to_outputs(&self, point: Point<f64, Logical>) -> Point<f64, Logical> {
        if self.point_in_any_output(point) {
            return point;
        }

        // Find the closest point on any output rectangle.
        let mut best = point;
        let mut best_dist = f64::MAX;

        for output in &self.outputs {
            let Some(geo) = self.space.output_geometry(output) else {
                continue;
            };
            let clamped_x = point.x.clamp(f64::from(geo.loc.x), f64::from(geo.loc.x + geo.size.w) - 1.0);
            let clamped_y = point.y.clamp(f64::from(geo.loc.y), f64::from(geo.loc.y + geo.size.h) - 1.0);
            let dx = point.x - clamped_x;
            let dy = point.y - clamped_y;
            let dist = dx * dx + dy * dy;
            if dist < best_dist {
                best_dist = dist;
                best = (clamped_x, clamped_y).into();
            }
        }

        best
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
delegate_layer_shell!(State);
delegate_data_control!(State);
delegate_content_type!(State);
delegate_virtual_keyboard_manager!(State);
