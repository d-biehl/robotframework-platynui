//! Windows platform backend factory.
//!
//! Exposes [`create_windows_bundle`] and a registered [`PlatformFactory`] so a
//! runtime on Windows builds its own [`PlatformBundle`] of native devices
//! instead of leasing process-global singletons. Each call runs the one-time
//! DPI-awareness init (memoised process-wide) and then constructs a fresh set of
//! devices; dropping the bundle releases them — notably the highlight provider
//! joins its overlay thread (see the `highlight` module).
//!
//! Windows has no display-session ambiguity the way Linux does (X11 vs Wayland),
//! so the factory serves whenever the runtime has not explicitly selected a
//! different backend.

use std::sync::Arc;

use platynui_core::config::RuntimeConfig;
use platynui_core::platform::{
    DesktopInfoProvider, HighlightProvider, KeyboardDevice, PlatformBundle, PlatformError, PlatformFactory,
    PointerDevice, ScreenshotProvider, WindowManager,
};
use platynui_core::register_platform_factory;

use crate::desktop::WindowsDesktopProvider;
use crate::highlight::WindowsHighlightProvider;
use crate::java::WindowsJavaClassifier;
use crate::keyboard::WindowsKeyboardDevice;
use crate::pointer::WindowsPointerDevice;
use crate::screenshot::WindowsScreenshotProvider;
use crate::window_manager::Win32WindowManager;

/// Windows platform backend (id `"windows"`).
struct WindowsPlatformFactory;

impl PlatformFactory for WindowsPlatformFactory {
    fn id(&self) -> &'static str {
        "windows"
    }

    fn can_serve(&self, config: &RuntimeConfig) -> bool {
        // No session ambiguity on Windows: serve unless another backend is
        // explicitly requested.
        matches!(config.platform_backend(), None | Some("windows"))
    }

    fn create(&self, config: &RuntimeConfig) -> Result<PlatformBundle, PlatformError> {
        create_windows_bundle(config)
    }
}

static WINDOWS_PLATFORM_FACTORY: WindowsPlatformFactory = WindowsPlatformFactory;
register_platform_factory!(&WINDOWS_PLATFORM_FACTORY);

/// Build a per-runtime Windows [`PlatformBundle`].
///
/// Ensures the process DPI-awareness context is set (a genuine once-per-process
/// Win32 operation, memoised in the `init` module), then constructs the six
/// native devices. The returned bundle owns them: dropping it releases the devices and
/// joins the highlight overlay thread, so a later runtime starts from a clean
/// slate.
///
/// Windows reads nothing from `config` today — every device talks to the local
/// Win32 session directly — but the parameter is kept for parity with the other
/// backends' `create_*_bundle` functions and future selectors.
///
/// # Errors
///
/// Returns [`PlatformError`] if setting the process DPI-awareness context fails.
pub fn create_windows_bundle(_config: &RuntimeConfig) -> Result<PlatformBundle, PlatformError> {
    crate::init::ensure_dpi_awareness()?;

    let bundle = PlatformBundle {
        pointer: Arc::new(WindowsPointerDevice) as Arc<dyn PointerDevice>,
        keyboard: Arc::new(WindowsKeyboardDevice) as Arc<dyn KeyboardDevice>,
        screenshot: Arc::new(WindowsScreenshotProvider) as Arc<dyn ScreenshotProvider>,
        highlight: Arc::new(WindowsHighlightProvider::new()) as Arc<dyn HighlightProvider>,
        window_manager: Arc::new(Win32WindowManager) as Arc<dyn WindowManager>,
        desktop_info: Arc::new(WindowsDesktopProvider) as Arc<dyn DesktopInfoProvider>,
        java_classifier: Some(Arc::new(WindowsJavaClassifier::new())),
    };

    tracing::info!("Windows platform bundle created");
    Ok(bundle)
}
