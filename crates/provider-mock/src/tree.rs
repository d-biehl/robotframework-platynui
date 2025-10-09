use crate::focus;
use crate::input;
use crate::node::{MockNode, NodePatternContext, attr};
use crate::window;
use platynui_core::provider::ProviderDescriptor;
use platynui_core::types::{Point, Rect};
use platynui_core::ui::attribute_names::{activation_target, element, focusable};
use platynui_core::ui::{
    FocusableAction, Namespace, PatternId, PatternRegistry, RuntimeId, UiAttribute, UiNode, UiPattern, UiValue,
};
use quick_xml::de::from_str;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, LazyLock, RwLock};

type NodeList = Vec<Arc<MockNode>>;
pub(crate) type ProviderTree = (NodeList, NodeList, HashMap<String, Arc<MockNode>>);

static CURRENT_TREE: LazyLock<RwLock<StaticMockTree>> = LazyLock::new(|| RwLock::new(StaticMockTree::default()));

type InstantiatedTree = (NodeList, NodeList, NodeList);

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

    pub fn namespace(&self) -> Namespace {
        self.namespace
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &UiValue {
        &self.value
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
    text: Option<String>,
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
            text: None,
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

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
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
        let roots: NodeList =
            self.roots.iter().map(|spec| instantiate_node(spec, descriptor, None, &mut all)).collect();
        let flat: NodeList =
            self.flat_specs.iter().map(|spec| instantiate_node(spec, descriptor, None, &mut all)).collect();
        (roots, flat, all)
    }
}

impl Default for StaticMockTree {
    fn default() -> Self {
        const XML: &str = include_str!("../assets/mock_tree.xml");
        Self::from_xml(XML).expect("embedded mock_tree.xml could not be parsed")
    }
}

impl StaticMockTree {
    pub fn from_xml(xml: &str) -> Result<Self, MockTreeLoadError> {
        let parsed: XmlTree = from_str(xml).map_err(MockTreeLoadError::Xml)?;
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
    let mut spec = NodeSpec::new(namespace, node.role.clone(), node.name.clone(), node.runtime_id.clone());

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
        spec.text = Some(text.clone());
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
        spec.attributes.push(AttributeSpec::new(attr_namespace, attr.name, parse_attribute_value(&attr.value)));
    }

    spec.expose_flat = node.expose_flat.unwrap_or(false);
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

fn push_bounds_attributes(spec: &mut NodeSpec, namespace: Namespace, rect: Rect, visible: bool, enabled: bool) {
    push_visibility_attributes(spec, namespace, visible, enabled);
    spec.attributes.push(AttributeSpec::new(namespace, element::BOUNDS, UiValue::Rect(rect)));
}

fn push_visibility_attributes(spec: &mut NodeSpec, namespace: Namespace, visible: bool, enabled: bool) {
    spec.attributes.push(AttributeSpec::new(namespace, element::IS_VISIBLE, UiValue::from(visible)));
    spec.attributes.push(AttributeSpec::new(namespace, element::IS_ENABLED, UiValue::from(enabled)));
}

fn push_activation_point_attributes(spec: &mut NodeSpec, namespace: Namespace, point: Point) {
    spec.attributes.push(AttributeSpec::new(namespace, activation_target::ACTIVATION_POINT, UiValue::Point(point)));
}

#[derive(Debug)]
pub enum MockTreeLoadError {
    Xml(quick_xml::DeError),
    UnknownNamespace(String),
    InvalidRect(String),
    InvalidPoint(String),
}

impl fmt::Display for MockTreeLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MockTreeLoadError::Xml(err) => write!(f, "XML parsing failed: {err}"),
            MockTreeLoadError::UnknownNamespace(ns) => {
                write!(f, "unknown namespace '{ns}' in mock tree")
            }
            MockTreeLoadError::InvalidRect(value) => {
                write!(f, "invalid bounds format '{value}' (expected x,y,width,height)")
            }
            MockTreeLoadError::InvalidPoint(value) => {
                write!(f, "invalid point format '{value}' (expected x,y)")
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
    input::reset_text_buffers();
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
    let runtime_id = RuntimeId::from(spec.runtime_id.as_str());

    let runtime_patterns = PatternRegistry::new();
    let mut dynamic_attributes: Vec<Arc<dyn UiAttribute>> = Vec::new();
    let mut declared_patterns: Vec<PatternId> = Vec::new();

    if let Some(text) = spec.text.as_ref() {
        dynamic_attributes.push(input::register_text_attribute(spec.namespace, &runtime_id, text));
    }

    let has_window_surface = spec.patterns.iter().any(|pattern| pattern == "WindowSurface");
    let has_focusable = spec.patterns.iter().any(|pattern| pattern == "Focusable");
    let window_config = has_window_surface.then(|| window::derive_config(&spec.attributes));

    let initial_focus = if has_focusable {
        spec.attributes.iter().any(|attr| {
            attr.namespace() == Namespace::Control
                && attr.name() == focusable::IS_FOCUSED
                && matches!(attr.value(), UiValue::Bool(true))
        })
    } else {
        false
    };

    for pattern in &spec.patterns {
        match pattern.as_str() {
            "Focusable" => {
                let action_runtime_id = runtime_id.clone();
                runtime_patterns.register_lazy(PatternId::from("Focusable"), move || {
                    let target = action_runtime_id.clone();
                    let pattern: Arc<dyn UiPattern> =
                        Arc::new(FocusableAction::new(move || focus::request_focus(target.clone())));
                    Some(pattern)
                });
                dynamic_attributes.push(focus::focus_attribute(spec.namespace, runtime_id.clone()));
            }
            "WindowSurface" => {}
            other => declared_patterns.push(PatternId::from(other)),
        }
    }

    if let Some(config) = window_config {
        dynamic_attributes.extend(window::register_window(
            runtime_id.clone(),
            spec.namespace,
            config,
            &runtime_patterns,
        ));
    }

    if has_focusable && initial_focus {
        let _ = focus::request_focus(runtime_id.clone());
    }

    let pattern_context = NodePatternContext { runtime_patterns, declared_patterns, order_key: spec.order_key };

    let technology = descriptor.technology.as_str();

    let mut attributes: Vec<Arc<dyn UiAttribute>> = spec
        .attributes
        .iter()
        .filter(|attr_spec| {
            if has_window_surface && window::should_filter_attribute(attr_spec.name()) {
                return false;
            }
            if has_focusable && attr_spec.namespace() == Namespace::Control && attr_spec.name() == focusable::IS_FOCUSED
            {
                return false;
            }
            true
        })
        .map(|attr_spec| attr(attr_spec.namespace(), attr_spec.name().to_owned(), attr_spec.value().clone()))
        .collect();
    attributes.extend(dynamic_attributes);

    let node = MockNode::new(
        spec.namespace,
        spec.role.clone(),
        spec.name.clone(),
        runtime_id.as_str(),
        technology,
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
