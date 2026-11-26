//! `XWayland` integration — runs an X11 server for legacy X11 applications.
//!
//! With `--xwayland`, the compositor spawns an `Xwayland` process and holds back
//! its readiness and child program until the server is ready and its X11 window
//! manager runs (spec: *compositor-session-lifecycle*). A server that does not get
//! there — missing, not executable, exits early, stays silent past
//! `--xwayland-timeout`, or its window manager fails — is a startup error: the
//! cause is logged at ERROR level with the server's last output, the server is
//! killed through its pidfd if it may still run, and the compositor exits with
//! `1`. X11 windows are mapped into the same `Space<Window>` as Wayland windows,
//! and the `XwmHandler` implementation handles their lifecycle.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::os::unix::io::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::Stdio;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use smithay::{
    desktop::Window,
    reexports::{
        calloop::{
            RegistrationToken,
            timer::{TimeoutAction, Timer},
        },
        wayland_server::Client,
    },
    utils::{Logical, Rectangle, Size},
    wayland::{
        selection::SelectionTarget,
        xwayland_keyboard_grab::XWaylandKeyboardGrabHandler,
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        X11Surface, X11Wm, XWayland, XWaylandClientData, XWaylandEvent, XwmHandler,
        xwm::{Reorder, ResizeEdge, X11Window, XwmId},
    },
};

use crate::state::State;

// -- X11 rectangles --
//
// This compositor places a window's buffer at its `Space` element location, so
// for an X11 window the element location is the window's X11 origin, and the
// compositor configures X11 windows with their full X11 rectangle. smithay's
// `X11Surface::geometry` is the *visible* area instead — for a window with
// `_GTK_FRAME_EXTENTS` (GTK's client-drawn shadows) it excludes the shadow and
// starts inside the window. It is for reporting (the window rectangles
// PlatynUI sees), never for configuring: an X11 window's own rectangle is
// `X11Surface::last_configure`.

/// The X11 size an X11 window has now (shadow included).
pub(crate) fn x11_size(x11: &X11Surface) -> Size<i32, Logical> {
    x11.last_configure().size
}

/// The size to keep or restore for `window`: the X11 size for an X11 window,
/// the window geometry otherwise.
pub(crate) fn window_size(window: &Window) -> Size<i32, Logical> {
    window.x11_surface().map_or_else(|| window.geometry().size, x11_size)
}

/// Tell an X11 client where it is now: its Space location (its X11 origin)
/// with its X11 size unchanged. Clients need this to place their
/// override-redirect windows (menus, tooltips) in screen coordinates.
pub(crate) fn configure_at_space_location(space: &smithay::desktop::Space<Window>, window: &Window, what: &str) {
    if let Some(x11) = window.x11_surface()
        && let Some(loc) = space.element_location(window)
        && let Err(err) = x11.configure(Rectangle::new(loc, x11_size(x11)))
    {
        tracing::warn!(%err, what, "failed to configure X11 window position");
    }
}

/// Find the smithay `Window` in the space that wraps a given [`X11Surface`].
fn find_x11_window(state: &State, surface: &X11Surface) -> Option<Window> {
    state.space.elements().find(|w| w.x11_surface().is_some_and(|x| x.window_id() == surface.window_id())).cloned()
}

/// Remove an X11 window from the space and close its foreign-toplevel handles.
fn remove_x11_window(state: &mut State, surface: &X11Surface) {
    if let Some(element) = find_x11_window(state, surface) {
        crate::handlers::foreign_toplevel::close_toplevel(state, &element);
        state.pre_maximize_positions.retain(|(w, _, _)| w != &element);
        state.pre_fullscreen_states.retain(|(w, _, _)| w != &element);
        state.space.unmap_elem(&element);
    }
}

/// Where a requested `XWayland` stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XWaylandPhase {
    /// `--xwayland` was not given.
    #[default]
    NotRequested,
    /// The server was started and is not usable yet.
    Starting,
    /// The server is ready and its X11 window manager runs.
    Running,
    /// The server failed to start or terminated during the session.
    Terminated,
}

/// The state of a requested `XWayland`.
pub struct XWaylandState {
    /// The X11 window manager connection, once it runs. Kept after the server
    /// terminates, so a queued smithay callback still finds it.
    pub wm: Option<X11Wm>,
    /// The server's Wayland client: its liveness tells whether the process still
    /// runs, and its client data holds the process's pidfd.
    pub client: Client,
    /// The display number `XWayland` is using (e.g., `:1`).
    pub display: u32,
    /// The event source that reports readiness, until the startup ends.
    source: Option<RegistrationToken>,
    /// When the server was started, for `--xwayland-timeout`.
    started: Instant,
    /// The server's forwarded stdout and stderr.
    output: Arc<ServerOutput>,
}

/// Whether a failed server's process may still run when the startup fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerProcess {
    /// Its Wayland client disconnected: the process exited.
    Exited,
    /// It may still run, so it is killed.
    MayRun,
}

/// How often the startup watch checks the server.
const WATCH_INTERVAL: Duration = Duration::from_millis(100);
/// How many of the server's last output lines a startup error quotes.
const TAIL_LINES: usize = 8;
/// How long a startup error waits for the readers of an ended server to see end of file.
const OUTPUT_EOF_WAIT: Duration = Duration::from_millis(500);

/// The last lines `Xwayland` wrote to stdout and stderr, kept for a startup error.
struct ServerOutput {
    tail: Mutex<OutputTail>,
    closed: Condvar,
}

struct OutputTail {
    lines: VecDeque<String>,
    open_pipes: u8,
}

impl ServerOutput {
    fn new(pipes: u8) -> Self {
        Self { tail: Mutex::new(OutputTail { lines: VecDeque::new(), open_pipes: pipes }), closed: Condvar::new() }
    }

    /// Log each line of `pipe` under the `xwayland` target and keep the last ones.
    fn forward(self: &Arc<Self>, stream: &'static str, pipe: std::io::PipeReader) {
        let output = Arc::clone(self);
        let reader = move || {
            for line in BufReader::new(pipe).lines() {
                let Ok(line) = line else { break };
                tracing::debug!(target: "xwayland", stream, "{line}");
                let mut tail = output.tail.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                if tail.lines.len() == TAIL_LINES {
                    tail.lines.pop_front();
                }
                tail.lines.push_back(line);
            }
            let mut tail = output.tail.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            tail.open_pipes = tail.open_pipes.saturating_sub(1);
            output.closed.notify_all();
        };
        if let Err(err) = std::thread::Builder::new().name(format!("xwayland-{stream}")).spawn(reader) {
            tracing::warn!(%err, stream, "failed to start the Xwayland output reader");
        }
    }

    /// The last lines, after waiting up to [`OUTPUT_EOF_WAIT`] for both pipes to close
    /// when `ended` (so an exited server's final lines are not lost to scheduling).
    fn last_lines(&self, ended: bool) -> Vec<String> {
        let tail = self.tail.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let tail = if ended {
            self.closed
                .wait_timeout_while(tail, OUTPUT_EOF_WAIT, |tail| tail.open_pipes > 0)
                .map_or_else(|err| err.into_inner().0, |(tail, _)| tail)
        } else {
            tail
        };
        tail.lines.iter().cloned().collect()
    }
}

/// Kill a server that failed to start through its pidfd, which refers to exactly that
/// process, so a reused process ID can never hit another one. Returns whether the
/// process is gone (killed now or already exited).
fn kill_server(client: &Client) -> bool {
    let Some(pidfd) = client.get_data::<XWaylandClientData>().and_then(XWaylandClientData::pidfd) else {
        tracing::warn!("cannot terminate Xwayland: smithay has no process handle (pidfd) for it");
        return false;
    };
    match rustix::process::pidfd_send_signal(pidfd, rustix::process::Signal::KILL) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => true,
        Err(err) => {
            tracing::warn!(%err, "failed to kill Xwayland");
            false
        }
    }
}

/// The startup error for a spawn failure, named by its cause.
fn spawn_error_cause(err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::NotFound => format!("Xwayland not found on PATH: {err}"),
        std::io::ErrorKind::PermissionDenied => format!("Xwayland is not executable: {err}"),
        _ => format!("failed to start Xwayland: {err}"),
    }
}

impl State {
    /// Start `XWayland` for `--xwayland`.
    ///
    /// Registers the server as a calloop event source plus a startup watch. The
    /// session becomes ready in `xwayland_ready` once the window manager
    /// runs; every way the server fails to get there records a startup error.
    pub fn start_xwayland(&mut self) {
        self.xwayland_phase = XWaylandPhase::Starting;
        let pipes = std::io::pipe().and_then(|stdout| Ok((stdout, std::io::pipe()?)));
        let ((stdout_read, stdout_write), (stderr_read, stderr_write)) = match pipes {
            Ok(pipes) => pipes,
            Err(err) => {
                self.xwayland_phase = XWaylandPhase::Terminated;
                self.record_startup_error(format!("failed to create the pipes for Xwayland's output: {err}"));
                return;
            }
        };

        let spawned = XWayland::spawn(
            &self.display_handle,
            None,
            std::iter::empty::<(OsString, OsString)>(),
            std::iter::empty::<OsString>(),
            true,
            Stdio::from(stdout_write),
            Stdio::from(stderr_write),
            |_| (),
        );
        let (xwayland, client) = match spawned {
            Ok(spawned) => spawned,
            Err(err) => {
                self.xwayland_phase = XWaylandPhase::Terminated;
                self.record_startup_error(spawn_error_cause(&err));
                return;
            }
        };

        let output = Arc::new(ServerOutput::new(2));
        output.forward("stdout", stdout_read);
        output.forward("stderr", stderr_read);
        self.xwayland =
            Some(XWaylandState { wm: None, client, display: 0, source: None, started: Instant::now(), output });

        let source = self.loop_handle.insert_source(xwayland, |event, (), state| match event {
            XWaylandEvent::Ready { x11_socket, display_number } => state.xwayland_ready(x11_socket, display_number),
            XWaylandEvent::Error => {
                if state.xwayland_phase == XWaylandPhase::Starting {
                    state.fail_xwayland_start(
                        "XWayland failed while starting: reading its display descriptor failed".into(),
                        ServerProcess::MayRun,
                    );
                }
            }
        });
        match source {
            Ok(token) => {
                if let Some(xwayland) = self.xwayland.as_mut() {
                    xwayland.source = Some(token);
                }
            }
            Err(err) => {
                self.fail_xwayland_start(
                    format!("failed to register the XWayland event source: {err}"),
                    ServerProcess::MayRun,
                );
                return;
            }
        }

        let watch = self
            .loop_handle
            .insert_source(Timer::from_duration(WATCH_INTERVAL), |_, (), state| state.watch_xwayland_start());
        if let Err(err) = watch {
            self.fail_xwayland_start(
                format!("failed to register the XWayland startup watch: {err}"),
                ServerProcess::MayRun,
            );
        }
    }

    /// One tick of the startup watch: ends it once `XWayland` runs, and fails the start
    /// when the server exited or `--xwayland-timeout` passed.
    fn watch_xwayland_start(&mut self) -> TimeoutAction {
        if self.xwayland_phase != XWaylandPhase::Starting {
            return TimeoutAction::Drop;
        }
        let Some(xwayland) = self.xwayland.as_ref() else { return TimeoutAction::Drop };
        // A dead client's data is gone from the display; its ID carries a serial, so a
        // reused slot does not read as alive.
        if self.display_handle.backend_handle().get_client_data(xwayland.client.id()).is_err() {
            self.fail_xwayland_start("XWayland exited before it was ready".into(), ServerProcess::Exited);
            return TimeoutAction::Drop;
        }
        if xwayland.started.elapsed() >= self.xwayland_timeout {
            let secs = self.xwayland_timeout.as_secs();
            self.fail_xwayland_start(format!("XWayland did not become ready within {secs} s"), ServerProcess::MayRun);
            return TimeoutAction::Drop;
        }
        TimeoutAction::ToDuration(WATCH_INTERVAL)
    }

    /// The server reported a display: start its window manager first, and only then
    /// hand out `DISPLAY`, announce readiness and start the child.
    fn xwayland_ready(&mut self, x11_socket: UnixStream, display_number: u32) {
        if self.xwayland_phase != XWaylandPhase::Starting {
            return;
        }
        tracing::info!(display = display_number, "XWayland ready");
        let Some(client) = self.xwayland.as_ref().map(|xwayland| xwayland.client.clone()) else { return };
        let dh = self.display_handle.clone();
        match X11Wm::start_wm(self.loop_handle.clone(), &dh, x11_socket, client) {
            Ok(wm) => {
                if let Some(xwayland) = self.xwayland.as_mut() {
                    xwayland.wm = Some(wm);
                    xwayland.display = display_number;
                    xwayland.source = None;
                }
                self.xwayland_phase = XWaylandPhase::Running;
                tracing::info!(display = display_number, "X11 window manager started");
                #[allow(unsafe_code)]
                // SAFETY: Called from the single-threaded event loop.
                unsafe {
                    std::env::set_var("DISPLAY", format!(":{display_number}"));
                }
                self.finish_startup();
            }
            Err(err) => {
                self.fail_xwayland_start(format!("X11 window manager failed to start: {err}"), ServerProcess::MayRun);
            }
        }
    }

    /// Fail the start of `XWayland`: kill the server if it may still run, drop its event
    /// source, and record the startup error with the server's last output.
    fn fail_xwayland_start(&mut self, cause: String, process: ServerProcess) {
        self.xwayland_phase = XWaylandPhase::Terminated;
        let Some(xwayland) = self.xwayland.as_mut() else {
            self.record_startup_error(cause);
            return;
        };
        // Kill before the source is dropped, so smithay's reaper collects the exit as usual.
        let ended = process == ServerProcess::Exited || kill_server(&xwayland.client);
        if let Some(token) = xwayland.source.take() {
            self.loop_handle.remove(token);
        }
        let lines = xwayland.output.last_lines(ended);
        let cause =
            if lines.is_empty() { cause } else { format!("{cause}; Xwayland's last output: {}", lines.join(" | ")) };
        self.record_startup_error(cause);
    }

    /// A session that ends while `XWayland` is still starting never became usable:
    /// record that as the startup error and kill the server.
    pub(crate) fn abandon_starting_xwayland(&mut self) {
        if self.xwayland_phase == XWaylandPhase::Starting {
            self.fail_xwayland_start(
                "the session ended while XWayland was still starting".into(),
                ServerProcess::MayRun,
            );
        }
    }

    /// Remove every X11 window of a terminated server: the mapped ones from the space
    /// and the minimized ones from the minimized list, closing their foreign-toplevel
    /// handles either way.
    fn remove_all_x11_windows(&mut self) {
        let mapped: Vec<Window> = self.space.elements().filter(|w| w.x11_surface().is_some()).cloned().collect();
        let minimized: Vec<Window> =
            self.minimized_windows.iter().filter(|(w, _)| w.x11_surface().is_some()).map(|(w, _)| w.clone()).collect();
        for window in &mapped {
            crate::handlers::foreign_toplevel::close_toplevel(self, window);
            self.pre_maximize_positions.retain(|(w, _, _)| w != window);
            self.pre_fullscreen_states.retain(|(w, _, _)| w != window);
            self.space.unmap_elem(window);
        }
        for window in &minimized {
            crate::handlers::foreign_toplevel::close_toplevel(self, window);
        }
        self.minimized_windows.retain(|(w, _)| w.x11_surface().is_none());
    }
}

// -- XWaylandShellHandler --

impl XWaylandShellHandler for State {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        self.xwayland_shell_state.as_mut().expect("XWaylandShellState not initialized")
    }
}

// -- XWaylandKeyboardGrabHandler --

impl XWaylandKeyboardGrabHandler for State {
    fn keyboard_focus_for_xsurface(
        &self,
        surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    ) -> Option<crate::focus::KeyboardFocusTarget> {
        self.space.elements().find_map(|window| {
            let dominated = window.x11_surface()?.wl_surface()?;
            (dominated == *surface).then(|| crate::focus::KeyboardFocusTarget::Window(window.clone()))
        })
    }
}

// -- XwmHandler --

impl XwmHandler for State {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwayland.as_mut().and_then(|s| s.wm.as_mut()).expect("`XWayland` WM not initialized")
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {
        // Window created but not yet requesting to be mapped — nothing to do
    }

    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {
        // Override-redirect windows (tooltips, menus) — tracked when mapped
    }

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        tracing::debug!(
            title = window.title(),
            class = ?window.class(),
            override_redirect = window.is_override_redirect(),
            is_decorated = window.is_decorated(),
            "X11 window map request",
        );

        if let Err(err) = window.set_mapped(true) {
            tracing::warn!(%err, "failed to set X11 window as mapped");
        }

        let smithay_window = Window::new_x11_window(window);
        let usable_origin = self.usable_geometry_for_output(&self.output.clone()).loc;
        crate::workspace::map_window(&mut self.space, smithay_window.clone(), usable_origin);

        // X11 windows that get SSD need their position shifted down for the title bar.
        // (Wayland windows get this in XdgDecorationHandler::new_decoration, but X11
        // windows don't participate in xdg-decoration negotiation.)
        if crate::decorations::window_has_ssd(&smithay_window)
            && let Some(loc) = self.space.element_location(&smithay_window)
        {
            self.space.map_element(smithay_window.clone(), (loc.x, loc.y + crate::decorations::TITLEBAR_HEIGHT), false);
        }

        // Tell the X11 client its actual compositor position so it can
        // calculate correct screen coordinates for override-redirect
        // windows (menus, tooltips, dropdowns).  Without this, the client
        // still thinks it is at its initial (often 0,0) position and any
        // popup it opens ends up in the wrong place.
        configure_at_space_location(&self.space, &smithay_window, "after mapping");

        // Announce to foreign-toplevel protocols
        crate::handlers::foreign_toplevel::announce_new_toplevel(self, &smithay_window);
    }

    fn map_window_notify(&mut self, _xwm: XwmId, _window: X11Surface) {
        // Notification that mapping succeeded — nothing extra needed
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        // Override-redirect windows (menus, tooltips, dropdowns) set their own
        // position in X11 screen coordinates — honour that instead of cascading.
        let geo = window.last_configure();
        tracing::debug!(
            x = geo.loc.x,
            y = geo.loc.y,
            w = geo.size.w,
            h = geo.size.h,
            "mapping override-redirect X11 window at its requested position",
        );
        let smithay_window = Window::new_x11_window(window);
        self.space.map_element(smithay_window, geo.loc, false);
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        tracing::debug!(title = window.title(), "X11 window unmapped");
        remove_x11_window(self, &window);
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        tracing::debug!(title = window.title(), "X11 window destroyed");
        remove_x11_window(self, &window);
    }

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        // The request is in X11 coordinates: answer with X11 rectangles.
        let geo = window.last_configure();
        let is_mapped = find_x11_window(self, &window).is_some();

        if is_mapped && !window.is_override_redirect() {
            // Mapped regular windows: allow resize but preserve the compositor-
            // managed position.  Letting the client move itself would desync
            // the Space element location and break popup coordinate math.
            let new_w = w.map_or(geo.size.w, u32::cast_signed);
            let new_h = h.map_or(geo.size.h, u32::cast_signed);

            if (new_w, new_h) == (geo.size.w, geo.size.h) {
                // Ack with current state — no change.
                if let Err(err) = window.configure(geo) {
                    tracing::warn!(%err, "failed to ack X11 configure request");
                }
            } else if let Err(err) = window.configure(Rectangle::new(geo.loc, (new_w, new_h).into())) {
                tracing::warn!(%err, "failed to configure X11 window resize");
            }
        } else {
            // Unmapped windows and override-redirect windows: grant whatever
            // the client requests (they manage their own position).
            let new_x = x.unwrap_or(geo.loc.x);
            let new_y = y.unwrap_or(geo.loc.y);
            let new_w = w.unwrap_or(geo.size.w.unsigned_abs());
            let new_h = h.unwrap_or(geo.size.h.unsigned_abs());

            if let Err(err) = window
                .configure(Rectangle::new((new_x, new_y).into(), (new_w.cast_signed(), new_h.cast_signed()).into()))
            {
                tracing::warn!(%err, "failed to configure X11 window");
            }
        }
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        geometry: Rectangle<i32, Logical>,
        _above: Option<X11Window>,
    ) {
        // Update the window position in our space if it was reconfigured
        if let Some(element) = find_x11_window(self, &window) {
            self.space.map_element(element, geometry.loc, true);
        }
    }

    fn resize_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32, resize_edge: ResizeEdge) {
        let Some(element) = find_x11_window(self, &window) else {
            return;
        };

        let edge = match resize_edge {
            ResizeEdge::Top => crate::decorations::Focus::ResizeTop,
            ResizeEdge::Bottom => crate::decorations::Focus::ResizeBottom,
            ResizeEdge::Left => crate::decorations::Focus::ResizeLeft,
            ResizeEdge::Right => crate::decorations::Focus::ResizeRight,
            ResizeEdge::TopLeft => crate::decorations::Focus::ResizeTopLeft,
            ResizeEdge::TopRight => crate::decorations::Focus::ResizeTopRight,
            ResizeEdge::BottomLeft => crate::decorations::Focus::ResizeBottomLeft,
            ResizeEdge::BottomRight => crate::decorations::Focus::ResizeBottomRight,
        };

        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let keyboard = self.keyboard();
        keyboard.set_focus(self, Some(crate::focus::KeyboardFocusTarget::Window(element.clone())), serial);
        self.space.raise_element(&element, true);
        crate::grabs::handle_resize_request(self, &self.seat.clone(), &element, edge, serial);
    }

    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32) {
        let Some(element) = find_x11_window(self, &window) else {
            return;
        };

        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let keyboard = self.keyboard();
        keyboard.set_focus(self, Some(crate::focus::KeyboardFocusTarget::Window(element.clone())), serial);
        self.space.raise_element(&element, true);
        crate::grabs::handle_move_request(self, &self.seat.clone(), &element, serial);
    }

    fn maximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let Some(element) = find_x11_window(self, &window) else {
            return;
        };

        if window.is_maximized() {
            tracing::debug!(title = window.title(), "X11 maximize_request: already maximized");
            return;
        }

        // Save current position and size before maximizing.
        let current_loc = self.space.element_location(&element).unwrap_or_default();
        let current_size = Some(x11_size(&window));
        self.pre_maximize_positions.retain(|(w, _, _)| w != &element);
        self.pre_maximize_positions.push((element.clone(), current_loc, current_size));

        // Maximize to usable area minus titlebar.
        let usable_geo = self.usable_geometry_for_window(&element);
        let y_offset =
            if crate::decorations::window_has_ssd(&element) { crate::decorations::TITLEBAR_HEIGHT } else { 0 };
        let max_size = smithay::utils::Size::from((usable_geo.size.w, usable_geo.size.h - y_offset));

        if let Err(err) = window.set_maximized(true) {
            tracing::warn!(%err, "failed to maximize X11 window");
        }
        if let Err(err) =
            window.configure(Rectangle::new((usable_geo.loc.x, usable_geo.loc.y + y_offset).into(), max_size))
        {
            tracing::warn!(%err, "failed to configure X11 window for maximize");
        }
        self.space.map_element(element, (usable_geo.loc.x, usable_geo.loc.y + y_offset), true);

        tracing::debug!(title = window.title(), "X11 window maximized via _NET_WM_STATE");
    }

    fn unmaximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let Some(element) = find_x11_window(self, &window) else {
            return;
        };

        if !window.is_maximized() {
            tracing::debug!(title = window.title(), "X11 unmaximize_request: not maximized");
            return;
        }

        // Restore saved position and size.
        let saved = self
            .pre_maximize_positions
            .iter()
            .position(|(w, _, _)| w == &element)
            .map(|i| self.pre_maximize_positions.remove(i));

        if let Err(err) = window.set_maximized(false) {
            tracing::warn!(%err, "failed to unmaximize X11 window");
        }

        if let Some((_, pos, size)) = saved {
            if let Err(err) = window.configure(size.map(|s| Rectangle::new(pos, s))) {
                tracing::warn!(%err, "failed to configure X11 window after unmaximize");
            }
            self.space.map_element(element, pos, true);
        } else if let Err(err) = window.configure(None) {
            tracing::warn!(%err, "failed to configure X11 window after unmaximize");
        }

        tracing::debug!(title = window.title(), "X11 window unmaximized via _NET_WM_STATE");
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let Some(element) = find_x11_window(self, &window) else {
            return;
        };

        let output_geo = self.output_geometry_for_window(&element);

        // Save the current position and size before going fullscreen.
        let current_loc = self.space.element_location(&element).unwrap_or_default();
        let current_size = Some(x11_size(&window));
        self.pre_fullscreen_states.retain(|(w, _, _)| w != &element);
        self.pre_fullscreen_states.push((element.clone(), current_loc, current_size));

        // Set fullscreen and resize to output dimensions.
        if let Err(err) = window.set_fullscreen(true) {
            tracing::warn!(%err, "failed to set X11 window fullscreen");
        }
        if let Err(err) = window.configure(output_geo) {
            tracing::warn!(%err, "failed to configure X11 window for fullscreen");
        }
        self.space.map_element(element, output_geo.loc, true);

        tracing::debug!(output_w = output_geo.size.w, output_h = output_geo.size.h, "X11 window set to fullscreen",);
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let Some(element) = find_x11_window(self, &window) else {
            return;
        };

        // Restore saved position and size.
        let saved = self
            .pre_fullscreen_states
            .iter()
            .position(|(w, _, _)| w == &element)
            .map(|i| self.pre_fullscreen_states.remove(i));

        if let Err(err) = window.set_fullscreen(false) {
            tracing::warn!(%err, "failed to unset X11 window fullscreen");
        }

        if let Some((_, pos, size)) = saved {
            if let Some(size) = size
                && let Err(err) = window.configure(Rectangle::new(pos, size))
            {
                tracing::warn!(%err, "failed to configure X11 window after unfullscreen");
            }
            self.space.map_element(element, pos, true);
        }

        tracing::debug!("X11 window restored from fullscreen");
    }

    fn allow_selection_access(&mut self, _xwm: XwmId, _selection: SelectionTarget) -> bool {
        // Allow X11 clients to access clipboard
        true
    }

    fn send_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_type: String, fd: OwnedFd) {
        // Forward clipboard from Wayland to X11
        match selection {
            SelectionTarget::Clipboard => {
                if let Err(err) = smithay::wayland::selection::data_device::request_data_device_client_selection(
                    &self.seat, mime_type, fd,
                ) {
                    tracing::warn!(%err, "failed to send clipboard selection to XWayland");
                }
            }
            SelectionTarget::Primary => {
                if let Err(err) = smithay::wayland::selection::primary_selection::request_primary_client_selection(
                    &self.seat, mime_type, fd,
                ) {
                    tracing::warn!(%err, "failed to send primary selection to XWayland");
                }
            }
        }
    }

    fn new_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        tracing::debug!(?selection, ?mime_types, "X11 new selection");
        // X11 client set a new selection — propagate to Wayland clients
        match selection {
            SelectionTarget::Clipboard => {
                smithay::wayland::selection::data_device::set_data_device_selection(
                    &self.display_handle,
                    &self.seat,
                    mime_types,
                    (),
                );
            }
            SelectionTarget::Primary => {
                smithay::wayland::selection::primary_selection::set_primary_selection(
                    &self.display_handle,
                    &self.seat,
                    mime_types,
                    (),
                );
            }
        }
    }

    fn cleared_selection(&mut self, _xwm: XwmId, selection: SelectionTarget) {
        match selection {
            SelectionTarget::Clipboard => {
                smithay::wayland::selection::data_device::clear_data_device_selection(&self.display_handle, &self.seat);
            }
            SelectionTarget::Primary => {
                smithay::wayland::selection::primary_selection::clear_primary_selection(
                    &self.display_handle,
                    &self.seat,
                );
            }
        }
    }

    fn disconnected(&mut self, _xwm: XwmId) {
        // smithay logs its own line when the process ends; this one is the
        // compositor's, and says what it did about it.
        tracing::error!("XWayland terminated during the session; removing its X11 windows and keeping Wayland clients");
        self.remove_all_x11_windows();
        self.xwayland_phase = XWaylandPhase::Terminated;
    }
}
