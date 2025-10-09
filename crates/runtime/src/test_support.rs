use crate::Runtime;
use crate::runtime::PlatformOverrides;
use platynui_core::platform::{
    DesktopInfoProvider, HighlightProvider, KeyboardDevice, PointerDevice, ScreenshotProvider,
};
use platynui_core::provider::UiTreeProviderFactory;
use platynui_platform_mock::{MOCK_HIGHLIGHT, MOCK_KEYBOARD, MOCK_PLATFORM, MOCK_POINTER, MOCK_SCREENSHOT};
use rstest::fixture;

/// rstest fixture: Runtime with mock provider and mock platform devices
#[fixture]
pub fn rt_runtime_mock() -> Runtime {
    return runtime_with_factories_and_mock_platform(&[&platynui_provider_mock::MOCK_PROVIDER_FACTORY]);
}

/// Builds a Runtime from the given provider factories and injects all mock
/// platform providers (highlight, screenshot, pointer, keyboard).
pub fn runtime_with_factories_and_mock_platform(factories: &[&'static dyn UiTreeProviderFactory]) -> Runtime {
    Runtime::new_with_factories_and_platforms(
        factories,
        PlatformOverrides {
            desktop_info: Some(&MOCK_PLATFORM as &'static dyn DesktopInfoProvider),
            highlight: Some(&MOCK_HIGHLIGHT as &'static dyn HighlightProvider),
            screenshot: Some(&MOCK_SCREENSHOT as &'static dyn ScreenshotProvider),
            pointer: Some(&MOCK_POINTER as &'static dyn PointerDevice),
            keyboard: Some(&MOCK_KEYBOARD as &'static dyn KeyboardDevice),
        },
    )
    .expect("runtime")
}
