//! In-memory mock platform implementation for PlatynUI tests.
//!
//! The real implementation will expose deterministic devices and window
//! management primitives so integration tests can run without native APIs.

use platynui_core::platform::{PlatformError, PlatformModule};
use platynui_core::register_platform_module;

static MOCK_PLATFORM: MockPlatform = MockPlatform;

register_platform_module!(&MOCK_PLATFORM);

#[derive(Debug)]
struct MockPlatform;

impl MockPlatform {
    const NAME: &'static str = "Mock Platform";
}

impl PlatformModule for MockPlatform {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn initialize(&self) -> Result<(), PlatformError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::platform_modules;
    use rstest::rstest;

    #[rstest]
    fn mock_platform_is_registered() {
        let names: Vec<_> = platform_modules().map(|module| module.name()).collect();
        assert!(names.contains(&MockPlatform::NAME));
    }

    #[rstest]
    fn initialize_returns_ok() {
        assert!(MOCK_PLATFORM.initialize().is_ok());
    }
}
