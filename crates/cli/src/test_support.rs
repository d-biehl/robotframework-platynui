use platynui_core::config::{ConfigMap, RuntimeConfig};
use platynui_platform_mock as _;
use platynui_provider_mock::MOCK_PROVIDER_FACTORY;
use platynui_runtime::Runtime;
use rstest::fixture;

pub fn runtime_mock_full() -> Runtime {
    // Select the mock platform backend (pointer/keyboard/highlight/screenshot/
    // window-manager) via config; `platynui_platform_mock` is linked above so its
    // factory is registered.
    let config = RuntimeConfig::new(ConfigMap::new().with("backend", "mock"), ConfigMap::new());
    Runtime::new_with_factories_and_config(&[&MOCK_PROVIDER_FACTORY], config).expect("runtime")
}

/// rstest fixture: Runtime with mock provider and full mock platform stack
#[fixture]
pub fn runtime() -> Runtime {
    return runtime_mock_full();
}
