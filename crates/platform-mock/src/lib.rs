//! In-memory mock platform implementation for PlatynUI tests.
//!
//! The real implementation will expose deterministic devices and window
//! management primitives so integration tests can run without native APIs.

use platynui_core::platform::{
    DesktopInfo, DesktopInfoProvider, MonitorInfo, PlatformError, PlatformModule,
};
use platynui_core::register_desktop_info_provider;
use platynui_core::register_platform_module;
use platynui_core::types::Rect;
use platynui_core::ui::{RuntimeId, TechnologyId};

static MOCK_PLATFORM: MockPlatform = MockPlatform;

register_platform_module!(&MOCK_PLATFORM);
register_desktop_info_provider!(&MOCK_PLATFORM);

#[derive(Debug)]
struct MockPlatform;

impl MockPlatform {
    const NAME: &'static str = "Mock Platform";
}

impl PlatformModule for MockPlatform {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn initialize(&self) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl DesktopInfoProvider for MockPlatform {
    fn desktop_info(&self) -> Result<DesktopInfo, PlatformError> {
        let mut primary = MonitorInfo::new("mock-monitor-1", Rect::new(0.0, 0.0, 1920.0, 1080.0));
        primary.name = Some("Mock Primary".into());
        primary.is_primary = true;
        primary.scale_factor = Some(1.0);

        Ok(DesktopInfo {
            runtime_id: RuntimeId::from("mock-desktop"),
            name: "Mock Desktop".into(),
            technology: TechnologyId::from("MockPlatform"),
            bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
            os_name: "MockOS".into(),
            os_version: "1.0".into(),
            monitors: vec![primary],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::{desktop_info_providers, platform_modules};
    use rstest::rstest;

    #[rstest]
    fn mock_platform_is_registered() {
        let names: Vec<_> = platform_modules().map(|module| module.name()).collect();
        assert!(names.contains(&MockPlatform::NAME));
    }

    #[rstest]
    fn initialize_returns_ok() {
        assert!(MOCK_PLATFORM.initialize().is_ok());
    }

    #[rstest]
    fn desktop_info_provider_is_registered() {
        let infos: Vec<_> =
            desktop_info_providers().filter_map(|provider| provider.desktop_info().ok()).collect();
        assert!(!infos.is_empty());
        let info = &infos[0];
        assert_eq!(info.os_name, "MockOS");
        assert_eq!(info.display_count(), 1);
    }
}
