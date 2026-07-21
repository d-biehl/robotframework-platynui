//! X11 based platform integration for PlatynUI on Unix systems.
//!
//! This crate wires Linux/X11 specific devices (pointer, keyboard, screenshot,
//! highlight) plus desktop-info and window-manager helpers together into a
//! per-runtime [`PlatformBundle`](platynui_core::platform::PlatformBundle).
//!
//! Rather than registering process-global device singletons over a shared
//! `OnceLock` connection, the crate exposes [`create_x11_bundle`]: each call
//! resolves the target display, opens one owned [`X11Connection`], and hands
//! every device an `Arc` clone of it. When the runtime drops the bundle, the
//! last `Arc` clone closes the connection and the highlight thread is joined —
//! so a later runtime always reconnects cleanly instead of reusing a
//! torn-down global cell.
//!
//! [`create_x11_bundle`] probes the required extensions eagerly (fail-fast):
//! XTEST (critical, hard error if absent), RANDR and an EWMH window manager
//! (optional, graceful degradation). This surfaces configuration issues at
//! bundle construction rather than lazily when a device is first used.
//!
//! **XInitThreads is NOT needed.** This crate uses `x11rb::RustConnection`
//! (pure Rust, no libX11 C bindings); `RustConnection` is `Send + Sync` and
//! serialises its own requests, so all devices share one connection without an
//! extra `Mutex`.

#[cfg(target_os = "linux")]
pub mod desktop;
#[cfg(target_os = "linux")]
pub mod highlight;
#[cfg(target_os = "linux")]
pub mod keyboard;
#[cfg(target_os = "linux")]
pub mod pointer;
#[cfg(target_os = "linux")]
pub mod screenshot;
#[cfg(target_os = "linux")]
pub mod window_manager;
#[cfg(target_os = "linux")]
mod x11util;

#[cfg(target_os = "linux")]
pub use x11util::X11Connection;

/// Build a per-runtime X11 [`PlatformBundle`](platynui_core::platform::PlatformBundle)
/// for the session named by `config`.
///
/// Resolves the display once (`platform.x11.display` → `$DISPLAY`), opens one
/// [`X11Connection`] shared by every device through an `Arc`, and eagerly
/// probes the extensions the backend depends on. The returned bundle owns the
/// connection: dropping it closes the connection and stops the highlight
/// thread, so the next runtime reconnects from scratch.
///
/// # Errors
///
/// Returns [`PlatformError`](platynui_core::platform::PlatformError) if the
/// display cannot be resolved or connected, or if the critical XTEST extension
/// is unavailable. Missing RANDR or EWMH support only degrades gracefully (a
/// warning), it does not fail the build.
#[cfg(target_os = "linux")]
pub fn create_x11_bundle(
    config: &platynui_core::config::RuntimeConfig,
) -> Result<platynui_core::platform::PlatformBundle, platynui_core::platform::PlatformError> {
    use std::sync::Arc;

    use platynui_core::platform::{
        DesktopInfoProvider, HighlightProvider, KeyboardDevice, PlatformBundle, PlatformError, PointerDevice,
        ScreenshotProvider, WindowManager,
    };
    use tracing::{debug, info, warn};
    use x11rb::protocol::xproto::ConnectionExt as _;

    use crate::x11util::{self, X11Connection};

    // Resolve the display once (config value → $DISPLAY) and open one shared
    // connection that every device in the bundle will hold an `Arc` clone of.
    // (Named `disp` rather than `display` because the `tracing` macros import
    // `tracing::field::display`, which would shadow a local `display`.)
    let disp = x11util::resolve_display(x11util::configured_display(config).as_deref())?;
    let conn = X11Connection::connect(Some(&disp))?;

    // --- XTEST (critical) ---
    // Required for pointer and keyboard injection; fail fast when it is absent.
    let xtest_available =
        conn.conn.query_extension(b"XTEST").ok().and_then(|c| c.reply().ok()).is_some_and(|r| r.present);
    if !xtest_available {
        return Err(PlatformError::CapabilityUnavailable {
            capability: "XTEST extension",
            details: Some("pointer and keyboard injection will not work".into()),
        });
    }
    debug!("XTEST extension available");

    // --- RANDR (optional, graceful degradation) ---
    let randr_available =
        conn.conn.query_extension(b"RANDR").ok().and_then(|c| c.reply().ok()).is_some_and(|r| r.present);
    if randr_available {
        debug!("RANDR extension available");
    } else {
        warn!("RANDR extension not available — monitor enumeration will fall back to root window geometry");
    }

    // --- EWMH window manager (optional, graceful degradation) ---
    match crate::window_manager::check_ewmh_wm_support(&conn) {
        Ok(true) => debug!("EWMH window manager support confirmed"),
        Ok(false) => warn!("EWMH window manager not detected — window management operations may be limited"),
        Err(e) => warn!("EWMH WM detection failed: {e}"),
    }

    // Build the six devices sharing the one connection; highlight owns its own
    // overlay thread (with a separate connection) bound to the same display.
    let bundle = PlatformBundle {
        pointer: Arc::new(crate::pointer::LinuxPointerDevice::new(conn.clone())) as Arc<dyn PointerDevice>,
        keyboard: Arc::new(crate::keyboard::LinuxKeyboardDevice::new(conn.clone())) as Arc<dyn KeyboardDevice>,
        screenshot: Arc::new(crate::screenshot::LinuxScreenshot::new(conn.clone())) as Arc<dyn ScreenshotProvider>,
        highlight: Arc::new(crate::highlight::LinuxHighlightProvider::new(disp.clone())) as Arc<dyn HighlightProvider>,
        window_manager: Arc::new(crate::window_manager::X11EwmhWindowManager::new(conn.clone()))
            as Arc<dyn WindowManager>,
        desktop_info: Arc::new(crate::desktop::LinuxDesktopInfo::new(conn.clone())) as Arc<dyn DesktopInfoProvider>,
        java_classifier: None,
    };

    info!(display = %disp, "Linux X11 platform bundle created");
    Ok(bundle)
}

// Non-Linux targets keep a tiny marker to allow cross-platform builds.
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinuxX11PlatformStub;
