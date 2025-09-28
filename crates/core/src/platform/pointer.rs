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

/// Global runtime controlled pointer settings.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerSettings {
    pub double_click_time: Duration,
    pub double_click_size: Size,
    pub default_button: PointerButton,
    pub press_release_delay: Duration,
    pub after_input_delay: Duration,
    pub after_click_delay: Duration,
    pub before_next_click_delay: Duration,
    pub multi_click_delay: Duration,
    pub multi_click_threshold: Duration,
    pub after_move_delay: Duration,
    pub ensure_move_timeout: Duration,
    pub ensure_move_threshold: f64,
    pub scroll_step: ScrollDelta,
    pub scroll_delay: Duration,
    pub move_time_per_pixel: Duration,
}

impl Default for PointerSettings {
    fn default() -> Self {
        Self {
            double_click_time: Duration::from_millis(500),
            double_click_size: Size::new(4.0, 4.0),
            default_button: PointerButton::Left,
            press_release_delay: Duration::from_millis(50),
            after_input_delay: Duration::from_millis(35),
            after_click_delay: Duration::from_millis(80),
            before_next_click_delay: Duration::from_millis(120),
            multi_click_delay: Duration::from_millis(500),
            multi_click_threshold: Duration::from_millis(900),
            after_move_delay: Duration::from_millis(40),
            ensure_move_timeout: Duration::from_millis(250),
            ensure_move_threshold: 2.0,
            scroll_step: ScrollDelta::new(0.0, -120.0),
            scroll_delay: Duration::from_millis(40),
            move_time_per_pixel: Duration::from_micros(800),
        }
    }
}

/// Motion profile applied by the runtime engine.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerProfile {
    pub mode: PointerMotionMode,
    pub steps_per_pixel: f64,
    pub max_move_duration: Duration,
    pub speed_factor: f64,
    pub acceleration_profile: PointerAccelerationProfile,
    pub overshoot_ratio: f64,
    pub overshoot_settle_steps: u32,
    pub curve_amplitude: f64,
    pub jitter_amplitude: f64,
    pub after_move_delay: Duration,
    pub after_input_delay: Duration,
    pub press_release_delay: Duration,
    pub after_click_delay: Duration,
    pub before_next_click_delay: Duration,
    pub multi_click_delay: Duration,
    pub ensure_move_position: bool,
    pub ensure_move_threshold: f64,
    pub ensure_move_timeout: Duration,
    pub scroll_step: ScrollDelta,
    pub scroll_delay: Duration,
    pub move_time_per_pixel: Duration,
}

impl PointerProfile {
    pub fn named_default(settings: &PointerSettings) -> Self {
        Self {
            mode: PointerMotionMode::Linear,
            steps_per_pixel: 1.5,
            max_move_duration: Duration::from_millis(600),
            speed_factor: 1.0,
            acceleration_profile: PointerAccelerationProfile::SmoothStep,
            overshoot_ratio: 0.08,
            overshoot_settle_steps: 3,
            curve_amplitude: 4.0,
            jitter_amplitude: 1.5,
            after_move_delay: settings.after_move_delay,
            after_input_delay: settings.after_input_delay,
            press_release_delay: settings.press_release_delay,
            after_click_delay: settings.after_click_delay,
            before_next_click_delay: settings.before_next_click_delay,
            multi_click_delay: settings.multi_click_delay,
            ensure_move_position: true,
            ensure_move_threshold: settings.ensure_move_threshold,
            ensure_move_timeout: settings.ensure_move_timeout,
            scroll_step: settings.scroll_step,
            scroll_delay: settings.scroll_delay,
            move_time_per_pixel: settings.move_time_per_pixel,
        }
    }

    pub fn with_mode(mut self, mode: PointerMotionMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_speed_factor(mut self, speed: f64) -> Self {
        self.speed_factor = speed;
        self
    }

    pub fn with_curve_amplitude(mut self, amplitude: f64) -> Self {
        self.curve_amplitude = amplitude;
        self
    }
}

impl Default for PointerProfile {
    fn default() -> Self {
        PointerProfile::named_default(&PointerSettings::default())
    }
}

/// Per-call overrides for pointer actions.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PointerOverrides {
    pub origin: Option<PointOrigin>,
    pub profile: Option<PointerProfile>,
    pub after_move_delay: Option<Duration>,
    pub after_input_delay: Option<Duration>,
    pub press_release_delay: Option<Duration>,
    pub after_click_delay: Option<Duration>,
    pub before_next_click_delay: Option<Duration>,
    pub multi_click_delay: Option<Duration>,
    pub ensure_move_threshold: Option<f64>,
    pub ensure_move_timeout: Option<Duration>,
    pub scroll_step: Option<ScrollDelta>,
    pub scroll_delay: Option<Duration>,
    pub max_move_duration: Option<Duration>,
    pub move_time_per_pixel: Option<Duration>,
}

impl PointerOverrides {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn origin(mut self, origin: PointOrigin) -> Self {
        self.origin = Some(origin);
        self
    }

    pub fn profile(mut self, profile: PointerProfile) -> Self {
        self.profile = Some(profile);
        self
    }

    pub fn after_move_delay(mut self, delay: Duration) -> Self {
        self.after_move_delay = Some(delay);
        self
    }

    pub fn after_input_delay(mut self, delay: Duration) -> Self {
        self.after_input_delay = Some(delay);
        self
    }

    pub fn press_release_delay(mut self, delay: Duration) -> Self {
        self.press_release_delay = Some(delay);
        self
    }

    pub fn after_click_delay(mut self, delay: Duration) -> Self {
        self.after_click_delay = Some(delay);
        self
    }

    pub fn before_next_click_delay(mut self, delay: Duration) -> Self {
        self.before_next_click_delay = Some(delay);
        self
    }

    pub fn multi_click_delay(mut self, delay: Duration) -> Self {
        self.multi_click_delay = Some(delay);
        self
    }

    pub fn ensure_move_threshold(mut self, threshold: f64) -> Self {
        self.ensure_move_threshold = Some(threshold);
        self
    }

    pub fn ensure_move_timeout(mut self, timeout: Duration) -> Self {
        self.ensure_move_timeout = Some(timeout);
        self
    }

    pub fn scroll_step(mut self, delta: ScrollDelta) -> Self {
        self.scroll_step = Some(delta);
        self
    }

    pub fn scroll_delay(mut self, delay: Duration) -> Self {
        self.scroll_delay = Some(delay);
        self
    }

    pub fn move_duration(mut self, duration: Duration) -> Self {
        self.max_move_duration = Some(duration);
        self
    }

    pub fn move_time_per_pixel(mut self, duration: Duration) -> Self {
        self.move_time_per_pixel = Some(duration);
        self
    }
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
    use crate::types::{Point, Rect, Size};
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

    #[rstest]
    fn pointer_overrides_builder_keeps_defaults() {
        let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
        let overrides = PointerOverrides::new()
            .origin(PointOrigin::Bounds(rect))
            .after_move_delay(Duration::from_millis(10))
            .scroll_step(ScrollDelta::new(12.0, -24.0));

        assert_eq!(overrides.origin, Some(PointOrigin::Bounds(rect)));
        assert_eq!(overrides.scroll_step, Some(ScrollDelta::new(12.0, -24.0)));
    }

    #[rstest]
    fn default_settings_match_expectations() {
        let settings = PointerSettings::default();
        assert_eq!(settings.default_button, PointerButton::Left);
        assert!(settings.double_click_time >= Duration::from_millis(300));
    }
}
