use crate::x11util::X11Connection;
use platynui_core::platform::{PlatformError, PointerButton, PointerDevice, ScrollDelta};
use platynui_core::types::Point;
use std::sync::Arc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::xtest;

pub struct LinuxPointerDevice {
    conn: Arc<X11Connection>,
}

impl LinuxPointerDevice {
    pub fn new(conn: Arc<X11Connection>) -> Self {
        Self { conn }
    }
}

impl PointerDevice for LinuxPointerDevice {
    fn position(&self) -> Result<Point, PlatformError> {
        let reply = self.conn.conn.query_pointer(self.conn.root).map_err(to_pf)?.reply().map_err(to_pf)?;
        Ok(Point::new(f64::from(reply.root_x), f64::from(reply.root_y)))
    }

    fn move_to(&self, point: Point) -> Result<(), PlatformError> {
        let x = point.x().round().clamp(i16::MIN as f64, i16::MAX as f64) as i16;
        let y = point.y().round().clamp(i16::MIN as f64, i16::MAX as f64) as i16;
        // Use XTest motion (type 6 = MotionNotify) so injected moves and button events share the same path.
        xtest::fake_input(&self.conn.conn, 6, 0, 0, self.conn.root, x, y, 0).map_err(to_pf)?;
        self.conn.conn.flush().map_err(to_pf)
    }

    fn press(&self, button: PointerButton) -> Result<(), PlatformError> {
        send_button(&self.conn, button, true)
    }

    fn release(&self, button: PointerButton) -> Result<(), PlatformError> {
        send_button(&self.conn, button, false)
    }

    fn scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        let steps_v = steps(delta.vertical);
        let steps_h = steps(delta.horizontal);
        for _ in 0..steps_v.abs() {
            send_raw_button(&self.conn, if steps_v > 0 { 4 } else { 5 }, true)?;
            send_raw_button(&self.conn, if steps_v > 0 { 4 } else { 5 }, false)?;
        }
        for _ in 0..steps_h.abs() {
            send_raw_button(&self.conn, if steps_h > 0 { 6 } else { 7 }, true)?;
            send_raw_button(&self.conn, if steps_h > 0 { 6 } else { 7 }, false)?;
        }
        self.conn.conn.flush().map_err(to_pf)
    }
}

fn steps(v: f64) -> i32 {
    if v == 0.0 { 0 } else { (v / 120.0).round() as i32 }
}

fn send_button(conn: &X11Connection, button: PointerButton, press: bool) -> Result<(), PlatformError> {
    let code = match button {
        PointerButton::Left => 1,
        PointerButton::Middle => 2,
        PointerButton::Right => 3,
        PointerButton::Other(1) => 8,
        PointerButton::Other(2) => 9,
        PointerButton::Other(n) => {
            return Err(PlatformError::CapabilityUnavailable {
                capability: "X11 pointer button",
                details: Some(format!("unsupported Other({n})")),
            });
        }
    };
    send_raw_button(conn, code, press)
}

fn send_raw_button(conn: &X11Connection, code: u8, press: bool) -> Result<(), PlatformError> {
    let type_code: u8 = if press { 4 } else { 5 }; // 4=ButtonPress, 5=ButtonRelease
    xtest::fake_input(&conn.conn, type_code, code, 0, conn.root, 0, 0, 0).map_err(to_pf)?;
    conn.conn.flush().map_err(to_pf)
}

fn to_pf<E: std::fmt::Display>(e: E) -> PlatformError {
    // Pointer failures after a successful connect are operational.
    PlatformError::OperationFailed { operation: "x11 pointer", details: Some(e.to_string()) }
}
