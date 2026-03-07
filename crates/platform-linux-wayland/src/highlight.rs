use crate::wayland_util::connection;
use platynui_core::platform::{HighlightProvider, HighlightRequest, PlatformError, PlatformErrorKind};
use platynui_core::register_highlight_provider;
use std::sync::Mutex;
use std::sync::mpsc;

/// Command sent to the highlight overlay thread.
#[allow(dead_code)]
enum OverlayCommand {
    Show(HighlightRequest),
    Clear,
    Shutdown,
}

/// Wayland highlight provider using layer-shell protocols to render
/// semi-transparent overlay rectangles on top of target UI elements.
///
/// Fallback chain:
/// - `wlr-layer-shell-unstable-v1` (wlroots-based compositors)
/// - `ext-layer-shell-v1` (`KWin`)
/// - No-op with warning on Mutter (no layer-shell support)
///
/// Rendering is done with `tiny-skia` (CPU 2D rendering) to produce
/// ARGB buffers for `wl_shm`.
pub struct WaylandHighlightProvider;

static OVERLAY_SENDER: Mutex<Option<mpsc::Sender<OverlayCommand>>> = Mutex::new(None);

impl HighlightProvider for WaylandHighlightProvider {
    fn highlight(&self, request: &HighlightRequest) -> Result<(), PlatformError> {
        let guard = connection()?;
        let caps = &guard.capabilities;

        if !caps.wlr_layer_shell && !caps.ext_layer_shell {
            tracing::warn!("no layer-shell protocol available — highlight overlay not supported");
            return Err(PlatformError::new(
                PlatformErrorKind::CapabilityUnavailable,
                "no layer-shell protocol available for highlight overlays",
            ));
        }

        // TODO(Phase 4, Step 27): Implement layer-shell based highlight rendering.
        // Architecture:
        // 1. Spawn a dedicated thread that owns a Wayland event loop
        // 2. Create layer-shell surface(s) on the overlay layer
        // 3. Render highlight rectangles with tiny-skia into wl_shm buffers
        // 4. Use command channel for Show/Clear/Shutdown control

        let _ = request;
        Err(PlatformError::new(
            PlatformErrorKind::CapabilityUnavailable,
            "highlight overlay rendering not yet implemented",
        ))
    }

    fn clear(&self) -> Result<(), PlatformError> {
        let sender_guard = OVERLAY_SENDER.lock().map_err(|_| to_pf("overlay sender mutex poisoned"))?;
        if let Some(sender) = sender_guard.as_ref() {
            let _ = sender.send(OverlayCommand::Clear);
        }
        Ok(())
    }
}

/// Shut down the highlight overlay thread (called from `PlatformModule::shutdown`).
pub fn shutdown_highlight() {
    if let Ok(mut guard) = OVERLAY_SENDER.lock()
        && let Some(sender) = guard.take()
    {
        let _ = sender.send(OverlayCommand::Shutdown);
    }
}

fn to_pf<E: std::fmt::Display>(e: E) -> PlatformError {
    PlatformError::new(PlatformErrorKind::OperationFailed, format!("wayland highlight: {e}"))
}

static HIGHLIGHT: WaylandHighlightProvider = WaylandHighlightProvider;

register_highlight_provider!(&HIGHLIGHT);
