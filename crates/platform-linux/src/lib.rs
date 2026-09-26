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
            // Checked first, so its warnings appear even when the build fails.
            if let Some(settings) = config.platform(self.id()) {
                for key in settings.unknown_keys(WAYLAND_KNOWN_KEYS) {
                    tracing::warn!(component = WAYLAND_COMPONENT, key = %key, "unknown setting; it is ignored");
                }
            }
            platynui_platform_linux_wayland::create_wayland_bundle(config)
        }
    }

    /// The Wayland backend's settings as diagnostics name them.
    const WAYLAND_COMPONENT: &str = "platform.wayland";

    /// The settings the Wayland backend reads: none.
    const WAYLAND_KNOWN_KEYS: &[&str] = &[];

    static WAYLAND_FACTORY: WaylandFactory = WaylandFactory;
    register_platform_factory!(&WAYLAND_FACTORY);

    /// Each factory checks its settings before it builds anything. A build
    /// that succeeds would reach the host's display server, so these tests
    /// run in a child process whose environment names no session, where every
    /// build fails after the check.
    #[cfg(test)]
    mod tests {
        use super::*;
        use platynui_core::config::ConfigMap;
        use std::process::Command;
        use std::sync::{Arc, Mutex};

        /// Set in the child process that runs a test without a display session.
        const WITHOUT_SESSION: &str = "PLATYNUI_TEST_WITHOUT_SESSION";

        /// Runs the test `name` of this module again in a child process
        /// without `DISPLAY`, `WAYLAND_DISPLAY`, `WAYLAND_SOCKET` and
        /// `XDG_RUNTIME_DIR`, and asserts that it passed there. Returns `true`
        /// in the parent, which has nothing more to do, and `false` in the
        /// child, which runs the test body.
        fn rerun_without_session(name: &str) -> bool {
            if std::env::var_os(WITHOUT_SESSION).is_some() {
                return false;
            }
            let module = module_path!().split_once("::").map_or(module_path!(), |(_, path)| path);
            let output = Command::new(std::env::current_exe().expect("test binary"))
                .args([&format!("{module}::{name}"), "--exact", "--nocapture", "--test-threads=1"])
                .env(WITHOUT_SESSION, "1")
                .env_remove("DISPLAY")
                .env_remove("WAYLAND_DISPLAY")
                .env_remove("WAYLAND_SOCKET")
                .env_remove("XDG_RUNTIME_DIR")
                .output()
                .expect("run the test binary");
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(output.status.success(), "{stdout}{stderr}");
            assert!(stdout.contains("1 passed"), "the child ran the test: {stdout}{stderr}");
            true
        }

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

        fn platform(id: &str, settings: ConfigMap) -> RuntimeConfig {
            RuntimeConfig::new(ConfigMap::new().with("backend", id).with(id, settings), ConfigMap::new())
        }

        #[test]
        fn x11_warns_for_a_mistyped_display_although_the_build_then_fails() {
            if rerun_without_session("x11_warns_for_a_mistyped_display_although_the_build_then_fails") {
                return;
            }
            let config = platform("x11", ConfigMap::new().with("display", 1_i64));

            let (result, warnings) = warnings(|| X11_FACTORY.create(&config));

            assert!(result.is_err(), "no X server is reachable without DISPLAY");
            assert_eq!(warnings.len(), 1, "{warnings:?}");
            for expected in ["component=\"platform.x11\"", "key=display", "expected=\"string\"", "found=\"integer\""] {
                assert!(warnings[0].contains(expected), "missing {expected}: {warnings:?}");
            }
        }

        #[test]
        fn x11_warns_once_for_an_unknown_setting() {
            if rerun_without_session("x11_warns_once_for_an_unknown_setting") {
                return;
            }
            let config = platform("x11", ConfigMap::new().with("bogus", 1_i64));

            let (result, warnings) = warnings(|| X11_FACTORY.create(&config));

            assert!(result.is_err(), "no X server is reachable without DISPLAY");
            assert_eq!(warnings.len(), 1, "{warnings:?}");
            assert!(warnings[0].contains("component=\"platform.x11\"") && warnings[0].contains("key=bogus"));
        }

        #[test]
        fn wayland_warns_once_for_an_unknown_setting() {
            if rerun_without_session("wayland_warns_once_for_an_unknown_setting") {
                return;
            }
            let config = platform("wayland", ConfigMap::new().with("bogus", 1_i64));

            let (result, warnings) = warnings(|| WAYLAND_FACTORY.create(&config));

            assert!(result.is_err(), "no compositor is reachable without WAYLAND_DISPLAY and XDG_RUNTIME_DIR");
            assert_eq!(warnings.len(), 1, "{warnings:?}");
            assert!(warnings[0].contains("component=\"platform.wayland\"") && warnings[0].contains("key=bogus"));
        }
    }
}

// Non-Linux targets keep a tiny marker to allow cross-platform builds.
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinuxPlatformStub;
