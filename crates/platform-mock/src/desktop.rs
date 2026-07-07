use platynui_core::platform::{DesktopInfo, DesktopInfoProvider, MonitorInfo, PlatformError};
use platynui_core::types::Rect;
use platynui_core::ui::{RuntimeId, TechnologyId};

pub static MOCK_PLATFORM: MockPlatform = MockPlatform;

// Mock platform is selected explicitly via `platform.backend="mock"`; it is the
// bundle's DesktopInfoProvider and is also available via the MOCK_PLATFORM handle.

#[derive(Debug)]
pub struct MockPlatform;

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
    use rstest::rstest;

    #[rstest]
    fn desktop_info_reports_expected_values() {
        let info = MOCK_PLATFORM.desktop_info().unwrap();
        assert_eq!(info.os_name, "MockOS");
        assert_eq!(info.display_count(), 3);
        assert_eq!(info.bounds, Rect::new(-2160.0, -840.0, 7920.0, 3840.0));
    }
}
