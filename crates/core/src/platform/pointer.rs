use crate::platform::PlatformError;
use crate::types::{Point, Rect, Size};
use std::time::Duration;

/// Mouse or pointing device buttons.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Default)]
pub enum PointerButton {
    #[default]
    Left,
    Right,
    Middle,
    Other(u16),
}

/// Scroll delta expressed in desktop coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollDelta {
    pub horizontal: f64,
    pub vertical: f64,
}

impl ScrollDelta {
    pub const fn new(horizontal: f64, vertical: f64) -> Self {
        Self { horizontal, vertical }
    }
}

impl Default for ScrollDelta {
    fn default() -> Self {
        ScrollDelta::new(0.0, -120.0)
    }
}

/// Determines how coordinates supplied in overrides are interpreted.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum PointOrigin {
    #[default]
    Desktop,
    Bounds(Rect),
    Absolute(Point),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerMotionMode {
    Direct,
    Linear,
    Bezier,
    Overshoot,
    Jitter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerAccelerationProfile {
    Constant,
    EaseIn,
    EaseOut,
    SmoothStep,
}

/// Trait that platform crates implement to drive pointer events.
pub trait PointerDevice: Send + Sync {
    fn position(&self) -> Result<Point, PlatformError>;
    fn move_to(&self, point: Point) -> Result<(), PlatformError>;
    fn press(&self, button: PointerButton) -> Result<(), PlatformError>;
    fn release(&self, button: PointerButton) -> Result<(), PlatformError>;
    fn scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError>;
    fn double_click_time(&self) -> Result<Option<Duration>, PlatformError> {
        Ok(None)
    }
    fn double_click_size(&self) -> Result<Option<Size>, PlatformError> {
        Ok(None)
    }
}

pub struct PointerRegistration {
    pub device: &'static dyn PointerDevice,
}

inventory::collect!(PointerRegistration);

pub fn pointer_devices() -> impl Iterator<Item = &'static dyn PointerDevice> {
    inventory::iter::<PointerRegistration>.into_iter().map(|entry| entry.device)
}

#[macro_export]
macro_rules! register_pointer_device {
    ($device:expr) => {
        inventory::submit! {
            $crate::platform::PointerRegistration { device: $device }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{PlatformError, PlatformErrorKind};
    use crate::types::{Point, Size};
    use rstest::rstest;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct StubPointerDevice {
        move_calls: AtomicUsize,
    }

    impl StubPointerDevice {
        const fn new() -> Self {
            Self { move_calls: AtomicUsize::new(0) }
        }
    }

    impl PointerDevice for StubPointerDevice {
        fn position(&self) -> Result<Point, PlatformError> {
            Ok(Point::new(0.0, 0.0))
        }

        fn move_to(&self, _point: Point) -> Result<(), PlatformError> {
            self.move_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn press(&self, _button: PointerButton) -> Result<(), PlatformError> {
            Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "press"))
        }

        fn release(&self, _button: PointerButton) -> Result<(), PlatformError> {
            Ok(())
        }

        fn scroll(&self, _delta: ScrollDelta) -> Result<(), PlatformError> {
            Ok(())
        }

        fn double_click_time(&self) -> Result<Option<Duration>, PlatformError> {
            Ok(Some(Duration::from_millis(300)))
        }

        fn double_click_size(&self) -> Result<Option<Size>, PlatformError> {
            Ok(Some(Size::new(4.0, 4.0)))
        }
    }

    static STUB_POINTER: StubPointerDevice = StubPointerDevice::new();

    register_pointer_device!(&STUB_POINTER);

    #[rstest]
    fn pointer_registration_exposes_device() {
        let devices: Vec<_> = pointer_devices().collect();
        assert!(devices.iter().any(|device| device.position().is_ok()));
    }
}
