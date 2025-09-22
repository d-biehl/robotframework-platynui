use std::sync::Arc;

use platynui_core::provider::{ProviderError, UiTreeProvider};

use crate::provider::ProviderRegistry;
use crate::provider::event::{ProviderEventDispatcher, ProviderEventSink};
use platynui_core::provider::ProviderEvent;
use platynui_core::ui::UiNode;
use platynui_core::ui::identifiers::TechnologyId;

use crate::EvaluateOptions;

/// Central orchestrator that owns provider instances and the provider event dispatcher.
pub struct Runtime {
    registry: ProviderRegistry,
    providers: Vec<Arc<dyn UiTreeProvider>>,
    dispatcher: Arc<ProviderEventDispatcher>,
}

impl Runtime {
    /// Discovers all registered providers, instantiates them and prepares the event pipeline.
    pub fn new() -> Result<Self, ProviderError> {
        let registry = ProviderRegistry::discover();
        let dispatcher = Arc::new(ProviderEventDispatcher::new());
        let providers = registry.instantiate_all()?;
        for provider in &providers {
            provider.subscribe_events(dispatcher.clone())?;
        }

        Ok(Self { registry, providers, dispatcher })
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
        self.providers
            .iter()
            .filter(move |provider| provider.descriptor().technology == *technology)
    }

    /// Access to the shared provider event dispatcher.
    pub fn event_dispatcher(&self) -> Arc<ProviderEventDispatcher> {
        Arc::clone(&self.dispatcher)
    }

    /// Convenience helper that preconfigures `EvaluateOptions` with the runtime
    /// resolver so callers do not have to wire it manually.
    pub fn evaluate_options(&self, desktop: Arc<dyn UiNode>) -> EvaluateOptions {
        EvaluateOptions::new(desktop)
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
        self.dispatcher.shutdown();
        for provider in &self.providers {
            provider.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::provider::{
        ProviderDescriptor, ProviderEvent, ProviderEventKind, ProviderEventListener, ProviderKind,
        UiTreeProviderFactory, register_provider,
    };
    use platynui_core::ui::identifiers::TechnologyId;
    use platynui_core::ui::{Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiValue};
    #[allow(unused_imports)]
    use platynui_platform_mock as _;
    #[allow(unused_imports)]
    use platynui_provider_mock as _;
    use rstest::rstest;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, LazyLock, Mutex, Weak};

    struct StubAttribute;
    impl UiAttribute for StubAttribute {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn name(&self) -> &str {
            "Role"
        }
        fn value(&self) -> UiValue {
            UiValue::from("Stub")
        }
    }

    struct StubNode {
        runtime_id: RuntimeId,
        parent: Mutex<Option<Weak<dyn UiNode>>>,
    }

    impl StubNode {
        fn new(id: &str) -> Self {
            Self { runtime_id: RuntimeId::from(id), parent: Mutex::new(None) }
        }

        fn set_parent(&self, parent: &Arc<dyn UiNode>) {
            *self.parent.lock().unwrap() = Some(Arc::downgrade(parent));
        }
    }

    impl UiNode for StubNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &str {
            "Button"
        }
        fn name(&self) -> &str {
            "Stub"
        }
        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }
        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            self.parent.lock().unwrap().clone()
        }
        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_> {
            Box::new(Vec::<Arc<dyn UiNode>>::new().into_iter())
        }
        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_> {
            Box::new(vec![Arc::new(StubAttribute) as Arc<dyn UiAttribute>].into_iter())
        }
        fn supported_patterns(&self) -> &[PatternId] {
            &[]
        }
        fn invalidate(&self) {}
    }

    struct StubDesktop;

    impl UiNode for StubDesktop {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }

        fn role(&self) -> &str {
            "Desktop"
        }

        fn name(&self) -> &str {
            "Desktop"
        }

        fn runtime_id(&self) -> &RuntimeId {
            static RUNTIME_ID: LazyLock<RuntimeId> =
                LazyLock::new(|| RuntimeId::from("mock-desktop"));
            &RUNTIME_ID
        }

        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            None
        }

        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_> {
            Box::new(std::iter::empty())
        }

        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_> {
            Box::new(std::iter::empty())
        }

        fn supported_patterns(&self) -> &[PatternId] {
            &[]
        }

        fn invalidate(&self) {}
    }

    static SHUTDOWN_TRIGGERED: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));
    static SUBSCRIPTION_REGISTERED: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));

    struct StubProvider {
        descriptor: &'static ProviderDescriptor,
        node: Arc<StubNode>,
    }

    impl StubProvider {
        fn new(descriptor: &'static ProviderDescriptor) -> Self {
            Self { descriptor, node: Arc::new(StubNode::new(descriptor.id)) }
        }
    }

    impl UiTreeProvider for StubProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            self.descriptor
        }
        fn get_nodes(
            &self,
            parent: Arc<dyn UiNode>,
        ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
            self.node.set_parent(&parent);
            Ok(Box::new(std::iter::once(self.node.clone() as Arc<dyn UiNode>)))
        }
        fn subscribe_events(
            &self,
            listener: Arc<dyn ProviderEventListener>,
        ) -> Result<(), ProviderError> {
            listener.on_event(ProviderEvent { kind: ProviderEventKind::TreeInvalidated });
            SUBSCRIPTION_REGISTERED.store(true, Ordering::SeqCst);
            Ok(())
        }
        fn shutdown(&self) {
            SHUTDOWN_TRIGGERED.store(true, Ordering::SeqCst);
        }
    }

    struct StubFactory;

    impl StubFactory {
        fn descriptor_static() -> &'static ProviderDescriptor {
            static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
                ProviderDescriptor::new(
                    "runtime-stub",
                    "Runtime Stub",
                    TechnologyId::from("RuntimeTech"),
                    ProviderKind::Native,
                )
            });
            &DESCRIPTOR
        }
    }

    impl UiTreeProviderFactory for StubFactory {
        fn descriptor(&self) -> &ProviderDescriptor {
            Self::descriptor_static()
        }

        fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
            Ok(Arc::new(StubProvider::new(Self::descriptor_static())))
        }
    }

    static RUNTIME_FACTORY: StubFactory = StubFactory;

    register_provider!(&RUNTIME_FACTORY);

    struct RecordingSink {
        events: Mutex<Vec<ProviderEventKind>>,
    }

    impl RecordingSink {
        fn new() -> Self {
            Self { events: Mutex::new(Vec::new()) }
        }
    }

    impl ProviderEventSink for RecordingSink {
        fn dispatch(&self, event: ProviderEvent) {
            self.events.lock().unwrap().push(event.kind);
        }
    }

    #[rstest]
    fn runtime_initializes_providers() {
        SHUTDOWN_TRIGGERED.store(false, Ordering::SeqCst);
        SUBSCRIPTION_REGISTERED.store(false, Ordering::SeqCst);

        let runtime = Runtime::new().expect("runtime initializes");
        let providers: Vec<_> = runtime.providers().collect();
        assert!(!providers.is_empty());
        assert!(providers.iter().any(|provider| provider.descriptor().id == "runtime-stub"));
        assert!(SUBSCRIPTION_REGISTERED.load(Ordering::SeqCst));
    }

    #[rstest]
    fn runtime_dispatcher_forwards_events() {
        let runtime = Runtime::new().expect("runtime initializes");
        let sink = Arc::new(RecordingSink::new());
        runtime.register_event_sink(sink.clone());

        runtime.dispatch_event(ProviderEvent { kind: ProviderEventKind::TreeInvalidated });

        let events = sink.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ProviderEventKind::TreeInvalidated));
    }

    #[rstest]
    fn runtime_filters_providers_by_technology() {
        let runtime = Runtime::new().expect("runtime initializes");
        let tech = TechnologyId::from("RuntimeTech");
        let providers: Vec<_> = runtime.providers_for(&tech).collect();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].descriptor().id, "runtime-stub");
    }

    #[rstest]
    fn runtime_shutdown_invokes_provider_shutdown() {
        SHUTDOWN_TRIGGERED.store(false, Ordering::SeqCst);
        let mut runtime = Runtime::new().expect("runtime initializes");
        runtime.shutdown();
        assert!(SHUTDOWN_TRIGGERED.load(Ordering::SeqCst));
    }

    #[rstest]
    fn provider_nodes_link_parent() {
        let runtime = Runtime::new().expect("runtime initializes");
        let parent: Arc<dyn UiNode> = Arc::new(StubNode::new("parent"));
        let node = runtime
            .providers()
            .find(|provider| provider.descriptor().id == "runtime-stub")
            .and_then(|provider| {
                provider.get_nodes(Arc::clone(&parent)).ok().and_then(|mut nodes| nodes.next())
            })
            .expect("runtime stub provider node available");
        assert!(node.parent().is_some());
    }

    #[rstest]
    fn mock_provider_attaches_to_desktop() {
        let runtime = Runtime::new().expect("runtime initializes");
        let desktop: Arc<dyn UiNode> = Arc::new(StubDesktop);
        let app = runtime
            .providers()
            .find(|provider| provider.descriptor().id == "mock")
            .and_then(|provider| provider.get_nodes(Arc::clone(&desktop)).ok())
            .and_then(|mut nodes| nodes.next())
            .expect("mock provider root node");

        assert_eq!(app.namespace(), Namespace::App);
        let parent = app.parent().and_then(|weak| weak.upgrade()).expect("desktop parent");
        assert_eq!(parent.runtime_id().as_str(), desktop.runtime_id().as_str());
    }
}
