//! A minimal X11 client for the Xwayland integration tests: it opens plain
//! windows (optionally with GTK's `_GTK_FRAME_EXTENTS` shadow) and reads back
//! where the X server believes they are.
#![allow(dead_code)]

use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, CreateWindowAux, PropMode, Window, WindowClass};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

pub struct X11 {
    pub conn: RustConnection,
    pub root: Window,
}

impl X11 {
    pub fn connect(display: &str) -> Self {
        let (conn, screen) =
            x11rb::connect(Some(display)).unwrap_or_else(|err| panic!("cannot connect to {display}: {err}"));
        let root = conn.setup().roots[screen].root;
        Self { conn, root }
    }

    pub fn atom(&self, name: &str) -> u32 {
        self.conn.intern_atom(false, name.as_bytes()).expect("intern").reply().expect("atom").atom
    }

    /// Create a window filled with `color`; `extents` become its
    /// `_GTK_FRAME_EXTENTS` (left, right, top, bottom).
    #[allow(clippy::too_many_arguments)]
    pub fn window(
        &self,
        title: &str,
        x: i16,
        y: i16,
        w: u16,
        h: u16,
        color: u32,
        extents: u32,
        override_redirect: bool,
    ) -> Window {
        let win = self.conn.generate_id().expect("id");
        let aux = CreateWindowAux::new().background_pixel(color).override_redirect(u32::from(override_redirect));
        self.conn
            .create_window(0, win, self.root, x, y, w, h, 0, WindowClass::INPUT_OUTPUT, 0, &aux)
            .expect("create window");
        self.conn
            .change_property8(PropMode::REPLACE, win, AtomEnum::WM_NAME, AtomEnum::STRING, title.as_bytes())
            .expect("title");
        let extents_atom = self.atom("_GTK_FRAME_EXTENTS");
        self.conn
            .change_property32(PropMode::REPLACE, win, extents_atom, AtomEnum::CARDINAL, &[extents; 4])
            .expect("frame extents");
        if !override_redirect {
            // Client-side decorations, as GTK announces them: no WM frame.
            let motif = self.atom("_MOTIF_WM_HINTS");
            self.conn.change_property32(PropMode::REPLACE, win, motif, motif, &[2, 0, 0, 0, 0]).expect("motif hints");
        }
        self.conn.map_window(win).expect("map");
        self.conn.flush().expect("flush");
        win
    }

    /// The window's origin in root coordinates and its size, as the X server sees them.
    pub fn rect(&self, win: Window) -> (i32, i32, u32, u32) {
        let geo = self.conn.get_geometry(win).expect("geometry").reply().expect("geometry reply");
        let origin =
            self.conn.translate_coordinates(win, self.root, 0, 0).expect("translate").reply().expect("translate reply");
        (i32::from(origin.dst_x), i32::from(origin.dst_y), u32::from(geo.width), u32::from(geo.height))
    }

    /// Wait until `cond` holds for the window's rect, failing after 10 s.
    pub fn wait_rect(
        &self,
        win: Window,
        what: &str,
        cond: impl Fn((i32, i32, u32, u32)) -> bool,
    ) -> (i32, i32, u32, u32) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let rect = self.rect(win);
            if cond(rect) {
                return rect;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}; last rect {rect:?}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
