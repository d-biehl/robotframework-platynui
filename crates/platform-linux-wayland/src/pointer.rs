use platynui_core::platform::{PlatformError, PlatformErrorKind, PointerButton, PointerDevice, ScrollDelta};
use platynui_core::register_pointer_device;
use platynui_core::types::Point;
use reis::ei;
use reis::event::DeviceCapability;

pub struct WaylandPointerDevice;

impl PointerDevice for WaylandPointerDevice {
    fn position(&self) -> Result<Point, PlatformError> {
        // Wayland does not expose global pointer position to clients.
        tracing::debug!("pointer position unavailable under Wayland — returning (0, 0)");
        Ok(Point::new(0.0, 0.0))
    }

    fn move_to(&self, point: Point) -> Result<(), PlatformError> {
        with_ei_device(DeviceCapability::PointerAbsolute, |connection, device| {
            let abs = device
                .interface::<ei::PointerAbsolute>()
                .ok_or_else(|| to_pf("device missing PointerAbsolute interface"))?;

            let serial = connection.serial();
            let device_proxy = device.device();
            device_proxy.start_emulating(serial, 1);
            #[expect(clippy::cast_possible_truncation)]
            abs.motion_absolute(point.x() as f32, point.y() as f32);
            device_proxy.frame(serial, crate::eis::timestamp_us());
            device_proxy.stop_emulating(serial);
            connection.flush().map_err(|e| to_pf(format!("flush: {e}")))?;
            Ok(())
        })
    }

    fn press(&self, button: PointerButton) -> Result<(), PlatformError> {
        let code = button_to_evdev(button)?;
        with_ei_device(DeviceCapability::Button, |connection, device| {
            let btn = device.interface::<ei::Button>().ok_or_else(|| to_pf("device missing Button interface"))?;

            let serial = connection.serial();
            let device_proxy = device.device();
            device_proxy.start_emulating(serial, 1);
            btn.button(code, ei::button::ButtonState::Press);
            device_proxy.frame(serial, crate::eis::timestamp_us());
            device_proxy.stop_emulating(serial);
            connection.flush().map_err(|e| to_pf(format!("flush: {e}")))?;
            Ok(())
        })
    }

    fn release(&self, button: PointerButton) -> Result<(), PlatformError> {
        let code = button_to_evdev(button)?;
        with_ei_device(DeviceCapability::Button, |connection, device| {
            let btn = device.interface::<ei::Button>().ok_or_else(|| to_pf("device missing Button interface"))?;

            let serial = connection.serial();
            let device_proxy = device.device();
            device_proxy.start_emulating(serial, 1);
            btn.button(code, ei::button::ButtonState::Released);
            device_proxy.frame(serial, crate::eis::timestamp_us());
            device_proxy.stop_emulating(serial);
            connection.flush().map_err(|e| to_pf(format!("flush: {e}")))?;
            Ok(())
        })
    }

    fn scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        with_ei_device(DeviceCapability::Scroll, |connection, device| {
            let scroll = device.interface::<ei::Scroll>().ok_or_else(|| to_pf("device missing Scroll interface"))?;

            let serial = connection.serial();
            let device_proxy = device.device();
            device_proxy.start_emulating(serial, 1);
            #[expect(clippy::cast_possible_truncation)]
            scroll.scroll(delta.horizontal as f32, delta.vertical as f32);
            device_proxy.frame(serial, crate::eis::timestamp_us());
            device_proxy.stop_emulating(serial);
            connection.flush().map_err(|e| to_pf(format!("flush: {e}")))?;
            Ok(())
        })
    }
}

// ---------------------------------------------------------------------------
// EIS device access
// ---------------------------------------------------------------------------

/// Execute an operation on an EIS device with the required capability.
fn with_ei_device(
    required: DeviceCapability,
    action: impl FnOnce(&reis::event::Connection, &reis::event::Device) -> Result<(), PlatformError>,
) -> Result<(), PlatformError> {
    // Verify Wayland connection is alive before attempting EIS.
    let guard = crate::wayland_util::connection()?;
    drop(guard);

    let mut session = crate::eis::establish_session("platynui-wayland-pointer")
        .map_err(|e| to_pf(format!("EIS session: {e}")))?;

    let device = crate::eis::find_device(&mut session, required)
        .map_err(|e| to_pf(format!("EIS device: {e}")))?;

    action(&session.connection, &device)
}

/// Map `PointerButton` to Linux evdev button codes.
fn button_to_evdev(button: PointerButton) -> Result<u32, PlatformError> {
    // BTN_LEFT = 0x110, BTN_RIGHT = 0x111, BTN_MIDDLE = 0x112
    // BTN_SIDE = 0x113, BTN_EXTRA = 0x114
    match button {
        PointerButton::Left => Ok(0x110),
        PointerButton::Right => Ok(0x111),
        PointerButton::Middle => Ok(0x112),
        PointerButton::Other(1) => Ok(0x113),
        PointerButton::Other(2) => Ok(0x114),
        PointerButton::Other(n) => Err(PlatformError::new(
            PlatformErrorKind::CapabilityUnavailable,
            format!("unsupported pointer button Other({n})"),
        )),
    }
}

fn to_pf<E: std::fmt::Display>(e: E) -> PlatformError {
    PlatformError::new(PlatformErrorKind::OperationFailed, format!("wayland pointer: {e}"))
}

static DEVICE: WaylandPointerDevice = WaylandPointerDevice;

register_pointer_device!(&DEVICE);
