//! Minimal raw Wayland client used by the compositor's integration tests.
//!
//! The egui test app cannot create `xdg_popups` (its menus render in-window),
//! so the popup tests speak the Wayland protocol directly: map a toplevel, then
//! an `xdg_popup` anchored inside it, and keep the connection alive while the
//! test queries the control socket.
//!
//! Shared by `ipc_tests.rs` and `pidns_tests.rs` through `#[path]`; each of them
//! uses a different part of it, hence the blanket `dead_code` allowance.
#![allow(dead_code)]

use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use wayland_client::protocol::{wl_buffer, wl_compositor, wl_registry, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base};

pub const TOPLEVEL_SIZE: (i32, i32) = (300, 200);
/// Anchor rect origin inside the toplevel; with a 1×1 rect and
/// bottom-right anchor/gravity the popup's top-left lands at +1/+1 of it.
pub const POPUP_ANCHOR: (i32, i32) = (30, 40);
pub const POPUP_OFFSET: (i32, i32) = (POPUP_ANCHOR.0 + 1, POPUP_ANCHOR.1 + 1);
pub const POPUP_SIZE: (i32, i32) = (100, 80);

#[derive(Clone, Copy)]
pub enum SurfaceRole {
    Toplevel,
    Popup,
}

#[derive(Default)]
pub struct State {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    toplevel_configured: bool,
    popup_configured: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
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
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(4), qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind(name, 1, qh, ()));
                }
                "xdg_wm_base" => {
                    state.wm_base = Some(registry.bind(name, version.min(2), qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
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

impl Dispatch<xdg_surface::XdgSurface, SurfaceRole> for State {
    fn event(
        state: &mut Self,
        surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        role: &SurfaceRole,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
            match role {
                SurfaceRole::Toplevel => state.toplevel_configured = true,
                SurfaceRole::Popup => state.popup_configured = true,
            }
        }
    }
}

wayland_client::delegate_noop!(State: ignore wl_compositor::WlCompositor);
wayland_client::delegate_noop!(State: ignore wl_surface::WlSurface);
wayland_client::delegate_noop!(State: ignore wl_shm::WlShm);
wayland_client::delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
wayland_client::delegate_noop!(State: ignore wl_buffer::WlBuffer);
wayland_client::delegate_noop!(State: ignore xdg_positioner::XdgPositioner);
wayland_client::delegate_noop!(State: ignore xdg_toplevel::XdgToplevel);
wayland_client::delegate_noop!(State: ignore xdg_popup::XdgPopup);

/// A connected client holding a mapped toplevel and one mapped popup.
/// Dropping it closes the connection (and thereby dismisses everything).
pub struct Fixture {
    state: State,
    queue: EventQueue<State>,
    popup: xdg_popup::XdgPopup,
    popup_xdg_surface: xdg_surface::XdgSurface,
    popup_surface: wl_surface::WlSurface,
}

impl Fixture {
    /// Destroy the popup and flush, so the compositor drops it from its
    /// popup tree while toplevel and connection stay alive.
    pub fn dismiss_popup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.popup.destroy();
        self.popup_xdg_surface.destroy();
        self.popup_surface.destroy();
        self.queue.roundtrip(&mut self.state)?;
        Ok(())
    }
}

fn create_buffer(
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<State>,
    (width, height): (i32, i32),
) -> Result<wl_buffer::WlBuffer, Box<dyn std::error::Error>> {
    let stride = width * 4;
    let size = stride * height;
    let file = tempfile::tempfile()?;
    file.set_len(u64::try_from(size)?)?;
    let pool = shm.create_pool(file.as_fd(), size, qh, ());
    Ok(pool.create_buffer(0, width, height, stride, wl_shm::Format::Xrgb8888, qh, ()))
}

fn dispatch_until(
    queue: &mut EventQueue<State>,
    state: &mut State,
    what: &str,
    cond: impl Fn(&State) -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !cond(state) {
        if Instant::now() > deadline {
            return Err(format!("timed out waiting for {what}").into());
        }
        queue.blocking_dispatch(state)?;
    }
    Ok(())
}

/// Connect to the compositor socket, map a [`TOPLEVEL_SIZE`] toplevel and
/// a [`POPUP_SIZE`] popup at [`POPUP_OFFSET`] inside it.
pub fn open_toplevel_with_popup(
    runtime_dir: &Path,
    socket_name: &str,
    app_id: &str,
) -> Result<Fixture, Box<dyn std::error::Error>> {
    let stream = UnixStream::connect(runtime_dir.join(socket_name))?;
    let conn = Connection::from_socket(stream)?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let display = conn.display();
    let _registry = display.get_registry(&qh, ());

    let mut state = State::default();
    queue.roundtrip(&mut state)?;
    let (Some(compositor), Some(shm), Some(wm_base)) =
        (state.compositor.clone(), state.shm.clone(), state.wm_base.clone())
    else {
        return Err("compositor did not advertise wl_compositor/wl_shm/xdg_wm_base".into());
    };

    // Map the toplevel: initial commit → configure/ack → buffer commit.
    let surface = compositor.create_surface(&qh, ());
    let xdg = wm_base.get_xdg_surface(&surface, &qh, SurfaceRole::Toplevel);
    let toplevel = xdg.get_toplevel(&qh, ());
    toplevel.set_app_id(app_id.into());
    toplevel.set_title("Popup Fixture".into());
    surface.commit();
    dispatch_until(&mut queue, &mut state, "toplevel configure", |s| s.toplevel_configured)?;
    surface.attach(Some(&create_buffer(&shm, &qh, TOPLEVEL_SIZE)?), 0, 0);
    surface.commit();
    queue.roundtrip(&mut state)?;

    // Map the popup the same way, anchored bottom-right of a 1×1 rect.
    let positioner = wm_base.create_positioner(&qh, ());
    positioner.set_size(POPUP_SIZE.0, POPUP_SIZE.1);
    positioner.set_anchor_rect(POPUP_ANCHOR.0, POPUP_ANCHOR.1, 1, 1);
    positioner.set_anchor(xdg_positioner::Anchor::BottomRight);
    positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
    let popup_surface = compositor.create_surface(&qh, ());
    let popup_xdg = wm_base.get_xdg_surface(&popup_surface, &qh, SurfaceRole::Popup);
    let popup = popup_xdg.get_popup(Some(&xdg), &positioner, &qh, ());
    popup_surface.commit();
    dispatch_until(&mut queue, &mut state, "popup configure", |s| s.popup_configured)?;
    popup_surface.attach(Some(&create_buffer(&shm, &qh, POPUP_SIZE)?), 0, 0);
    popup_surface.commit();
    queue.roundtrip(&mut state)?;

    Ok(Fixture { state, queue, popup, popup_xdg_surface: popup_xdg, popup_surface })
}
