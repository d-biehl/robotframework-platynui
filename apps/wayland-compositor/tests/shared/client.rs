//! Raw Wayland client for the compositor's integration tests.
//!
//! Speaks the protocols the tests drive directly — xdg-shell toplevels and
//! popups with solid-color `wl_shm` buffers, the seat's pointer and keyboard
//! (recording what they receive), `zwlr_virtual_pointer_v1`,
//! `zwp_virtual_keyboard_v1`, `wp_pointer_warp_v1`, and the
//! `ext-image-capture-source` / `ext-image-copy-capture` /
//! `ext-foreign-toplevel-list` trio — so each test can state a spec scenario in
//! a few lines.
//!
//! Every wait has a deadline: [`Client::dispatch_until`] polls the socket
//! instead of blocking, so a missing event fails the test instead of hanging it.
#![allow(dead_code)]

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::FileExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::ObjectId;
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
    ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
};
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_cursor_session_v1::{self, ExtImageCopyCaptureCursorSessionV1},
    ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1},
    ext_image_copy_capture_manager_v1::{self, ExtImageCopyCaptureManagerV1},
    ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1},
};
use wayland_protocols::wp::pointer_warp::v1::client::wp_pointer_warp_v1::WpPointerWarpV1;
use wayland_protocols::xdg::shell::client::{xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1, zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

pub type BoxError = Box<dyn std::error::Error>;

/// Default deadline for one awaited condition.
pub const WAIT: Duration = Duration::from_secs(10);

// ─── Recorded events ────────────────────────────────────────────────────

/// What the seat pointer received.
#[derive(Debug, Clone, PartialEq)]
pub enum PointerEvent {
    Enter { serial: u32, surface: ObjectId, x: f64, y: f64 },
    Leave { serial: u32, surface: ObjectId },
    Motion { x: f64, y: f64 },
    Button { serial: u32, button: u32, pressed: bool },
}

/// What the seat keyboard received.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyboardEvent {
    Keymap(String),
    Enter { surface: ObjectId },
    Leave,
    Key { key: u32, pressed: bool },
    Modifiers { depressed: u32, latched: u32, locked: u32, group: u32 },
}

/// Constraints and state a capture session announced.
#[derive(Debug, Clone, Default)]
pub struct SessionInfo {
    pub buffer_size: Option<(u32, u32)>,
    pub shm_formats: Vec<wl_shm::Format>,
    pub done: bool,
    pub stopped: bool,
}

/// How a capture frame ended (if it has).
#[derive(Debug, Clone, PartialEq)]
pub enum FrameOutcome {
    Pending,
    Ready,
    Failed(WEnum<ext_image_copy_capture_frame_v1::FailureReason>),
}

/// Events of a capture frame.
#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub outcome: FrameOutcome,
    pub transform: bool,
    pub damage: bool,
    pub presentation_time: bool,
}

impl Default for FrameInfo {
    fn default() -> Self {
        Self { outcome: FrameOutcome::Pending, transform: false, damage: false, presentation_time: false }
    }
}

/// Events of a cursor capture session.
#[derive(Debug, Clone, Default)]
pub struct CursorSessionInfo {
    pub entered: bool,
    pub left: bool,
    pub position: Option<(i32, i32)>,
    pub hotspot: Option<(i32, i32)>,
}

/// What a `wl_output` announced.
#[derive(Debug, Clone, Default)]
pub struct OutputInfo {
    pub name: String,
    /// Global position (`wl_output.geometry`).
    pub position: (i32, i32),
    /// Current mode in physical pixels.
    pub mode: (i32, i32),
    pub scale: i32,
}

/// An `ext_foreign_toplevel_handle_v1` and what it announced.
#[derive(Debug, Clone, Default)]
pub struct HandleInfo {
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub closed: bool,
}

// ─── Client state (all dispatch targets) ────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum SurfaceRole {
    Toplevel,
    Popup,
}

#[derive(Default)]
pub struct ClientState {
    pub globals: Vec<(u32, String, u32)>,
    registry: Option<wl_registry::WlRegistry>,
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    seat: Option<wl_seat::WlSeat>,
    pub pointer: Option<wl_pointer::WlPointer>,
    pub keyboard: Option<wl_keyboard::WlKeyboard>,
    pub outputs: Vec<wl_output::WlOutput>,
    pub output_info: HashMap<ObjectId, OutputInfo>,
    configured: HashMap<ObjectId, bool>,
    pub pointer_events: Vec<PointerEvent>,
    pub keyboard_events: Vec<KeyboardEvent>,
    pub sessions: HashMap<ObjectId, SessionInfo>,
    pub frames: HashMap<ObjectId, FrameInfo>,
    pub cursor_sessions: HashMap<ObjectId, CursorSessionInfo>,
    pub handles: Vec<(ExtForeignToplevelHandleV1, HandleInfo)>,
}

impl ClientState {
    /// The serial of the last pointer enter on `surface`, if any.
    pub fn last_enter_serial(&self, surface: &wl_surface::WlSurface) -> Option<u32> {
        let id = surface.id();
        self.pointer_events.iter().rev().find_map(|e| match e {
            PointerEvent::Enter { serial, surface, .. } if *surface == id => Some(*serial),
            _ => None,
        })
    }

    /// Whether the pointer is currently over `surface` (last enter/leave).
    pub fn pointer_is_over(&self, surface: &wl_surface::WlSurface) -> bool {
        let id = surface.id();
        self.pointer_events
            .iter()
            .rev()
            .find_map(|e| match e {
                PointerEvent::Enter { surface, .. } => Some(*surface == id),
                PointerEvent::Leave { surface, .. } if *surface == id => Some(false),
                _ => None,
            })
            .unwrap_or(false)
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for ClientState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_compositor" => state.compositor = Some(registry.bind(name, version.min(4), qh, ())),
                "wl_shm" => state.shm = Some(registry.bind(name, 1, qh, ())),
                "xdg_wm_base" => state.wm_base = Some(registry.bind(name, version.min(2), qh, ())),
                "wl_seat" if state.seat.is_none() => state.seat = Some(registry.bind(name, version.min(7), qh, ())),
                "wl_output" => state.outputs.push(registry.bind(name, version.min(4), qh, ())),
                _ => {}
            }
            state.globals.push((name, interface, version));
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for ClientState {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        (): &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let info = state.output_info.entry(output.id()).or_default();
        match event {
            wl_output::Event::Geometry { x, y, .. } => info.position = (x, y),
            wl_output::Event::Mode { width, height, .. } => info.mode = (width, height),
            wl_output::Event::Scale { factor } => info.scale = factor,
            wl_output::Event::Name { name } => info.name = name,
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for ClientState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        (): &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities: WEnum::Value(caps) } = event {
            if caps.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qh, ()));
            }
            if caps.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for ClientState {
    fn event(
        state: &mut Self,
        _pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        (): &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let recorded = match event {
            wl_pointer::Event::Enter { serial, surface, surface_x, surface_y } => {
                Some(PointerEvent::Enter { serial, surface: surface.id(), x: surface_x, y: surface_y })
            }
            wl_pointer::Event::Leave { serial, surface } => Some(PointerEvent::Leave { serial, surface: surface.id() }),
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                Some(PointerEvent::Motion { x: surface_x, y: surface_y })
            }
            wl_pointer::Event::Button { serial, button, state: s, .. } => Some(PointerEvent::Button {
                serial,
                button,
                pressed: matches!(s, WEnum::Value(wl_pointer::ButtonState::Pressed)),
            }),
            _ => None,
        };
        state.pointer_events.extend(recorded);
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for ClientState {
    fn event(
        state: &mut Self,
        _keyboard: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        (): &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let recorded = match event {
            wl_keyboard::Event::Keymap { fd, size, .. } => Some(KeyboardEvent::Keymap(read_keymap(fd, size))),
            wl_keyboard::Event::Enter { surface, .. } => Some(KeyboardEvent::Enter { surface: surface.id() }),
            wl_keyboard::Event::Leave { .. } => Some(KeyboardEvent::Leave),
            wl_keyboard::Event::Key { key, state: s, .. } => {
                Some(KeyboardEvent::Key { key, pressed: matches!(s, WEnum::Value(wl_keyboard::KeyState::Pressed)) })
            }
            wl_keyboard::Event::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. } => {
                Some(KeyboardEvent::Modifiers {
                    depressed: mods_depressed,
                    latched: mods_latched,
                    locked: mods_locked,
                    group,
                })
            }
            _ => None,
        };
        state.keyboard_events.extend(recorded);
    }
}

fn read_keymap(fd: OwnedFd, size: u32) -> String {
    let mut buf = vec![0u8; size as usize];
    let file = File::from(fd);
    let _ = file.read_exact_at(&mut buf, 0);
    String::from_utf8_lossy(&buf).trim_end_matches('\0').to_string()
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for ClientState {
    fn event(
        _state: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        (): &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ObjectId> for ClientState {
    fn event(
        state: &mut Self,
        xdg: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        surface: &ObjectId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg.ack_configure(serial);
            state.configured.insert(surface.clone(), true);
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        _list: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        (): &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            state.handles.push((toplevel, HandleInfo::default()));
        }
    }

    wayland_client::event_created_child!(ClientState, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        (): &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some((_, info)) = state.handles.iter_mut().find(|(h, _)| h == handle) else {
            return;
        };
        match event {
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => info.app_id = Some(app_id),
            ext_foreign_toplevel_handle_v1::Event::Title { title } => info.title = Some(title),
            ext_foreign_toplevel_handle_v1::Event::Closed => info.closed = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        session: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        (): &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let info = state.sessions.entry(session.id()).or_default();
        match event {
            ext_image_copy_capture_session_v1::Event::BufferSize { width, height } => {
                info.buffer_size = Some((width, height));
            }
            ext_image_copy_capture_session_v1::Event::ShmFormat { format: WEnum::Value(format) } => {
                info.shm_formats.push(format);
            }
            ext_image_copy_capture_session_v1::Event::Done => info.done = true,
            ext_image_copy_capture_session_v1::Event::Stopped => info.stopped = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        frame: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        (): &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let info = state.frames.entry(frame.id()).or_default();
        match event {
            ext_image_copy_capture_frame_v1::Event::Transform { .. } => info.transform = true,
            ext_image_copy_capture_frame_v1::Event::Damage { .. } => info.damage = true,
            ext_image_copy_capture_frame_v1::Event::PresentationTime { .. } => info.presentation_time = true,
            ext_image_copy_capture_frame_v1::Event::Ready => info.outcome = FrameOutcome::Ready,
            ext_image_copy_capture_frame_v1::Event::Failed { reason } => info.outcome = FrameOutcome::Failed(reason),
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureCursorSessionV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        session: &ExtImageCopyCaptureCursorSessionV1,
        event: ext_image_copy_capture_cursor_session_v1::Event,
        (): &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let info = state.cursor_sessions.entry(session.id()).or_default();
        match event {
            ext_image_copy_capture_cursor_session_v1::Event::Enter => info.entered = true,
            ext_image_copy_capture_cursor_session_v1::Event::Leave => info.left = true,
            ext_image_copy_capture_cursor_session_v1::Event::Position { x, y } => info.position = Some((x, y)),
            ext_image_copy_capture_cursor_session_v1::Event::Hotspot { x, y } => info.hotspot = Some((x, y)),
            _ => {}
        }
    }
}

wayland_client::delegate_noop!(ClientState: ignore wl_compositor::WlCompositor);
wayland_client::delegate_noop!(ClientState: ignore wl_surface::WlSurface);
wayland_client::delegate_noop!(ClientState: ignore wl_shm::WlShm);
wayland_client::delegate_noop!(ClientState: ignore wl_shm_pool::WlShmPool);
wayland_client::delegate_noop!(ClientState: ignore wl_buffer::WlBuffer);
wayland_client::delegate_noop!(ClientState: ignore xdg_positioner::XdgPositioner);
wayland_client::delegate_noop!(ClientState: ignore xdg_toplevel::XdgToplevel);
wayland_client::delegate_noop!(ClientState: ignore xdg_popup::XdgPopup);
wayland_client::delegate_noop!(ClientState: ignore ZwlrVirtualPointerManagerV1);
wayland_client::delegate_noop!(ClientState: ignore ZwlrVirtualPointerV1);
wayland_client::delegate_noop!(ClientState: ignore ZwpVirtualKeyboardManagerV1);
wayland_client::delegate_noop!(ClientState: ignore ZwpVirtualKeyboardV1);
wayland_client::delegate_noop!(ClientState: ignore WpPointerWarpV1);
wayland_client::delegate_noop!(ClientState: ignore ExtOutputImageCaptureSourceManagerV1);
wayland_client::delegate_noop!(ClientState: ignore ExtForeignToplevelImageCaptureSourceManagerV1);
wayland_client::delegate_noop!(ClientState: ignore ExtImageCaptureSourceV1);
wayland_client::delegate_noop!(ClientState: ignore ExtImageCopyCaptureManagerV1);

// ─── Surfaces and buffers ───────────────────────────────────────────────

/// A `wl_shm` buffer backed by a file the test can read back.
pub struct ShmBuffer {
    pub buffer: wl_buffer::WlBuffer,
    file: File,
    pub width: i32,
    pub height: i32,
}

impl ShmBuffer {
    /// The ARGB8888/XRGB8888 pixel at (x, y) as `0xAARRGGBB` (alpha masked for XRGB).
    pub fn pixel(&self, x: i32, y: i32) -> u32 {
        let mut px = [0u8; 4];
        let offset = u64::try_from((y * self.width + x) * 4).expect("offset");
        self.file.read_exact_at(&mut px, offset).expect("read pixel");
        u32::from_le_bytes(px)
    }
}

/// A mapped xdg toplevel.
pub struct Toplevel {
    pub surface: wl_surface::WlSurface,
    pub xdg_surface: xdg_surface::XdgSurface,
    /// The `xdg_toplevel` role object.
    pub role: xdg_toplevel::XdgToplevel,
    pub size: (i32, i32),
}

/// A mapped xdg popup.
pub struct Popup {
    pub surface: wl_surface::WlSurface,
    pub xdg_surface: xdg_surface::XdgSurface,
    /// The `xdg_popup` role object.
    pub role: xdg_popup::XdgPopup,
    pub size: (i32, i32),
}

// ─── The client ─────────────────────────────────────────────────────────

/// A connected raw Wayland client; dropping it closes the connection.
pub struct Client {
    pub conn: Connection,
    queue: EventQueue<ClientState>,
    pub qh: QueueHandle<ClientState>,
    pub state: ClientState,
}

impl Client {
    /// Connect to `runtime_dir/socket_name` and collect the globals.
    pub fn connect(socket: &Path) -> Result<Self, BoxError> {
        let stream = UnixStream::connect(socket)?;
        let conn = Connection::from_socket(stream)?;
        let queue = conn.new_event_queue();
        let qh = queue.handle();
        let registry = conn.display().get_registry(&qh, ());
        let mut client =
            Self { conn, queue, qh, state: ClientState { registry: Some(registry), ..Default::default() } };
        client.roundtrip()?;
        client.roundtrip()?; // seat capabilities → pointer/keyboard objects
        Ok(client)
    }

    /// The output whose global position is `position`.
    pub fn output_at(&self, position: (i32, i32)) -> Result<wl_output::WlOutput, BoxError> {
        self.state
            .outputs
            .iter()
            .find(|o| self.state.output_info.get(&o.id()).is_some_and(|i| i.position == position))
            .cloned()
            .ok_or_else(|| format!("no output at {position:?}: {:?}", self.state.output_info).into())
    }

    /// The advertised interface names.
    pub fn interfaces(&self) -> Vec<String> {
        self.state.globals.iter().map(|(_, i, _)| i.clone()).collect()
    }

    /// Bind a global by interface name.
    pub fn bind<I>(&mut self, interface: &str, max_version: u32) -> Result<I, BoxError>
    where
        I: Proxy + 'static,
        ClientState: Dispatch<I, ()>,
    {
        let (name, _, version) = self
            .state
            .globals
            .iter()
            .find(|(_, i, _)| i == interface)
            .cloned()
            .ok_or_else(|| format!("{interface} is not advertised"))?;
        let registry = self.state.registry.as_ref().expect("registry");
        Ok(registry.bind::<I, (), ClientState>(name, version.min(max_version), &self.qh, ()))
    }

    pub fn roundtrip(&mut self) -> Result<(), BoxError> {
        self.queue.roundtrip(&mut self.state)?;
        Ok(())
    }

    /// Read and dispatch whatever arrives within `timeout`.
    pub fn dispatch_for(&mut self, timeout: Duration) -> Result<(), BoxError> {
        let deadline = Instant::now() + timeout;
        loop {
            self.dispatch_once(deadline)?;
            if Instant::now() >= deadline {
                return Ok(());
            }
        }
    }

    /// Dispatch until `cond` holds, failing after [`WAIT`].
    pub fn dispatch_until(&mut self, what: &str, cond: impl Fn(&ClientState) -> bool) -> Result<(), BoxError> {
        let deadline = Instant::now() + WAIT;
        while !cond(&self.state) {
            if Instant::now() >= deadline {
                return Err(format!("timed out waiting for {what}").into());
            }
            self.dispatch_once(deadline)?;
        }
        Ok(())
    }

    fn dispatch_once(&mut self, deadline: Instant) -> Result<(), BoxError> {
        self.queue.flush()?;
        if self.queue.dispatch_pending(&mut self.state)? > 0 {
            return Ok(());
        }
        if let Some(guard) = self.queue.prepare_read() {
            let remaining = deadline.saturating_duration_since(Instant::now()).min(Duration::from_millis(100));
            let timeout = Timespec {
                tv_sec: remaining.as_secs().try_into().unwrap_or(0),
                tv_nsec: remaining.subsec_nanos().into(),
            };
            let readable = {
                let fd = guard.connection_fd();
                let mut fds = [PollFd::new(&fd, PollFlags::IN)];
                poll(&mut fds, Some(&timeout)).map(|n| n > 0)?
            };
            if readable {
                guard.read()?;
            }
        }
        self.queue.dispatch_pending(&mut self.state)?;
        Ok(())
    }

    /// The protocol error that ended this connection, if any.
    pub fn protocol_error(&self) -> Option<wayland_client::backend::protocol::ProtocolError> {
        self.conn.protocol_error()
    }

    /// Create a `wl_shm` buffer filled with `color` (`0xAARRGGBB`).
    pub fn buffer(
        &mut self,
        (width, height): (i32, i32),
        format: wl_shm::Format,
        color: u32,
    ) -> Result<ShmBuffer, BoxError> {
        let shm = self.state.shm.clone().ok_or("no wl_shm")?;
        let stride = width * 4;
        let size = stride * height;
        let file = tempfile::tempfile()?;
        let row: Vec<u8> = std::iter::repeat_n(color.to_le_bytes(), usize::try_from(width)?).flatten().collect();
        for y in 0..height {
            file.write_all_at(&row, u64::try_from(y * stride)?)?;
        }
        let pool = shm.create_pool(file.as_fd(), size, &self.qh, ());
        let buffer = pool.create_buffer(0, width, height, stride, format, &self.qh, ());
        pool.destroy();
        Ok(ShmBuffer { buffer, file, width, height })
    }

    /// Map a toplevel of `size` filled with `color`, and wait until the
    /// compositor has it.
    pub fn toplevel(&mut self, app_id: &str, size: (i32, i32), color: u32) -> Result<Toplevel, BoxError> {
        let compositor = self.state.compositor.clone().ok_or("no wl_compositor")?;
        let wm_base = self.state.wm_base.clone().ok_or("no xdg_wm_base")?;
        let surface = compositor.create_surface(&self.qh, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, &self.qh, surface.id());
        let toplevel = xdg_surface.get_toplevel(&self.qh, ());
        toplevel.set_app_id(app_id.into());
        toplevel.set_title(app_id.into());
        surface.commit();
        let id = surface.id();
        self.dispatch_until("toplevel configure", |s| s.configured.get(&id).copied().unwrap_or(false))?;
        let buffer = self.buffer(size, wl_shm::Format::Argb8888, color)?;
        surface.attach(Some(&buffer.buffer), 0, 0);
        surface.damage_buffer(0, 0, size.0, size.1);
        surface.commit();
        self.roundtrip()?;
        Ok(Toplevel { surface, xdg_surface, role: toplevel, size })
    }

    /// Map a popup of `size` at `offset` inside `parent`, filled with `color`.
    pub fn popup(
        &mut self,
        parent: &Toplevel,
        offset: (i32, i32),
        size: (i32, i32),
        color: u32,
    ) -> Result<Popup, BoxError> {
        let compositor = self.state.compositor.clone().ok_or("no wl_compositor")?;
        let wm_base = self.state.wm_base.clone().ok_or("no xdg_wm_base")?;
        let positioner = wm_base.create_positioner(&self.qh, ());
        positioner.set_size(size.0, size.1);
        // A 1×1 anchor rect, anchored and gravitating bottom-right: the
        // popup's top-left lands at offset.
        positioner.set_anchor_rect(offset.0 - 1, offset.1 - 1, 1, 1);
        positioner.set_anchor(xdg_positioner::Anchor::BottomRight);
        positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
        let surface = compositor.create_surface(&self.qh, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, &self.qh, surface.id());
        let popup = xdg_surface.get_popup(Some(&parent.xdg_surface), &positioner, &self.qh, ());
        surface.commit();
        let id = surface.id();
        self.dispatch_until("popup configure", |s| s.configured.get(&id).copied().unwrap_or(false))?;
        let buffer = self.buffer(size, wl_shm::Format::Argb8888, color)?;
        surface.attach(Some(&buffer.buffer), 0, 0);
        surface.damage_buffer(0, 0, size.0, size.1);
        surface.commit();
        self.roundtrip()?;
        Ok(Popup { surface, xdg_surface, role: popup, size })
    }

    // ─── Emulated input ──────────────────────────────────────────────────

    /// A virtual pointer on the seat, optionally bound to `output`.
    pub fn virtual_pointer(&mut self, output: Option<&wl_output::WlOutput>) -> Result<ZwlrVirtualPointerV1, BoxError> {
        let manager: ZwlrVirtualPointerManagerV1 = self.bind("zwlr_virtual_pointer_manager_v1", 2)?;
        let seat = self.state.seat.clone();
        let pointer = match output {
            Some(output) => manager.create_virtual_pointer_with_output(seat.as_ref(), Some(output), &self.qh, ()),
            None => manager.create_virtual_pointer(seat.as_ref(), &self.qh, ()),
        };
        self.roundtrip()?;
        Ok(pointer)
    }

    /// A virtual keyboard on the seat with `keymap` (XKB text) uploaded.
    pub fn virtual_keyboard(&mut self, keymap: &str) -> Result<ZwpVirtualKeyboardV1, BoxError> {
        let manager: ZwpVirtualKeyboardManagerV1 = self.bind("zwp_virtual_keyboard_manager_v1", 1)?;
        let seat = self.state.seat.clone().ok_or("no wl_seat")?;
        let keyboard = manager.create_virtual_keyboard(&seat, &self.qh, ());
        let mut bytes = keymap.as_bytes().to_vec();
        bytes.push(0);
        let file = tempfile::tempfile()?;
        file.write_all_at(&bytes, 0)?;
        keyboard.keymap(wl_keyboard::KeymapFormat::XkbV1.into(), file.as_fd(), u32::try_from(bytes.len())?);
        self.roundtrip()?;
        Ok(keyboard)
    }

    /// Bind `wp_pointer_warp_v1`.
    pub fn pointer_warp(&mut self) -> Result<WpPointerWarpV1, BoxError> {
        self.bind("wp_pointer_warp_v1", 1)
    }

    // ─── Screen capture ──────────────────────────────────────────────────

    /// The `ext_foreign_toplevel_list_v1` handles announced so far, after
    /// binding the list once.
    pub fn bind_toplevel_list(&mut self) -> Result<ExtForeignToplevelListV1, BoxError> {
        let list: ExtForeignToplevelListV1 = self.bind("ext_foreign_toplevel_list_v1", 1)?;
        self.roundtrip()?;
        self.roundtrip()?;
        Ok(list)
    }

    /// The foreign-toplevel handle of the window with `app_id`.
    pub fn handle_for(&mut self, app_id: &str) -> Result<ExtForeignToplevelHandleV1, BoxError> {
        self.dispatch_until("foreign-toplevel handle", |s| {
            s.handles.iter().any(|(_, i)| i.app_id.as_deref() == Some(app_id))
        })?;
        Ok(self.state.handles.iter().find(|(_, i)| i.app_id.as_deref() == Some(app_id)).expect("handle").0.clone())
    }

    /// A capture source for an output.
    pub fn output_source(&mut self, output: &wl_output::WlOutput) -> Result<ExtImageCaptureSourceV1, BoxError> {
        let manager: ExtOutputImageCaptureSourceManagerV1 =
            self.bind("ext_output_image_capture_source_manager_v1", 1)?;
        Ok(manager.create_source(output, &self.qh, ()))
    }

    /// A capture source for a foreign toplevel.
    pub fn toplevel_source(
        &mut self,
        handle: &ExtForeignToplevelHandleV1,
    ) -> Result<ExtImageCaptureSourceV1, BoxError> {
        let manager: ExtForeignToplevelImageCaptureSourceManagerV1 =
            self.bind("ext_foreign_toplevel_image_capture_source_manager_v1", 1)?;
        Ok(manager.create_source(handle, &self.qh, ()))
    }

    /// A capture session on `source`; waits for its constraints (`done`).
    pub fn session(
        &mut self,
        source: &ExtImageCaptureSourceV1,
        paint_cursors: bool,
    ) -> Result<ExtImageCopyCaptureSessionV1, BoxError> {
        let manager: ExtImageCopyCaptureManagerV1 = self.bind("ext_image_copy_capture_manager_v1", 1)?;
        let options = if paint_cursors {
            ext_image_copy_capture_manager_v1::Options::PaintCursors
        } else {
            ext_image_copy_capture_manager_v1::Options::empty()
        };
        let session = manager.create_session(source, options, &self.qh, ());
        let id = session.id();
        self.dispatch_until("session constraints", |s| s.sessions.get(&id).is_some_and(|i| i.done))?;
        Ok(session)
    }

    /// A cursor session on `source` for the seat pointer.
    pub fn cursor_session(
        &mut self,
        source: &ExtImageCaptureSourceV1,
    ) -> Result<ExtImageCopyCaptureCursorSessionV1, BoxError> {
        let manager: ExtImageCopyCaptureManagerV1 = self.bind("ext_image_copy_capture_manager_v1", 1)?;
        let pointer = self.state.pointer.clone().ok_or("no wl_pointer")?;
        let session = manager.create_pointer_cursor_session(source, &pointer, &self.qh, ());
        self.roundtrip()?;
        Ok(session)
    }

    /// The capture session of a cursor session (the cursor image); waits for
    /// its constraints.
    pub fn cursor_capture_session(
        &mut self,
        cursor: &ExtImageCopyCaptureCursorSessionV1,
    ) -> Result<ExtImageCopyCaptureSessionV1, BoxError> {
        let session = cursor.get_capture_session(&self.qh, ());
        let id = session.id();
        self.dispatch_until("cursor session constraints", |s| s.sessions.get(&id).is_some_and(|i| i.done))?;
        Ok(session)
    }

    /// The events a cursor session received.
    pub fn cursor_session_info(&self, cursor: &ExtImageCopyCaptureCursorSessionV1) -> CursorSessionInfo {
        self.state.cursor_sessions.get(&cursor.id()).cloned().unwrap_or_default()
    }

    /// The constraints a session announced.
    pub fn session_info(&self, session: &ExtImageCopyCaptureSessionV1) -> SessionInfo {
        self.state.sessions.get(&session.id()).cloned().unwrap_or_default()
    }

    /// Create a frame, attach `buffer`, capture, and wait for the outcome.
    pub fn capture(
        &mut self,
        session: &ExtImageCopyCaptureSessionV1,
        buffer: &ShmBuffer,
    ) -> Result<FrameInfo, BoxError> {
        let frame = session.create_frame(&self.qh, ());
        frame.attach_buffer(&buffer.buffer);
        frame.damage_buffer(0, 0, buffer.width, buffer.height);
        frame.capture();
        let id = frame.id();
        self.dispatch_until("frame outcome", |s| {
            s.frames.get(&id).is_some_and(|f| f.outcome != FrameOutcome::Pending)
        })?;
        let info = self.state.frames.get(&id).cloned().unwrap_or_default();
        frame.destroy();
        Ok(info)
    }
}

/// Read a whole file into a string (for keymaps written by the compositor).
pub fn read_to_string(mut file: File) -> String {
    let mut s = String::new();
    let _ = file.read_to_string(&mut s);
    s
}
