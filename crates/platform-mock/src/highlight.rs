use platynui_core::platform::{HighlightProvider, HighlightRequest, PlatformError};
use std::sync::Mutex;

pub static MOCK_HIGHLIGHT: MockHighlight = MockHighlight::new();

#[derive(Debug)]
pub struct MockHighlight {
    log: Mutex<Vec<HighlightRequest>>,
    clear_calls: Mutex<usize>,
}

impl MockHighlight {
    const fn new() -> Self {
        Self { log: Mutex::new(Vec::new()), clear_calls: Mutex::new(0) }
    }

    fn record(&self, request: &HighlightRequest) {
        let mut log = self.log.lock().expect("highlight log poisoned");
        log.push(request.clone());
    }

    fn mark_clear(&self) {
        let mut count = self.clear_calls.lock().expect("highlight clear count poisoned");
        *count += 1;
    }
}

impl HighlightProvider for MockHighlight {
    fn highlight(&self, request: &HighlightRequest) -> Result<(), PlatformError> {
        self.record(request);
        Ok(())
    }

    fn clear(&self) -> Result<(), PlatformError> {
        self.mark_clear();
        Ok(())
    }
}

pub fn take_highlight_log() -> Vec<HighlightRequest> {
    let mut log = MOCK_HIGHLIGHT.log.lock().expect("highlight log poisoned");
    log.drain(..).collect()
}

pub fn highlight_clear_count() -> usize {
    *MOCK_HIGHLIGHT.clear_calls.lock().expect("highlight clear count poisoned")
}

pub fn reset_highlight_state() {
    MOCK_HIGHLIGHT.log.lock().expect("highlight log poisoned").clear();
    *MOCK_HIGHLIGHT.clear_calls.lock().expect("highlight clear count poisoned") = 0;
}

// Expose provider reference for explicit injection in tests/integration code.
// Test helpers for exposing internal state

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::{HighlightRequest, highlight_providers};
    use platynui_core::types::Rect;
    use rstest::rstest;
    use serial_test::serial;

    #[rstest]
    #[serial]
    fn highlight_provider_not_auto_registered() {
        reset_highlight_state();
        let providers: Vec<_> = highlight_providers().collect();
        // Mock provider should NOT be in the registry
        let mock_in_registry = providers.iter().any(|p| std::ptr::eq(*p, &MOCK_HIGHLIGHT as &dyn HighlightProvider));
        assert!(!mock_in_registry, "Mock highlight provider should not be auto-registered");

        // Use direct reference for testing the provider itself
        let request = HighlightRequest::new(Rect::new(0.0, 0.0, 100.0, 50.0));
        MOCK_HIGHLIGHT.highlight(&request).unwrap();
        let log = take_highlight_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].rects[0], Rect::new(0.0, 0.0, 100.0, 50.0));

        MOCK_HIGHLIGHT.clear().unwrap();
        assert_eq!(highlight_clear_count(), 1);
    }
}
