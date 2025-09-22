mod error;
mod module;
mod registration;

pub use error::{PlatformError, PlatformErrorKind};
pub use module::PlatformModule;
pub use registration::{PlatformRegistration, platform_modules};

#[macro_export]
macro_rules! register_platform_module {
    ($module:expr) => {
        inventory::submit! {
            $crate::platform::PlatformRegistration { module: $module }
        }
    };
}

pub use register_platform_module;
