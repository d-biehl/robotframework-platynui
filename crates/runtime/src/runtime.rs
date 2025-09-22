use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use platynui_core::platform::{
    DesktopInfo, MonitorInfo, PlatformError, PlatformErrorKind, desktop_info_providers,
};
use platynui_core::provider::{ProviderError, ProviderErrorKind, ProviderEvent, UiTreeProvider};
use platynui_core::types::Rect;
use platynui_core::ui::attribute_names;
use platynui_core::ui::identifiers::TechnologyId;
use platynui_core::ui::{
    Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiValue, supported_patterns_value,
};

use crate::provider::ProviderRegistry;
use crate::provider::event::{ProviderEventDispatcher, ProviderEventSink};

use crate::EvaluateOptions;

/// Central orchestrator that owns provider instances and the provider event dispatcher.
pub struct Runtime {
    registry: ProviderRegistry,
    providers: Vec<Arc<dyn UiTreeProvider>>,
    dispatcher: Arc<ProviderEventDispatcher>,
    desktop: Arc<DesktopNode>,
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

        let desktop = build_desktop_node().map_err(map_desktop_error)?;

        Ok(Self { registry, providers, dispatcher, desktop })
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
    /// desktop node so callers do not have to wire it manually.
    pub fn evaluate_options(&self) -> EvaluateOptions {
        EvaluateOptions::new(self.desktop_node())
    }

    pub fn desktop_node(&self) -> Arc<dyn UiNode> {
        self.desktop.as_ui_node()
    }

    pub fn desktop_info(&self) -> &DesktopInfo {
        self.desktop.info()
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

fn build_desktop_node() -> Result<Arc<DesktopNode>, PlatformError> {
    let mut providers = desktop_info_providers();
    let provider = providers.next().ok_or_else(|| {
        PlatformError::new(
            PlatformErrorKind::UnsupportedPlatform,
            "no DesktopInfoProvider registered",
        )
    })?;
    let info = provider.desktop_info()?;
    Ok(DesktopNode::new(info))
}

fn map_desktop_error(err: PlatformError) -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::InitializationFailed,
        format!("desktop initialization failed: {err}"),
    )
}

struct DesktopNode {
    info: DesktopInfo,
    attributes: Vec<Arc<dyn UiAttribute>>,
    supported: Vec<PatternId>,
    children: Mutex<Vec<Arc<dyn UiNode>>>,
}

impl DesktopNode {
    fn new(info: DesktopInfo) -> Arc<Self> {
        let namespace = Namespace::Control;
        let mut attributes: Vec<Arc<dyn UiAttribute>> = Vec::new();
        let supported = vec![PatternId::from("Desktop")];

        attributes.push(attr(namespace, attribute_names::common::ROLE, UiValue::from("Desktop")));
        attributes.push(attr(
            namespace,
            attribute_names::common::NAME,
            UiValue::from(info.name.clone()),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::common::RUNTIME_ID,
            UiValue::from(info.runtime_id.as_str().to_owned()),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::common::TECHNOLOGY,
            UiValue::from(info.technology.as_str().to_owned()),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::common::SUPPORTED_PATTERNS,
            supported_patterns_value(&supported),
        ));

        attributes.push(attr(
            namespace,
            attribute_names::element::BOUNDS,
            UiValue::from(info.bounds),
        ));
        attributes.extend(rect_alias_attributes(namespace, "Bounds", &info.bounds));
        attributes.push(attr(namespace, attribute_names::element::IS_VISIBLE, UiValue::from(true)));
        attributes.push(attr(namespace, attribute_names::element::IS_ENABLED, UiValue::from(true)));
        attributes.push(attr(
            namespace,
            attribute_names::element::IS_OFFSCREEN,
            UiValue::from(false),
        ));

        attributes.push(attr(
            namespace,
            attribute_names::desktop::DISPLAY_COUNT,
            UiValue::from(info.display_count() as i64),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::desktop::OS_NAME,
            UiValue::from(info.os_name.clone()),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::desktop::OS_VERSION,
            UiValue::from(info.os_version.clone()),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::desktop::MONITORS,
            UiValue::Array(info.monitors.iter().map(monitor_to_value).collect()),
        ));

        Arc::new(Self { info, attributes, supported, children: Mutex::new(Vec::new()) })
    }

    fn info(&self) -> &DesktopInfo {
        &self.info
    }

    fn as_ui_node(self: &Arc<Self>) -> Arc<dyn UiNode> {
        Arc::clone(self) as Arc<dyn UiNode>
    }

    #[allow(dead_code)]
    fn attach_child(&self, child: Arc<dyn UiNode>) {
        self.children.lock().unwrap().push(child);
    }
}

impl UiNode for DesktopNode {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }

    fn role(&self) -> &str {
        "Desktop"
    }

    fn name(&self) -> &str {
        &self.info.name
    }

    fn runtime_id(&self) -> &RuntimeId {
        &self.info.runtime_id
    }

    fn parent(&self) -> Option<std::sync::Weak<dyn UiNode>> {
        None
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_> {
        let snapshot = self.children.lock().unwrap().clone();
        Box::new(snapshot.into_iter())
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_> {
        Box::new(self.attributes.clone().into_iter())
    }

    fn supported_patterns(&self) -> &[PatternId] {
        &self.supported
    }

    fn invalidate(&self) {}
}

fn attr(namespace: Namespace, name: impl Into<String>, value: UiValue) -> Arc<dyn UiAttribute> {
    Arc::new(DesktopAttribute { namespace, name: name.into(), value })
}

fn rect_alias_attributes(
    namespace: Namespace,
    base: &str,
    rect: &Rect,
) -> Vec<Arc<dyn UiAttribute>> {
    vec![
        attr(namespace, format!("{base}.X"), UiValue::from(rect.x())),
        attr(namespace, format!("{base}.Y"), UiValue::from(rect.y())),
        attr(namespace, format!("{base}.Width"), UiValue::from(rect.width())),
        attr(namespace, format!("{base}.Height"), UiValue::from(rect.height())),
    ]
}

fn monitor_to_value(info: &MonitorInfo) -> UiValue {
    let mut map = BTreeMap::new();
    map.insert("Id".to_string(), UiValue::from(info.id.clone()));
    if let Some(name) = &info.name {
        map.insert("Name".to_string(), UiValue::from(name.clone()));
    }
    map.insert("Bounds".to_string(), UiValue::from(info.bounds));
    map.insert("IsPrimary".to_string(), UiValue::from(info.is_primary));
    if let Some(scale) = info.scale_factor {
        map.insert("ScaleFactor".to_string(), UiValue::from(scale));
    }
    UiValue::Object(map)
}

struct DesktopAttribute {
    namespace: Namespace,
    name: String,
    value: UiValue,
}

impl UiAttribute for DesktopAttribute {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn value(&self) -> UiValue {
        self.value.clone()
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
        let desktop = runtime.desktop_node();
        let app = runtime
            .providers()
            .find(|provider| provider.descriptor().id == "mock")
            .and_then(|provider| provider.get_nodes(Arc::clone(&desktop)).ok())
            .and_then(|mut nodes| nodes.next())
            .expect("mock provider root node");

        assert_eq!(app.namespace(), Namespace::App);
        let parent = app.parent().and_then(|weak| weak.upgrade()).expect("desktop parent");
        assert_eq!(parent.runtime_id().as_str(), runtime.desktop_info().runtime_id.as_str());
    }
}
