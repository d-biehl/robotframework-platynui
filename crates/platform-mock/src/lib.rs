//! In-memory mock platform implementation for PlatynUI tests.
//!
//! The real implementation will expose deterministic devices and window
//! management primitives so integration tests can run without native APIs.

mod desktop;
mod highlight;
mod keyboard;
mod pointer;
mod screenshot;

// Export mock components as public static references for direct use
pub use desktop::MOCK_PLATFORM;
pub use highlight::MOCK_HIGHLIGHT;
pub use keyboard::MOCK_KEYBOARD;
pub use pointer::MOCK_POINTER;
pub use screenshot::MOCK_SCREENSHOT;

// Export test helper functions
pub use highlight::{highlight_clear_count, reset_highlight_state, take_highlight_log};
pub use keyboard::{KeyboardLogEntry, reset_keyboard_state, take_keyboard_log};
pub use pointer::{PointerLogEntry, reset_pointer_state, take_pointer_log};
pub use screenshot::{reset_screenshot_state, take_screenshot_log};

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::{HighlightProvider, HighlightRequest};
    use platynui_core::types::Rect;
    use rstest::rstest;
    use serial_test::serial;

    #[rstest]
    #[serial]
    fn highlight_helpers_expose_state() {
        reset_highlight_state();

        // Use direct reference to mock highlight provider
        let request = HighlightRequest::new(Rect::new(0.0, 0.0, 50.0, 50.0));
        MOCK_HIGHLIGHT.highlight(&request).unwrap();
        assert_eq!(take_highlight_log().len(), 1);
    }
}
