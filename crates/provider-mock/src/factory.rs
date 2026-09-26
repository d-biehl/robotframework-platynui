use crate::events;
use crate::provider::MockProvider;
use platynui_core::config::RuntimeConfig;
use platynui_core::provider::{
    ProviderDescriptor, ProviderError, ProviderEventCapabilities, ProviderKind, UiTreeProvider, UiTreeProviderFactory,
};
use platynui_core::ui::identifiers::TechnologyId;
use std::sync::{Arc, LazyLock};

pub const PROVIDER_ID: &str = "mock";
pub const PROVIDER_NAME: &str = "PlatynUI Mock Provider";
pub const TECHNOLOGY: &str = "Mock";

/// The mock provider's settings as diagnostics name them.
const COMPONENT: &str = "providers.mock";

/// The settings the mock provider reads: none besides the reserved `enabled`.
const KNOWN_KEYS: &[&str] = &[];

#[cfg(test)]
pub const APP_RUNTIME_ID: &str = "mock://app/main";
#[cfg(test)]
pub const WINDOW_RUNTIME_ID: &str = "mock://window/main";
#[cfg(test)]
pub const BUTTON_RUNTIME_ID: &str = "mock://button/ok";

pub static MOCK_PROVIDER_FACTORY: MockProviderFactory = MockProviderFactory;

pub struct MockProviderFactory;

impl MockProviderFactory {
    #[must_use]
    pub fn descriptor_static() -> &'static ProviderDescriptor {
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
            ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from(TECHNOLOGY), ProviderKind::Native)
                .with_event_capabilities(ProviderEventCapabilities::STRUCTURE_WITH_PROPERTIES)
        });
        &DESCRIPTOR
    }
}

impl UiTreeProviderFactory for MockProviderFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        Self::descriptor_static()
    }

    fn create(&self, config: &RuntimeConfig) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        if let Some(settings) = config.provider(PROVIDER_ID) {
            for key in settings.unknown_keys(KNOWN_KEYS) {
                tracing::warn!(component = COMPONENT, key = %key, "unknown setting; it is ignored");
            }
        }
        let provider: Arc<MockProvider> = Arc::new(MockProvider::new(Self::descriptor_static()));
        events::register_active_instance(&provider);
        Ok(provider)
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use platynui_core::config::ConfigMap;
    use serial_test::serial;
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
    #[serial]
    fn an_unknown_setting_warns_once_and_the_build_proceeds() {
        let providers = ConfigMap::new().with(PROVIDER_ID, ConfigMap::new().with("bogus", 1_i64).with("enabled", true));
        let config = RuntimeConfig::new(ConfigMap::new(), providers);

        let warnings = warnings(|| {
            MOCK_PROVIDER_FACTORY.create(&config).expect("the mock provider builds");
        });

        assert_eq!(warnings.len(), 1, "the reserved `enabled` is known: {warnings:?}");
        assert!(warnings[0].contains("component=\"providers.mock\""), "{warnings:?}");
        assert!(warnings[0].contains("key=bogus"), "{warnings:?}");
        assert!(warnings[0].contains("unknown setting; it is ignored"), "{warnings:?}");
    }
}
