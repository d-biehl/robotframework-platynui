mod desktop;
mod error;
mod highlight;
mod module;
mod registration;
mod screenshot;

pub use desktop::{
    DesktopInfo, DesktopInfoProvider, DesktopInfoRegistration, MonitorInfo, desktop_info_providers,
};
pub use error::{PlatformError, PlatformErrorKind};
pub use highlight::{
    HighlightProvider, HighlightRegistration, HighlightRequest, highlight_providers,
};
pub use module::PlatformModule;
pub use registration::{PlatformRegistration, platform_modules};
pub use screenshot::{
    PixelFormat, Screenshot, ScreenshotProvider, ScreenshotRegistration, ScreenshotRequest,
    screenshot_providers,
};

#[macro_export]
macro_rules! register_platform_module {
    ($module:expr) => {
        inventory::submit! {
            $crate::platform::PlatformRegistration { module: $module }
        }
    };
}

pub use crate::register_highlight_provider;
pub use crate::register_screenshot_provider;
pub use register_platform_module;
