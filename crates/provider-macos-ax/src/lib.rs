//! macOS Accessibility (AX) `UiTree` provider (stub).
//!
//! This crate exposes a minimal provider factory so tests and consumers can
//! construct a `Runtime` with a macOS AX provider via
//! `Runtime::new_with_factories(&[&MACOS_AX_FACTORY])`. The actual
//! AXUIElement-backed implementation will be added incrementally. Until then
//! the provider lists no elements, and the first provider it creates in a
//! process warns that queries on this desktop find nothing.

use platynui_core::config::RuntimeConfig;
use platynui_core::provider::{ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory};
use platynui_core::ui::{TechnologyId, UiNode};
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Once;

pub const PROVIDER_ID: &str = "macos-ax";
pub const PROVIDER_NAME: &str = "macOS Accessibility";
pub static TECHNOLOGY: LazyLock<TechnologyId> = LazyLock::new(|| TechnologyId::from("AX"));

/// The component name of this provider's settings (`providers.macos-ax`).
const COMPONENT: &str = "providers.macos-ax";

/// The settings this provider reads: none yet.
const KNOWN_KEYS: &[&str] = &[];

pub struct MacOsAxFactory {
    /// Runs the warning that the provider is a stub. The registered factory
    /// is the static [`MACOS_AX_FACTORY`], so the warning is given once per
    /// process.
    stub_reported: Once,
}

impl MacOsAxFactory {
    const fn new() -> Self {
        Self { stub_reported: Once::new() }
    }
}

impl UiTreeProviderFactory for MacOsAxFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
            ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from("AX"), ProviderKind::Native)
        });
        &DESCRIPTOR
    }

    fn create(&self, config: &RuntimeConfig) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        check_config(config);
        self.stub_reported.call_once(|| {
            tracing::warn!(
                provider = PROVIDER_ID,
                "macOS accessibility is not implemented yet; queries on this desktop find nothing"
            );
        });
        Ok(Arc::new(MacOsAxProvider::new()))
    }
}

/// Warn for each key of this provider's settings that it does not read.
fn check_config(config: &RuntimeConfig) {
    let Some(settings) = config.provider(PROVIDER_ID) else {
        return;
    };
    for key in settings.unknown_keys(KNOWN_KEYS) {
        tracing::warn!(component = COMPONENT, key = %key, "unknown setting; it is ignored");
    }
}

struct MacOsAxProvider {
    descriptor: &'static ProviderDescriptor,
}

impl MacOsAxProvider {
    fn new() -> Self {
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
            ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from("AX"), ProviderKind::Native)
        });
        Self { descriptor: &DESCRIPTOR }
    }
}

impl UiTreeProvider for MacOsAxProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
    }
    fn get_nodes(
        &self,
        _parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
        Ok(Box::new(std::iter::empty()))
    }
}

pub static MACOS_AX_FACTORY: MacOsAxFactory = MacOsAxFactory::new();

// Auto-register the macOS AX provider when linked
platynui_core::register_provider!(&MACOS_AX_FACTORY);

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::config::ConfigMap;
    use std::sync::Mutex;

    /// Run `f` with its tracing output captured.
    fn logged<R>(f: impl FnOnce() -> R) -> (R, String) {
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

        let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Captured(Arc::clone(&buffer));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let log = String::from_utf8(buffer.lock().expect("log buffer").clone()).expect("utf-8 log");
        (result, log)
    }

    fn warnings(log: &str) -> Vec<&str> {
        log.lines().filter(|line| line.contains(" WARN ")).collect()
    }

    /// A factory of its own, so that the test does not depend on whether the
    /// registered one has already reported the stub.
    fn factory() -> MacOsAxFactory {
        MacOsAxFactory::new()
    }

    #[test]
    fn two_provider_creations_give_one_warning() {
        let factory = factory();
        let ((), log) = logged(|| {
            factory.create(&RuntimeConfig::default()).expect("first provider");
            factory.create(&RuntimeConfig::default()).expect("second provider");
        });

        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 1, "{log}");
        for expected in ["macOS accessibility is not implemented yet", "queries on this desktop find nothing"] {
            assert!(warnings[0].contains(expected), "the warning says `{expected}`: {}", warnings[0]);
        }
    }

    #[test]
    fn an_unknown_setting_gives_one_warning_naming_the_component_and_the_key() {
        let factory = factory();
        factory.create(&RuntimeConfig::default()).expect("provider that reports the stub");
        let settings = ConfigMap::new().with("bogus", 1_i64).with("enabled", true);
        let config = RuntimeConfig::new(ConfigMap::new(), ConfigMap::new().with(PROVIDER_ID, settings));

        let (provider, log) = logged(|| factory.create(&config));

        assert!(provider.is_ok(), "the provider is still created");
        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 1, "one warning for `bogus`, none for the reserved `enabled`\n{log}");
        for expected in ["unknown setting; it is ignored", r#"component="providers.macos-ax""#, "key=bogus"] {
            assert!(warnings[0].contains(expected), "the warning names `{expected}`: {}", warnings[0]);
        }
    }
}
