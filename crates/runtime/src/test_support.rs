use crate::Runtime;
use crate::runtime::PlatformOverrides;
use platynui_core::platform::{
    HighlightProvider, KeyboardDevice, PointerDevice, ScreenshotProvider,
};
use platynui_core::provider::UiTreeProviderFactory;
use platynui_platform_mock::{
    highlight_provider, keyboard_device, pointer_device, screenshot_provider,
};
use rstest::fixture;

/// rstest fixture: Runtime with mock provider and mock platform devices
#[fixture]
pub fn rt_runtime_mock() -> Runtime {
    return runtime_with_factories_and_mock_platform(&[&platynui_provider_mock::MOCK_PROVIDER_FACTORY]);
}

/// Builds a Runtime from the given provider factories and injects all mock
/// platform providers (highlight, screenshot, pointer, keyboard).
pub fn runtime_with_factories_and_mock_platform(
    factories: &[&'static dyn UiTreeProviderFactory],
) -> Runtime {
    Runtime::new_with_factories_and_platforms(
        factories,
        PlatformOverrides {
            highlight: Some(highlight_provider() as &'static dyn HighlightProvider),
            screenshot: Some(screenshot_provider() as &'static dyn ScreenshotProvider),
            pointer: Some(pointer_device() as &'static dyn PointerDevice),
            keyboard: Some(keyboard_device() as &'static dyn KeyboardDevice),
        },
    )
    .expect("runtime")
}
