//! `wlr-foreign-toplevel-management-v1` + `ext-foreign-toplevel-list-v1` handlers.
//!
//! These protocols enable taskbars (ironbar, waybar) and automation tools to
//! discover, monitor, and control opened windows.
//!
//! - **wlr-foreign-toplevel-management-v1** (v3): Full read/write — title, `app_id`,
//!   state (maximized/minimized/activated/fullscreen), output enter/leave, plus
//!   requests: activate, close, set/unset maximized/minimized/fullscreen.
//!   Implemented manually because smithay does not provide it.
//!
//! - **ext-foreign-toplevel-list-v1**: Read-only list with title + `app_id`.
//!   Uses smithay's `delegate_foreign_toplevel_list!()`.

use std::sync::Mutex;

use smithay::{
    delegate_foreign_toplevel_list,
    desktop::Window,
    output::Output,
    reexports::{
        wayland_protocols_wlr::foreign_toplevel::v1::server::{
            zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
            zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
        },
        wayland_server::{
            Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, Weak,
            backend::{ClientId, GlobalId},
        },
    },
    wayland::{
        compositor,
        foreign_toplevel_list::{ForeignToplevelListHandler, ForeignToplevelListState},
        seat::WaylandFocus,
        shell::xdg::XdgToplevelSurfaceData,
    },
};

use crate::state::State;

// ── wlr-foreign-toplevel-management state constants ─────────────────────────
//
// From the `zwlr_foreign_toplevel_handle_v1::state` enum — each value is sent
// as a native-endian `u32` inside the state byte array.

/// `zwlr_foreign_toplevel_handle_v1::state::maximized`
const WLR_STATE_MAXIMIZED: u32 = 0;
/// `zwlr_foreign_toplevel_handle_v1::state::minimized`
const WLR_STATE_MINIMIZED: u32 = 1;
/// `zwlr_foreign_toplevel_handle_v1::state::activated`
const WLR_STATE_ACTIVATED: u32 = 2;
/// `zwlr_foreign_toplevel_handle_v1::state::fullscreen`
const WLR_STATE_FULLSCREEN: u32 = 3;

/// Append a state flag to the wlr state byte array.
fn push_state(states: &mut Vec<u8>, flag: u32) {
    states.extend_from_slice(&flag.to_ne_bytes());
}

/// Remove a state flag (first occurrence) from the wlr state byte array.
fn strip_state(states: &mut Vec<u8>, flag: u32) {
    let bytes = flag.to_ne_bytes();
    let size = std::mem::size_of::<u32>();
    if let Some(pos) = states.windows(size).position(|chunk| chunk == bytes) {
        states.drain(pos..pos + size);
    }
}

// ── wlr-foreign-toplevel-management-v1 ──────────────────────────────────────

/// Per-handle data attached to each `ZwlrForeignToplevelHandleV1` resource.
#[derive(Debug, Clone)]
pub struct WlrToplevelHandle {
    inner: std::sync::Arc<Mutex<WlrToplevelHandleInner>>,
}

#[derive(Debug)]
struct WlrToplevelHandleInner {
    /// All client-side resource instances for this handle.
    instances: Vec<Weak<ZwlrForeignToplevelHandleV1>>,
    /// The smithay `Window` this handle represents.
    window: Window,
    /// Last-sent title (to avoid redundant events).
    title: String,
    /// Last-sent `app_id` (to avoid redundant events).
    app_id: String,
    /// Last-sent state bitfield (to avoid redundant events).
    last_state: Vec<u8>,
    /// Whether the handle has been closed.
    closed: bool,
}

impl WlrToplevelHandle {
    fn new(window: Window, title: String, app_id: String) -> Self {
        Self {
            inner: std::sync::Arc::new(Mutex::new(WlrToplevelHandleInner {
                instances: Vec::new(),
                window,
                title,
                app_id,
                last_state: Vec::new(),
                closed: false,
            })),
        }
    }

    /// Send title event to all instances.
    pub fn send_title(&self, title: &str) {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        if inner.title == title {
            return;
        }
        inner.title = title.to_string();
        for inst in &inner.instances {
            if let Ok(inst) = inst.upgrade() {
                inst.title(title.to_string());
            }
        }
    }

    /// Send `app_id` event to all instances.
    pub fn send_app_id(&self, app_id: &str) {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        if inner.app_id == app_id {
            return;
        }
        inner.app_id = app_id.to_string();
        for inst in &inner.instances {
            if let Ok(inst) = inst.upgrade() {
                inst.app_id(app_id.to_string());
            }
        }
    }

    /// Send state event to all instances.
    pub fn send_state(&self, state: &[u8]) {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        if inner.last_state == state {
            return;
        }
        tracing::trace!(
            old_state = ?inner.last_state,
            new_state = ?state,
            instances = inner.instances.len(),
            "send_state: sending state change",
        );
        inner.last_state = state.to_vec();
        for inst in &inner.instances {
            if let Ok(inst) = inst.upgrade() {
                inst.state(state.to_vec());
            }
        }
    }

    /// Send done event to all instances (finalizes a batch of updates).
    pub fn send_done(&self) {
        let inner = self.inner.lock().expect("mutex poisoned");
        for inst in &inner.instances {
            if let Ok(inst) = inst.upgrade() {
                inst.done();
            }
        }
    }

    /// Send closed event and mark as dead.
    pub fn send_closed(&self) {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        if inner.closed {
            return;
        }
        inner.closed = true;
        for inst in inner.instances.drain(..) {
            if let Ok(inst) = inst.upgrade() {
                inst.closed();
            }
        }
    }

    pub fn is_closed(&self) -> bool {
        self.inner.lock().expect("mutex poisoned").closed
    }

    pub fn window(&self) -> Window {
        self.inner.lock().expect("mutex poisoned").window.clone()
    }

    /// Create a new protocol resource instance for `client`, send the initial
    /// state, and register it.
    ///
    /// `outputs` are the compositor outputs the window is on — used to send
    /// `output_enter` events so taskbars know which output the window belongs
    /// to.
    fn init_instance<D: Dispatch<ZwlrForeignToplevelHandleV1, WlrToplevelHandle> + 'static>(
        &self,
        dh: &DisplayHandle,
        client: &Client,
        manager: &ZwlrForeignToplevelManagerV1,
        outputs: &[Output],
    ) {
        let inner = self.inner.lock().expect("mutex poisoned");
        if inner.closed {
            return;
        }

        let Ok(handle) =
            client.create_resource::<ZwlrForeignToplevelHandleV1, _, D>(dh, manager.version(), self.clone())
        else {
            tracing::warn!("foreign-toplevel: failed to create handle resource for client");
            return;
        };

        // Announce the new handle to the manager
        manager.toplevel(&handle);

        // Send initial state
        handle.title(inner.title.clone());
        handle.app_id(inner.app_id.clone());
        if !inner.last_state.is_empty() {
            handle.state(inner.last_state.clone());
        }

        // Send output_enter for each output so taskbars know where
        // to display this window.  Without this, taskbars that filter
        // by output (e.g. ironbar) will not show the window at all.
        for output in outputs {
            for wl_output in output.client_outputs(client) {
                handle.output_enter(&wl_output);
            }
        }

        handle.done();

        drop(inner);
        self.inner.lock().expect("mutex poisoned").instances.push(handle.downgrade());
    }

    fn remove_instance(&self, instance: &ZwlrForeignToplevelHandleV1) {
        let mut inner = self.inner.lock().expect("mutex poisoned");
        inner.instances.retain(|i| i.upgrade().is_ok_and(|inst| &inst != instance));
    }
}

/// Global data for `zwlr_foreign_toplevel_manager_v1`.
pub struct WlrForeignToplevelManagerGlobalData {
    filter: Box<dyn Fn(&Client) -> bool + Send + Sync>,
}

impl std::fmt::Debug for WlrForeignToplevelManagerGlobalData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WlrForeignToplevelManagerGlobalData").finish_non_exhaustive()
    }
}

/// State for the `wlr-foreign-toplevel-management-v1` global.
#[derive(Debug)]
pub struct WlrForeignToplevelManagerState {
    global: GlobalId,
    /// All known toplevel handles (includes closed ones until cleanup).
    pub handles: Vec<WlrToplevelHandle>,
    /// All bound manager instances.
    manager_instances: Vec<ZwlrForeignToplevelManagerV1>,
    dh: DisplayHandle,
}

impl WlrForeignToplevelManagerState {
    /// Register the global.
    pub fn new<D>(dh: &DisplayHandle) -> Self
    where
        D: GlobalDispatch<ZwlrForeignToplevelManagerV1, WlrForeignToplevelManagerGlobalData>
            + Dispatch<ZwlrForeignToplevelManagerV1, ()>
            + Dispatch<ZwlrForeignToplevelHandleV1, WlrToplevelHandle>
            + 'static,
    {
        Self::new_with_filter::<D>(dh, |_| true)
    }

    /// Register the global with a client filter.
    pub fn new_with_filter<D>(dh: &DisplayHandle, filter: impl Fn(&Client) -> bool + Send + Sync + 'static) -> Self
    where
        D: GlobalDispatch<ZwlrForeignToplevelManagerV1, WlrForeignToplevelManagerGlobalData>
            + Dispatch<ZwlrForeignToplevelManagerV1, ()>
            + Dispatch<ZwlrForeignToplevelHandleV1, WlrToplevelHandle>
            + 'static,
    {
        let global = dh.create_global::<D, ZwlrForeignToplevelManagerV1, _>(
            3, // protocol version 3 (with parent events)
            WlrForeignToplevelManagerGlobalData { filter: Box::new(filter) },
        );
        Self { global, handles: Vec::new(), manager_instances: Vec::new(), dh: dh.clone() }
    }

    /// The global ID.
    pub fn global(&self) -> GlobalId {
        self.global.clone()
    }

    /// Announce a new toplevel to all bound managers.
    ///
    /// `outputs` are passed through to `init_instance` for `output_enter` events.
    pub fn new_toplevel(&mut self, window: Window, title: &str, app_id: &str, outputs: &[Output]) -> WlrToplevelHandle {
        let handle = WlrToplevelHandle::new(window, title.to_string(), app_id.to_string());

        for manager in &self.manager_instances {
            let Some(client) = manager.client() else {
                tracing::debug!("foreign-toplevel: skipping disconnected manager client");
                continue;
            };
            handle.init_instance::<State>(&self.dh, &client, manager, outputs);
        }

        self.handles.push(handle.clone());
        handle
    }

    /// Remove a toplevel — sends `closed` event if not already sent.
    pub fn remove_toplevel(&mut self, handle: &WlrToplevelHandle) {
        handle.send_closed();
        self.handles.retain(|h| !std::sync::Arc::ptr_eq(&h.inner, &handle.inner));
    }

    /// Clean up handles that have been closed.
    pub fn cleanup_closed(&mut self) {
        self.handles.retain(|h| !h.is_closed());
    }
}

// ── GlobalDispatch for manager ──────────────────────────────────────────────

impl GlobalDispatch<ZwlrForeignToplevelManagerV1, WlrForeignToplevelManagerGlobalData> for State {
    fn bind(
        state: &mut State,
        dh: &DisplayHandle,
        client: &Client,
        resource: New<ZwlrForeignToplevelManagerV1>,
        _global_data: &WlrForeignToplevelManagerGlobalData,
        data_init: &mut DataInit<'_, State>,
    ) {
        let manager = data_init.init(resource, ());

        // Collect per-handle output info before iterating, to avoid borrow
        // conflicts with state fields.
        let outputs = state.outputs.clone();

        // Send all existing (non-closed) toplevels to the new client
        state.wlr_foreign_toplevel_state.handles.retain(|h| !h.is_closed());
        for handle in &state.wlr_foreign_toplevel_state.handles {
            handle.init_instance::<State>(dh, client, &manager, &outputs);
        }

        tracing::debug!(
            handles = state.wlr_foreign_toplevel_state.handles.len(),
            "foreign-toplevel: new manager bound, sent existing toplevels",
        );

        state.wlr_foreign_toplevel_state.manager_instances.push(manager);
    }

    fn can_view(client: Client, global_data: &WlrForeignToplevelManagerGlobalData) -> bool {
        (global_data.filter)(&client)
    }
}

// ── Dispatch for manager requests ───────────────────────────────────────────

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for State {
    fn request(
        state: &mut State,
        _client: &Client,
        manager: &ZwlrForeignToplevelManagerV1,
        request: zwlr_foreign_toplevel_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        if let zwlr_foreign_toplevel_manager_v1::Request::Stop = request {
            // Client no longer wants events — send finished and clean up.
            manager.finished();
            state.wlr_foreign_toplevel_state.manager_instances.retain(|m| m != manager);
        }
    }

    fn destroyed(state: &mut State, _client: ClientId, resource: &ZwlrForeignToplevelManagerV1, _data: &()) {
        state.wlr_foreign_toplevel_state.manager_instances.retain(|m| m != resource);
    }
}

// ── Dispatch for handle requests ────────────────────────────────────────────

impl Dispatch<ZwlrForeignToplevelHandleV1, WlrToplevelHandle> for State {
    fn request(
        state: &mut State,
        _client: &Client,
        _resource: &ZwlrForeignToplevelHandleV1,
        request: zwlr_foreign_toplevel_handle_v1::Request,
        handle: &WlrToplevelHandle,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        if handle.is_closed() {
            return;
        }

        let window = handle.window();
        match request {
            zwlr_foreign_toplevel_handle_v1::Request::Activate { seat: _ } => {
                tracing::debug!("foreign-toplevel: activate request");
                activate_window(state, &window);
            }
            zwlr_foreign_toplevel_handle_v1::Request::Close => {
                tracing::debug!("foreign-toplevel: close request");
                close_window(&window);
            }
            zwlr_foreign_toplevel_handle_v1::Request::SetMaximized => {
                tracing::debug!("foreign-toplevel: set_maximized request");
                if let Some(toplevel) = window.toplevel() {
                    let surface = toplevel.clone();
                    crate::handlers::xdg_shell::do_maximize(state, &surface);
                }
            }
            zwlr_foreign_toplevel_handle_v1::Request::UnsetMaximized => {
                tracing::debug!("foreign-toplevel: unset_maximized request");
                if let Some(toplevel) = window.toplevel() {
                    let surface = toplevel.clone();
                    crate::handlers::xdg_shell::do_unmaximize(state, &surface);
                }
            }
            zwlr_foreign_toplevel_handle_v1::Request::SetMinimized => {
                tracing::debug!("foreign-toplevel: set_minimized request");
                minimize_window(state, &window);
            }
            zwlr_foreign_toplevel_handle_v1::Request::UnsetMinimized => {
                tracing::debug!("foreign-toplevel: unset_minimized request");
                unminimize_window(state, &window);
            }
            zwlr_foreign_toplevel_handle_v1::Request::SetFullscreen { output: _ } => {
                tracing::debug!("foreign-toplevel: set_fullscreen request");
                if let Some(toplevel) = window.toplevel() {
                    let surface = toplevel.clone();
                    crate::handlers::xdg_shell::do_fullscreen(state, &surface, None);
                }
            }
            zwlr_foreign_toplevel_handle_v1::Request::UnsetFullscreen => {
                tracing::debug!("foreign-toplevel: unset_fullscreen request");
                if let Some(toplevel) = window.toplevel() {
                    let surface = toplevel.clone();
                    crate::handlers::xdg_shell::do_unfullscreen(state, &surface);
                }
            }
            // SetRectangle: hint for minimize animation — we currently ignore it.
            // Destroy + unknown: no-op.
            _ => {}
        }
    }

    fn destroyed(
        _state: &mut State,
        _client: ClientId,
        resource: &ZwlrForeignToplevelHandleV1,
        handle: &WlrToplevelHandle,
    ) {
        handle.remove_instance(resource);
    }
}

// ── ext-foreign-toplevel-list-v1 (smithay built-in) ─────────────────────────

impl ForeignToplevelListHandler for State {
    fn foreign_toplevel_list_state(&mut self) -> &mut ForeignToplevelListState {
        &mut self.ext_foreign_toplevel_list_state
    }
}

delegate_foreign_toplevel_list!(State);

// NOTE: No delegate_global_dispatch!/delegate_dispatch! macros here — we have
// manual GlobalDispatch/Dispatch impls above which provide the same thing.

// ── Helper functions ────────────────────────────────────────────────────────

/// Build the wlr state byte array from window state flags.
pub fn build_wlr_state(window: &Window) -> Vec<u8> {
    let mut states = Vec::new();

    if let Some(toplevel) = window.toplevel() {
        let current = toplevel.current_state();
        if current
            .states
            .contains(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized)
        {
            push_state(&mut states, WLR_STATE_MAXIMIZED);
        }
        if current
            .states
            .contains(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Activated)
        {
            push_state(&mut states, WLR_STATE_ACTIVATED);
        }
        if current
            .states
            .contains(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Fullscreen)
        {
            push_state(&mut states, WLR_STATE_FULLSCREEN);
        }
    } else if let Some(x11) = window.x11_surface() {
        if x11.is_maximized() {
            push_state(&mut states, WLR_STATE_MAXIMIZED);
        }
        if x11.is_activated() {
            push_state(&mut states, WLR_STATE_ACTIVATED);
        }
        if x11.is_fullscreen() {
            push_state(&mut states, WLR_STATE_FULLSCREEN);
        }
    }

    // Minimized is handled by the caller via `build_wlr_state_with_minimized`.
    states
}

/// Build the state including a minimized flag.
///
/// When `is_minimized` is `true`, the `activated` state is stripped because
/// a minimized window is not considered focused/activated by taskbar clients.
pub fn build_wlr_state_with_minimized(window: &Window, is_minimized: bool) -> Vec<u8> {
    let mut states = build_wlr_state(window);
    if is_minimized {
        // A minimized window must not appear as activated — strip the
        // activated flag so taskbars (ironbar) see it as minimized-only.
        strip_state(&mut states, WLR_STATE_ACTIVATED);
        push_state(&mut states, WLR_STATE_MINIMIZED);
    }
    states
}

/// Extract title from a window (Wayland or X11).
pub fn window_title(window: &Window) -> String {
    if let Some(toplevel) = window.toplevel() {
        compositor::with_states(toplevel.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .and_then(|data| data.lock().ok())
                .and_then(|data| data.title.clone())
        })
        .unwrap_or_default()
    } else if let Some(x11) = window.x11_surface() {
        x11.title()
    } else {
        String::new()
    }
}

/// Extract `app_id` from a window (Wayland or X11).
pub fn window_app_id(window: &Window) -> String {
    if let Some(toplevel) = window.toplevel() {
        compositor::with_states(toplevel.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .and_then(|data| data.lock().ok())
                .and_then(|data| data.app_id.clone())
        })
        .unwrap_or_default()
    } else if let Some(x11) = window.x11_surface() {
        x11.class()
    } else {
        String::new()
    }
}

/// Activate (focus) a window.
///
/// Un-minimizes the window if needed, raises it, and sets keyboard focus.
/// The `SeatHandler::focus_changed` callback handles notifying foreign-toplevel
/// clients about activated/deactivated state and X11 `set_activated` calls.
fn activate_window(state: &mut State, window: &Window) {
    // Un-minimize first if needed
    if let Some(idx) = state.minimized_windows.iter().position(|(w, _)| w == window) {
        let (win, pos) = state.minimized_windows.remove(idx);
        state.space.map_element(win.clone(), pos, true);
    }

    // Raise and focus — this triggers `focus_changed` which updates
    // foreign-toplevel state and X11 activated flags.
    state.space.raise_element(window, true);
    if window.wl_surface().is_some() {
        let keyboard = state.keyboard();
        keyboard.set_focus(
            state,
            Some(crate::focus::KeyboardFocusTarget::from(window.clone())),
            smithay::utils::SERIAL_COUNTER.next_serial(),
        );
    }
}

/// Close a window via its protocol surface.
fn close_window(window: &Window) {
    if let Some(toplevel) = window.toplevel() {
        toplevel.send_close();
    } else if let Some(x11) = window.x11_surface()
        && let Err(err) = x11.close()
    {
        tracing::debug!(%err, "X11 close request failed (surface may be destroyed)");
    }
}

/// Minimize a window (unmap from space, store in `minimized_windows`).
///
/// Moves keyboard focus to the next visible window (or clears it) and then
/// sends the minimized state to foreign-toplevel clients (ironbar, taskbars)
/// so they can update their UI accordingly.
pub(crate) fn minimize_window(state: &mut State, window: &Window) {
    if let Some(loc) = state.space.element_location(window) {
        state.minimized_windows.push((window.clone(), loc));
        state.space.unmap_elem(window);

        // Move keyboard focus away from the now-minimized window.
        let next_window = state.space.elements().next_back().cloned();
        let keyboard = state.keyboard();
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        if let Some(next) = next_window {
            keyboard.set_focus(state, Some(crate::focus::KeyboardFocusTarget::Window(next.clone())), serial);
            state.space.raise_element(&next, true);
        } else {
            keyboard.set_focus(state, Option::<crate::focus::KeyboardFocusTarget>::None, serial);
        }

        send_foreign_toplevel_state(state, window);
    }
}

/// Unminimize a window (restore from `minimized_windows`).
///
/// Sends the updated state to foreign-toplevel clients so they know the
/// window is no longer minimized.
pub(crate) fn unminimize_window(state: &mut State, window: &Window) {
    if let Some(idx) = state.minimized_windows.iter().position(|(w, _)| w == window) {
        let (win, pos) = state.minimized_windows.remove(idx);
        state.space.map_element(win, pos, true);
        send_foreign_toplevel_state(state, window);
    }
}

/// Send the current foreign-toplevel state for a window (including minimized flag).
///
/// This notifies all bound taskbar/panel clients about the window's current
/// state so they can update their UI (e.g. show the window as minimized).
pub(crate) fn send_foreign_toplevel_state(state: &State, window: &Window) {
    if let Some(wlr_handle) =
        state.wlr_foreign_toplevel_state.handles.iter().find(|h| !h.is_closed() && h.window() == *window).cloned()
    {
        let is_minimized = state.minimized_windows.iter().any(|(w, _)| w == window);
        let new_state = build_wlr_state_with_minimized(window, is_minimized);
        tracing::trace!(
            is_minimized,
            state_bytes = ?new_state,
            "send_foreign_toplevel_state: sending state update",
        );
        wlr_handle.send_state(&new_state);
        wlr_handle.send_done();
    } else {
        tracing::warn!("send_foreign_toplevel_state: no wlr handle found for window");
    }
}

/// Send foreign-toplevel state with an explicit `activated` override.
///
/// This is needed in `focus_changed` because XDG toplevels still report the
/// old `Activated` state in `current_state()` until the client acknowledges
/// the new configure.  By passing `is_activated` explicitly we ensure ironbar
/// (and other taskbars) get the correct state immediately.
pub(crate) fn send_foreign_toplevel_state_activated(state: &State, window: &Window, is_activated: bool) {
    if let Some(wlr_handle) =
        state.wlr_foreign_toplevel_state.handles.iter().find(|h| !h.is_closed() && h.window() == *window).cloned()
    {
        let is_minimized = state.minimized_windows.iter().any(|(w, _)| w == window);
        let mut new_state = build_wlr_state_with_minimized(window, is_minimized);

        // Strip existing activated flag (may be stale from XDG current_state).
        strip_state(&mut new_state, WLR_STATE_ACTIVATED);
        // Add activated if requested.
        if is_activated {
            push_state(&mut new_state, WLR_STATE_ACTIVATED);
        }

        tracing::trace!(
            is_minimized,
            is_activated,
            state_bytes = ?new_state,
            "send_foreign_toplevel_state_activated: sending state update",
        );
        wlr_handle.send_state(&new_state);
        wlr_handle.send_done();
    }
}

// ── Public lifecycle helpers ────────────────────────────────────────────────
//
// Called from `xdg_shell`, `xwayland`, and `compositor` handlers to keep both
// foreign-toplevel protocols in sync with the compositor's window list.

/// Announce a new window to both foreign-toplevel protocols.
///
/// Must be called after the window has been mapped into the space.
pub fn announce_new_toplevel(state: &mut State, window: &Window) {
    let title = window_title(window);
    let app_id = window_app_id(window);

    // Determine which outputs this window is on (for output_enter events).
    let window_outputs = vec![state.output_for_window(window).clone()];

    // wlr-foreign-toplevel-management
    let wlr_handle = state.wlr_foreign_toplevel_state.new_toplevel(window.clone(), &title, &app_id, &window_outputs);

    // Send initial state (activated, etc.)
    let wlr_state = build_wlr_state(window);
    wlr_handle.send_state(&wlr_state);
    wlr_handle.send_done();

    // ext-foreign-toplevel-list (read-only)
    let ext_handle = state.ext_foreign_toplevel_list_state.new_toplevel::<State>(&title, &app_id);
    state.ext_toplevel_handles.push((window.clone(), ext_handle));

    tracing::debug!(title, app_id, "foreign-toplevel: announced new toplevel");
}

/// Close a window's foreign-toplevel handles in both protocols.
///
/// Must be called when the window is about to be destroyed or unmapped permanently.
pub fn close_toplevel(state: &mut State, window: &Window) {
    // wlr-foreign-toplevel-management: find and close
    let wlr_handle =
        state.wlr_foreign_toplevel_state.handles.iter().find(|h| !h.is_closed() && h.window() == *window).cloned();
    if let Some(handle) = wlr_handle {
        state.wlr_foreign_toplevel_state.remove_toplevel(&handle);
    }

    // ext-foreign-toplevel-list: find and close
    if let Some(idx) = state.ext_toplevel_handles.iter().position(|(w, _)| w == window) {
        let (_, ext_handle) = state.ext_toplevel_handles.remove(idx);
        ext_handle.send_closed();
    }

    tracing::debug!("foreign-toplevel: closed toplevel");
}

/// Check whether a window has already been announced to the foreign-toplevel
/// protocols.
fn is_announced(state: &State, window: &Window) -> bool {
    state.wlr_foreign_toplevel_state.handles.iter().any(|h| !h.is_closed() && h.window() == *window)
}

/// Check and forward title / `app_id` changes for a mapped window.
///
/// Call this from the compositor `commit` handler so that updates are sent
/// promptly when the client calls `set_title` / `set_app_id`.
///
/// If the window has not been announced yet (e.g. because it was just created
/// in `new_toplevel` with empty title / `app_id`), this function will announce
/// it on the first commit where title or `app_id` become available.  This
/// deferred announcement avoids sending empty metadata which causes taskbars
/// like ironbar to ignore the handle.
pub fn update_toplevel_metadata(state: &mut State, window: &Window) {
    let title = window_title(window);
    let app_id = window_app_id(window);

    // Lazy announce: if this window has not been announced yet and has
    // meaningful metadata, announce it now.
    if !is_announced(state, window) {
        if title.is_empty() && app_id.is_empty() {
            // Still no metadata — wait for the next commit.
            return;
        }
        announce_new_toplevel(state, window);
        // announce_new_toplevel already sent title/app_id/state/done,
        // so we can return early.
        return;
    }

    // wlr-foreign-toplevel-management: diff-aware sends
    if let Some(wlr_handle) =
        state.wlr_foreign_toplevel_state.handles.iter().find(|h| !h.is_closed() && h.window() == *window).cloned()
    {
        // send_title / send_app_id internally skip if unchanged
        wlr_handle.send_title(&title);
        wlr_handle.send_app_id(&app_id);

        // Update state (activated, maximized, fullscreen).
        // Use `last_focused_window` as the authoritative activated source
        // because XDG `current_state()` is stale until the client acks the
        // configure — relying on it would re-broadcast `Activated` for
        // background windows and confuse taskbars.
        let is_minimized = state.minimized_windows.iter().any(|(w, _)| w == window);
        let is_activated = state.last_focused_window.as_ref().is_some_and(|fw| fw == window);
        let mut new_state = build_wlr_state_with_minimized(window, is_minimized);

        // Strip stale activated flag from XDG state and set our authoritative one.
        strip_state(&mut new_state, WLR_STATE_ACTIVATED);
        if is_activated {
            push_state(&mut new_state, WLR_STATE_ACTIVATED);
        }

        wlr_handle.send_state(&new_state);

        // Always send done to finalize the batch (if anything changed)
        // Note: send_title/send_app_id/send_state are internally diff-checked,
        // but we call done unconditionally — a no-op done is harmless.
        wlr_handle.send_done();
    }

    // ext-foreign-toplevel-list: update title/app_id
    if let Some((_, ext_handle)) = state.ext_toplevel_handles.iter().find(|(w, _)| w == window) {
        ext_handle.send_title(&title);
        ext_handle.send_app_id(&app_id);
        ext_handle.send_done();
    }
}
