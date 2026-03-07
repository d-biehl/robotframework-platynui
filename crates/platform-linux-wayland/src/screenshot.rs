use crate::wayland_util::connection;
use platynui_core::platform::{PlatformError, PlatformErrorKind, Screenshot, ScreenshotProvider, ScreenshotRequest};
use platynui_core::register_screenshot_provider;

/// Wayland screenshot provider with a fallback chain:
///
/// 1. `ext-image-copy-capture-v1` (standard protocol — `KWin`, Sway 1.11+,
///    Hyprland, Mir, cosmic-comp, niri)
/// 2. `wlr-screencopy-unstable-v1` (legacy — older wlroots)
/// 3. `org.freedesktop.portal.Screenshot` (D-Bus portal — Mutter/GNOME)
pub struct WaylandScreenshot;

impl ScreenshotProvider for WaylandScreenshot {
    fn capture(&self, request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
        let guard = connection()?;
        let caps = &guard.capabilities;

        if caps.ext_image_copy_capture {
            return capture_ext_image_copy(request);
        }

        if caps.wlr_screencopy {
            return capture_wlr_screencopy(request);
        }

        // Portal fallback (always available on GNOME, KDE, etc.)
        capture_portal(request)
    }
}

/// Capture using `ext-image-copy-capture-v1`.
fn capture_ext_image_copy(request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
    // TODO(Phase 4, Step 26): Implement ext-image-copy-capture-v1.
    // This requires:
    // 1. Bind ext_image_capture_source_manager_v1 (for output/toplevel source)
    // 2. Bind ext_image_copy_capture_manager_v1
    // 3. Create capture session → configure buffer
    // 4. Capture frame → read pixel data from wl_shm buffer
    let _ = request;
    Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "ext-image-copy-capture not yet implemented"))
}

/// Capture using `wlr-screencopy-unstable-v1` (legacy fallback).
fn capture_wlr_screencopy(request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
    // TODO(Phase 4, Step 26): Implement wlr-screencopy as legacy fallback.
    let _ = request;
    Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "wlr-screencopy not yet implemented"))
}

/// Capture via the xdg-desktop-portal Screenshot D-Bus API.
fn capture_portal(request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
    // TODO(Phase 4, Step 26): Implement portal screenshot via zbus:
    // 1. Connect to org.freedesktop.portal.Screenshot
    // 2. Call Screenshot() with options
    // 3. Read the returned file URI, load pixel data
    let _ = request;
    Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "portal screenshot not yet implemented"))
}

static SHOT: WaylandScreenshot = WaylandScreenshot;

register_screenshot_provider!(&SHOT);
