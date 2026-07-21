mod desktop;
mod error;
mod factory;
mod highlight;
pub mod java;
mod keyboard;
mod pointer;
mod screenshot;
pub mod window_claims;
mod window_manager;

pub use desktop::{DesktopInfo, DesktopInfoProvider, MonitorInfo};
pub use error::PlatformError;
pub use factory::{PlatformBundle, PlatformFactory, PlatformFactoryRegistration, platform_factories};
pub use highlight::{HighlightProvider, HighlightRequest};
pub use java::{JavaClassification, JavaClassifier, JavaToolkit};
pub use keyboard::{
    KeyCode, KeyCodeError, KeyState, KeyboardDevice, KeyboardError, KeyboardEvent, KeyboardOverrides, KeyboardProfile,
};
pub use pointer::{
    PointOrigin, PointerAccelerationProfile, PointerButton, PointerDevice, PointerMotionMode, ScrollDelta,
};
pub use screenshot::{PixelFormat, Screenshot, ScreenshotProvider, ScreenshotRequest};
pub use window_manager::{WindowHit, WindowId, WindowManager};

pub use crate::register_platform_factory;
