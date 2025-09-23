//! Deterministic mock UiTree provider for testing the runtime and CLI wiring.
//!
//! The provider surfaces a deterministic, but now richer, static tree with
//! multiple Anwendungen, Fenstern und verschachtelten Controls. Die Struktur
//! liefert stabile `RuntimeId`s und vielfältige Layout-Varianten, damit höhere
//! Schichten Enumeration, XPath-Abfragen, Highlighting und Vertragsprüfungen
//! ohne native Plattformabhängigkeiten testen können.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, LazyLock, Mutex, RwLock, Weak};

use platynui_core::provider::{
    ProviderDescriptor, ProviderError, ProviderEvent, ProviderEventCapabilities, ProviderEventKind,
    ProviderEventListener, ProviderKind, UiTreeProvider, UiTreeProviderFactory,
};
use platynui_core::register_provider;
use platynui_core::types::{Point, Rect};
use platynui_core::ui::attribute_names::{activation_target, common, element, text_content};
use platynui_core::ui::{
    Namespace, PatternId, PatternRegistry, RuntimeId, TechnologyId, UiAttribute, UiNode, UiPattern,
    UiValue, supported_patterns_value,
};
use serde::Deserialize;

#[cfg(test)]
use platynui_core::provider::provider_factories;

static MOCK_PROVIDER_FACTORY: MockProviderFactory = MockProviderFactory;

register_provider!(&MOCK_PROVIDER_FACTORY);

const PROVIDER_ID: &str = "mock";
const PROVIDER_NAME: &str = "PlatynUI Mock Provider";
const TECHNOLOGY: &str = "Mock";
#[cfg(test)]
const APP_RUNTIME_ID: &str = "mock://app/main";
#[cfg(test)]
const WINDOW_RUNTIME_ID: &str = "mock://window/main";
#[cfg(test)]
const BUTTON_RUNTIME_ID: &str = "mock://button/ok";

struct MockProviderFactory;

impl MockProviderFactory {
    fn descriptor_static() -> &'static ProviderDescriptor {
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
            ProviderDescriptor::new(
                PROVIDER_ID,
                PROVIDER_NAME,
                TechnologyId::from(TECHNOLOGY),
                ProviderKind::Native,
            )
            .with_event_capabilities(ProviderEventCapabilities::STRUCTURE_WITH_PROPERTIES)
        });
        &DESCRIPTOR
    }
}

impl UiTreeProviderFactory for MockProviderFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        Self::descriptor_static()
    }

    fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        let provider: Arc<MockProvider> = Arc::new(MockProvider::new(Self::descriptor_static()));
        register_active_instance(&provider);
        Ok(provider)
    }
}

struct MockProvider {
    descriptor: &'static ProviderDescriptor,
    roots: Vec<Arc<MockNode>>,
    flat_nodes: Vec<Arc<MockNode>>,
    nodes: HashMap<String, Arc<MockNode>>,
    listeners: RwLock<Vec<Arc<dyn ProviderEventListener>>>,
}

impl MockProvider {
    fn new(descriptor: &'static ProviderDescriptor) -> Self {
        let (roots, flat_nodes, nodes) = tree::instantiate_nodes(descriptor);
        Self { descriptor, roots, flat_nodes, nodes, listeners: RwLock::new(Vec::new()) }
    }

    fn children_for_parent(&self, parent: &Arc<dyn UiNode>) -> Vec<Arc<MockNode>> {
        if let Some(node) = self.nodes.get(parent.runtime_id().as_str()) {
            node.children_snapshot()
        } else if parent.namespace() == Namespace::Control && parent.role() == "Desktop" {
            let mut nodes = self.roots.clone();
            for child in &self.flat_nodes {
                child.set_parent(parent);
                nodes.push(Arc::clone(child));
            }
            nodes
        } else {
            Vec::new()
        }
    }

    fn notify_listeners(&self, event: ProviderEventKind) {
        let snapshot = {
            let listeners = self.listeners.read().unwrap();
            listeners.clone()
        };
        let event = ProviderEvent { kind: event };
        for listener in snapshot {
            listener.on_event(event.clone());
        }
    }
}

impl UiTreeProvider for MockProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
    }

    fn get_nodes(
        &self,
        parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
        let children = self.children_for_parent(&parent);

        for child in &children {
            child.set_parent(&parent);
        }

        Ok(Box::new(children.into_iter().map(|child| -> Arc<dyn UiNode> { child })))
    }

    fn subscribe_events(
        &self,
        listener: Arc<dyn ProviderEventListener>,
    ) -> Result<(), ProviderError> {
        listener.on_event(ProviderEvent { kind: ProviderEventKind::TreeInvalidated });
        self.listeners.write().unwrap().push(listener);
        Ok(())
    }
}

struct MockNode {
    namespace: Namespace,
    role: String,
    name: String,
    runtime_id: RuntimeId,
    attributes: Vec<Arc<dyn UiAttribute>>,
    runtime_patterns: PatternRegistry,
    supported_patterns: Vec<PatternId>,
    order_key: Option<u64>,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    children: Mutex<Vec<Arc<MockNode>>>,
}

struct NodePatternContext {
    runtime_patterns: PatternRegistry,
    supported_patterns: Vec<PatternId>,
    order_key: Option<u64>,
}

impl MockNode {
    fn new(
        namespace: Namespace,
        role: impl Into<String>,
        name: impl Into<String>,
        runtime_id: &str,
        descriptor: &ProviderDescriptor,
        mut additional_attributes: Vec<Arc<dyn UiAttribute>>,
        pattern_context: NodePatternContext,
    ) -> Arc<Self> {
        let technology = descriptor.technology.as_str().to_owned();
        let runtime_id = RuntimeId::from(runtime_id);
        let role_string = role.into();
        let name_string = name.into();
        let mut attributes = vec![
            attr(namespace, common::ROLE, UiValue::from(role_string.clone())),
            attr(namespace, common::NAME, UiValue::from(name_string.clone())),
            attr(namespace, common::RUNTIME_ID, UiValue::from(runtime_id.as_str().to_owned())),
            attr(namespace, common::TECHNOLOGY, UiValue::from(technology)),
        ];

        let NodePatternContext { runtime_patterns, supported_patterns, order_key } =
            pattern_context;
        let supported_patterns = if runtime_patterns.is_empty() {
            supported_patterns
        } else {
            runtime_patterns.supported().to_vec()
        };

        attributes.push(attr(
            namespace,
            common::SUPPORTED_PATTERNS,
            supported_patterns_value(&supported_patterns),
        ));
        attributes.append(&mut additional_attributes);

        Arc::new(Self {
            namespace,
            role: role_string,
            name: name_string,
            runtime_id,
            attributes,
            runtime_patterns,
            supported_patterns,
            order_key,
            parent: Mutex::new(None),
            children: Mutex::new(Vec::new()),
        })
    }

    fn children_snapshot(&self) -> Vec<Arc<MockNode>> {
        self.children.lock().unwrap().clone()
    }

    fn set_parent(&self, parent: &Arc<dyn UiNode>) {
        *self.parent.lock().unwrap() = Some(Arc::downgrade(parent));
    }

    fn add_child(parent: &Arc<Self>, child: Arc<Self>) {
        let parent_clone = Arc::clone(parent);
        let parent_trait: Arc<dyn UiNode> = parent_clone;
        child.set_parent(&parent_trait);
        parent.children.lock().unwrap().push(child);
    }
}

impl UiNode for MockNode {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn role(&self) -> &str {
        &self.role
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn runtime_id(&self) -> &RuntimeId {
        &self.runtime_id
    }

    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent.lock().unwrap().clone()
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_> {
        let snapshot = self.children.lock().unwrap().clone();
        Box::new(snapshot.into_iter().map(|child| -> Arc<dyn UiNode> { child }))
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_> {
        Box::new(self.attributes.clone().into_iter())
    }

    fn supported_patterns(&self) -> &[PatternId] {
        &self.supported_patterns
    }

    fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
        self.runtime_patterns.get(pattern)
    }

    fn doc_order_key(&self) -> Option<u64> {
        self.order_key
    }

    fn invalidate(&self) {}
}

/// Global registry of active provider instances so tests/CLI can emit mock events.
static ACTIVE_PROVIDERS: LazyLock<RwLock<Vec<Weak<MockProvider>>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));

fn register_active_instance(provider: &Arc<MockProvider>) {
    let weak = Arc::downgrade(provider);
    let mut list = ACTIVE_PROVIDERS.write().unwrap();
    list.retain(|weak_provider| weak_provider.upgrade().is_some());
    list.push(weak);
}

fn active_providers() -> Vec<Arc<MockProvider>> {
    let mut list = ACTIVE_PROVIDERS.write().unwrap();
    list.retain(|weak_provider| weak_provider.upgrade().is_some());
    list.iter().filter_map(|weak_provider| weak_provider.upgrade()).collect()
}

/// Emits an event to all active mock provider instances without mutating the tree.
pub fn emit_event(event: ProviderEventKind) {
    for provider in active_providers() {
        provider.notify_listeners(event.clone());
    }
}

/// Emits a node-updated event for the given runtime id, cloning the current node state.
pub fn emit_node_updated(runtime_id: &str) {
    for provider in active_providers() {
        if let Some(node) = provider.nodes.get(runtime_id) {
            let node_clone = Arc::clone(node);
            let node_trait: Arc<dyn UiNode> = node_clone;
            provider.notify_listeners(ProviderEventKind::NodeUpdated { node: node_trait });
        }
    }
}

/// Returns a clone of the current node for the provided runtime id, if available.
pub fn node_by_runtime_id(runtime_id: &str) -> Option<Arc<dyn UiNode>> {
    for provider in active_providers() {
        if let Some(node) = provider.nodes.get(runtime_id) {
            let node_clone = Arc::clone(node);
            let node_trait: Arc<dyn UiNode> = node_clone;
            return Some(node_trait);
        }
    }
    None
}

pub mod tree {
    use super::*;
    use std::collections::HashMap;
    use std::sync::LazyLock;

    static CURRENT_TREE: LazyLock<RwLock<StaticMockTree>> =
        LazyLock::new(|| RwLock::new(StaticMockTree::default()));

    type NodeList = Vec<Arc<MockNode>>;
    type InstantiatedTree = (NodeList, NodeList, NodeList);
    type NodeMap = HashMap<String, Arc<MockNode>>;
    type ProviderTree = (NodeList, NodeList, NodeMap);

    #[derive(Clone, Debug)]
    pub struct AttributeSpec {
        namespace: Namespace,
        name: String,
        value: UiValue,
    }

    impl AttributeSpec {
        pub fn new(namespace: Namespace, name: impl Into<String>, value: UiValue) -> Self {
            Self { namespace, name: name.into(), value }
        }
    }

    impl From<(Namespace, &'static str, UiValue)> for AttributeSpec {
        fn from(value: (Namespace, &'static str, UiValue)) -> Self {
            AttributeSpec::new(value.0, value.1, value.2)
        }
    }

    impl From<(Namespace, String, UiValue)> for AttributeSpec {
        fn from(value: (Namespace, String, UiValue)) -> Self {
            AttributeSpec::new(value.0, value.1, value.2)
        }
    }

    #[derive(Clone, Debug)]
    pub struct NodeSpec {
        namespace: Namespace,
        role: String,
        name: String,
        runtime_id: String,
        attributes: Vec<AttributeSpec>,
        patterns: Vec<String>,
        children: Vec<NodeSpec>,
        expose_flat: bool,
        order_key: Option<u64>,
    }

    impl NodeSpec {
        pub fn new(
            namespace: Namespace,
            role: impl Into<String>,
            name: impl Into<String>,
            runtime_id: impl Into<String>,
        ) -> Self {
            Self {
                namespace,
                role: role.into(),
                name: name.into(),
                runtime_id: runtime_id.into(),
                attributes: Vec::new(),
                patterns: Vec::new(),
                children: Vec::new(),
                expose_flat: false,
                order_key: None,
            }
        }

        pub fn with_attribute(mut self, attribute: impl Into<AttributeSpec>) -> Self {
            self.attributes.push(attribute.into());
            self
        }

        pub fn with_child(mut self, child: NodeSpec) -> Self {
            self.children.push(child);
            self
        }

        pub fn with_pattern(mut self, pattern: impl Into<String>) -> Self {
            self.patterns.push(pattern.into());
            self
        }

        pub fn with_patterns<I, S>(mut self, patterns: I) -> Self
        where
            I: IntoIterator<Item = S>,
            S: Into<String>,
        {
            self.patterns.extend(patterns.into_iter().map(Into::into));
            self
        }

        pub fn with_expose_flat(mut self, expose: bool) -> Self {
            self.expose_flat = expose;
            self
        }

        pub fn expose_flat(&self) -> bool {
            self.expose_flat
        }
    }

    fn collect_flat_specs(spec: &NodeSpec, flat_specs: &mut Vec<NodeSpec>) {
        if spec.expose_flat {
            flat_specs.push(clone_for_flat(spec));
        }
        for child in &spec.children {
            collect_flat_specs(child, flat_specs);
        }
    }

    fn clone_for_flat(spec: &NodeSpec) -> NodeSpec {
        let mut clone = spec.clone();
        clone.expose_flat = false;
        clone.order_key = None;
        clone.children = spec.children.iter().map(clone_for_flat).collect();
        clone
    }

    fn assign_order_keys(spec: &mut NodeSpec, counter: &mut u64) {
        spec.order_key = Some(*counter);
        *counter += 1;
        for child in &mut spec.children {
            assign_order_keys(child, counter);
        }
    }

    #[derive(Clone, Debug)]
    pub struct StaticMockTree {
        roots: Vec<NodeSpec>,
        flat_specs: Vec<NodeSpec>,
    }

    impl StaticMockTree {
        pub fn new(roots: Vec<NodeSpec>) -> Self {
            let mut flat_specs = Vec::new();
            for spec in &roots {
                collect_flat_specs(spec, &mut flat_specs);
            }

            let mut roots = roots;
            let mut counter = 0u64;
            for spec in &mut roots {
                assign_order_keys(spec, &mut counter);
            }
            for spec in &mut flat_specs {
                assign_order_keys(spec, &mut counter);
            }

            Self { roots, flat_specs }
        }

        pub fn roots(&self) -> &[NodeSpec] {
            &self.roots
        }

        pub fn flat_specs(&self) -> &[NodeSpec] {
            &self.flat_specs
        }

        fn instantiate(&self, descriptor: &ProviderDescriptor) -> InstantiatedTree {
            let mut all = Vec::new();
            let roots: NodeList = self
                .roots
                .iter()
                .map(|spec| instantiate_node(spec, descriptor, None, &mut all))
                .collect();
            let flat: NodeList = self
                .flat_specs
                .iter()
                .map(|spec| instantiate_node(spec, descriptor, None, &mut all))
                .collect();
            (roots, flat, all)
        }
    }

    impl Default for StaticMockTree {
        fn default() -> Self {
            const XML: &str = include_str!("../assets/mock_tree.xml");
            Self::from_xml(XML).expect("embedded mock_tree.xml konnte nicht geparst werden")
        }
    }

    impl StaticMockTree {
        fn from_xml(xml: &str) -> Result<Self, MockTreeLoadError> {
            let parsed: XmlTree = quick_xml::de::from_str(xml).map_err(MockTreeLoadError::Xml)?;
            let mut roots = Vec::new();
            for node in parsed.nodes {
                roots.push(build_node(node)?);
            }
            Ok(StaticMockTree::new(roots))
        }
    }

    #[derive(Debug, Deserialize)]
    struct XmlTree {
        #[serde(rename = "node", default)]
        nodes: Vec<XmlNode>,
    }

    #[derive(Debug, Deserialize)]
    struct XmlNode {
        #[serde(rename = "@namespace")]
        namespace: String,
        #[serde(rename = "@role")]
        role: String,
        #[serde(rename = "@name")]
        name: String,
        #[serde(rename = "@runtime_id")]
        runtime_id: String,
        #[serde(rename = "@bounds")]
        bounds: Option<String>,
        #[serde(rename = "@activation_point")]
        activation_point: Option<String>,
        #[serde(rename = "@visible")]
        visible: Option<bool>,
        #[serde(rename = "@enabled")]
        enabled: Option<bool>,
        #[serde(rename = "@expose_flat")]
        expose_flat: Option<bool>,
        #[serde(rename = "attribute", default)]
        attributes: Vec<XmlAttribute>,
        #[serde(rename = "patterns")]
        patterns: Option<XmlPatternList>,
        #[serde(rename = "text")]
        text: Option<String>,
        #[serde(rename = "node", default)]
        children: Vec<XmlNode>,
    }

    #[derive(Debug, Deserialize)]
    struct XmlAttribute {
        #[serde(rename = "@namespace")]
        namespace: Option<String>,
        #[serde(rename = "@name")]
        name: String,
        #[serde(rename = "@value")]
        value: String,
    }

    #[derive(Debug, Deserialize)]
    struct XmlPatternList {
        #[serde(rename = "$text")]
        value: Option<String>,
    }

    fn build_node(node: XmlNode) -> Result<NodeSpec, MockTreeLoadError> {
        let namespace = parse_namespace(&node.namespace)?;
        let mut spec =
            NodeSpec::new(namespace, node.role.clone(), node.name.clone(), node.runtime_id.clone());

        if let Some(patterns) = parse_patterns(node.patterns.as_ref()) {
            spec.patterns.extend(patterns);
        }

        let visible = node.visible.unwrap_or(true);
        let enabled = node.enabled.unwrap_or(true);

        if let Some(bounds) = node.bounds.as_ref() {
            let rect = parse_rect(bounds)?;
            push_bounds_attributes(&mut spec, namespace, rect, visible, enabled);
        } else {
            push_visibility_attributes(&mut spec, namespace, visible, enabled);
        }

        if let Some(text) = node.text.as_ref() {
            spec.attributes.push(AttributeSpec::new(
                namespace,
                text_content::TEXT,
                UiValue::from(text.clone()),
            ));
        }

        if let Some(point_str) = node.activation_point.as_ref() {
            let point = parse_point(point_str)?;
            push_activation_point_attributes(&mut spec, namespace, point);
        }

        for attr in node.attributes {
            let attr_namespace = match attr.namespace {
                Some(ns) => parse_namespace(&ns)?,
                None => namespace,
            };
            spec.attributes.push(AttributeSpec::new(
                attr_namespace,
                attr.name,
                parse_attribute_value(&attr.value),
            ));
        }

        let expose_flat = node.expose_flat.unwrap_or(false);
        spec.expose_flat = expose_flat;
        for child in node.children {
            spec.children.push(build_node(child)?);
        }

        Ok(spec)
    }

    fn parse_namespace(value: &str) -> Result<Namespace, MockTreeLoadError> {
        match value {
            "control" => Ok(Namespace::Control),
            "item" => Ok(Namespace::Item),
            "app" => Ok(Namespace::App),
            "native" => Ok(Namespace::Native),
            other => Err(MockTreeLoadError::UnknownNamespace(other.to_owned())),
        }
    }

    fn parse_rect(value: &str) -> Result<Rect, MockTreeLoadError> {
        let parts: Vec<f64> = value
            .split(',')
            .map(|chunk| chunk.trim().parse::<f64>())
            .collect::<Result<_, _>>()
            .map_err(|_| MockTreeLoadError::InvalidRect(value.to_owned()))?;
        if parts.len() != 4 {
            return Err(MockTreeLoadError::InvalidRect(value.to_owned()));
        }
        Ok(Rect::new(parts[0], parts[1], parts[2], parts[3]))
    }

    fn parse_point(value: &str) -> Result<Point, MockTreeLoadError> {
        let parts: Vec<f64> = value
            .split(',')
            .map(|chunk| chunk.trim().parse::<f64>())
            .collect::<Result<_, _>>()
            .map_err(|_| MockTreeLoadError::InvalidPoint(value.to_owned()))?;
        if parts.len() != 2 {
            return Err(MockTreeLoadError::InvalidPoint(value.to_owned()));
        }
        Ok(Point::new(parts[0], parts[1]))
    }

    fn parse_attribute_value(value: &str) -> UiValue {
        if let Ok(boolean) = value.parse::<bool>() {
            return UiValue::from(boolean);
        }
        if let Ok(integer) = value.parse::<i64>() {
            return UiValue::from(integer);
        }
        if let Ok(number) = value.parse::<f64>() {
            return UiValue::from(number);
        }
        UiValue::from(value.to_owned())
    }

    fn parse_patterns(list: Option<&XmlPatternList>) -> Option<Vec<String>> {
        let raw = list.and_then(|p| p.value.as_ref())?;
        let entries = raw
            .split(',')
            .map(|entry| entry.trim())
            .filter(|entry| !entry.is_empty())
            .map(|entry| entry.to_owned())
            .collect::<Vec<_>>();
        if entries.is_empty() { None } else { Some(entries) }
    }

    fn push_bounds_attributes(
        spec: &mut NodeSpec,
        namespace: Namespace,
        rect: Rect,
        visible: bool,
        enabled: bool,
    ) {
        push_visibility_attributes(spec, namespace, visible, enabled);
        spec.attributes.push(AttributeSpec::new(namespace, element::BOUNDS, UiValue::Rect(rect)));
        for attr in rect_alias_attributes(namespace, "Bounds", rect) {
            spec.attributes.push(attr);
        }
    }

    fn push_visibility_attributes(
        spec: &mut NodeSpec,
        namespace: Namespace,
        visible: bool,
        enabled: bool,
    ) {
        spec.attributes.push(AttributeSpec::new(
            namespace,
            element::IS_VISIBLE,
            UiValue::from(visible),
        ));
        spec.attributes.push(AttributeSpec::new(
            namespace,
            element::IS_ENABLED,
            UiValue::from(enabled),
        ));
    }

    fn push_activation_point_attributes(spec: &mut NodeSpec, namespace: Namespace, point: Point) {
        spec.attributes.push(AttributeSpec::new(
            namespace,
            activation_target::ACTIVATION_POINT,
            UiValue::Point(point),
        ));
        spec.attributes.push(AttributeSpec::new(
            namespace,
            "ActivationPoint.X",
            UiValue::from(point.x()),
        ));
        spec.attributes.push(AttributeSpec::new(
            namespace,
            "ActivationPoint.Y",
            UiValue::from(point.y()),
        ));
    }

    fn rect_alias_attributes(namespace: Namespace, base: &str, rect: Rect) -> [AttributeSpec; 4] {
        [
            AttributeSpec::new(namespace, format!("{base}.X"), UiValue::from(rect.x())),
            AttributeSpec::new(namespace, format!("{base}.Y"), UiValue::from(rect.y())),
            AttributeSpec::new(namespace, format!("{base}.Width"), UiValue::from(rect.width())),
            AttributeSpec::new(namespace, format!("{base}.Height"), UiValue::from(rect.height())),
        ]
    }

    #[derive(Debug)]
    enum MockTreeLoadError {
        Xml(quick_xml::DeError),
        UnknownNamespace(String),
        InvalidRect(String),
        InvalidPoint(String),
    }

    impl fmt::Display for MockTreeLoadError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                MockTreeLoadError::Xml(err) => write!(f, "XML-Parsing fehlgeschlagen: {err}"),
                MockTreeLoadError::UnknownNamespace(ns) => {
                    write!(f, "unbekannter Namespace '{ns}' im Mock-Baum")
                }
                MockTreeLoadError::InvalidRect(value) => {
                    write!(f, "ungültiges Bounds-Format '{value}' (erwartet x,y,width,height)")
                }
                MockTreeLoadError::InvalidPoint(value) => {
                    write!(f, "ungültiges Point-Format '{value}' (erwartet x,y)")
                }
            }
        }
    }

    impl std::error::Error for MockTreeLoadError {}

    impl From<quick_xml::DeError> for MockTreeLoadError {
        fn from(err: quick_xml::DeError) -> Self {
            MockTreeLoadError::Xml(err)
        }
    }

    pub struct TreeGuard {
        previous: StaticMockTree,
    }

    impl Drop for TreeGuard {
        fn drop(&mut self) {
            *CURRENT_TREE.write().unwrap() = self.previous.clone();
        }
    }

    pub fn install_mock_tree(tree: StaticMockTree) -> TreeGuard {
        let mut lock = CURRENT_TREE.write().unwrap();
        let previous = lock.clone();
        *lock = tree;
        TreeGuard { previous }
    }

    pub fn reset_mock_tree() {
        *CURRENT_TREE.write().unwrap() = StaticMockTree::default();
    }

    pub(crate) fn instantiate_nodes(descriptor: &ProviderDescriptor) -> ProviderTree {
        let tree = CURRENT_TREE.read().unwrap().clone();
        let (roots, flat_nodes, all_nodes) = tree.instantiate(descriptor);
        let mut map = HashMap::new();
        for node in all_nodes {
            map.insert(node.runtime_id().as_str().to_owned(), node.clone());
        }
        (roots, flat_nodes, map)
    }

    fn instantiate_node(
        spec: &NodeSpec,
        descriptor: &ProviderDescriptor,
        parent: Option<&Arc<MockNode>>,
        all: &mut Vec<Arc<MockNode>>,
    ) -> Arc<MockNode> {
        let attributes: Vec<Arc<dyn UiAttribute>> = spec
            .attributes
            .iter()
            .map(|attr| super::attr(attr.namespace, attr.name.clone(), attr.value.clone()))
            .collect();

        let pattern_context = NodePatternContext {
            runtime_patterns: PatternRegistry::new(),
            supported_patterns: spec.patterns.iter().map(|p| PatternId::from(p.as_str())).collect(),
            order_key: spec.order_key,
        };

        let node = MockNode::new(
            spec.namespace,
            spec.role.clone(),
            spec.name.clone(),
            spec.runtime_id.as_str(),
            descriptor,
            attributes,
            pattern_context,
        );

        if let Some(parent_node) = parent {
            MockNode::add_child(parent_node, Arc::clone(&node));
        }

        all.push(Arc::clone(&node));

        for child in &spec.children {
            instantiate_node(child, descriptor, Some(&node), all);
        }

        node
    }
}

pub use tree::{
    AttributeSpec, NodeSpec, StaticMockTree, TreeGuard, install_mock_tree, reset_mock_tree,
};

#[derive(Clone)]
struct StaticAttribute {
    namespace: Namespace,
    name: String,
    value: UiValue,
}

impl UiAttribute for StaticAttribute {
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

fn attr(namespace: Namespace, name: impl Into<String>, value: UiValue) -> Arc<dyn UiAttribute> {
    Arc::new(StaticAttribute { namespace, name: name.into(), value })
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::types::Point;
    use platynui_core::ui::attribute_names::{activation_target, element, text_content};
    use platynui_core::ui::contract::testkit::{
        AttributeExpectation, NodeExpectation, PatternExpectation, require_node, verify_node,
    };
    use rstest::rstest;
    use serial_test::serial;

    const ELEMENT_EXPECTATIONS: [AttributeExpectation; 3] = [
        AttributeExpectation::required(Namespace::Control, element::BOUNDS),
        AttributeExpectation::required(Namespace::Control, element::IS_VISIBLE),
        AttributeExpectation::required(Namespace::Control, element::IS_ENABLED),
    ];

    const TEXT_CONTENT_EXPECTATIONS: [AttributeExpectation; 1] =
        [AttributeExpectation::required(Namespace::Control, text_content::TEXT)];

    const ACTIVATION_TARGET_EXPECTATIONS: [AttributeExpectation; 1] =
        [AttributeExpectation::required(Namespace::Control, activation_target::ACTIVATION_POINT)];

    struct DesktopNode;

    impl UiNode for DesktopNode {
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
            static ID: LazyLock<RuntimeId> = LazyLock::new(|| RuntimeId::from("mock://desktop"));
            &ID
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

    fn mock_provider() -> Arc<dyn UiTreeProvider> {
        reset_mock_tree();
        provider_factories()
            .find(|factory| factory.descriptor().id == PROVIDER_ID)
            .expect("mock provider registered")
            .create()
            .expect("mock provider instantiation")
    }

    fn find_by_runtime_id(node: Arc<dyn UiNode>, target: &str) -> Option<Arc<dyn UiNode>> {
        if node.runtime_id().as_str() == target {
            return Some(node);
        }
        for child in node.children() {
            if let Some(found) = find_by_runtime_id(Arc::clone(&child), target) {
                return Some(found);
            }
        }
        None
    }

    #[rstest]
    fn provider_registration_present() {
        let ids: Vec<_> = provider_factories().map(|factory| factory.descriptor().id).collect();
        assert!(ids.contains(&PROVIDER_ID));
    }

    #[rstest]
    #[serial]
    fn custom_tree_overrides_defaults() {
        let tree = StaticMockTree::new(vec![
            NodeSpec::new(Namespace::App, "Application", "Custom App", "mock://app/custom")
                .with_pattern("Application")
                .with_child(
                    NodeSpec::new(
                        Namespace::Control,
                        "Window",
                        "Custom Window",
                        "mock://window/custom",
                    )
                    .with_pattern("Element")
                    .with_child(
                        NodeSpec::new(
                            Namespace::Control,
                            "Button",
                            "Launch",
                            "mock://button/custom",
                        )
                        .with_pattern("Element"),
                    ),
                ),
        ]);

        let guard = install_mock_tree(tree);
        let provider = MockProvider::new(MockProviderFactory::descriptor_static());
        drop(guard);
        let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
        let mut roots =
            provider.get_nodes(Arc::clone(&desktop)).expect("custom tree root").collect::<Vec<_>>();

        assert_eq!(roots.len(), 1);
        let app = roots.pop().unwrap();
        assert_eq!(app.runtime_id().as_str(), "mock://app/custom");
    }

    #[rstest]
    #[serial]
    fn root_application_is_returned_with_parent() {
        let provider = mock_provider();
        let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
        let mut roots =
            provider.get_nodes(Arc::clone(&desktop)).expect("root enumeration").collect::<Vec<_>>();
        assert!(!roots.is_empty());
        let app = roots.remove(0);
        assert_eq!(app.namespace(), Namespace::App);
        assert_eq!(app.runtime_id().as_str(), APP_RUNTIME_ID);
        let parent = app.parent().and_then(|weak| weak.upgrade()).expect("desktop parent");
        assert_eq!(parent.runtime_id().as_str(), desktop.runtime_id().as_str());
    }

    #[rstest]
    #[serial]
    fn contract_expectations_for_button_hold() {
        let provider = mock_provider();
        let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
        let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
        let mut windows = provider.get_nodes(Arc::clone(&app)).unwrap();
        let window = windows
            .find(|node| node.runtime_id().as_str() == WINDOW_RUNTIME_ID)
            .expect("main window present");

        let button =
            find_by_runtime_id(window, BUTTON_RUNTIME_ID).expect("button reachable in mock tree");

        let expectations = NodeExpectation::default()
            .with_pattern(PatternExpectation::new(
                PatternId::from("Element"),
                &ELEMENT_EXPECTATIONS,
            ))
            .with_pattern(PatternExpectation::new(
                PatternId::from("TextContent"),
                &TEXT_CONTENT_EXPECTATIONS,
            ))
            .with_pattern(PatternExpectation::new(
                PatternId::from("ActivationTarget"),
                &ACTIVATION_TARGET_EXPECTATIONS,
            ));

        require_node(button.as_ref(), &expectations).expect("button contract satisfied");
        let issues = verify_node(button.as_ref(), &expectations);
        assert!(issues.is_empty(), "contract issues: {issues:?}");
    }

    #[rstest]
    #[serial]
    fn rect_aliases_present() {
        let provider = mock_provider();
        let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
        let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
        let mut windows = provider.get_nodes(Arc::clone(&app)).unwrap();
        let window = windows
            .find(|node| node.runtime_id().as_str() == WINDOW_RUNTIME_ID)
            .expect("main window present");

        let mut alias_names =
            window.attributes().map(|attr| attr.name().to_owned()).collect::<Vec<_>>();
        alias_names.sort();
        assert!(alias_names.contains(&"Bounds.X".to_owned()));
        assert!(alias_names.contains(&"Bounds.Y".to_owned()));
        assert!(alias_names.contains(&"Bounds.Width".to_owned()));
        assert!(alias_names.contains(&"Bounds.Height".to_owned()));
    }

    #[rstest]
    #[serial]
    fn activation_point_aliases_present() {
        let provider = mock_provider();
        let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
        let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
        let mut windows = provider.get_nodes(Arc::clone(&app)).unwrap();
        let window = windows
            .find(|node| node.runtime_id().as_str() == WINDOW_RUNTIME_ID)
            .expect("main window present");
        let button = find_by_runtime_id(window, BUTTON_RUNTIME_ID).expect("mock ok button present");

        let activation_point = button
            .attributes()
            .find(|attr| attr.name() == activation_target::ACTIVATION_POINT)
            .expect("activation point exists")
            .value();

        assert!(matches!(activation_point, UiValue::Point(Point { .. })));

        let aliases: Vec<_> = button
            .attributes()
            .filter(|attr| attr.name().starts_with("ActivationPoint."))
            .map(|attr| attr.name().to_owned())
            .collect();
        assert!(aliases.contains(&"ActivationPoint.X".to_owned()));
        assert!(aliases.contains(&"ActivationPoint.Y".to_owned()));
    }
}
