use platynui_platform_mock::{MOCK_HIGHLIGHT, MOCK_KEYBOARD, MOCK_PLATFORM, MOCK_POINTER, MOCK_SCREENSHOT};
use platynui_provider_mock::MOCK_PROVIDER_FACTORY;
use platynui_runtime::{Runtime, runtime::PlatformOverrides};
use rstest::fixture;

pub fn runtime_mock_full() -> Runtime {
    Runtime::new_with_factories_and_platforms(
        &[&MOCK_PROVIDER_FACTORY],
        PlatformOverrides {
            desktop_info: Some(&MOCK_PLATFORM),
            highlight: Some(&MOCK_HIGHLIGHT),
            screenshot: Some(&MOCK_SCREENSHOT),
            pointer: Some(&MOCK_POINTER),
            keyboard: Some(&MOCK_KEYBOARD),
        },
    )
    .expect("runtime")
}

/// rstest fixture: Runtime with mock provider and full mock platform stack
#[fixture]
pub fn runtime() -> Runtime {
    return runtime_mock_full();
}
