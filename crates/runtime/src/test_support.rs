use crate::Runtime;
use platynui_core::config::{ConfigMap, RuntimeConfig};
use platynui_core::provider::UiTreeProviderFactory;
use platynui_platform_mock as _;
use rstest::fixture;
use std::sync::{Arc, Mutex};

/// rstest fixture: Runtime with the mock provider and the mock platform backend.
#[fixture]
pub fn rt_runtime_mock() -> Runtime {
    return runtime_with_factories_and_mock_platform(&[&platynui_provider_mock::MOCK_PROVIDER_FACTORY]);
}

/// A [`RuntimeConfig`] that selects the mock platform backend
/// (`platform.backend = "mock"`), so the runtime builds its bundle from the
/// mock pointer/keyboard/highlight/screenshot/window-manager devices.
#[must_use]
pub fn mock_config() -> RuntimeConfig {
    RuntimeConfig::new(ConfigMap::new().with("backend", "mock"), ConfigMap::new())
}

/// Builds a Runtime from the given provider factories, bound to the mock
/// platform backend.
///
/// # Panics
///
/// Panics if the runtime cannot be constructed, e.g. when a provider factory
/// fails or the mock platform backend is not linked.
pub fn runtime_with_factories_and_mock_platform(factories: &[&'static dyn UiTreeProviderFactory]) -> Runtime {
    Runtime::new_with_factories_and_config(factories, mock_config()).expect("runtime")
}

/// Runs `f` with the tracing records of this thread captured as text, one
/// record per line, at every level.
///
/// # Panics
///
/// Panics if the capture buffer is poisoned or does not hold UTF-8.
pub fn logged<R>(f: impl FnOnce() -> R) -> (R, String) {
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
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .without_time()
        .with_writer(move || writer.clone())
        .finish();
    let result = tracing::subscriber::with_default(subscriber, f);
    let log = String::from_utf8(buffer.lock().expect("log buffer").clone()).expect("utf-8 log");
    (result, log)
}

/// The captured records at `level` (`"ERROR"`, `"WARN"`, `"INFO"`, `"DEBUG"`, `"TRACE"`).
#[must_use]
pub fn records<'a>(log: &'a str, level: &str) -> Vec<&'a str> {
    log.lines().filter(|line| line.trim_start().starts_with(level)).collect()
}
