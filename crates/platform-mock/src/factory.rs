//! Mock platform backend factory.
//!
//! Provides a [`PlatformFactory`] selected only when a runtime explicitly asks
//! for it via `config.platform.backend = "mock"` (e.g. `Runtime::new_with_mock`
//! or the test fixtures); it is never auto-detected. Its devices drive this
//! crate's shared in-memory mock state.

use platynui_core::config::RuntimeConfig;
use platynui_core::platform::{PlatformBundle, PlatformError, PlatformFactory};
use platynui_core::register_platform_factory;
use std::sync::Arc;

use crate::desktop::MockPlatform;
use crate::highlight::MockHighlight;
use crate::keyboard::MockKeyboardDevice;
use crate::pointer::MockPointerDevice;
use crate::screenshot::MockScreenshot;
use crate::window_manager::MockWindowManager;

/// Mock platform backend (id `"mock"`).
pub struct MockPlatformFactory;

impl PlatformFactory for MockPlatformFactory {
    fn id(&self) -> &'static str {
        "mock"
    }

    fn can_serve(&self, config: &RuntimeConfig) -> bool {
        // Opt-in only: never auto-detected, only when explicitly requested.
        config.platform_backend() == Some("mock")
    }

    fn create(&self, _config: &RuntimeConfig) -> Result<PlatformBundle, PlatformError> {
        Ok(create_mock_bundle())
    }
}

/// Build a [`PlatformBundle`] of the in-memory mock devices.
pub fn create_mock_bundle() -> PlatformBundle {
    PlatformBundle {
        pointer: Arc::new(MockPointerDevice::new()),
        keyboard: Arc::new(MockKeyboardDevice::new()),
        screenshot: Arc::new(MockScreenshot::new()),
        highlight: Arc::new(MockHighlight::new()),
        window_manager: Arc::new(MockWindowManager::new()),
        desktop_info: Arc::new(MockPlatform),
    }
}

/// Registered mock platform factory.
pub static MOCK_PLATFORM_FACTORY: MockPlatformFactory = MockPlatformFactory;
register_platform_factory!(&MOCK_PLATFORM_FACTORY);
