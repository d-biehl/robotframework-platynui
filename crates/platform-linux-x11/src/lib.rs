//! X11 based platform integration for PlatynUI on Unix systems.
//!
//! This crate wires Linux/X11 specific devices (pointer, keyboard, screenshot,
//! highlight) and desktop info helpers into the runtime via the registration
//! macros provided by `platynui-core`.
//!
//! The `PlatformModule::initialize()` eagerly connects to the X11 display,
//! probes required extensions (XTEST, RANDR), and logs their availability.
//! This fail-fast approach surfaces configuration issues at startup rather
//! than lazily when a device is first used.
//!
//! **XInitThreads is NOT needed.** This crate uses `x11rb::RustConnection`
//! (pure Rust, no libX11 C bindings). Thread safety is handled by the
//! `Mutex<X11Handle>` wrapper in `x11util`.

#[cfg(target_os = "linux")]
mod init {
    use platynui_core::platform::{PlatformError, PlatformErrorKind, PlatformModule};
    use platynui_core::register_platform_module;
    use tracing::{debug, info, warn};
    use x11rb::protocol::xproto::ConnectionExt as _;

    struct LinuxX11Module;

    impl PlatformModule for LinuxX11Module {
        fn name(&self) -> &'static str {
            "Linux X11 Platform"
        }

        fn initialize(&self) -> Result<(), PlatformError> {
            // Eagerly establish the shared X11 connection. This populates the
            // OnceLock singleton in x11util so subsequent device calls don't
            // encounter a cold-start surprise.
            let guard = crate::x11util::connection()?;

            // Probe extensions the crate depends on. XTEST is critical for
            // pointer (and future keyboard) input injection. RANDR is used
            // for monitor enumeration but has a root-geometry fallback.
            let conn = &guard.conn;

            // --- XTEST (critical) ---
            let xtest_available =
                conn.query_extension(b"XTEST").ok().and_then(|c| c.reply().ok()).is_some_and(|r| r.present);

            if !xtest_available {
                return Err(PlatformError::new(
                    PlatformErrorKind::CapabilityUnavailable,
                    "XTEST extension not available — pointer/keyboard injection will not work",
                ));
            }
            debug!("XTEST extension available");

            // --- RANDR (optional, graceful degradation) ---
            let randr_available =
                conn.query_extension(b"RANDR").ok().and_then(|c| c.reply().ok()).is_some_and(|r| r.present);

            if randr_available {
                debug!("RANDR extension available");
            } else {
                warn!("RANDR extension not available — monitor enumeration will fall back to root window geometry");
            }

            // --- EWMH window manager (optional, graceful degradation) ---
            // Drop the x11util guard before calling check_ewmh_wm_support so
            // the shared connection is not double-locked.
            drop(guard);
            match crate::window_manager::check_ewmh_wm_support() {
                Ok(true) => debug!("EWMH window manager support confirmed"),
                Ok(false) => warn!("EWMH window manager not detected — window management operations may be limited"),
                Err(e) => warn!("EWMH WM detection failed: {e}"),
            }

            info!("Linux X11 platform initialized");
            Ok(())
        }
    }

    static MODULE: LinuxX11Module = LinuxX11Module;

    // Register the platform module so the runtime can initialize it at startup.
    register_platform_module!(&MODULE);
}

// Phase 1 devices (minimal implementations)
#[cfg(target_os = "linux")]
mod desktop;
#[cfg(target_os = "linux")]
mod highlight;
#[cfg(target_os = "linux")]
mod keyboard;
#[cfg(target_os = "linux")]
mod pointer;
#[cfg(target_os = "linux")]
mod screenshot;
#[cfg(target_os = "linux")]
mod window_manager;
#[cfg(target_os = "linux")]
mod x11util;

// Non-Linux targets keep a tiny marker to allow cross-platform builds.
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinuxX11PlatformStub;
