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
    pub fn new() -> Result<Self, ProviderError> {
        let registry = ProviderRegistry::discover();
        Self::from_registry_with_config(registry, RuntimeConfig::default())
    }

    /// Builds a Runtime that only includes providers with the given `ids`.
    /// This is useful for tests to restrict the active providers deterministically.
    pub fn new_with_provider_ids(ids: &[&str]) -> Result<Self, ProviderError> {
        let registry = ProviderRegistry::discover().filter_by_ids(ids);
        Self::from_registry_with_config(registry, RuntimeConfig::default())
    }

    /// Builds a Runtime from an explicit list of provider factories.
    /// No inventory discovery is performed.
    pub fn new_with_factories(factories: &[&'static dyn UiTreeProviderFactory]) -> Result<Self, ProviderError> {
        let registry = ProviderRegistry::with_factories(factories);
        Self::from_registry_with_config(registry, RuntimeConfig::default())
    }

    /// Discovers all registered providers and binds the runtime to the session
    /// described by `config` (platform backend selection + per-component settings).
    pub fn new_with_config(config: RuntimeConfig) -> Result<Self, ProviderError> {
        let registry = ProviderRegistry::discover();
        Self::from_registry_with_config(registry, config)
    }

    /// Builds a Runtime from an explicit list of provider factories bound to the
    /// session described by `config`.
    pub fn new_with_factories_and_config(
        factories: &[&'static dyn UiTreeProviderFactory],
        config: RuntimeConfig,
    ) -> Result<Self, ProviderError> {
        let registry = ProviderRegistry::with_factories(factories);
        Self::from_registry_with_config(registry, config)
    }

    fn from_registry_with_config(registry: ProviderRegistry, config: RuntimeConfig) -> Result<Self, ProviderError> {
        let dispatcher = Arc::new(ProviderEventDispatcher::new());
        let provider_instances = registry.instantiate_all(&config)?;
        tracing::debug!(count = provider_instances.len(), "instantiated providers");
        let mut providers: Vec<Arc<dyn UiTreeProvider>> = Vec::with_capacity(provider_instances.len());
        for provider in provider_instances {
            let listener = Arc::new(RuntimeEventListener::new(dispatcher.clone()));
            provider.subscribe_events(listener)?;
            providers.push(provider);
        }

        // Select and build the per-runtime platform bundle for this session.
        let platform = select_platform(&config)?;
        tracing::debug!(platform = platform.is_some(), "platform bundle selected");

        // Thread this session's window manager into every provider so provider
        // nodes target this runtime's session, not a process-global one.
        if let Some(bundle) = &platform {
            for provider in &providers {
                provider.set_window_manager(bundle.window_manager.clone());
            }
        }

        // Desktop info comes from the bundle when a platform is available, else a
        // fallback (headless / provider-only runtimes).
        let desktop = match &platform {
            Some(bundle) => bundle.desktop_info.desktop_info().map_err(map_desktop_error)?,
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

        let providers_for_desktop: Vec<Arc<dyn UiTreeProvider>> = providers.to_vec();

        let provider_count = providers.len();
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

        // Surface config sections that no registered backend / active provider
        // claimed — a portability aid (a dict may carry every OS's keys) and a
        // typo hint. Tolerant by design: unclaimed ids are ignored, not errors.
        let registered_platform_ids: Vec<&str> = platform_factories().map(|factory| factory.id()).collect();
        for id in runtime.config.platform_component_ids() {
            if !registered_platform_ids.contains(&id) {
                tracing::debug!(id, "config platform.<id> matched no registered platform backend");
            }
        }
        for id in runtime.config.provider_component_ids() {
            if !runtime.providers.iter().any(|provider| provider.descriptor().id == id) {
                tracing::debug!(id, "config providers.<id> matched no active provider");
            }
        }

        tracing::info!(providers = provider_count, "Runtime initialized");
        Ok(runtime)
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

/// Selects and builds the per-runtime [`PlatformBundle`] for this session.
///
/// When `config` forces a backend (`platform.backend`), that backend must be
/// registered and able to serve the environment, or construction fails. Without
/// a forced backend, the first factory whose `can_serve` accepts the environment
/// wins; if none does, the runtime has no platform (`Ok(None)`) — e.g. a headless
/// provider-only test.
fn select_platform(config: &RuntimeConfig) -> Result<Option<PlatformBundle>, ProviderError> {
    let factories: Vec<_> = platform_factories().collect();

    if let Some(id) = config.platform_backend() {
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
        Ok(Some(bundle))
    } else {
        for factory in &factories {
            if factory.can_serve(config) {
                let bundle = factory.create(config).map_err(|err| ProviderError::InitializationFailed {
                    provider: "runtime",
                    details: Some(err.to_string()),
                })?;
                return Ok(Some(bundle));
            }
        }
        Ok(None)
    }
}

fn map_desktop_error(err: PlatformError) -> ProviderError {
    ProviderError::InitializationFailed { provider: "desktop", details: Some(err.to_string()) }
}

fn fallback_desktop_info() -> DesktopInfo {
    tracing::warn!("using fallback desktop info — no DesktopInfoProvider available");
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
}
