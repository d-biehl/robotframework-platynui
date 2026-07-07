//! Per-runtime platform backend factories.
//!
//! A [`PlatformFactory`] is the platform-layer analogue of
//! [`UiTreeProviderFactory`](crate::provider::UiTreeProviderFactory): it builds
//! a fresh [`PlatformBundle`] of devices for one runtime, bound to the session
//! described by the runtime's [`RuntimeConfig`]. This replaces the process-
//! global `&'static` device singletons + reference-counted lease model, so a
//! runtime owns its platform connection and releases it on drop.

use std::sync::Arc;

use crate::config::RuntimeConfig;

use super::{
    DesktopInfoProvider, HighlightProvider, KeyboardDevice, PlatformError, PointerDevice, ScreenshotProvider,
    WindowManager,
};

/// The owned set of platform devices a [`PlatformFactory`] produces for one runtime.
///
/// A runtime owns exactly one bundle and drops it on shutdown; the devices
/// share their session's connection internally (each holds an `Arc` to it), so
/// dropping the bundle closes that connection and stops any per-session
/// threads. Trait objects are `Arc`, not `Box`, so a device can also be shared
/// with a runtime-side engine (notably the pointer engine) without a
/// self-referential borrow on the runtime.
pub struct PlatformBundle {
    pub pointer: Arc<dyn PointerDevice>,
    pub keyboard: Arc<dyn KeyboardDevice>,
    pub screenshot: Arc<dyn ScreenshotProvider>,
    pub highlight: Arc<dyn HighlightProvider>,
    pub window_manager: Arc<dyn WindowManager>,
    pub desktop_info: Arc<dyn DesktopInfoProvider>,
}

/// Factory that builds a per-runtime [`PlatformBundle`] for one session.
///
/// Registered via [`register_platform_factory!`](crate::register_platform_factory)
/// and discovered through [`platform_factories`]. The runtime picks a factory by
/// the config's `platform.backend` selector when present, otherwise the first
/// whose [`can_serve`](PlatformFactory::can_serve) accepts the current
/// environment.
pub trait PlatformFactory: Send + Sync {
    /// Stable backend id, e.g. `"x11"`, `"wayland"`, `"windows"`, `"mock"`.
    fn id(&self) -> &'static str;

    /// Whether this backend can serve the session described by `config` and the
    /// current environment (e.g. X11 checks for a reachable `DISPLAY`).
    fn can_serve(&self, config: &RuntimeConfig) -> bool;

    /// Build the platform bundle for this runtime, binding to the session named
    /// by `config` (falling back to the environment for any unset value). On
    /// failure the factory must release anything it already opened.
    fn create(&self, config: &RuntimeConfig) -> Result<PlatformBundle, PlatformError>;
}

/// Inventory registration entry for a [`PlatformFactory`].
pub struct PlatformFactoryRegistration {
    pub factory: &'static dyn PlatformFactory,
}

inventory::collect!(PlatformFactoryRegistration);

/// Iterate every registered platform factory.
pub fn platform_factories() -> impl Iterator<Item = &'static dyn PlatformFactory> {
    inventory::iter::<PlatformFactoryRegistration>.into_iter().map(|entry| entry.factory)
}

/// Register a [`PlatformFactory`] with the inventory so the runtime can discover it.
#[macro_export]
macro_rules! register_platform_factory {
    ($factory:expr) => {
        inventory::submit! {
            $crate::platform::PlatformFactoryRegistration { factory: $factory }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigMap, PLATFORM_BACKEND_KEY, RuntimeConfig};

    struct DummyFactory;

    impl PlatformFactory for DummyFactory {
        fn id(&self) -> &'static str {
            "dummy-platform-factory"
        }

        fn can_serve(&self, config: &RuntimeConfig) -> bool {
            // Serve only when not explicitly asked for a different backend.
            config.platform_backend().is_none_or(|backend| backend == self.id())
        }

        fn create(&self, _config: &RuntimeConfig) -> Result<PlatformBundle, PlatformError> {
            Err(PlatformError::UnsupportedPlatform { platform: "dummy", details: None })
        }
    }

    static DUMMY: DummyFactory = DummyFactory;
    register_platform_factory!(&DUMMY);

    #[test]
    fn registered_factory_is_discoverable() {
        let factories: Vec<_> = platform_factories().collect();
        assert!(factories.iter().any(|factory| factory.id() == "dummy-platform-factory"));
    }

    #[test]
    fn can_serve_honors_backend_selector() {
        let factory = DummyFactory;
        assert!(factory.can_serve(&RuntimeConfig::default()));
        let forced_other = RuntimeConfig::new(ConfigMap::new().with(PLATFORM_BACKEND_KEY, "x11"), ConfigMap::new());
        assert!(!factory.can_serve(&forced_other));
    }
}
