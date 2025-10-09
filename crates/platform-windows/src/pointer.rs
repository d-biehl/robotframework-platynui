use std::mem::size_of;
use std::sync::OnceLock;
use std::time::Duration;

use platynui_core::platform::{
    PlatformError, PlatformErrorKind, PointerButton, PointerDevice, ScrollDelta, register_pointer_device,
};
use platynui_core::types::{Point, Size};
use windows::Win32::Foundation::GetLastError;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetDoubleClickTime, INPUT, INPUT_0, INPUT_MOUSE, MOUSE_EVENT_FLAGS, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_WHEEL, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, SendInput,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CURSORINFO, GetCursorInfo, GetSystemMetrics, SM_CXDOUBLECLK, SM_CYDOUBLECLK, SetCursorPos, XBUTTON1, XBUTTON2,
};
use windows::core::Error;

struct WindowsPointerDevice;

impl PointerDevice for WindowsPointerDevice {
    fn position(&self) -> Result<Point, PlatformError> {
        let mut info = CURSORINFO { cbSize: size_of::<CURSORINFO>() as u32, ..Default::default() };
        unsafe { GetCursorInfo(&mut info) }.map_err(|err| win_error("GetCursorInfo", err))?;
        Ok(Point::new(info.ptScreenPos.x as f64, info.ptScreenPos.y as f64))
    }

    fn move_to(&self, point: Point) -> Result<(), PlatformError> {
        let x = point.x().round().clamp(i32::MIN as f64, i32::MAX as f64) as i32;
        let y = point.y().round().clamp(i32::MIN as f64, i32::MAX as f64) as i32;

        unsafe { SetCursorPos(x, y) }.map_err(|err| win_error("SetCursorPos", err))
    }

    fn press(&self, button: PointerButton) -> Result<(), PlatformError> {
        let (flags, data) = press_flags(button)?;
        send_mouse_input(flags, data, 0, 0)
    }

    fn release(&self, button: PointerButton) -> Result<(), PlatformError> {
        let (flags, data) = release_flags(button)?;
        send_mouse_input(flags, data, 0, 0)
    }

    fn scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        if delta.vertical != 0.0 {
            let data = scroll_data(delta.vertical);
            send_mouse_input(MOUSEEVENTF_WHEEL, data, 0, 0)?;
        }
        if delta.horizontal != 0.0 {
            let data = scroll_data(delta.horizontal);
            send_mouse_input(MOUSEEVENTF_HWHEEL, data, 0, 0)?;
        }
        Ok(())
    }

    fn double_click_time(&self) -> Result<Option<Duration>, PlatformError> {
        let value = unsafe { GetDoubleClickTime() };
        if value == 0 { Err(last_error("GetDoubleClickTime")) } else { Ok(Some(Duration::from_millis(value as u64))) }
    }

    fn double_click_size(&self) -> Result<Option<Size>, PlatformError> {
        let width = unsafe { GetSystemMetrics(SM_CXDOUBLECLK) };
        let height = unsafe { GetSystemMetrics(SM_CYDOUBLECLK) };
        if width <= 0 || height <= 0 {
            Err(last_error("GetSystemMetrics(SM_C*DOUBLECLK)"))
        } else {
            Ok(Some(Size::new(width as f64, height as f64)))
        }
    }
}

fn send_mouse_input(flags: MOUSE_EVENT_FLAGS, data: u32, dx: i32, dy: i32) -> Result<(), PlatformError> {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy, mouseData: data, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    };

    let sent = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
    if sent == 0 { Err(last_error("SendInput")) } else { Ok(()) }
}

fn press_flags(button: PointerButton) -> Result<(MOUSE_EVENT_FLAGS, u32), PlatformError> {
    match button {
        PointerButton::Left => Ok((MOUSEEVENTF_LEFTDOWN, 0)),
        PointerButton::Right => Ok((MOUSEEVENTF_RIGHTDOWN, 0)),
        PointerButton::Middle => Ok((MOUSEEVENTF_MIDDLEDOWN, 0)),
        PointerButton::Other(1) => Ok((MOUSEEVENTF_XDOWN, u32::from(XBUTTON1))),
        PointerButton::Other(2) => Ok((MOUSEEVENTF_XDOWN, u32::from(XBUTTON2))),
        PointerButton::Other(code) => Err(PlatformError::new(
            PlatformErrorKind::CapabilityUnavailable,
            format!("unsupported XButton code {code}"),
        )),
    }
}

fn release_flags(button: PointerButton) -> Result<(MOUSE_EVENT_FLAGS, u32), PlatformError> {
    match button {
        PointerButton::Left => Ok((MOUSEEVENTF_LEFTUP, 0)),
        PointerButton::Right => Ok((MOUSEEVENTF_RIGHTUP, 0)),
        PointerButton::Middle => Ok((MOUSEEVENTF_MIDDLEUP, 0)),
        PointerButton::Other(1) => Ok((MOUSEEVENTF_XUP, u32::from(XBUTTON1))),
        PointerButton::Other(2) => Ok((MOUSEEVENTF_XUP, u32::from(XBUTTON2))),
        PointerButton::Other(code) => Err(PlatformError::new(
            PlatformErrorKind::CapabilityUnavailable,
            format!("unsupported XButton code {code}"),
        )),
    }
}

fn scroll_data(delta: f64) -> u32 {
    let value = delta.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32;
    value as u32
}

fn last_error(context: &str) -> PlatformError {
    let code = unsafe { GetLastError() };
    PlatformError::new(PlatformErrorKind::CapabilityUnavailable, format!("{context} failed: {code:?}"))
}

fn win_error(context: &str, err: Error) -> PlatformError {
    PlatformError::new(PlatformErrorKind::CapabilityUnavailable, format!("{context} failed: {err:?}"))
}

static DEVICE: OnceLock<WindowsPointerDevice> = OnceLock::new();

fn device_instance() -> &'static WindowsPointerDevice {
    DEVICE.get_or_init(|| WindowsPointerDevice)
}

struct RegisteredPointer;

static REGISTERED_POINTER: RegisteredPointer = RegisteredPointer;

register_pointer_device!(&REGISTERED_POINTER);

impl PointerDevice for RegisteredPointer {
    fn position(&self) -> Result<Point, PlatformError> {
        device_instance().position()
    }

    fn move_to(&self, point: Point) -> Result<(), PlatformError> {
        device_instance().move_to(point)
    }

    fn press(&self, button: PointerButton) -> Result<(), PlatformError> {
        device_instance().press(button)
    }

    fn release(&self, button: PointerButton) -> Result<(), PlatformError> {
        device_instance().release(button)
    }

    fn scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        device_instance().scroll(delta)
    }

    fn double_click_time(&self) -> Result<Option<Duration>, PlatformError> {
        device_instance().double_click_time()
    }

    fn double_click_size(&self) -> Result<Option<Size>, PlatformError> {
        device_instance().double_click_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_cast_preserves_sign() {
        assert_eq!(scroll_data(120.0) as i32, 120);
        assert_eq!(scroll_data(-240.0) as i32, -240);
    }

    #[test]
    fn press_release_flag_mapping() {
        assert_eq!(press_flags(PointerButton::Left).unwrap().0, MOUSEEVENTF_LEFTDOWN);
        assert_eq!(release_flags(PointerButton::Right).unwrap().0, MOUSEEVENTF_RIGHTUP);
        assert!(press_flags(PointerButton::Other(42)).is_err());
    }
}
