//! X11 based platform integration for PlatynUI on Unix systems.
//!
//! This crate wires Linux/X11 specific devices (pointer, keyboard, screenshot,
//! highlight) and desktop info helpers into the runtime via the registration
//! macros provided by `platynui-core`.
//!
//! Phase 0 provides a minimal `PlatformModule` so the crate can be linked by
//! applications without side effects. Devices will be registered in later
//! phases once their implementations are ready.

#[cfg(target_os = "linux")]
mod init {
    use platynui_core::platform::{PlatformError, PlatformModule};
    use platynui_core::register_platform_module;

    struct LinuxX11Module;

    impl PlatformModule for LinuxX11Module {
        fn name(&self) -> &'static str {
            "Linux X11 Platform"
        }

        fn initialize(&self) -> Result<(), PlatformError> {
            // Phase 0: No side effects yet. Future phases will set up X11/XKB/XTest
            // connections, probe extensions (RandR, MIT-SHM), and prepare global
            // state required by device providers.
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
mod x11util;

// Non-Linux targets keep a tiny marker to allow cross-platform builds.
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinuxX11PlatformStub;
