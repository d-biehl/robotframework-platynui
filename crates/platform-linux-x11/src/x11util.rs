use platynui_core::platform::{PlatformError, PlatformErrorKind};
use std::env;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::mpsc;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::Window;
use x11rb::rust_connection::RustConnection;

pub struct X11Handle {
    pub conn: RustConnection,
    pub root: Window,
}

static X11: OnceLock<Mutex<Option<X11Handle>>> = OnceLock::new();

pub fn connection() -> Result<X11Guard, PlatformError> {
    let disp = env::var("DISPLAY")
        .map_err(|_| PlatformError::new(PlatformErrorKind::UnsupportedPlatform, "X11 DISPLAY not set"))?;

    let cell = X11.get_or_init(|| {
        tracing::debug!(display = %disp, "establishing X11 connection");
        match connect_raw(&disp) {
            Ok((conn, screen_num)) => {
                let root = conn.setup().roots[screen_num].root;
                tracing::info!(display = %disp, screen = screen_num, root, "X11 connection established");
                Mutex::new(Some(X11Handle { conn, root }))
            }
            Err(err) => {
                tracing::error!(display = %disp, %err, "X11 connection failed");
                Mutex::new(None)
            }
        }
    });

    let guard =
        cell.lock().map_err(|_| PlatformError::new(PlatformErrorKind::InitializationFailed, "x11 mutex poisoned"))?;

    if guard.is_none() {
        return Err(PlatformError::new(
            PlatformErrorKind::InitializationFailed,
            "X11 connection not available (shutdown or failed to connect)",
        ));
    }

    Ok(X11Guard(guard))
}

/// RAII guard that dereferences to [`X11Handle`].  Returned by [`connection()`].
pub struct X11Guard(std::sync::MutexGuard<'static, Option<X11Handle>>);

impl std::ops::Deref for X11Guard {
    type Target = X11Handle;
    fn deref(&self) -> &X11Handle {
        // SAFETY: `connection()` only returns `X11Guard` when the `Option` is `Some`.
        self.0.as_ref().expect("X11Guard created with None")
    }
}

impl std::ops::DerefMut for X11Guard {
    fn deref_mut(&mut self) -> &mut X11Handle {
        self.0.as_mut().expect("X11Guard created with None")
    }
}

/// Drops the shared X11 connection, closing the file descriptor to the
/// X display server.  Subsequent calls to [`connection()`] will return an
/// error.
pub fn shutdown_connection() {
    if let Some(cell) = X11.get()
        && let Ok(mut guard) = cell.lock()
        && let Some(handle) = guard.take()
    {
        tracing::debug!("X11 connection closed");
        drop(handle);
    }
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
