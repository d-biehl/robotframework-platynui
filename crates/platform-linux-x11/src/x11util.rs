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
}

impl X11Connection {
    /// Connect to `display` (falling back to `$DISPLAY` when `None`) and resolve
    /// the root window of its default screen.
    pub fn connect(display: Option<&str>) -> Result<Arc<X11Connection>, PlatformError> {
        let disp = resolve_display(display)?;
        tracing::debug!(display = %disp, "establishing X11 connection");
        let (conn, screen_num) = connect_raw(&disp).map_err(|details| PlatformError::InitializationFailed {
            component: "x11 connection",
            details: Some(details),
        })?;
        let root = conn.setup().roots[screen_num].root;
        tracing::info!(display = %disp, screen = screen_num, root, "X11 connection established");
        Ok(Arc::new(X11Connection { conn, root }))
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

/// The X11 display named by `config` (`platform.x11.display`), if any. The
/// caller falls back to the environment via [`resolve_display`].
pub fn configured_display(config: &RuntimeConfig) -> Option<String> {
    config.platform("x11").and_then(|x11| x11.get_str("display")).map(str::to_owned)
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

    let timeout = Duration::from_millis(500);
    match rx.recv_timeout(timeout) {
        Ok(res) => res,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            tracing::warn!(display = disp_name, timeout_ms = timeout.as_millis() as u64, "X11 connect timed out");
            Err("x11 connect timed out".to_string())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("x11 connect worker exited".to_string()),
    }
}
