//! In-memory mock platform implementation for PlatynUI tests.
//!
//! The real implementation will expose deterministic devices and window
//! management primitives so integration tests can run without native APIs.

mod desktop;
mod highlight;
mod pointer;
mod screenshot;

pub use highlight::{highlight_clear_count, reset_highlight_state, take_highlight_log};
pub use pointer::{PointerLogEntry, reset_pointer_state, take_pointer_log};
pub use screenshot::{reset_screenshot_state, take_screenshot_log};

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::{HighlightRequest, highlight_providers};
    use platynui_core::types::Rect;
    use rstest::rstest;

    #[rstest]
    fn highlight_helpers_expose_state() {
        reset_highlight_state();
        let providers: Vec<_> = highlight_providers().collect();
        assert!(!providers.is_empty());

        let request = HighlightRequest::new(Rect::new(0.0, 0.0, 50.0, 50.0));
        providers[0].highlight(&[request]).unwrap();
        assert_eq!(take_highlight_log().len(), 1);
    }
}
