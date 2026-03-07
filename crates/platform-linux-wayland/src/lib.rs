//! Wayland-based platform integration for `PlatynUI` on Linux.
//!
//! This crate wires Linux/Wayland specific devices (pointer, keyboard,
//! screenshot, highlight) and desktop info helpers into the runtime via
//! the registration macros provided by `platynui-core`.
//!
//! Input injection uses two backends:
//! - **libei** (via `reis`) for Mutter (GNOME) and `KWin` (KDE)
//! - **wlr virtual pointer/keyboard protocols** for wlroots-based compositors
//!
//! The `PlatformModule::initialize()` connects to the Wayland display, probes
//! `wl_registry` for available protocols, and detects input injection capability.
//! EIS (libei) availability is checked via `LIBEI_SOCKET` or the `RemoteDesktop`
//! portal.

// Dependencies used by stub modules pending full implementation.
#[cfg(target_os = "linux")]
use tiny_skia as _;
#[cfg(target_os = "linux")]
use wayland_backend as _;
#[cfg(target_os = "linux")]
use wayland_protocols as _;
#[cfg(target_os = "linux")]
use wayland_protocols_wlr as _;
#[cfg(target_os = "linux")]
use xkbcommon as _;

#[cfg(target_os = "linux")]
mod init {
    use platynui_core::platform::{PlatformError, PlatformModule};
    use platynui_core::register_platform_module;
    use tracing::{debug, info, warn};

    struct WaylandPlatformModule;

    impl PlatformModule for WaylandPlatformModule {
        fn name(&self) -> &'static str {
            "Linux Wayland Platform"
        }

        fn initialize(&self) -> Result<(), PlatformError> {
            // Establish the shared Wayland connection and probe available protocols.
            let guard = crate::wayland_util::connection()?;
            let caps = &guard.capabilities;

            // --- Input injection capability ---
            let has_libei = std::env::var("LIBEI_SOCKET").is_ok();
            let has_wlr_input = caps.wlr_virtual_pointer || caps.wlr_virtual_keyboard;

            if has_libei {
                debug!("libei input path available (LIBEI_SOCKET set)");
            }
            if caps.wlr_virtual_pointer {
                debug!("wlr virtual pointer protocol available");
            }
            if caps.wlr_virtual_keyboard {
                debug!("wlr virtual keyboard protocol available");
            }
            if !has_libei && !has_wlr_input {
                warn!("no input injection mechanism detected — pointer/keyboard operations will fail");
            }

            // --- Window management ---
            if caps.wlr_foreign_toplevel {
                debug!("wlr-foreign-toplevel-management available");
            }
            if caps.ext_foreign_toplevel_list {
                debug!("ext-foreign-toplevel-list available");
            }
            if !caps.wlr_foreign_toplevel && !caps.ext_foreign_toplevel_list {
                warn!("no toplevel management protocol — window operations will be limited");
            }

            // --- Screenshot ---
            if caps.ext_image_copy_capture {
                debug!("ext-image-copy-capture available");
            } else if caps.wlr_screencopy {
                debug!("wlr-screencopy available (legacy fallback)");
            } else {
                warn!("no screenshot protocol — will attempt portal fallback");
            }

            // --- Highlight overlay ---
            if caps.wlr_layer_shell {
                debug!("wlr-layer-shell available for highlight overlays");
            } else if caps.ext_layer_shell {
                debug!("ext-layer-shell available for highlight overlays");
            } else {
                warn!("no layer-shell protocol — highlight overlays not available");
            }

            // --- Desktop info ---
            if caps.output_count > 0 {
                debug!(outputs = caps.output_count, "wl_output monitors detected");
            } else {
                warn!("no wl_output globals — monitor info will be fallback");
            }

            info!("Linux Wayland platform initialized");
            Ok(())
        }

        fn shutdown(&self) {
            info!("Linux Wayland platform shutting down");
            crate::highlight::shutdown_highlight();
            crate::wayland_util::shutdown_connection();
        }
    }

    static MODULE: WaylandPlatformModule = WaylandPlatformModule;

    register_platform_module!(&MODULE);
}

#[cfg(target_os = "linux")]
mod coordinates;
#[cfg(target_os = "linux")]
mod desktop;
#[cfg(target_os = "linux")]
mod eis;
#[cfg(target_os = "linux")]
mod highlight;
#[cfg(target_os = "linux")]
mod keyboard;
#[cfg(target_os = "linux")]
mod pointer;
#[cfg(target_os = "linux")]
mod screenshot;
#[cfg(target_os = "linux")]
mod session_detect;
#[cfg(target_os = "linux")]
mod wayland_util;
#[cfg(target_os = "linux")]
mod window_manager;

// Non-Linux targets keep a tiny marker to allow cross-platform builds.
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinuxWaylandPlatformStub;
