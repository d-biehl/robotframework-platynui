use platynui_core::config::RuntimeConfig;
use platynui_core::platform::PlatformError;
use std::env;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::Window;
use x11rb::rust_connection::RustConnection;

/// An owned X11 connection bound to one display.
///
/// Held by the devices of a single runtime through an `Arc` (each device keeps
/// a clone); when the last clone drops, the connection — and its file
/// descriptor to the X server — is closed. This replaces the former process-
/// global `OnceLock` connection, so a new runtime always establishes a fresh
/// connection and teardown never leaves a cell that cannot be rebuilt.
///
/// `RustConnection` is `Send + Sync` and serialises its own requests, so the
/// devices share it directly without an extra `Mutex`.
pub struct X11Connection {
    pub conn: RustConnection,
    pub root: Window,
    /// The display name this connection was opened on, for diagnostics.
    pub display: String,
}

impl X11Connection {
    /// Connect to `display` (falling back to `$DISPLAY` when `None`) and resolve
    /// the root window of its default screen.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError::UnsupportedPlatform`] when `display` is `None`
    /// and `DISPLAY` is not set, and [`PlatformError::InitializationFailed`]
    /// when the connection fails or does not complete within the connect
    /// timeout.
    pub fn connect(display: Option<&str>) -> Result<Arc<X11Connection>, PlatformError> {
        let disp = resolve_display(display)?;
        tracing::debug!(display = %disp, "establishing X11 connection");
        let (conn, screen_num) = connect_raw(&disp).map_err(|details| PlatformError::InitializationFailed {
            component: "x11 connection",
            details: Some(details),
        })?;
        let root = conn.setup().roots[screen_num].root;
        tracing::info!(display = %disp, screen = screen_num, root, "X11 connection established");
        Ok(Arc::new(X11Connection { conn, root, display: disp }))
    }
}

/// Resolve the X11 display name for a runtime: the `platform.x11.display`
/// config value if present, else the `DISPLAY` environment variable.
pub fn resolve_display(display: Option<&str>) -> Result<String, PlatformError> {
    if let Some(disp) = display {
        return Ok(disp.to_owned());
    }
    env::var("DISPLAY")
        .map_err(|_| PlatformError::UnsupportedPlatform { platform: "X11", details: Some("DISPLAY is not set".into()) })
}

/// The X11 backend's settings as diagnostics name them.
const COMPONENT: &str = "platform.x11";

/// `platform.x11.display`: the display to connect to instead of `$DISPLAY`.
const DISPLAY_KEY: &str = "display";

/// The settings the X11 backend reads.
const KNOWN_KEYS: &[&str] = &[DISPLAY_KEY];

/// Checks the X11 backend's settings and returns the display they name, if
/// any. The caller falls back to the environment via [`resolve_display`].
///
/// This is the first step of building the backend: it warns for each unknown
/// key and for a `display` that is not a string, which then falls back to the
/// environment, before anything can fail.
pub fn configured_display(config: &RuntimeConfig) -> Option<String> {
    let settings = config.platform("x11")?;
    for key in settings.unknown_keys(KNOWN_KEYS) {
        tracing::warn!(component = COMPONENT, key = %key, "unknown setting; it is ignored");
    }
    match settings.try_str(DISPLAY_KEY) {
        Ok(display) => display.map(str::to_owned),
        Err(mismatch) => {
            tracing::warn!(
                component = COMPONENT,
                key = %mismatch.key,
                expected = mismatch.expected,
                found = mismatch.found,
                "setting has the wrong type; the default applies"
            );
            None
        }
    }
}

/// Open a raw `RustConnection` to `disp_name`, bounding the connect attempt with
/// a timeout so a dead or firewalled display cannot hang startup.
pub fn connect_raw(disp_name: &str) -> Result<(RustConnection, usize), String> {
    let (tx, rx) = mpsc::channel();
    let disp = disp_name.to_owned();
    std::thread::spawn(move || {
        let res = x11rb::connect(Some(&disp)).map_err(|e| format!("x11 connect: {e}"));
        let _ = tx.send(res);
    });

    let timeout_ms: u64 = 500;
    let timeout = Duration::from_millis(timeout_ms);
    match rx.recv_timeout(timeout) {
        Ok(res) => res,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            tracing::warn!(display = disp_name, timeout_ms, "X11 connect timed out");
            Err("x11 connect timed out".to_string())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("x11 connect worker exited".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::config::ConfigMap;
    use std::sync::Mutex;

    /// Runs `f` and returns its result and the warnings it logged on this thread.
    fn warnings<R>(f: impl FnOnce() -> R) -> (R, Vec<String>) {
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

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = Captured(Arc::clone(&buffer));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .without_time()
            .with_writer(move || writer.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let log = String::from_utf8(buffer.lock().expect("log buffer").clone()).expect("utf-8 log");
        (result, log.lines().filter(|line| line.trim_start().starts_with("WARN")).map(str::to_owned).collect())
    }

    fn x11(settings: ConfigMap) -> RuntimeConfig {
        RuntimeConfig::new(ConfigMap::new().with("x11", settings), ConfigMap::new())
    }

    #[test]
    fn a_display_setting_is_read() {
        let (display, warnings) = warnings(|| configured_display(&x11(ConfigMap::new().with("display", ":7"))));
        assert_eq!(display.as_deref(), Some(":7"));
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn an_unknown_setting_warns_once() {
        let (display, warnings) = warnings(|| configured_display(&x11(ConfigMap::new().with("dispaly", ":1"))));
        assert_eq!(display, None, "the environment applies");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("component=\"platform.x11\"") && warnings[0].contains("key=dispaly"));
    }

    #[test]
    fn a_display_of_the_wrong_type_warns_and_the_environment_applies() {
        let (display, warnings) = warnings(|| configured_display(&x11(ConfigMap::new().with("display", 1_i64))));
        assert_eq!(display, None);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        for expected in ["component=\"platform.x11\"", "key=display", "expected=\"string\"", "found=\"integer\""] {
            assert!(warnings[0].contains(expected), "missing {expected}: {warnings:?}");
        }
    }
}
