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

/// The mock platform's settings as diagnostics name them.
const COMPONENT: &str = "platform.mock";

/// The settings the mock platform reads: none.
const KNOWN_KEYS: &[&str] = &[];

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

    fn create(&self, config: &RuntimeConfig) -> Result<PlatformBundle, PlatformError> {
        if let Some(settings) = config.platform(self.id()) {
            for key in settings.unknown_keys(KNOWN_KEYS) {
                tracing::warn!(component = COMPONENT, key = %key, "unknown setting; it is ignored");
            }
        }
        Ok(create_mock_bundle())
    }
}

/// Build a [`PlatformBundle`] of the in-memory mock devices.
#[must_use]
pub fn create_mock_bundle() -> PlatformBundle {
    PlatformBundle {
        pointer: Arc::new(MockPointerDevice::new()),
        keyboard: Arc::new(MockKeyboardDevice::new()),
        screenshot: Arc::new(MockScreenshot::new()),
        highlight: Arc::new(MockHighlight::new()),
        window_manager: Arc::new(MockWindowManager::new()),
        desktop_info: Arc::new(MockPlatform),
        java_classifier: None,
    }
}

/// Registered mock platform factory.
pub static MOCK_PLATFORM_FACTORY: MockPlatformFactory = MockPlatformFactory;
register_platform_factory!(&MOCK_PLATFORM_FACTORY);

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::config::ConfigMap;
    use std::sync::Mutex;

    /// Runs `f` and returns the warnings it logged on this thread.
    fn warnings(f: impl FnOnce()) -> Vec<String> {
        #[derive(Clone)]
        struct Captured(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Captured {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().expect("log buffer").extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = Captured(Arc::clone(&buffer));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .without_time()
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, f);
        let log = String::from_utf8(buffer.lock().expect("log buffer").clone()).expect("utf-8 log");
        log.lines().filter(|line| line.trim_start().starts_with("WARN")).map(str::to_owned).collect()
    }

    #[test]
    fn an_unknown_setting_warns_once_and_the_build_proceeds() {
        let config = RuntimeConfig::new(
            ConfigMap::new().with("backend", "mock").with("mock", ConfigMap::new().with("bogus", 1_i64)),
            ConfigMap::new(),
        );

        let warnings = warnings(|| {
            MOCK_PLATFORM_FACTORY.create(&config).expect("the mock bundle builds");
        });

        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("component=\"platform.mock\""), "{warnings:?}");
        assert!(warnings[0].contains("key=bogus"), "{warnings:?}");
        assert!(warnings[0].contains("unknown setting; it is ignored"), "{warnings:?}");
    }
}
