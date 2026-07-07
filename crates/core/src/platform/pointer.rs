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
#[must_use]
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
