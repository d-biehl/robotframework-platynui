use platynui_core::platform::{DesktopInfo, DesktopInfoProvider, MonitorInfo, PlatformError, PlatformModule};
use platynui_core::types::Rect;
use platynui_core::ui::{RuntimeId, TechnologyId};

pub static MOCK_PLATFORM: MockPlatform = MockPlatform;

// Mock platform does NOT auto-register - only available via explicit handles

#[derive(Debug)]
pub struct MockPlatform;

impl MockPlatform {
    pub(crate) const NAME: &'static str = "Mock Platform";
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
        let mut primary = MonitorInfo::new("mock-monitor-center", Rect::new(0.0, 0.0, 3840.0, 2160.0));
        primary.name = Some("Mock Primary Center".into());
        primary.is_primary = true;
        primary.scale_factor = Some(1.0);

        let mut left = MonitorInfo::new("mock-monitor-left", Rect::new(-2160.0, -840.0, 2160.0, 3840.0));
        left.name = Some("Mock Left Portrait".into());
        left.scale_factor = Some(1.0);

        let mut right = MonitorInfo::new("mock-monitor-right", Rect::new(3840.0, 540.0, 1920.0, 1080.0));
        right.name = Some("Mock Right FHD".into());
        right.scale_factor = Some(1.0);

        let desktop_bounds = {
            let first = primary.bounds.union(&left.bounds);
            first.union(&right.bounds)
        };

        Ok(DesktopInfo {
            runtime_id: RuntimeId::from("mock://desktop"),
            name: "Mock Desktop".into(),
            technology: TechnologyId::from("MockPlatform"),
            bounds: desktop_bounds,
            os_name: "MockOS".into(),
            os_version: "1.0".into(),
            monitors: vec![left, primary, right],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::{desktop_info_providers, platform_modules};
    use rstest::rstest;

    #[rstest]
    fn mock_platform_not_auto_registered() {
        let names: Vec<_> = platform_modules().map(|module| module.name()).collect();
        assert!(!names.contains(&MockPlatform::NAME), "Mock platform should not be auto-registered");
    }

    #[rstest]
    fn initialize_returns_ok() {
        assert!(MOCK_PLATFORM.initialize().is_ok());
    }

    #[rstest]
    fn desktop_info_provider_not_auto_registered() {
        let infos: Vec<_> = desktop_info_providers().filter_map(|provider| provider.desktop_info().ok()).collect();
        // Mock provider should NOT be in the registry
        let mock_in_registry = infos.iter().any(|info| info.os_name == "MockOS");
        assert!(!mock_in_registry, "Mock desktop info provider should not be auto-registered");

        // Use direct reference for testing the provider itself
        let info = MOCK_PLATFORM.desktop_info().unwrap();
        assert_eq!(info.os_name, "MockOS");
        assert_eq!(info.display_count(), 3);
        assert_eq!(info.bounds, Rect::new(-2160.0, -840.0, 7920.0, 3840.0));
    }
}
