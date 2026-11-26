//! Compositor state — central struct holding all protocol states and runtime data.

use std::collections::{HashMap, HashSet};

use smithay::{
    desktop::{PopupManager, Space, Window, layer_map_for_output},
    input::{Seat, SeatState, keyboard::XkbConfig, pointer::CursorImageStatus},
    output::{Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{LoopHandle, LoopSignal},
        wayland_server::DisplayHandle,
    },
    utils::{Clock, Logical, Monotonic, Physical, Point, Rectangle, Size},
    wayland::{
        alpha_modifier::AlphaModifierState,
        compositor::CompositorState,
        content_type::ContentTypeState,
        cursor_shape::CursorShapeManagerState,
        dmabuf::DmabufState,
        foreign_toplevel_list::{ForeignToplevelHandle, ForeignToplevelListState},
        fractional_scale::{FractionalScaleHandler, FractionalScaleManagerState},
        idle_inhibit::IdleInhibitManagerState,
        idle_notify::IdleNotifierState,
        input_method::InputMethodManagerState,
        keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitState,
        output::{OutputHandler, OutputManagerState},
        pointer_constraints::PointerConstraintsState,
        pointer_gestures::PointerGesturesState,
        presentation::PresentationState,
        relative_pointer::RelativePointerManagerState,
        security_context::SecurityContextState,
        selection::{
            data_device::DataDeviceState, ext_data_control::DataControlState as ExtDataControlState,
            primary_selection::PrimarySelectionState, wlr_data_control::DataControlState,
        },
        session_lock::SessionLockManagerState,
        shell::{
            wlr_layer::WlrLayerShellState,
            xdg::{
                XdgShellState,
                decoration::XdgDecorationState,
                dialog::{ToplevelDialogHint, XdgDialogState},
            },
        },
        shm::ShmState,
        single_pixel_buffer::SinglePixelBufferState,
        tablet_manager::TabletManagerState,
        text_input::TextInputManagerState,
        viewporter::ViewporterState,
        virtual_keyboard::VirtualKeyboardManagerState,
        xdg_activation::XdgActivationState,
        xdg_foreign::XdgForeignState,
        xdg_system_bell::XdgSystemBellState,
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
    pub dmabuf_feedback: Option<smithay::wayland::dmabuf::DmabufFeedback>,

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

    // -- Phase 3b: Additional protocol support --
    pub alpha_modifier_state: AlphaModifierState,
    pub idle_inhibit_state: IdleInhibitManagerState,
    /// Surfaces that currently hold an idle inhibitor.
    pub idle_inhibit_surfaces: HashSet<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>,
    pub xdg_dialog_state: XdgDialogState,
    pub xdg_system_bell_state: XdgSystemBellState,
    pub pointer_gestures_state: PointerGesturesState,
    pub tablet_manager_state: TabletManagerState,
    pub tearing_control_global: smithay::reexports::wayland_server::backend::GlobalId,
    pub toplevel_drag_global: smithay::reexports::wayland_server::backend::GlobalId,
    pub toplevel_icon_global: smithay::reexports::wayland_server::backend::GlobalId,
    pub toplevel_tag_global: smithay::reexports::wayland_server::backend::GlobalId,
    /// Per-toplevel icon pixel data set via `xdg-toplevel-icon-v1`.
    ///
    /// Keyed by the `ObjectId` of the toplevel's `wl_surface`.
    pub toplevel_icons: HashMap<
        smithay::reexports::wayland_server::backend::ObjectId,
        crate::handlers::toplevel_icon::ToplevelIconPixels,
    >,
    /// Per-toplevel tag and description set via `xdg-toplevel-tag-v1`.
    ///
    /// Keyed by the `ObjectId` of the toplevel's `wl_surface`.
    pub toplevel_tags: HashMap<
        smithay::reexports::wayland_server::backend::ObjectId,
        crate::handlers::toplevel_tag::ToplevelTagInfo,
    >,

    // -- Phase 3: Automation protocols --
    pub layer_shell_state: WlrLayerShellState,
    pub data_control_state: DataControlState,
    pub ext_data_control_state: ExtDataControlState,
    pub content_type_state: ContentTypeState,
    pub virtual_keyboard_state: VirtualKeyboardManagerState,
    pub virtual_pointer_state: smithay::wayland::virtual_pointer::VirtualPointerManagerState,
    /// Buttons each live virtual pointer holds — released when it goes away.
    pub virtual_pointer_buttons: HashMap<smithay::wayland::virtual_pointer::VirtualPointerDevice, HashSet<u32>>,
    pub pointer_warp_global: smithay::reexports::wayland_server::backend::GlobalId,
    pub output_management_global: smithay::reexports::wayland_server::backend::GlobalId,

    // -- EIS (Emulated Input Server) --
    /// Connected EI clients (see `crate::eis`).
    pub eis_clients: crate::eis::EisClients,

    /// Weak references to all bound output management instances.
    ///
    /// Used to send `finished` events on old head/mode objects and re-send
    /// updated state when outputs are reconfigured (e.g. via `wlr-randr`).
    pub output_managers: Vec<smithay::reexports::wayland_server::Weak<
        smithay::reexports::wayland_protocols_wlr::output_management::v1::server::zwlr_output_manager_v1::ZwlrOutputManagerV1,
    >>,

    // -- Screencopy protocol --
    pub screencopy: crate::handlers::screencopy::ScreencopyState,

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

    /// The keymap the compositor was configured with. A virtual keyboard
    /// activates its own keymap for its keys; input from every other source
    /// switches back to this one first (see `input::activate_keymap`).
    pub compositor_keymap: smithay::input::keyboard::KeymapFile,

    /// The XKB names the compositor keymap was built from, kept so that more
    /// keyboards (the EI keyboard devices) get the same keymap.
    pub xkb_names: XkbNames,

    /// Modifier masks (depressed, latched, locked) each live virtual keyboard
    /// set through its `modifiers` request — cleared again when it goes away.
    pub virtual_keyboard_mods:
        HashMap<smithay::wayland::virtual_keyboard::VirtualKeyboardDevice, crate::handlers::virtual_keyboard::Masks>,

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

    /// Active touch interaction with an SSD area (titlebar buttons).
    ///
    /// When a touch-down lands on an SSD button (Close/Maximize/Minimize),
    /// the action is deferred until touch-up — same as pointer buttons.
    /// Stores `(window, button, initiating slot, last-known touch position)`
    /// so that touch-up can verify the finger is still over the same button.
    pub touch_ssd_button:
        Option<(Window, crate::decorations::DecorationClick, smithay::backend::input::TouchSlot, Point<f64, Logical>)>,

    /// Open right-click context menu on an SSD titlebar (if any).
    pub context_menu: Option<crate::decorations::TitlebarContextMenu>,

    /// Active compositor-side highlight overlay requested via control IPC.
    pub highlight_overlay: crate::highlight::HighlightOverlay,

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
    /// The control socket's file; removed when the compositor exits.
    pub control_socket: Option<crate::control::ControlSocketFile>,

    // -- Security policy --
    pub security_policy: crate::security::SecurityPolicy,

    // -- Configuration --
    pub config: crate::config::CompositorConfig,

    // -- Child program --
    pub exit_with_child: bool,
    /// Child command to spawn after readiness (deferred if `XWayland` is pending).
    pub child_command: Vec<String>,
    /// The monitored child while it runs (`--exit-with-child`).
    pub child: Option<std::process::Child>,
    /// The child's result once it is known — its exit code, or the spawn failure
    /// code; the compositor exits with it (see [`Self::exit_code`]).
    pub child_exit_code: Option<u8>,

    // -- XWayland --
    pub xwayland: Option<crate::xwayland::XWaylandState>,
    /// Where a requested `XWayland` stands; the IPC status reports it as usable only
    /// while it runs.
    pub xwayland_phase: crate::xwayland::XWaylandPhase,
    /// How long a requested `XWayland` may take to become usable (`--xwayland-timeout`).
    pub xwayland_timeout: std::time::Duration,
    /// Why the session could not start, once a startup error is recorded; the
    /// compositor then exits with `1` whatever the child did (see [`Self::exit_code`]).
    pub startup_error: Option<String>,
    pub xwayland_shell_state: Option<smithay::wayland::xwayland_shell::XWaylandShellState>,
    pub xwayland_keyboard_grab_state: Option<smithay::wayland::xwayland_keyboard_grab::XWaylandKeyboardGrabState>,

    // -- UI rendering --
    pub titlebar_renderer: crate::ui::TitlebarRenderer,

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

    /// Render scheduling ping — set by the winit and DRM backends.
    ///
    /// When `Some`, pinging this triggers a single frame render on the next
    /// calloop iteration.  Protocol handlers (e.g. `wl_surface.commit`) call
    /// [`schedule_render()`](Self::schedule_render) to request a redraw.
    /// The headless backend leaves this `None`.
    pub render_ping: Option<smithay::reexports::calloop::ping::Ping>,
}

/// Owned copy of the XKB names (rules, model, layout, variant, options) a
/// keymap is built from. `XkbConfig` borrows its strings, so it cannot outlive
/// the CLI arguments it was resolved from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XkbNames {
    pub rules: String,
    pub model: String,
    pub layout: String,
    pub variant: String,
    pub options: Option<String>,
}

impl XkbNames {
    #[must_use]
    pub fn from_config(config: &XkbConfig<'_>) -> Self {
        Self {
            rules: config.rules.to_string(),
            model: config.model.to_string(),
            layout: config.layout.to_string(),
            variant: config.variant.to_string(),
            options: config.options.clone(),
        }
    }

    #[must_use]
    pub fn config(&self) -> XkbConfig<'_> {
        XkbConfig {
            rules: &self.rules,
            model: &self.model,
            layout: &self.layout,
            variant: &self.variant,
            options: self.options.clone(),
        }
    }
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
        let compositor_state = CompositorState::new_v6::<Self>(&dh);
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

        let dmabuf_state = DmabufState::new();

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

        // Phase 3b: Additional protocol support
        let alpha_modifier_state = AlphaModifierState::new::<Self>(&dh);
        let idle_inhibit_state = IdleInhibitManagerState::new::<Self>(&dh);
        let idle_inhibit_surfaces = HashSet::new();
        let xdg_dialog_state = XdgDialogState::new::<Self>(&dh);
        let xdg_system_bell_state = XdgSystemBellState::new::<Self>(&dh);
        let pointer_gestures_state = PointerGesturesState::new::<Self>(&dh);
        let tablet_manager_state = TabletManagerState::new::<Self>(&dh);
        let tearing_control_global = crate::handlers::tearing_control::init_tearing_control(&dh);
        let toplevel_drag_global = crate::handlers::toplevel_drag::init_toplevel_drag(&dh);
        let toplevel_icon_global = crate::handlers::toplevel_icon::init_toplevel_icon(&dh);
        let toplevel_tag_global = crate::handlers::toplevel_tag::init_toplevel_tag(&dh);

        // Phase 3: Automation protocols
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let data_control_state = DataControlState::new::<Self, _>(&dh, Some(&primary_selection_state), |_| true);
        let ext_data_control_state = ExtDataControlState::new::<Self, _>(&dh, Some(&primary_selection_state), |_| true);
        let content_type_state = ContentTypeState::new::<Self>(&dh);
        let virtual_keyboard_state = {
            let restrict = security_policy.is_restrictive();
            VirtualKeyboardManagerState::new::<Self, _>(&dh, move |_| !restrict)
        };
        let virtual_pointer_state = {
            let restrict = security_policy.is_restrictive();
            crate::handlers::virtual_pointer::init_virtual_pointer_manager(&dh, move |_| !restrict)
        };
        let pointer_warp_global = {
            let restrict = security_policy.is_restrictive();
            crate::handlers::pointer_warp::init_pointer_warp(&dh, move |_| !restrict)
        };
        let output_management_global = {
            let restrict = security_policy.is_restrictive();
            crate::handlers::output_management::init_output_management(&dh, move |_| !restrict)
        };
        let screencopy = {
            let restrict = security_policy.is_restrictive();
            crate::handlers::screencopy::ScreencopyState::new(&dh, move |_| !restrict)
        };

        // Foreign-toplevel protocols
        let wlr_foreign_toplevel_state =
            crate::handlers::foreign_toplevel::WlrForeignToplevelManagerState::new::<Self>(&dh);
        let ext_foreign_toplevel_list_state = ForeignToplevelListState::new::<Self>(&dh);

        let mut seat = seat_state.new_wl_seat(&dh, "seat0");
        let xkb_names = XkbNames::from_config(&xkb_config);
        let compositor_keymap = seat
            .add_keyboard(xkb_config, KEY_REPEAT_DELAY_MS, KEY_REPEAT_RATE)
            .expect("Failed to add keyboard to seat")
            .keymap_file();
        seat.add_pointer();
        seat.add_touch();

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
                    serial_number: String::new(),
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
                    serial_number: String::new(),
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
            dmabuf_feedback: None,
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
            alpha_modifier_state,
            idle_inhibit_state,
            idle_inhibit_surfaces,
            xdg_dialog_state,
            xdg_system_bell_state,
            pointer_gestures_state,
            tablet_manager_state,
            tearing_control_global,
            toplevel_drag_global,
            toplevel_icon_global,
            toplevel_tag_global,
            toplevel_icons: HashMap::new(),
            toplevel_tags: HashMap::new(),
            layer_shell_state,
            data_control_state,
            ext_data_control_state,
            content_type_state,
            virtual_keyboard_state,
            virtual_pointer_state,
            virtual_pointer_buttons: HashMap::new(),
            pointer_warp_global,
            output_management_global,
            eis_clients: HashMap::new(),
            output_managers: Vec::new(),
            screencopy,
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
            compositor_keymap,
            xkb_names,
            virtual_keyboard_mods: HashMap::new(),
            compositor_cursor_shape: crate::decorations::CursorShape::Default,
            cursor_theme: crate::cursor::CursorThemeState::new(),
            pre_maximize_positions: Vec::new(),
            pre_fullscreen_states: Vec::new(),
            minimized_windows: Vec::new(),
            last_focused_window: None,
            last_titlebar_click: None,
            pressed_titlebar_button: None,
            touch_ssd_button: None,
            context_menu: None,
            highlight_overlay: crate::highlight::HighlightOverlay::default(),
            output,
            outputs,
            clock,
            socket_name,
            start_time: std::time::Instant::now(),
            timeout,
            running: true,
            backend_name: "unknown",
            control_socket: None,
            print_env: false,
            ready_fd: None,
            security_policy,
            titlebar_renderer: crate::ui::TitlebarRenderer::new(&config.font.family, config.font.size),
            screenshot_renderer: None,
            config,
            exit_with_child: false,
            child_command: Vec::new(),
            child: None,
            child_exit_code: None,
            xwayland: None,
            xwayland_phase: crate::xwayland::XWaylandPhase::NotRequested,
            xwayland_timeout: std::time::Duration::from_secs(10),
            startup_error: None,
            xwayland_shell_state: None,
            xwayland_keyboard_grab_state: None,
            drm_backend: None,
            output_config_changed: false,
            window_scale: 1.0,
            software_cursor: false,
            render_ping: None,
        }
    }

    /// Request a frame render on the next calloop iteration.
    ///
    /// No-op if no backend has registered a render ping (e.g. DRM/headless).
    pub fn schedule_render(&self) {
        if let Some(ping) = &self.render_ping {
            ping.ping();
        }
    }

    /// Initialize the DMA-BUF global by querying the renderer for supported formats.
    pub fn init_dmabuf_from_renderer(&mut self, renderer: &smithay::backend::renderer::glow::GlowRenderer) {
        use smithay::backend::egl::EGLDevice;
        use smithay::backend::renderer::ImportDma;
        use smithay::wayland::dmabuf::DmabufFeedbackBuilder;

        let dmabuf_formats = renderer.dmabuf_formats();
        let render_node = EGLDevice::device_for_display(renderer.egl_context().display())
            .and_then(|device| device.try_get_render_node());

        match render_node {
            Ok(Some(node)) => match DmabufFeedbackBuilder::new(node.dev_id(), dmabuf_formats.clone()).build() {
                Ok(feedback) => {
                    self.dmabuf_state.create_global_with_default_feedback::<Self>(&self.display_handle, &feedback);
                    self.dmabuf_feedback = Some(feedback);
                    tracing::info!("DMA-BUF global created with renderer feedback (render node {node})");
                    return;
                }
                Err(err) => {
                    tracing::warn!(%err, "failed to build DMA-BUF feedback, falling back to format list");
                }
            },
            Ok(None) => {
                tracing::warn!("no render node available, creating DMA-BUF global without feedback");
            }
            Err(err) => {
                tracing::warn!(%err, "failed to query EGL device, creating DMA-BUF global without feedback");
            }
        }

        self.dmabuf_state.create_global::<Self>(&self.display_handle, dmabuf_formats);
    }

    /// Initialize the DMA-BUF global with a static set of common formats.
    pub fn init_dmabuf_with_fallback_formats(&mut self) {
        use smithay::backend::allocator::{Format, Fourcc, Modifier};
        let formats = vec![
            Format { code: Fourcc::Argb8888, modifier: Modifier::Linear },
            Format { code: Fourcc::Xrgb8888, modifier: Modifier::Linear },
            Format { code: Fourcc::Abgr8888, modifier: Modifier::Linear },
            Format { code: Fourcc::Xbgr8888, modifier: Modifier::Linear },
        ];
        self.dmabuf_state.create_global::<Self>(&self.display_handle, formats);
    }

    /// Spawn the child program (if configured) and optionally monitor its exit.
    ///
    /// Call this after compositor readiness is established (Wayland socket ready,
    /// optionally `XWayland` ready). The child inherits `WAYLAND_DISPLAY` and
    /// `XDG_RUNTIME_DIR` from the compositor environment; `DISPLAY` only when
    /// `XWayland` runs (see [`crate::child::spawn_child`] for why a leaked host
    /// `DISPLAY` must not reach the session).
    ///
    /// With `--exit-with-child`, a child that cannot be started ends the session
    /// right away with the shell's code for it (`127` not found, `126` not
    /// executable) instead of leaving the compositor running with nothing to wait for.
    pub fn spawn_child_if_requested(&mut self) {
        let x11 = self.xwayland_phase == crate::xwayland::XWaylandPhase::Running;
        match crate::child::spawn_child(&self.child_command, x11) {
            Ok(Some(child)) if self.exit_with_child => {
                self.child = Some(child);
                if let Err(err) = crate::child::monitor_child_exit(&self.loop_handle) {
                    tracing::error!(%err, "failed to register child exit monitor");
                    // Nothing would notice the child exit: end the session (see `exit_code`).
                    self.running = false;
                }
            }
            Err(err) if self.exit_with_child => {
                self.child_exit_code = Some(crate::child::spawn_error_exit_code(&err));
                self.running = false;
            }
            Ok(_) | Err(_) => {}
        }
    }

    /// Announce readiness and start the child program: the single place both happen,
    /// once every requested service is usable — at the end of the service setup
    /// without `--xwayland`, once `XWayland` and its window manager run with it.
    pub fn finish_startup(&mut self) {
        let x11_display =
            self.xwayland.as_ref().filter(|_| self.xwayland_phase == crate::xwayland::XWaylandPhase::Running);
        let x11_display = x11_display.map(|xwayland| xwayland.display);
        crate::ready::notify_ready(&self.socket_name, x11_display, self.ready_fd.take(), self.print_env);
        self.spawn_child_if_requested();
    }

    /// Record why the session could not start: log it at ERROR level, remember it for
    /// [`Self::exit_code`] and end the session. Setting `running` (rather than stopping
    /// the loop signal, which calloop resets when `run` begins) also works before the
    /// event loop has started.
    pub fn record_startup_error(&mut self, cause: String) {
        tracing::error!("{cause}");
        self.startup_error = Some(cause);
        self.running = false;
    }

    /// Exit code for the compositor process, called once the event loop has stopped.
    ///
    /// A startup error fails the session in every mode, and so does a session that
    /// ended while a requested `XWayland` was still starting: it never became usable.
    /// Otherwise, without `--exit-with-child` (or without a child command) this is
    /// success. With it, the compositor reports the child's result: the child's exit
    /// code once it has exited, the spawn failure code if it never started, and failure
    /// if the session ended while the child was still running (`--timeout`, the IPC
    /// `shutdown` command, `SIGTERM`/`SIGINT`, closing the window) — an unknown result
    /// never reads as success.
    pub fn exit_code(&mut self) -> std::process::ExitCode {
        // The session may have ended between the child's exit and the monitor's next poll.
        crate::child::poll_child_exit(self);
        self.abandon_starting_xwayland();
        std::process::ExitCode::from(session_exit_code(SessionOutcome {
            never_usable: self.startup_error.is_some()
                || self.xwayland_phase == crate::xwayland::XWaylandPhase::Starting,
            child_exit_code: self.child_exit_code,
            reports_child: self.exit_with_child && !self.child_command.is_empty(),
        }))
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

    /// Get the touch handle from the seat.
    ///
    /// # Panics
    ///
    /// Panics if the seat has no touch capability.  This should never
    /// happen because `State::new()` unconditionally adds touch.
    #[must_use]
    pub fn touch(&self) -> smithay::input::touch::TouchHandle<Self> {
        self.seat.get_touch().expect("seat has no touch capability")
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
                    if let Err(err) =
                        x11.configure(Rectangle::new((usable_geo.loc.x, usable_geo.loc.y + y_offset).into(), max_size))
                    {
                        tracing::warn!(%err, "failed to reconfigure maximized X11 window");
                    }
                    self.space.map_element(window.clone(), (usable_geo.loc.x, usable_geo.loc.y + y_offset), true);
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
                    let _ = x11.configure(Rectangle::new((new_x, new_y).into(), crate::xwayland::x11_size(x11)));
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
    pub fn send_frame_callbacks(&self, throttle: std::time::Duration) {
        use smithay::desktop::utils::surface_primary_scanout_output;

        let now = self.frame_clock_now();
        let throttle = Some(throttle);

        for window in self.space.elements() {
            let output = self
                .output_at_point({
                    let loc = self.space.element_location(window).unwrap_or_default();
                    let size = window.geometry().size;
                    (f64::from(loc.x + size.w / 2), f64::from(loc.y + size.h / 2)).into()
                })
                .clone();
            window.send_frame(&output, now, throttle, surface_primary_scanout_output);
        }

        for output in &self.outputs {
            let map = smithay::desktop::layer_map_for_output(output);
            for layer_surface in map.layers() {
                layer_surface.send_frame(output, now, throttle, surface_primary_scanout_output);
            }
        }
    }

    /// Update per-surface primary scanout output tracking after a render pass.
    pub fn update_primary_scanout_output(
        &self,
        output: &Output,
        render_element_states: &smithay::backend::renderer::element::RenderElementStates,
    ) {
        use smithay::backend::renderer::element::default_primary_scanout_output_compare;
        use smithay::desktop::utils::update_surface_primary_scanout_output;
        use smithay::wayland::fractional_scale::with_fractional_scale;

        let update = |surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
                      states: &smithay::wayland::compositor::SurfaceData| {
            let primary = update_surface_primary_scanout_output(
                surface,
                output,
                states,
                None,
                render_element_states,
                default_primary_scanout_output_compare,
            );
            if let Some(ref o) = primary {
                with_fractional_scale(states, |fraction_scale| {
                    fraction_scale.set_preferred_scale(o.current_scale().fractional_scale());
                });
            }
        };

        for window in self.space.elements() {
            window.with_surfaces(update);
        }

        for o in &self.outputs {
            let map = smithay::desktop::layer_map_for_output(o);
            for layer_surface in map.layers() {
                layer_surface.with_surfaces(update);
            }
        }
    }

    /// Collect presentation feedback from all surfaces for the given output.
    pub fn take_presentation_feedback(
        &self,
        _output: &Output,
        render_element_states: &smithay::backend::renderer::element::RenderElementStates,
        feedback: &mut smithay::desktop::utils::OutputPresentationFeedback,
    ) {
        use smithay::desktop::utils::{
            surface_presentation_feedback_flags_from_states, surface_primary_scanout_output,
        };

        for window in self.space.elements() {
            window.take_presentation_feedback(feedback, surface_primary_scanout_output, |surface, _| {
                surface_presentation_feedback_flags_from_states(surface, None, render_element_states)
            });
        }

        for o in &self.outputs {
            let map = smithay::desktop::layer_map_for_output(o);
            for layer_surface in map.layers() {
                layer_surface.take_presentation_feedback(feedback, surface_primary_scanout_output, |surface, _| {
                    surface_presentation_feedback_flags_from_states(surface, None, render_element_states)
                });
            }
        }
    }

    /// Send per-surface DMA-BUF feedback based on primary scanout output.
    pub fn send_dmabuf_feedback(
        &self,
        output: &Output,
        _render_element_states: &smithay::backend::renderer::element::RenderElementStates,
    ) {
        use smithay::desktop::utils::surface_primary_scanout_output;

        let Some(ref feedback) = self.dmabuf_feedback else {
            return;
        };

        for window in self.space.elements() {
            window.send_dmabuf_feedback(output, surface_primary_scanout_output, |_surface, _states| feedback);
        }

        for o in &self.outputs {
            let map = smithay::desktop::layer_map_for_output(o);
            for layer_surface in map.layers() {
                layer_surface
                    .send_dmabuf_feedback(output, surface_primary_scanout_output, |_surface, _states| feedback);
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

    /// Find a modal dialog child of the given window.
    ///
    /// Walks all mapped windows checking whether any is modal (via
    /// `xdg-dialog-v1`) and has `window`'s surface as its xdg parent.
    /// Returns the **topmost** modal child if one exists, following the
    /// chain recursively (a modal of a modal returns the leaf).
    ///
    /// GNOME/Mutter behaviour: attempting to focus a window that has a
    /// modal child should focus the modal instead.
    #[must_use]
    pub fn find_modal_child(&self, window: &Window) -> Option<Window> {
        use smithay::wayland::compositor;
        use smithay::wayland::seat::WaylandFocus;
        use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;

        let parent_surface = window.wl_surface()?;

        // Search all mapped windows for one whose parent is `window` and is modal.
        let modal = self
            .space
            .elements()
            .find(|w| {
                let Some(toplevel) = w.toplevel() else { return false };
                // Check parent matches
                let Some(parent) = toplevel.parent() else { return false };
                if parent != *parent_surface {
                    return false;
                }
                // Check if this toplevel is modal
                compositor::with_states(toplevel.wl_surface(), |states| {
                    states
                        .data_map
                        .get::<XdgToplevelSurfaceData>()
                        .is_some_and(|data| data.lock().is_ok_and(|d| d.dialog_hint == ToplevelDialogHint::Modal))
                })
            })
            .cloned();

        // Follow the chain: if the modal itself has a modal child, return that instead.
        if let Some(ref m) = modal
            && let Some(deeper) = self.find_modal_child(m)
        {
            return Some(deeper);
        }

        modal
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

// -- Protocol dispatch --
//
// Every smithay protocol state, and every protocol this compositor implements
// itself (`handlers/`), provides `Dispatch2`/`GlobalDispatch2` on its user-data
// types; this one macro routes all of them through `State`.

smithay::delegate_dispatch2!(State);

/// What decides the session's exit code once the event loop has stopped.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SessionOutcome {
    /// The session never became usable: a startup error was recorded, or it ended
    /// while a requested `XWayland` was still starting.
    pub(crate) never_usable: bool,
    /// The child's result, once known.
    pub(crate) child_exit_code: Option<u8>,
    /// The exit code reports the child's result: `--exit-with-child` with a child
    /// program after `--`.
    pub(crate) reports_child: bool,
}

/// The session's exit code: `1` for a session that never became usable, otherwise
/// the child's result under `--exit-with-child` (`1` while it is unknown), else `0`.
pub(crate) fn session_exit_code(outcome: SessionOutcome) -> u8 {
    if outcome.never_usable {
        return 1;
    }
    match outcome.child_exit_code {
        Some(code) => code,
        None if outcome.reports_child => 1,
        None => 0,
    }
}

#[cfg(test)]
mod xkb_names_tests {
    use smithay::input::keyboard::{XkbConfig, xkb};

    use super::XkbNames;

    fn keymap_text(config: &XkbConfig<'_>) -> String {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        xkb::Keymap::new_from_names(
            &context,
            config.rules,
            config.model,
            config.layout,
            config.variant,
            config.options.clone(),
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("compile keymap")
        .get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1)
    }

    #[test]
    fn the_owned_names_build_the_same_keymap() {
        let config = XkbConfig {
            rules: "",
            model: "pc105",
            layout: "de,us",
            variant: "nodeadkeys,",
            options: Some("grp:alt_shift_toggle".to_string()),
        };
        let names = XkbNames::from_config(&config);
        assert_eq!(keymap_text(&names.config()), keymap_text(&config));
    }
}

#[cfg(test)]
mod exit_code_tests {
    use super::{SessionOutcome, session_exit_code};

    fn outcome() -> SessionOutcome {
        SessionOutcome { never_usable: false, child_exit_code: None, reports_child: false }
    }

    #[test]
    fn a_session_that_never_became_usable_fails_in_every_mode() {
        let failed = SessionOutcome { never_usable: true, ..outcome() };
        assert_eq!(session_exit_code(failed), 1, "without --exit-with-child");
        let with_child = SessionOutcome { reports_child: true, ..failed };
        assert_eq!(session_exit_code(with_child), 1, "with --exit-with-child");
        let child_done = SessionOutcome { child_exit_code: Some(0), ..with_child };
        assert_eq!(session_exit_code(child_done), 1, "with a child result already recorded");
    }

    #[test]
    fn without_a_startup_error_the_child_result_decides() {
        assert_eq!(session_exit_code(outcome()), 0, "no child");
        let running = SessionOutcome { reports_child: true, ..outcome() };
        assert_eq!(session_exit_code(running), 1, "the child's result is unknown");
        let exited = SessionOutcome { child_exit_code: Some(3), ..running };
        assert_eq!(session_exit_code(exited), 3, "the child's own code");
    }
}
