use platynui_platform_mock::{
    highlight_provider, keyboard_device, pointer_device, screenshot_provider,
};
use platynui_provider_mock::MOCK_PROVIDER_FACTORY;
use platynui_runtime::{Runtime, runtime::PlatformOverrides};

pub fn runtime_mock_full() -> Runtime {
    Runtime::new_with_factories_and_platforms(
        &[&MOCK_PROVIDER_FACTORY],
        PlatformOverrides {
            highlight: Some(highlight_provider()),
            screenshot: Some(screenshot_provider()),
            pointer: Some(pointer_device()),
            keyboard: Some(keyboard_device()),
        },
    )
    .expect("runtime")
}
