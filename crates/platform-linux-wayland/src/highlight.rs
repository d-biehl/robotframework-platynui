use platynui_core::platform::{HighlightProvider, HighlightRequest, PlatformError};
use serde_json::json;

use crate::capabilities::CompositorType;

pub struct WaylandHighlightProvider;

impl HighlightProvider for WaylandHighlightProvider {
    fn highlight(&self, request: &HighlightRequest) -> Result<(), PlatformError> {
        if request.rects.is_empty() {
            return self.clear();
        }

        if crate::connection::compositor_type() != Some(CompositorType::PlatynUi) {
            tracing::warn!("Wayland highlight is currently only implemented for the PlatynUI compositor");
            return Ok(());
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
