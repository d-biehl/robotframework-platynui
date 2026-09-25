use platynui_core::platform::{HighlightProvider, HighlightRequest, PlatformError};
use serde_json::json;

use crate::capabilities::{CompositorType, unsupported_compositor};

pub struct WaylandHighlightProvider;

impl HighlightProvider for WaylandHighlightProvider {
    fn highlight(&self, request: &HighlightRequest) -> Result<(), PlatformError> {
        if request.rects.is_empty() {
            return self.clear();
        }

        // Only our own compositor draws highlights; anywhere else, reporting
        // success would claim a highlight nobody sees.
        let compositor = crate::connection::compositor_type();
        if compositor != Some(CompositorType::PlatynUi) {
            return Err(unsupported_compositor("Wayland highlight", compositor));
        }

        let rects: Vec<_> = request
            .rects()
            .copied()
            .map(|rect| json!({"x": rect.x(), "y": rect.y(), "width": rect.width(), "height": rect.height()}))
            .collect();
        let duration_ms = request.duration.map(duration_millis_u64);
        let _ = crate::control_ipc::send_command(
            &json!({"command": "show_highlight", "rects": rects, "duration_ms": duration_ms}),
            "show Wayland highlight",
        )?;
        Ok(())
    }

    fn clear(&self) -> Result<(), PlatformError> {
        // Nothing to clear where nothing can be drawn: no highlight is shown
        // afterwards, which is all clearing promises.
        if crate::connection::compositor_type() != Some(CompositorType::PlatynUi) {
            return Ok(());
        }

        let _ = crate::control_ipc::send_command(&json!({"command": "clear_highlight"}), "clear Wayland highlight")?;
        Ok(())
    }
}

fn duration_millis_u64(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::types::Rect;

    // No compositor is detected in a unit test (`compositor_type() == None`), the
    // condition `screenshot.rs` relies on too.

    #[test]
    fn highlighting_without_a_highlight_backend_reports_unavailable() {
        let request = HighlightRequest::new(Rect::new(10.0, 10.0, 50.0, 20.0));
        let err = WaylandHighlightProvider.highlight(&request).unwrap_err();
        assert!(matches!(err, PlatformError::CapabilityUnavailable { capability: "Wayland highlight", .. }), "{err}");
        assert!(err.to_string().contains("undetected Wayland compositor"), "names the compositor: {err}");
    }

    #[test]
    fn clearing_without_a_highlight_backend_succeeds() {
        // Its postcondition — nothing is shown — holds.
        assert!(WaylandHighlightProvider.clear().is_ok());
        assert!(WaylandHighlightProvider.highlight(&HighlightRequest::from_rects(Vec::new())).is_ok());
    }
}
