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
        PLATFORM_ID
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
/// Windows reads no setting from `config` today — every device talks to the
/// local Win32 session directly — so `platform.windows` is only checked: a key
/// in it is warned about as unknown, before anything can fail.
///
/// # Errors
///
/// Returns [`PlatformError`] if setting the process DPI-awareness context fails.
pub fn create_windows_bundle(config: &RuntimeConfig) -> Result<PlatformBundle, PlatformError> {
    check_settings(config);
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

/// The id of this backend, which is also its settings section (`platform.windows`).
const PLATFORM_ID: &str = "windows";

/// The settings `platform.windows` reads: none yet.
const KNOWN_SETTINGS: &[&str] = &[];

/// Warns for every key in `platform.windows`, since the backend reads none.
fn check_settings(config: &RuntimeConfig) {
    let Some(settings) = config.platform(PLATFORM_ID) else {
        return;
    };
    for key in settings.unknown_keys(KNOWN_SETTINGS) {
        tracing::warn!(component = "platform.windows", key = %key, "unknown setting; it is ignored");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::config::ConfigMap;
    use std::sync::Mutex;

    /// Runs `f` and returns what it logged, one line per record.
    fn logged<R>(f: impl FnOnce() -> R) -> (R, String) {
        #[derive(Clone)]
        struct Captured(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Captured {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().expect("log buffer").extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Captured(Arc::clone(&buffer));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .without_time()
            .with_writer(move || writer.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let log = String::from_utf8(buffer.lock().expect("log buffer").clone()).expect("utf-8 log");
        (result, log)
    }

    #[test]
    fn an_unknown_setting_is_reported_once() {
        let platform = ConfigMap::new().with(PLATFORM_ID, ConfigMap::new().with("bogus", 1_i64));
        let config = RuntimeConfig::new(platform, ConfigMap::new());

        // The outcome of the build does not matter: the check runs before anything can fail.
        let (_bundle, log) = logged(|| WINDOWS_PLATFORM_FACTORY.create(&config));

        let warnings: Vec<&str> = log.lines().filter(|line| line.contains(" WARN ")).collect();
        assert_eq!(warnings.len(), 1, "{log}");
        assert!(warnings[0].contains("platform.windows") && warnings[0].contains("key=bogus"), "{log}");
    }

    #[test]
    fn no_settings_report_nothing() {
        let (_bundle, log) = logged(|| WINDOWS_PLATFORM_FACTORY.create(&RuntimeConfig::default()));
        assert!(!log.contains(" WARN "), "{log}");
    }
}
