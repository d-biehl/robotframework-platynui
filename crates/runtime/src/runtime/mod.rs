mod desktop;
mod error;
mod evaluation;
mod input;
mod window;

#[cfg(test)]
mod test_fixtures;

pub use error::{BringToFrontError, FocusError, KeyboardActionError};

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use platynui_core::config::RuntimeConfig;
use platynui_core::platform::{DesktopInfo, KeyboardProfile, PlatformBundle, PlatformError, platform_factories};
use platynui_core::provider::{
    ProviderError, ProviderEvent, ProviderEventKind, ProviderEventListener, UiTreeProvider, UiTreeProviderFactory,
};
use platynui_core::types::Rect;
use platynui_core::ui::identifiers::TechnologyId;
use platynui_core::ui::{DESKTOP_RUNTIME_ID, RuntimeId};

use crate::pointer::{PointerEngine, PointerProfile, PointerSettings};
use crate::provider::ProviderRegistry;
use crate::provider::event::{ProviderEventDispatcher, ProviderEventSink};

use desktop::DesktopNode;

/// What an action that needs the platform says on a runtime without one.
pub(crate) const NO_PLATFORM_BACKEND: &str =
    "runtime has no platform backend (none could serve this session, or the runtime is shut down)";

/// The error of an action that needs the platform, on a runtime without one.
fn no_platform_backend() -> PlatformError {
    PlatformError::UnsupportedPlatform { platform: NO_PLATFORM_BACKEND, details: None }
}

/// Construction options that change how a runtime reports what it built, not
/// what it builds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeOptions {
    /// The caller has already reported why this runtime can have no platform
    /// backend and no provider, such as a test build of the Python extension
    /// used without its mock backend. The runtime then records a missing
    /// platform backend and a missing provider at debug instead of warning.
    pub missing_backends_reported: bool,
}

/// Central orchestrator that owns provider instances, its per-runtime platform
/// bundle, and the provider event dispatcher.
///
/// The runtime owns its [`PlatformBundle`] and drops it on shutdown, so it shares
/// no platform connection or mutable global with any other runtime.
pub struct Runtime {
    pub(super) registry: ProviderRegistry,
    pub(super) providers: Vec<Arc<dyn UiTreeProvider>>,
    pub(super) dispatcher: Arc<ProviderEventDispatcher>,
    config: RuntimeConfig,
    platform: Option<PlatformBundle>,
    desktop: Arc<DesktopNode>,
    pub(super) pointer_engine: Mutex<Option<PointerEngine<'static>>>,
    pub(super) xpath_cache: Mutex<crate::xpath::XdmCache>,
    pub(super) pointer_settings: Mutex<PointerSettings>,
    pub(super) pointer_profile: Mutex<PointerProfile>,
    pub(super) keyboard_profile: Mutex<KeyboardProfile>,
    pub(super) is_shutdown: AtomicBool,
}

// ProviderRuntimeState removed: DesktopNode streams children on-demand.

struct RuntimeEventListener {
    dispatcher: Arc<ProviderEventDispatcher>,
    // no state tracking required; events are forwarded directly
}

impl RuntimeEventListener {
    fn new(dispatcher: Arc<ProviderEventDispatcher>) -> Self {
        Self { dispatcher }
    }
}

impl ProviderEventListener for RuntimeEventListener {
    fn on_event(&self, event: ProviderEvent) {
        if let ProviderEventKind::NodeUpdated { node } = &event.kind {
            node.invalidate();
        }
        self.dispatcher.on_event(event);
    }
}

impl Runtime {
    /// Discovers all registered providers, instantiates them and prepares the event pipeline.
    ///
    /// Uses an empty [`RuntimeConfig`], so every backend falls back to the
    /// environment — today's behaviour.
    ///
    /// # Errors
    ///
    /// Returns the [`ProviderError`] of the first provider that fails to instantiate or to
    /// subscribe to events, and [`ProviderError::InitializationFailed`] if the platform backend
    /// cannot be selected or built, or its desktop information cannot be read.
    pub fn new() -> Result<Self, ProviderError> {
        let registry = ProviderRegistry::discover();
        Self::from_registry(registry, RuntimeConfig::default(), RuntimeOptions::default())
    }

    /// Builds a Runtime that only includes providers with the given `ids`.
    /// This is useful for tests to restrict the active providers deterministically.
    ///
    /// # Errors
    ///
    /// Returns the [`ProviderError`] of the first provider that fails to instantiate or to
    /// subscribe to events, and [`ProviderError::InitializationFailed`] if the platform backend
    /// cannot be selected or built, or its desktop information cannot be read.
    pub fn new_with_provider_ids(ids: &[&str]) -> Result<Self, ProviderError> {
        let registry = ProviderRegistry::discover().filter_by_ids(ids);
        Self::from_registry(registry, RuntimeConfig::default(), RuntimeOptions::default())
    }

    /// Builds a Runtime from an explicit list of provider factories.
    /// No inventory discovery is performed.
    ///
    /// # Errors
    ///
    /// Returns the [`ProviderError`] of the first provider that fails to instantiate or to
    /// subscribe to events, and [`ProviderError::InitializationFailed`] if the platform backend
    /// cannot be selected or built, or its desktop information cannot be read.
    pub fn new_with_factories(factories: &[&'static dyn UiTreeProviderFactory]) -> Result<Self, ProviderError> {
        let registry = ProviderRegistry::with_factories(factories);
        Self::from_registry(registry, RuntimeConfig::default(), RuntimeOptions::default())
    }

    /// Discovers all registered providers and binds the runtime to the session
    /// described by `config` (platform backend selection + per-component settings).
    ///
    /// # Errors
    ///
    /// Returns the [`ProviderError`] of the first provider that fails to instantiate or to
    /// subscribe to events, and [`ProviderError::InitializationFailed`] if the platform backend
    /// cannot be selected or built, or its desktop information cannot be read.
    pub fn new_with_config(config: RuntimeConfig) -> Result<Self, ProviderError> {
        Self::new_with_config_and_options(config, RuntimeOptions::default())
    }

    /// [`new_with_config`](Self::new_with_config) with construction `options`.
    ///
    /// # Errors
    ///
    /// Returns the [`ProviderError`] of the first provider that fails to instantiate or to
    /// subscribe to events, and [`ProviderError::InitializationFailed`] if the platform backend
    /// cannot be selected or built, or its desktop information cannot be read.
    pub fn new_with_config_and_options(config: RuntimeConfig, options: RuntimeOptions) -> Result<Self, ProviderError> {
        let registry = ProviderRegistry::discover();
        Self::from_registry(registry, config, options)
    }

    /// Builds a Runtime from an explicit list of provider factories bound to the
    /// session described by `config`.
    ///
    /// # Errors
    ///
    /// Returns the [`ProviderError`] of the first provider that fails to instantiate or to
    /// subscribe to events, and [`ProviderError::InitializationFailed`] if the platform backend
    /// cannot be selected or built, or its desktop information cannot be read.
    pub fn new_with_factories_and_config(
        factories: &[&'static dyn UiTreeProviderFactory],
        config: RuntimeConfig,
    ) -> Result<Self, ProviderError> {
        let registry = ProviderRegistry::with_factories(factories);
        Self::from_registry(registry, config, RuntimeOptions::default())
    }

    fn from_registry(
        registry: ProviderRegistry,
        config: RuntimeConfig,
        options: RuntimeOptions,
    ) -> Result<Self, ProviderError> {
        // The runtime's own setting, checked before anything can fail.
        let forced_backend = forced_backend(&config);

        let dispatcher = Arc::new(ProviderEventDispatcher::new());
        let provider_instances = registry.instantiate_all(&config)?;
        let mut providers: Vec<Arc<dyn UiTreeProvider>> = Vec::with_capacity(provider_instances.len());
        for provider in provider_instances {
            let listener = Arc::new(RuntimeEventListener::new(dispatcher.clone()));
            provider.subscribe_events(listener)?;
            providers.push(provider);
        }
        if providers.is_empty() {
            if options.missing_backends_reported {
                tracing::debug!("no UI tree provider is active; queries find only the desktop node");
            } else {
                tracing::warn!("no UI tree provider is active; queries find only the desktop node");
            }
        }

        // Select and build the per-runtime platform bundle for this session.
        let (backend, platform) = match select_platform(&config, forced_backend.as_deref(), options)? {
            Some((backend, bundle)) => (Some(backend), Some(bundle)),
            None => (None, None),
        };

        // Thread this session's window manager into every provider so provider
        // nodes target this runtime's session, not a process-global one.
        if let Some(bundle) = &platform {
            for provider in &providers {
                provider.set_window_manager(bundle.window_manager.clone());
                if let Some(classifier) = &bundle.java_classifier {
                    provider.set_java_classifier(Arc::clone(classifier));
                }
            }
        }

        // Desktop info comes from the bundle when a platform is available, else a
        // fallback (headless / provider-only runtimes).
        let desktop = match &platform {
            Some(bundle) => bundle.desktop_info.desktop_info().map_err(|err| map_desktop_error(&err))?,
            None => fallback_desktop_info(),
        };

        let mut pointer_settings = PointerSettings::default();
        if let Some(bundle) = &platform {
            if let Ok(Some(time)) = bundle.pointer.double_click_time() {
                pointer_settings.double_click_time = time;
            }
            if let Ok(Some(size)) = bundle.pointer.double_click_size() {
                pointer_settings.double_click_size = size;
            }
        }
        let pointer_profile = PointerProfile::named_default();
        let keyboard_profile = KeyboardProfile::default();
        let pointer_engine = platform.as_ref().map(|bundle| {
            PointerEngine::new(
                bundle.pointer.clone(),
                desktop.bounds,
                pointer_settings.clone(),
                pointer_profile.clone(),
                &default_sleep,
            )
        });

        let providers_for_desktop: Vec<Arc<dyn UiTreeProvider>> = providers.clone();

        let runtime = Self {
            registry,
            providers,
            dispatcher,
            config,
            platform,
            desktop: {
                let node = DesktopNode::new(desktop, providers_for_desktop);
                DesktopNode::init_self(&node);
                node
            },
            pointer_engine: Mutex::new(pointer_engine),
            xpath_cache: Mutex::new(crate::xpath::XdmCache::new()),
            pointer_settings: Mutex::new(pointer_settings),
            pointer_profile: Mutex::new(pointer_profile),
            keyboard_profile: Mutex::new(keyboard_profile),
            is_shutdown: AtomicBool::new(false),
        };

        runtime.log_unclaimed_settings(backend);
        let provider_ids: Vec<&str> = runtime.providers.iter().map(|provider| provider.descriptor().id).collect();
        let desktop_info = runtime.desktop.info();
        tracing::info!(
            backend = backend.unwrap_or("none"),
            forced = forced_backend.is_some(),
            providers = ?provider_ids,
            desktop = %desktop_info.bounds,
            monitors = desktop_info.monitors.len(),
            "runtime initialized"
        );
        Ok(runtime)
    }

    /// Records the config sections that neither the chosen platform `backend`
    /// nor an active provider claimed. They stay silent above debug, because
    /// a dict may carry every OS's blocks; a component that is built checks
    /// its own section for unknown keys and types.
    fn log_unclaimed_settings(&self, backend: Option<&str>) {
        for id in self.config.platform_component_ids() {
            if backend != Some(id) {
                tracing::debug!(
                    component = %format_args!("platform.{id}"),
                    "settings match no platform backend of this runtime; they are ignored"
                );
            }
        }
        for id in self.config.provider_component_ids() {
            if !self.providers.iter().any(|provider| provider.descriptor().id == id) {
                tracing::debug!(
                    component = %format_args!("providers.{id}"),
                    "settings match no active provider; they are ignored"
                );
            }
        }
    }

    /// Returns a reference to the provider registry (discovered entries including metadata).
    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    /// Returns the instantiated providers in priority order.
    pub fn providers(&self) -> impl Iterator<Item = &Arc<dyn UiTreeProvider>> {
        self.providers.iter()
    }

    /// Returns providers registered for the given technology identifier.
    pub fn providers_for<'a>(
        &'a self,
        technology: &'a TechnologyId,
    ) -> impl Iterator<Item = &'a Arc<dyn UiTreeProvider>> + 'a {
        self.providers.iter().filter(move |p| p.descriptor().technology == *technology)
    }

    /// Access to the shared provider event dispatcher.
    pub fn event_dispatcher(&self) -> Arc<ProviderEventDispatcher> {
        Arc::clone(&self.dispatcher)
    }

    /// Registers a new event sink that will receive provider events.
    pub fn register_event_sink(&self, sink: Arc<dyn ProviderEventSink>) {
        self.dispatcher.register(sink);
    }

    /// Utility mainly for tests to inject provider events.
    pub fn dispatch_event(&self, event: ProviderEvent) {
        self.dispatcher.dispatch(event);
    }

    /// Invokes shutdown on dispatcher and providers, then tears down the platform.
    pub fn shutdown(&mut self) {
        if self.is_shutdown.swap(true, Ordering::AcqRel) {
            return; // already shut down
        }
        tracing::info!(providers = self.providers.len(), "Runtime shutting down");
        self.dispatcher.shutdown();
        for provider in &self.providers {
            provider.shutdown();
        }
        // Tear down the platform deterministically: drop the pointer engine's
        // device clone first, then the bundle. Dropping the bundle releases this
        // runtime's platform connection (e.g. closes the X11 FD and joins the
        // highlight thread) — no shared global to reference-count.
        if let Ok(mut guard) = self.pointer_engine.lock() {
            *guard = None;
        }
        self.platform = None;
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // Ensure providers and dispatcher are shut down exactly once.
        self.shutdown();
    }
}

/// The backend `platform.backend` forces, if any. A value that is not a
/// string is reported, and auto-detection applies.
fn forced_backend(config: &RuntimeConfig) -> Option<String> {
    match config.try_platform_backend() {
        Ok(forced) => forced.map(str::to_owned),
        Err(mismatch) => {
            tracing::warn!(
                component = "platform",
                key = %mismatch.key,
                expected = mismatch.expected,
                found = mismatch.found,
                "setting has the wrong type; the default applies"
            );
            None
        }
    }
}

/// Selects and builds the per-runtime [`PlatformBundle`] for this session, and
/// returns it with the id of the backend that built it.
///
/// When `forced` names a backend (`platform.backend`), that backend must be
/// registered and able to serve the environment, or construction fails. Without
/// a forced backend, the first factory whose `can_serve` accepts the environment
/// wins; if none does, the runtime has no platform (`Ok(None)`) — e.g. a headless
/// provider-only test — and says what that costs.
fn select_platform(
    config: &RuntimeConfig,
    forced: Option<&str>,
    options: RuntimeOptions,
) -> Result<Option<(&'static str, PlatformBundle)>, ProviderError> {
    let factories: Vec<_> = platform_factories().collect();

    if let Some(id) = forced {
        let Some(factory) = factories.iter().find(|factory| factory.id() == id) else {
            return Err(ProviderError::InitializationFailed {
                provider: "runtime",
                details: Some(format!("no platform backend '{id}' is registered")),
            });
        };
        if !factory.can_serve(config) {
            return Err(ProviderError::InitializationFailed {
                provider: "runtime",
                details: Some(format!("platform backend '{id}' cannot serve this environment")),
            });
        }
        let bundle = factory.create(config).map_err(|err| ProviderError::InitializationFailed {
            provider: "runtime",
            details: Some(err.to_string()),
        })?;
        Ok(Some((factory.id(), bundle)))
    } else {
        for factory in &factories {
            if factory.can_serve(config) {
                let bundle = factory.create(config).map_err(|err| ProviderError::InitializationFailed {
                    provider: "runtime",
                    details: Some(err.to_string()),
                })?;
                return Ok(Some((factory.id(), bundle)));
            }
            tracing::debug!(backend = factory.id(), "platform backend cannot serve this session");
        }
        let backends: Vec<&str> = factories.iter().map(|factory| factory.id()).collect();
        if options.missing_backends_reported {
            tracing::debug!(
                backends = ?backends,
                "no platform backend can serve this session; pointer, keyboard, screenshot, highlight and window control are unavailable"
            );
        } else {
            tracing::warn!(
                backends = ?backends,
                "no platform backend can serve this session; pointer, keyboard, screenshot, highlight and window control are unavailable"
            );
        }
        Ok(None)
    }
}

fn map_desktop_error(err: &PlatformError) -> ProviderError {
    ProviderError::InitializationFailed { provider: "desktop", details: Some(err.to_string()) }
}

/// The desktop of a runtime without platform backend, which
/// [`select_platform`] has already reported.
fn fallback_desktop_info() -> DesktopInfo {
    let os_name = std::env::consts::OS;
    let os_version = fallback_os_version();
    DesktopInfo {
        runtime_id: RuntimeId::from(DESKTOP_RUNTIME_ID),
        name: format!("Fallback Desktop ({os_name})"),
        technology: TechnologyId::from("Fallback"),
        bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
        os_name: os_name.into(),
        os_version,
        monitors: Vec::new(),
    }
}

#[cfg(unix)]
fn fallback_os_version() -> String {
    rustix::system::uname().release().to_string_lossy().into_owned()
}

#[cfg(not(unix))]
fn fallback_os_version() -> String {
    String::new()
}

pub(super) fn default_sleep(duration: Duration) {
    if duration.is_zero() {
        return;
    }
    std::thread::sleep(duration);
}

#[cfg(test)]
mod tests {
    use super::test_fixtures::*;
    use super::*;
    use crate::test_support::{logged, records, runtime_with_factories_and_mock_platform as rt_with_pf};
    use platynui_core::config::ConfigMap;
    use platynui_core::platform::{HighlightRequest, ScreenshotRequest};
    use platynui_core::provider::{
        ProviderDescriptor, ProviderEvent, ProviderEventKind, ProviderEventListener, ProviderKind,
        UiTreeProviderFactory,
    };
    use platynui_core::ui::identifiers::TechnologyId;
    use platynui_core::ui::{Namespace, UiNode};
    use platynui_platform_mock as _;
    use platynui_provider_mock as _;
    use rstest::rstest;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, LazyLock};

    // --- Drop counter test infrastructure ---

    static DROP_COUNT: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));

    struct DropCounterProvider {
        desc: &'static ProviderDescriptor,
    }
    impl UiTreeProvider for DropCounterProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            self.desc
        }
        fn get_nodes(
            &self,
            _parent: Arc<dyn UiNode>,
        ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
            Ok(Box::new(std::iter::empty()))
        }
        fn subscribe_events(&self, _listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
            Ok(())
        }
        fn shutdown(&self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }
    struct DropCounterFactory;
    impl DropCounterFactory {
        fn descriptor_static() -> &'static ProviderDescriptor {
            static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
                ProviderDescriptor::new(
                    "runtime-drop-counter",
                    "Runtime Drop Counter",
                    TechnologyId::from("Runtime"),
                    ProviderKind::Native,
                )
            });
            &DESCRIPTOR
        }
    }
    impl UiTreeProviderFactory for DropCounterFactory {
        fn descriptor(&self) -> &ProviderDescriptor {
            Self::descriptor_static()
        }
        fn create(
            &self,
            _config: &platynui_core::config::RuntimeConfig,
        ) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
            Ok(Arc::new(DropCounterProvider { desc: Self::descriptor_static() }))
        }
    }
    static DROP_COUNTER_FACTORY: DropCounterFactory = DropCounterFactory;

    // --- Tests ---

    #[test]
    fn runtime_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Runtime>();
    }

    #[rstest]
    fn runtime_initializes_providers() {
        SHUTDOWN_TRIGGERED.store(false, Ordering::SeqCst);
        SUBSCRIPTION_REGISTERED.store(false, Ordering::SeqCst);

        // Build runtime after resetting flags so subscribe_events sets the flag now
        let runtime = Runtime::new_with_factories(&[&RUNTIME_FACTORY]).expect("runtime initializes");
        let providers: Vec<_> = runtime.providers().collect();
        assert!(!providers.is_empty());
        assert!(providers.iter().any(|provider| provider.descriptor().id == "runtime-stub"));
        assert!(SUBSCRIPTION_REGISTERED.load(Ordering::SeqCst));
    }

    #[rstest]
    fn runtime_dispatcher_forwards_events(rt_runtime_stub: Runtime) {
        let runtime = rt_runtime_stub;
        let sink = Arc::new(RecordingSink::new());
        runtime.register_event_sink(sink.clone());

        runtime.dispatch_event(ProviderEvent { kind: ProviderEventKind::TreeInvalidated });

        let events = sink.events.lock().unwrap();
        assert!(!events.is_empty());
        assert!(matches!(events.last().unwrap(), ProviderEventKind::TreeInvalidated));
    }

    #[rstest]
    fn runtime_filters_providers_by_technology(rt_runtime_stub: Runtime) {
        let runtime = rt_runtime_stub;
        let tech = TechnologyId::from("RuntimeTech");
        let providers: Vec<_> = runtime.providers_for(&tech).collect();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].descriptor().id, "runtime-stub");
    }

    #[rstest]
    fn runtime_shutdown_invokes_provider_shutdown(rt_runtime_stub: Runtime) {
        SHUTDOWN_TRIGGERED.store(false, Ordering::SeqCst);
        let mut runtime = rt_runtime_stub;
        runtime.shutdown();
        assert!(SHUTDOWN_TRIGGERED.load(Ordering::SeqCst));
    }

    #[test]
    fn runtime_drop_triggers_shutdown_once() {
        DROP_COUNT.store(0, Ordering::SeqCst);
        {
            let _rt = Runtime::new_with_factories(&[&DROP_COUNTER_FACTORY]).expect("runtime");
        } // drop here
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn runtime_shutdown_then_drop_is_idempotent() {
        DROP_COUNT.store(0, Ordering::SeqCst);
        {
            let mut rt = Runtime::new_with_factories(&[&DROP_COUNTER_FACTORY]).expect("runtime");
            rt.shutdown();
            assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1, "shutdown should be called once");
        } // drop should not call shutdown again
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1, "drop must be idempotent after shutdown");
    }

    #[rstest]
    fn provider_nodes_link_parent(rt_runtime_stub: Runtime) {
        let runtime = rt_runtime_stub;
        let parent: Arc<dyn UiNode> = Arc::new(StubNode::new("parent"));
        let node = runtime
            .providers()
            .find(|provider| provider.descriptor().id == "runtime-stub")
            .and_then(|provider| provider.get_nodes(Arc::clone(&parent)).ok().and_then(|mut nodes| nodes.next()))
            .expect("runtime stub provider node available");
        assert!(node.parent().is_some());
    }

    #[rstest]
    fn injected_provider_attaches_to_desktop(rt_runtime_stub: Runtime) {
        let runtime = rt_runtime_stub;
        let desktop = runtime.desktop_node();
        let app = runtime
            .providers()
            .find(|provider| provider.descriptor().id == "runtime-stub")
            .and_then(|provider| provider.get_nodes(Arc::clone(&desktop)).ok())
            .and_then(|mut nodes| nodes.next())
            .expect("injected provider root node");

        assert_eq!(app.namespace(), Namespace::Control);
        let parent = app.parent().and_then(|weak| weak.upgrade()).expect("desktop parent");
        assert_eq!(parent.runtime_id().as_str(), runtime.desktop_info().runtime_id.as_str());
    }

    // --- Configuration checks, decisions and the initialization record ---

    fn config(platform: ConfigMap) -> RuntimeConfig {
        RuntimeConfig::new(platform, ConfigMap::new())
    }

    fn build(
        factories: &[&'static dyn UiTreeProviderFactory],
        config: RuntimeConfig,
        options: RuntimeOptions,
    ) -> (Runtime, String) {
        let (runtime, log) =
            logged(|| Runtime::from_registry(ProviderRegistry::with_factories(factories), config, options));
        (runtime.expect("runtime"), log)
    }

    #[test]
    fn an_unknown_setting_of_the_mock_platform_warns_once() {
        let platform = ConfigMap::new().with("backend", "mock").with("mock", ConfigMap::new().with("bogus", 1_i64));
        let (_runtime, log) = build(&[&RUNTIME_FACTORY], config(platform), RuntimeOptions::default());

        let warnings = records(&log, "WARN");
        assert_eq!(warnings.len(), 1, "{log}");
        assert!(warnings[0].contains("platform.mock") && warnings[0].contains("bogus"), "{log}");
    }

    #[test]
    fn a_block_of_a_platform_that_is_not_built_is_recorded_at_debug_only() {
        let platform = ConfigMap::new().with("backend", "mock").with("windows", ConfigMap::new().with("bogus", 1_i64));
        let (_runtime, log) = build(&[&RUNTIME_FACTORY], config(platform), RuntimeOptions::default());

        assert!(records(&log, "WARN").is_empty(), "{log}");
        assert!(records(&log, "DEBUG").iter().any(|line| line.contains("platform.windows")), "{log}");
    }

    #[test]
    fn a_mistyped_backend_selector_warns_and_auto_detection_applies() {
        let platform = ConfigMap::new().with("backend", 1_i64);
        let (runtime, log) = build(&[&RUNTIME_FACTORY], config(platform), RuntimeOptions::default());

        let mismatches: Vec<_> = records(&log, "WARN").into_iter().filter(|line| line.contains("wrong type")).collect();
        assert_eq!(mismatches.len(), 1, "{log}");
        for expected in ["component=\"platform\"", "key=backend", "expected=\"string\"", "found=\"integer\""] {
            assert!(mismatches[0].contains(expected), "missing {expected}: {log}");
        }
        assert!(runtime.platform.is_none(), "no backend serves this test binary without a selector");
    }

    #[test]
    fn initialization_is_one_info_record_naming_backend_providers_and_desktop() {
        let (runtime, log) = build(&[&RUNTIME_FACTORY], config(mock_platform()), RuntimeOptions::default());

        let infos = records(&log, "INFO");
        assert_eq!(infos.len(), 1, "{log}");
        let monitors = format!("monitors={}", runtime.desktop_info().monitors.len());
        let desktop = format!("desktop={}", runtime.desktop_info().bounds);
        for expected in [
            "runtime initialized",
            "backend=\"mock\"",
            "forced=true",
            "providers=[\"runtime-stub\"]",
            desktop.as_str(),
            monitors.as_str(),
        ] {
            assert!(infos[0].contains(expected), "missing {expected}: {log}");
        }
    }

    #[test]
    fn a_runtime_without_provider_warns_that_queries_find_only_the_desktop() {
        let (_runtime, log) = build(&[], config(mock_platform()), RuntimeOptions::default());

        let warnings = records(&log, "WARN");
        assert_eq!(warnings.len(), 1, "{log}");
        assert!(warnings[0].contains("no UI tree provider is active"), "{log}");
        assert!(warnings[0].contains("only the desktop node"), "{log}");
    }

    #[test]
    fn a_session_no_backend_serves_warns_naming_the_candidates_and_what_is_lost() {
        let (runtime, log) = build(&[&RUNTIME_FACTORY], RuntimeConfig::default(), RuntimeOptions::default());

        assert!(runtime.platform.is_none());
        let warnings = records(&log, "WARN");
        assert_eq!(warnings.len(), 1, "{log}");
        assert!(warnings[0].contains("no platform backend can serve this session"), "{log}");
        assert!(warnings[0].contains("pointer, keyboard, screenshot, highlight and window control"), "{log}");
        assert!(warnings[0].contains("\"mock\""), "the candidates are named: {log}");
        let infos = records(&log, "INFO");
        assert!(infos.len() == 1 && infos[0].contains("backend=\"none\""), "{log}");
    }

    #[test]
    fn a_reported_test_build_records_the_missing_backends_at_debug() {
        let options = RuntimeOptions { missing_backends_reported: true };
        let (_runtime, log) = build(&[], RuntimeConfig::default(), options);

        assert!(records(&log, "WARN").is_empty(), "{log}");
        let debug = records(&log, "DEBUG");
        assert!(debug.iter().any(|line| line.contains("no UI tree provider is active")), "{log}");
        assert!(debug.iter().any(|line| line.contains("no platform backend can serve this session")), "{log}");
    }

    fn mock_platform() -> ConfigMap {
        ConfigMap::new().with("backend", "mock")
    }

    // --- A provider that fails to list its elements ---

    static FLAKY_FAILS: AtomicBool = AtomicBool::new(true);

    struct FlakyProvider;
    impl UiTreeProvider for FlakyProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            FlakyFactory::descriptor_static()
        }
        fn get_nodes(
            &self,
            _parent: Arc<dyn UiNode>,
        ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
            if FLAKY_FAILS.load(Ordering::SeqCst) {
                Err(ProviderError::CommunicationFailure { channel: "stub bus", details: Some("bus went away".into()) })
            } else {
                Ok(Box::new(std::iter::empty()))
            }
        }
        fn subscribe_events(&self, _listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
            Ok(())
        }
    }
    struct FlakyFactory;
    impl FlakyFactory {
        fn descriptor_static() -> &'static ProviderDescriptor {
            static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
                ProviderDescriptor::new(
                    "runtime-flaky",
                    "Runtime Flaky",
                    TechnologyId::from("Runtime"),
                    ProviderKind::Native,
                )
            });
            &DESCRIPTOR
        }
    }
    impl UiTreeProviderFactory for FlakyFactory {
        fn descriptor(&self) -> &ProviderDescriptor {
            Self::descriptor_static()
        }
        fn create(&self, _config: &RuntimeConfig) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
            Ok(Arc::new(FlakyProvider))
        }
    }
    static FLAKY_FACTORY: FlakyFactory = FlakyFactory;

    #[test]
    fn a_provider_that_keeps_failing_is_reported_once_per_episode() {
        FLAKY_FAILS.store(true, Ordering::SeqCst);
        let runtime = rt_with_pf(&[&FLAKY_FACTORY]);
        let desktop = runtime.desktop_node();
        let enumerate = || desktop.children().count();

        let ((), log) = logged(|| {
            for _ in 0..10 {
                enumerate();
            }
            FLAKY_FAILS.store(false, Ordering::SeqCst);
            enumerate();
            enumerate();
            FLAKY_FAILS.store(true, Ordering::SeqCst);
            enumerate();
        });

        let errors = records(&log, "ERROR");
        assert_eq!(errors.len(), 2, "one error per episode: {log}");
        for error in &errors {
            assert!(error.contains("provider=\"runtime-flaky\""), "{log}");
            assert!(error.contains("its elements are missing from query results"), "{log}");
            assert!(error.contains("bus went away"), "{log}");
        }
        let debug = records(&log, "DEBUG");
        let continuing = debug.iter().filter(|line| line.contains("still fails")).count();
        assert_eq!(continuing, 9, "{log}");
        let recovered: Vec<_> = debug.iter().filter(|line| line.contains("again")).collect();
        assert_eq!(recovered.len(), 1, "the recovery is recorded once: {log}");
        assert!(recovered[0].contains("runtime-flaky"), "{log}");
    }

    // --- A runtime without a platform backend ---

    #[test]
    fn a_runtime_without_platform_names_the_missing_backend_not_internal_types() {
        let (mut runtime, _) = build(&[&RUNTIME_FACTORY], RuntimeConfig::default(), RuntimeOptions::default());
        let expected = "runtime has no platform backend (none could serve this session, or the runtime is shut down)";

        let check = |runtime: &Runtime| {
            let request = HighlightRequest::new(Rect::new(0.0, 0.0, 10.0, 10.0));
            let messages = [
                runtime.highlight(&request).unwrap_err().to_string(),
                runtime.clear_highlight().unwrap_err().to_string(),
                runtime.screenshot(&ScreenshotRequest::entire_display()).unwrap_err().to_string(),
                runtime.pointer_position().unwrap_err().to_string(),
                runtime.pointer_click(None, None, None).unwrap_err().to_string(),
            ];
            for message in messages {
                assert!(message.contains(expected), "{message}");
                assert!(!message.contains("Provider") && !message.contains("Device"), "{message}");
            }
        };
        check(&runtime);

        let mut mock = rt_with_pf(&[&RUNTIME_FACTORY]);
        mock.shutdown();
        check(&mock);
        runtime.shutdown();
    }
}
