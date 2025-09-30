use platynui_core::platform::{HighlightProvider, HighlightRequest, PlatformError};
use platynui_core::register_highlight_provider;
use std::sync::Mutex;

static MOCK_HIGHLIGHT: MockHighlight = MockHighlight::new();

register_highlight_provider!(&MOCK_HIGHLIGHT);

#[derive(Debug)]
struct MockHighlight {
    log: Mutex<Vec<Vec<HighlightRequest>>>,
    clear_calls: Mutex<usize>,
}

impl MockHighlight {
    const fn new() -> Self {
        Self { log: Mutex::new(Vec::new()), clear_calls: Mutex::new(0) }
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

pub fn take_highlight_log() -> Vec<Vec<HighlightRequest>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::{HighlightRequest, highlight_providers};
    use platynui_core::types::Rect;
    use rstest::rstest;
    use serial_test::serial;

    #[rstest]
    #[serial]
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
