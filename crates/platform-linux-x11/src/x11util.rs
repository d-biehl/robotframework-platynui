use once_cell::sync::OnceCell;
use platynui_core::platform::{PlatformError, PlatformErrorKind};
use std::env;
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::Window;
use x11rb::rust_connection::RustConnection;

pub struct X11Handle {
    pub conn: RustConnection,
    pub root: Window,
}

static X11: OnceCell<Mutex<X11Handle>> = OnceCell::new();

pub fn connection() -> Result<std::sync::MutexGuard<'static, X11Handle>, PlatformError> {
    let disp = env::var("DISPLAY")
        .map_err(|_| PlatformError::new(PlatformErrorKind::UnsupportedPlatform, "X11 DISPLAY not set"))?;

    let cell = X11.get_or_try_init(|| {
        tracing::debug!(display = %disp, "establishing X11 connection");
        let (conn, screen_num) =
            connect_raw(&disp).map_err(|e| PlatformError::new(PlatformErrorKind::InitializationFailed, e))?;
        let root = conn.setup().roots[screen_num].root;
        tracing::info!(display = %disp, screen = screen_num, root, "X11 connection established");
        Ok::<Mutex<X11Handle>, PlatformError>(Mutex::new(X11Handle { conn, root }))
    })?;
    cell.lock().map_err(|_| PlatformError::new(PlatformErrorKind::InitializationFailed, "x11 mutex poisoned"))
}

pub fn root_window_from(handle: &X11Handle) -> Window {
    handle.root
}

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
