//! In-memory mock platform implementation for PlatynUI tests.
//!
//! The real implementation will expose deterministic devices and window
//! management primitives so integration tests can run without native APIs.

use platynui_core::platform::{
    DesktopInfo, DesktopInfoProvider, HighlightProvider, HighlightRequest, MonitorInfo,
    PlatformError, PlatformModule,
};
use platynui_core::register_desktop_info_provider;
use platynui_core::register_highlight_provider;
use platynui_core::register_platform_module;
use platynui_core::types::Rect;
use platynui_core::ui::{RuntimeId, TechnologyId};

static MOCK_PLATFORM: MockPlatform = MockPlatform;
static MOCK_HIGHLIGHT: MockHighlight = MockHighlight::new();

register_platform_module!(&MOCK_PLATFORM);
register_desktop_info_provider!(&MOCK_PLATFORM);
register_highlight_provider!(&MOCK_HIGHLIGHT);

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

#[derive(Debug)]
struct MockHighlight {
    log: std::sync::Mutex<Vec<Vec<HighlightRequest>>>,
    clear_calls: std::sync::Mutex<usize>,
}

impl MockHighlight {
    const fn new() -> Self {
        Self { log: std::sync::Mutex::new(Vec::new()), clear_calls: std::sync::Mutex::new(0) }
    }

    fn record(&self, requests: &[HighlightRequest]) {
        let mut log = self.log.lock().expect("highlight log poisoned");
        log.push(requests.to_vec());
    }

    fn mark_clear(&self) {
        let mut count = self.clear_calls.lock().expect("highlight clear count poisoned");
        *count += 1;
    }
}

impl HighlightProvider for MockHighlight {
    fn highlight(&self, requests: &[HighlightRequest]) -> Result<(), PlatformError> {
        self.record(requests);
        Ok(())
    }

    fn clear(&self) -> Result<(), PlatformError> {
        self.mark_clear();
        Ok(())
    }
}

/// Returns and clears the recorded highlight requests (FIFO order).
pub fn take_highlight_log() -> Vec<Vec<HighlightRequest>> {
    let mut log = MOCK_HIGHLIGHT.log.lock().expect("highlight log poisoned");
    log.drain(..).collect()
}

/// Returns how often `clear()` has been invoked since the last reset.
pub fn highlight_clear_count() -> usize {
    *MOCK_HIGHLIGHT.clear_calls.lock().expect("highlight clear count poisoned")
}

/// Resets highlight log and clear counter. Intended for use in tests.
pub fn reset_highlight_state() {
    MOCK_HIGHLIGHT.log.lock().expect("highlight log poisoned").clear();
    *MOCK_HIGHLIGHT.clear_calls.lock().expect("highlight clear count poisoned") = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::{
        HighlightRequest, desktop_info_providers, highlight_providers, platform_modules,
    };
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

    #[rstest]
    fn highlight_provider_is_registered() {
        reset_highlight_state();
        let providers: Vec<_> = highlight_providers().collect();
        assert!(!providers.is_empty());

        let request = HighlightRequest::new(Rect::new(0.0, 0.0, 100.0, 50.0));
        providers[0].highlight(&[request]).unwrap();
        let log = take_highlight_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0][0].bounds, Rect::new(0.0, 0.0, 100.0, 50.0));

        providers[0].clear().unwrap();
        assert_eq!(highlight_clear_count(), 1);
    }
}
