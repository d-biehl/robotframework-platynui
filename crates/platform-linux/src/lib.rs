//! Linux platform mediator for `PlatynUI`.
//!
//! Detects the display session type (X11 or Wayland) and registers a
//! [`PlatformFactory`](platynui_core::platform::PlatformFactory) for each. A
//! runtime selects one — by an explicit `platform.backend` in its config, or by
//! auto-detecting the session — and the chosen factory builds a per-runtime
//! [`PlatformBundle`](platynui_core::platform::PlatformBundle) from the matching
//! sub-platform crate (`platform-linux-x11` / `platform-linux-wayland`), each of
//! which exposes a `create_*_bundle` function. This crate owns only the
//! selection; it no longer registers process-global devices.

#[cfg(target_os = "linux")]
mod session;

#[cfg(target_os = "linux")]
pub use session::{SessionType, session_type};

#[cfg(target_os = "linux")]
mod mediator {
    use crate::session::{SessionType, session_type};
    use platynui_core::config::RuntimeConfig;
    use platynui_core::platform::{PlatformBundle, PlatformError, PlatformFactory};
    use platynui_core::register_platform_factory;

    /// X11 platform backend (id `"x11"`), fully per-runtime.
    struct X11Factory;

    impl PlatformFactory for X11Factory {
        fn id(&self) -> &'static str {
            "x11"
        }

        fn can_serve(&self, config: &RuntimeConfig) -> bool {
            match config.platform_backend() {
                Some(backend) => backend == self.id(),
                None => matches!(session_type(), Ok(SessionType::X11)),
            }
        }

        fn create(&self, config: &RuntimeConfig) -> Result<PlatformBundle, PlatformError> {
            platynui_platform_linux_x11::create_x11_bundle(config)
        }
    }

    static X11_FACTORY: X11Factory = X11Factory;
    register_platform_factory!(&X11_FACTORY);

    /// Wayland platform backend (id `"wayland"`). Still process-global-backed in
    /// this phase (see the change's non-goals); the factory wraps that global
    /// session init.
    struct WaylandFactory;

    impl PlatformFactory for WaylandFactory {
        fn id(&self) -> &'static str {
            "wayland"
        }

        fn can_serve(&self, config: &RuntimeConfig) -> bool {
            match config.platform_backend() {
                Some(backend) => backend == self.id(),
                None => matches!(session_type(), Ok(SessionType::Wayland)),
            }
        }

        fn create(&self, config: &RuntimeConfig) -> Result<PlatformBundle, PlatformError> {
            platynui_platform_linux_wayland::create_wayland_bundle(config)
        }
    }

    static WAYLAND_FACTORY: WaylandFactory = WaylandFactory;
    register_platform_factory!(&WAYLAND_FACTORY);
}

// Non-Linux targets keep a tiny marker to allow cross-platform builds.
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinuxPlatformStub;
