use crate::Runtime;
use platynui_core::config::{ConfigMap, RuntimeConfig};
use platynui_core::provider::UiTreeProviderFactory;
use platynui_platform_mock as _;
use rstest::fixture;

/// rstest fixture: Runtime with the mock provider and the mock platform backend.
#[fixture]
pub fn rt_runtime_mock() -> Runtime {
    return runtime_with_factories_and_mock_platform(&[&platynui_provider_mock::MOCK_PROVIDER_FACTORY]);
}

/// A [`RuntimeConfig`] that selects the mock platform backend
/// (`platform.backend = "mock"`), so the runtime builds its bundle from the
/// mock pointer/keyboard/highlight/screenshot/window-manager devices.
fn mock_config() -> RuntimeConfig {
    RuntimeConfig::new(ConfigMap::new().with("backend", "mock"), ConfigMap::new())
}

/// Builds a Runtime from the given provider factories, bound to the mock
/// platform backend.
pub fn runtime_with_factories_and_mock_platform(factories: &[&'static dyn UiTreeProviderFactory]) -> Runtime {
    Runtime::new_with_factories_and_config(factories, mock_config()).expect("runtime")
}
