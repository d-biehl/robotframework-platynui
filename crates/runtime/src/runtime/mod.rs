mod desktop;
mod error;
mod evaluation;
mod input;
mod platform_modules;
mod window;

#[cfg(test)]
mod test_fixtures;

pub use error::{BringToFrontError, FocusError, KeyboardActionError};
pub use platform_modules::PlatformOverrides;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use platynui_core::platform::{
    DesktopInfo, HighlightProvider, KeyboardDevice, KeyboardProfile, PlatformError, PointerDevice, ScreenshotProvider,
    desktop_info_providers, highlight_providers, keyboard_devices, pointer_devices, screenshot_providers,
};
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
use platform_modules::{PlatformModulesLease, platform_overrides_require_global_modules};

/// Central orchestrator that owns provider instances and the provider event dispatcher.
pub struct Runtime {
    pub(super) registry: ProviderRegistry,
    pub(super) providers: Vec<Arc<dyn UiTreeProvider>>,
    pub(super) dispatcher: Arc<ProviderEventDispatcher>,
    platform_guard: Option<PlatformModulesLease>,
    desktop: Arc<DesktopNode>,
    pub(super) highlight: Option<&'static dyn HighlightProvider>,
    pub(super) screenshot: Option<&'static dyn ScreenshotProvider>,
    pub(super) pointer: Option<&'static dyn PointerDevice>,
    pub(super) pointer_engine: Mutex<Option<PointerEngine<'static>>>,
    pub(super) xpath_cache: Mutex<crate::xpath::XdmCache>,
    pub(super) pointer_settings: Mutex<PointerSettings>,
    pub(super) pointer_profile: Mutex<PointerProfile>,
    pub(super) keyboard: Option<&'static dyn KeyboardDevice>,
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
    pub fn new() -> Result<Self, ProviderError> {
        let platform_guard = PlatformModulesLease::acquire()?;
        let registry = ProviderRegistry::discover();
        Self::from_registry_with_platforms(registry, None, Some(platform_guard))
    }

    /// Builds a Runtime that only includes providers with the given `ids`.
    /// This is useful for tests to restrict the active providers deterministically.
    pub fn new_with_provider_ids(ids: &[&str]) -> Result<Self, ProviderError> {
        let platform_guard = PlatformModulesLease::acquire()?;
        let registry = ProviderRegistry::discover().filter_by_ids(ids);
        Self::from_registry_with_platforms(registry, None, Some(platform_guard))
    }

    /// Builds a Runtime from an explicit list of provider factories.
    /// No inventory discovery is performed.
    pub fn new_with_factories(factories: &[&'static dyn UiTreeProviderFactory]) -> Result<Self, ProviderError> {
        let platform_guard = PlatformModulesLease::acquire()?;
        let registry = ProviderRegistry::with_factories(factories);
        Self::from_registry_with_platforms(registry, None, Some(platform_guard))
    }

    /// Builds a Runtime from factories plus explicit platform provider overrides.
    pub fn new_with_factories_and_platforms(
        factories: &[&'static dyn UiTreeProviderFactory],
        platforms: PlatformOverrides,
    ) -> Result<Self, ProviderError> {
        let platform_guard = if platform_overrides_require_global_modules(&platforms) {
            Some(PlatformModulesLease::acquire()?)
        } else {
            None
        };
        let registry = ProviderRegistry::with_factories(factories);
        Self::from_registry_with_platforms(registry, Some(platforms), platform_guard)
    }

    fn from_registry_with_platforms(
        registry: ProviderRegistry,
        platforms: Option<PlatformOverrides>,
        platform_guard: Option<PlatformModulesLease>,
    ) -> Result<Self, ProviderError> {
        let dispatcher = Arc::new(ProviderEventDispatcher::new());
        let provider_instances = registry.instantiate_all()?;
        tracing::debug!(count = provider_instances.len(), "instantiated providers");
        let mut providers: Vec<Arc<dyn UiTreeProvider>> = Vec::with_capacity(provider_instances.len());
        for provider in provider_instances {
            let listener = Arc::new(RuntimeEventListener::new(dispatcher.clone()));
            provider.subscribe_events(listener)?;
            providers.push(provider);
        }

        // Build desktop info first
        let desktop = if let Some(p) = &platforms {
            if let Some(provider) = p.desktop_info {
                provider.desktop_info().map_err(map_desktop_error)?
            } else {
                build_desktop_info().map_err(map_desktop_error)?
            }
        } else {
            build_desktop_info().map_err(map_desktop_error)?
        };

        let (highlight, screenshot, pointer, keyboard) = if let Some(p) = platforms {
            (
                p.highlight.or_else(|| highlight_providers().next()),
                p.screenshot.or_else(|| screenshot_providers().next()),
                p.pointer.or_else(|| pointer_devices().next()),
                p.keyboard.or_else(|| keyboard_devices().next()),
            )
        } else {
            (
                highlight_providers().next(),
                screenshot_providers().next(),
                pointer_devices().next(),
                keyboard_devices().next(),
            )
        };
        tracing::debug!(
            highlight = highlight.is_some(),
            screenshot = screenshot.is_some(),
            pointer = pointer.is_some(),
            keyboard = keyboard.is_some(),
            "platform devices discovered",
        );

        let mut pointer_settings = PointerSettings::default();
        if let Some(device) = pointer {
            if let Ok(Some(time)) = device.double_click_time() {
                pointer_settings.double_click_time = time;
            }
            if let Ok(Some(size)) = device.double_click_size() {
                pointer_settings.double_click_size = size;
            }
        }
        let pointer_profile = PointerProfile::named_default();
        let keyboard_profile = KeyboardProfile::default();
        let pointer_engine = pointer.map(|device| {
            PointerEngine::new(
                device,
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
            platform_guard,
            desktop: {
                let node = DesktopNode::new(desktop, providers_for_desktop);
                DesktopNode::init_self(&node);
                node
            },
            highlight,
            screenshot,
            pointer,
            pointer_engine: Mutex::new(pointer_engine),
            xpath_cache: Mutex::new(crate::xpath::XdmCache::new()),
            pointer_settings: Mutex::new(pointer_settings),
            pointer_profile: Mutex::new(pointer_profile),
            keyboard,
            keyboard_profile: Mutex::new(keyboard_profile),
            is_shutdown: AtomicBool::new(false),
        };
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

    /// Invokes shutdown on dispatcher and providers.
    pub fn shutdown(&mut self) {
        if self.is_shutdown.swap(true, Ordering::AcqRel) {
            return; // already shut down
        }
        tracing::info!(providers = self.providers.len(), "Runtime shutting down");
        self.dispatcher.shutdown();
        for provider in &self.providers {
            provider.shutdown();
        }
        if let Some(mut platform_guard) = self.platform_guard.take() {
            platform_guard.release();
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // Ensure providers and dispatcher are shut down exactly once.
        self.shutdown();
    }
}

fn build_desktop_info() -> Result<DesktopInfo, PlatformError> {
    let mut providers = desktop_info_providers();
    let info = if let Some(provider) = providers.next() { provider.desktop_info()? } else { fallback_desktop_info() };
    Ok(info)
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
    use super::platform_modules::PLATFORM_MODULES_STATE;
    use super::test_fixtures::*;
    use super::*;
    use platynui_core::platform::{PlatformError, PlatformModule};
    use platynui_core::provider::{
        ProviderDescriptor, ProviderEvent, ProviderEventKind, ProviderEventListener, ProviderKind,
        UiTreeProviderFactory,
    };
    use platynui_core::register_platform_module;
    use platynui_core::ui::identifiers::TechnologyId;
    use platynui_core::ui::{Namespace, UiNode};
    use platynui_platform_mock as _;
    use platynui_provider_mock as _;
    use rstest::rstest;
    use serial_test::serial;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, LazyLock};

    // --- Platform init order test infrastructure ---

    static TEST_PLATFORM_INITIALIZED: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));

    struct TestInitOrderPlatform;
    impl PlatformModule for TestInitOrderPlatform {
        fn name(&self) -> &'static str {
            "test-init-order-platform"
        }
        fn initialize(&self) -> Result<(), PlatformError> {
            TEST_PLATFORM_INITIALIZED.store(true, Ordering::SeqCst);
            Ok(())
        }
    }
    static TEST_PLATFORM: TestInitOrderPlatform = TestInitOrderPlatform;
    register_platform_module!(&TEST_PLATFORM);

    static TEST_LEASE_INITIALIZE_COUNT: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));
    static TEST_LEASE_SHUTDOWN_COUNT: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));

    struct TestLeasePlatform;
    impl PlatformModule for TestLeasePlatform {
        fn name(&self) -> &'static str {
            "test-runtime-platform-lease"
        }

        fn initialize(&self) -> Result<(), PlatformError> {
            TEST_LEASE_INITIALIZE_COUNT.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn shutdown(&self) {
            TEST_LEASE_SHUTDOWN_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    static TEST_LEASE_PLATFORM: TestLeasePlatform = TestLeasePlatform;
    register_platform_module!(&TEST_LEASE_PLATFORM);

    struct InitOrderProviderFactory;
    impl InitOrderProviderFactory {
        fn descriptor_static() -> &'static ProviderDescriptor {
            static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
                ProviderDescriptor::new(
                    "runtime-init-order",
                    "Runtime InitOrder",
                    TechnologyId::from("Runtime"),
                    ProviderKind::Native,
                )
            });
            &DESCRIPTOR
        }
    }

    impl UiTreeProviderFactory for InitOrderProviderFactory {
        fn descriptor(&self) -> &ProviderDescriptor {
            Self::descriptor_static()
        }

        fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
            assert!(
                TEST_PLATFORM_INITIALIZED.load(Ordering::SeqCst),
                "platform modules must be initialized before providers are created"
            );
            struct NoopProvider {
                desc: &'static ProviderDescriptor,
            }
            impl UiTreeProvider for NoopProvider {
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
                fn shutdown(&self) {}
            }
            Ok(Arc::new(NoopProvider { desc: Self::descriptor_static() }))
        }
    }
    static INIT_ORDER_PROVIDER: InitOrderProviderFactory = InitOrderProviderFactory;

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
        fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
            Ok(Arc::new(DropCounterProvider { desc: Self::descriptor_static() }))
        }
    }
    static DROP_COUNTER_FACTORY: DropCounterFactory = DropCounterFactory;

    // --- Tests ---

    #[test]
    fn platform_init_happens_before_provider_instantiation() {
        // The assertions happen inside the provider factory `create()`.
        let _runtime = Runtime::new_with_factories(&[&INIT_ORDER_PROVIDER]).expect("runtime initializes");
    }

    #[test]
    #[serial]
    fn platform_modules_remain_active_until_last_runtime_is_released() {
        TEST_LEASE_INITIALIZE_COUNT.store(0, Ordering::SeqCst);
        TEST_LEASE_SHUTDOWN_COUNT.store(0, Ordering::SeqCst);

        {
            let mut first = Runtime::new_with_factories(&[]).expect("first runtime initializes");
            let second = Runtime::new_with_factories(&[]).expect("second runtime initializes");

            assert_eq!(TEST_LEASE_INITIALIZE_COUNT.load(Ordering::SeqCst), 1);
            assert_eq!(TEST_LEASE_SHUTDOWN_COUNT.load(Ordering::SeqCst), 0);

            first.shutdown();

            assert_eq!(TEST_LEASE_INITIALIZE_COUNT.load(Ordering::SeqCst), 1);
            assert_eq!(TEST_LEASE_SHUTDOWN_COUNT.load(Ordering::SeqCst), 0);

            drop(second);
        }

        assert_eq!(TEST_LEASE_INITIALIZE_COUNT.load(Ordering::SeqCst), 1);
        assert_eq!(TEST_LEASE_SHUTDOWN_COUNT.load(Ordering::SeqCst), 1);

        let state = PLATFORM_MODULES_STATE.lock().expect("platform state lock");
        assert_eq!(state.active_runtimes, 0);
    }

    #[test]
    #[serial]
    fn explicit_platform_overrides_do_not_initialize_global_modules() {
        TEST_LEASE_INITIALIZE_COUNT.store(0, Ordering::SeqCst);
        TEST_LEASE_SHUTDOWN_COUNT.store(0, Ordering::SeqCst);

        let runtime = Runtime::new_with_factories_and_platforms(
            &[&platynui_provider_mock::MOCK_PROVIDER_FACTORY],
            PlatformOverrides {
                desktop_info: Some(&platynui_platform_mock::MOCK_PLATFORM),
                highlight: Some(&platynui_platform_mock::MOCK_HIGHLIGHT),
                screenshot: Some(&platynui_platform_mock::MOCK_SCREENSHOT),
                pointer: Some(&platynui_platform_mock::MOCK_POINTER),
                keyboard: Some(&platynui_platform_mock::MOCK_KEYBOARD),
            },
        )
        .expect("runtime initializes with explicit platforms");

        assert_eq!(TEST_LEASE_INITIALIZE_COUNT.load(Ordering::SeqCst), 0);
        assert_eq!(TEST_LEASE_SHUTDOWN_COUNT.load(Ordering::SeqCst), 0);

        drop(runtime);

        assert_eq!(TEST_LEASE_INITIALIZE_COUNT.load(Ordering::SeqCst), 0);
        assert_eq!(TEST_LEASE_SHUTDOWN_COUNT.load(Ordering::SeqCst), 0);

        let state = PLATFORM_MODULES_STATE.lock().expect("platform state lock");
        assert_eq!(state.active_runtimes, 0);
    }

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
