use crate::x11util::{X11Handle, connection, root_window_from};
use platynui_core::platform::{PlatformError, PlatformErrorKind, PointerButton, PointerDevice, ScrollDelta};
use platynui_core::register_pointer_device;
use platynui_core::types::Point;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::xtest;

pub struct LinuxPointerDevice;

impl PointerDevice for LinuxPointerDevice {
    fn position(&self) -> Result<Point, PlatformError> {
        with_conn(|guard| {
            let root = root_window_from(guard);
            let reply = guard.conn.query_pointer(root).map_err(to_pf)?.reply().map_err(to_pf)?;
            Ok(Point::new(f64::from(reply.root_x), f64::from(reply.root_y)))
        })
    }

    fn move_to(&self, point: Point) -> Result<(), PlatformError> {
        with_conn(|guard| {
            let x = point.x().round().clamp(i16::MIN as f64, i16::MAX as f64) as i16;
            let y = point.y().round().clamp(i16::MIN as f64, i16::MAX as f64) as i16;
            // Use XTest motion (type 6 = MotionNotify) so injected moves and button events share the same path.
            let root = root_window_from(guard);
            xtest::fake_input(&guard.conn, 6, 0, 0, root, x, y, 0).map_err(to_pf)?;
            guard.conn.flush().map_err(to_pf)
        })
    }

    fn press(&self, button: PointerButton) -> Result<(), PlatformError> {
        send_button(button, true)
    }

    fn release(&self, button: PointerButton) -> Result<(), PlatformError> {
        send_button(button, false)
    }

    fn scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        let steps_v = steps(delta.vertical);
        let steps_h = steps(delta.horizontal);
        with_conn(|guard| {
            for _ in 0..steps_v.abs() {
                send_raw_button(guard, if steps_v > 0 { 4 } else { 5 }, true)?;
                send_raw_button(guard, if steps_v > 0 { 4 } else { 5 }, false)?;
            }
            for _ in 0..steps_h.abs() {
                send_raw_button(guard, if steps_h > 0 { 6 } else { 7 }, true)?;
                send_raw_button(guard, if steps_h > 0 { 6 } else { 7 }, false)?;
            }
            guard.conn.flush().map_err(to_pf)
        })
    }
}

fn steps(v: f64) -> i32 {
    if v == 0.0 { 0 } else { (v / 120.0).round() as i32 }
}

fn send_button(button: PointerButton, press: bool) -> Result<(), PlatformError> {
    let code = match button {
        PointerButton::Left => 1,
        PointerButton::Middle => 2,
        PointerButton::Right => 3,
        PointerButton::Other(1) => 8,
        PointerButton::Other(2) => 9,
        PointerButton::Other(n) => {
            return Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                format!("unsupported X button code Other({n})"),
            ));
        }
    };
    with_conn(|guard| send_raw_button(guard, code, press))
}

fn send_raw_button(guard: &X11Handle, code: u8, press: bool) -> Result<(), PlatformError> {
    let type_code: u8 = if press { 4 } else { 5 }; // 4=ButtonPress, 5=ButtonRelease
    let root = root_window_from(guard);
    xtest::fake_input(&guard.conn, type_code, code, 0, root, 0, 0, 0).map_err(to_pf)?;
    guard.conn.flush().map_err(to_pf)
}

fn with_conn<T>(f: impl FnOnce(&X11Handle) -> Result<T, PlatformError>) -> Result<T, PlatformError> {
    let guard = connection()?;
    f(&guard)
}

fn to_pf<E: std::fmt::Display>(e: E) -> PlatformError {
    // Pointer failures after a successful connect are operational.
    PlatformError::new(PlatformErrorKind::OperationFailed, format!("x11: {e}"))
}

static DEVICE: LinuxPointerDevice = LinuxPointerDevice;

register_pointer_device!(&DEVICE);
