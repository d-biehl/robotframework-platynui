use std::sync::Arc;

use platynui_core::provider::ProviderError;
use platynui_core::ui::identifiers::RuntimeId;
use platynui_core::ui::{Namespace as UiNamespace, UiNode, UiValue};
use platynui_xpath::compiler;
use platynui_xpath::engine::evaluator;
use platynui_xpath::engine::runtime::{DynamicContextBuilder, StaticContextBuilder};
use platynui_xpath::model::{NodeKind, QName};
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use platynui_xpath::{self, XdmNode};
use thiserror::Error;

const CONTROL_NS_URI: &str = "urn:platynui:control";
const ITEM_NS_URI: &str = "urn:platynui:item";
const APP_NS_URI: &str = "urn:platynui:app";
const NATIVE_NS_URI: &str = "urn:platynui:native";

/// Resolves nodes by runtime identifier on demand (e.g. after provider reloads).
pub trait NodeResolver: Send + Sync {
    fn resolve(&self, runtime_id: &RuntimeId) -> Result<Option<Arc<dyn UiNode>>, ProviderError>;
}

#[derive(Clone)]
pub struct EvaluateOptions {
    desktop: Arc<dyn UiNode>,
    invalidate_before_eval: bool,
    resolver: Option<Arc<dyn NodeResolver>>,
}

impl EvaluateOptions {
    pub fn new(desktop: Arc<dyn UiNode>) -> Self {
        Self { desktop, invalidate_before_eval: false, resolver: None }
    }

    pub fn desktop(&self) -> Arc<dyn UiNode> {
        Arc::clone(&self.desktop)
    }

    pub fn with_invalidation(mut self, invalidate: bool) -> Self {
        self.invalidate_before_eval = invalidate;
        self
    }

    pub fn invalidate_before_eval(&self) -> bool {
        self.invalidate_before_eval
    }

    pub fn with_node_resolver(mut self, resolver: Arc<dyn NodeResolver>) -> Self {
        self.resolver = Some(resolver);
        self
    }

    pub fn node_resolver(&self) -> Option<Arc<dyn NodeResolver>> {
        self.resolver.as_ref().map(Arc::clone)
    }
}

#[derive(Debug, Error)]
pub enum EvaluateError {
    #[error("XPath evaluation failed: {0}")]
    XPath(#[from] platynui_xpath::engine::runtime::Error),
    #[error("context node not part of current evaluation (runtime id: {0})")]
    ContextNodeUnknown(String),
    #[error("provider error during context resolution: {0}")]
    Provider(#[from] ProviderError),
}

#[derive(Clone)]
pub struct EvaluatedAttribute {
    pub owner: Arc<dyn UiNode>,
    pub namespace: UiNamespace,
    pub name: String,
    pub value: UiValue,
}

impl std::fmt::Debug for EvaluatedAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvaluatedAttribute")
            .field("namespace", &self.namespace)
            .field("name", &self.name)
            .field("value", &self.value)
            .finish()
    }
}

#[derive(Clone)]
pub enum EvaluationItem {
    Node(Arc<dyn UiNode>),
    Attribute(EvaluatedAttribute),
    Value(UiValue),
}

impl std::fmt::Debug for EvaluationItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvaluationItem::Node(node) => {
                f.debug_tuple("Node").field(&node.runtime_id().as_str()).finish()
            }
            EvaluationItem::Attribute(attr) => f.debug_tuple("Attribute").field(attr).finish(),
            EvaluationItem::Value(value) => f.debug_tuple("Value").field(value).finish(),
        }
    }
}

pub fn evaluate(
    node: Option<Arc<dyn UiNode>>,
    xpath: &str,
    options: EvaluateOptions,
) -> Result<Vec<EvaluationItem>, EvaluateError> {
    let root = options.desktop();
    let context = if let Some(node_ref) = node.as_ref() {
        if let Some(resolver) = options.node_resolver() {
            let runtime_id = node_ref.runtime_id().clone();
            match resolver.resolve(&runtime_id)? {
                Some(resolved) => resolved,
                None => return Err(EvaluateError::ContextNodeUnknown(runtime_id.to_string())),
            }
        } else {
            Arc::clone(node_ref)
        }
    } else {
        root.clone()
    };

    if options.invalidate_before_eval() {
        context.invalidate();
    }

    let static_ctx = StaticContextBuilder::new()
        .with_default_element_namespace(CONTROL_NS_URI)
        .with_namespace("control", CONTROL_NS_URI)
        .with_namespace("item", ITEM_NS_URI)
        .with_namespace("app", APP_NS_URI)
        .with_namespace("native", NATIVE_NS_URI)
        .build();

    let compiled = compiler::compile_with_context(xpath, &static_ctx)?;

    let mut dyn_builder = DynamicContextBuilder::new();
    dyn_builder = dyn_builder.with_context_item(RuntimeXdmNode::element(context.clone()));
    let dyn_ctx = dyn_builder.build();

    let sequence = evaluator::evaluate(&compiled, &dyn_ctx)?;

    let mut items = Vec::new();
    for item in sequence {
        match item {
            XdmItem::Node(node) => match node {
                RuntimeXdmNode::Document(doc) => {
                    items.push(EvaluationItem::Node(doc.root.clone()));
                }
                RuntimeXdmNode::Element(element) => {
                    items.push(EvaluationItem::Node(element.node.clone()));
                }
                RuntimeXdmNode::Attribute(attr) => {
                    items.push(EvaluationItem::Attribute(attr.to_evaluated()));
                }
            },
            XdmItem::Atomic(atom) => {
                items.push(EvaluationItem::Value(atomic_to_ui_value(&atom)));
            }
        }
    }

    Ok(items)
}

#[derive(Clone)]
enum RuntimeXdmNode {
    Document(DocumentData),
    Element(ElementData),
    Attribute(AttributeData),
}

impl RuntimeXdmNode {
    fn document(root: Arc<dyn UiNode>) -> Self {
        let runtime_id = root.runtime_id().as_str().to_string();
        RuntimeXdmNode::Document(DocumentData { root, runtime_id })
    }

    fn element(node: Arc<dyn UiNode>) -> Self {
        let runtime_id = node.runtime_id().as_str().to_string();
        let namespace = node.namespace();
        let role = node.role().to_string();
        RuntimeXdmNode::Element(ElementData { node, runtime_id, namespace, role })
    }

    fn attribute(
        owner: Arc<dyn UiNode>,
        namespace: UiNamespace,
        name: String,
        value: UiValue,
        text: String,
    ) -> Self {
        let owner_runtime_id = owner.runtime_id().as_str().to_string();
        RuntimeXdmNode::Attribute(AttributeData {
            owner,
            owner_runtime_id,
            namespace,
            name,
            value,
            text,
        })
    }
}

impl PartialEq for RuntimeXdmNode {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RuntimeXdmNode::Document(a), RuntimeXdmNode::Document(b)) => {
                a.runtime_id == b.runtime_id
            }
            (RuntimeXdmNode::Element(a), RuntimeXdmNode::Element(b)) => {
                a.runtime_id == b.runtime_id
            }
            (RuntimeXdmNode::Attribute(a), RuntimeXdmNode::Attribute(b)) => {
                a.owner_runtime_id == b.owner_runtime_id
                    && a.namespace == b.namespace
                    && a.name == b.name
            }
            _ => false,
        }
    }
}

impl Eq for RuntimeXdmNode {}

impl std::fmt::Debug for RuntimeXdmNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeXdmNode::Document(doc) => {
                f.debug_struct("Document").field("runtime_id", &doc.runtime_id).finish()
            }
            RuntimeXdmNode::Element(elem) => f
                .debug_struct("Element")
                .field("runtime_id", &elem.runtime_id)
                .field("role", &elem.role)
                .finish(),
            RuntimeXdmNode::Attribute(attr) => f
                .debug_struct("Attribute")
                .field("owner", &attr.owner_runtime_id)
                .field("name", &attr.name)
                .finish(),
        }
    }
}

impl XdmNode for RuntimeXdmNode {
    type Children<'a>
        = std::vec::IntoIter<RuntimeXdmNode>
    where
        Self: 'a;
    type Attributes<'a>
        = std::vec::IntoIter<RuntimeXdmNode>
    where
        Self: 'a;
    type Namespaces<'a>
        = std::vec::IntoIter<RuntimeXdmNode>
    where
        Self: 'a;

    fn kind(&self) -> NodeKind {
        match self {
            RuntimeXdmNode::Document(_) => NodeKind::Document,
            RuntimeXdmNode::Element(_) => NodeKind::Element,
            RuntimeXdmNode::Attribute(_) => NodeKind::Attribute,
        }
    }

    fn name(&self) -> Option<QName> {
        match self {
            RuntimeXdmNode::Document(_) => None,
            RuntimeXdmNode::Element(elem) => Some(element_qname(elem.namespace, &elem.role)),
            RuntimeXdmNode::Attribute(attr) => Some(attribute_qname(attr.namespace, &attr.name)),
        }
    }

    fn string_value(&self) -> String {
        match self {
            RuntimeXdmNode::Document(_) => String::new(),
            RuntimeXdmNode::Element(_) => String::new(),
            RuntimeXdmNode::Attribute(attr) => attr.text.clone(),
        }
    }

    fn base_uri(&self) -> Option<String> {
        None
    }

    fn parent(&self) -> Option<Self> {
        match self {
            RuntimeXdmNode::Document(_) => None,
            RuntimeXdmNode::Element(elem) => match elem.node.parent() {
                Some(parent) => parent.upgrade().map(RuntimeXdmNode::element),
                None => Some(RuntimeXdmNode::document(elem.node.clone())),
            },
            RuntimeXdmNode::Attribute(attr) => Some(RuntimeXdmNode::element(attr.owner.clone())),
        }
    }

    fn children(&self) -> Self::Children<'_> {
        match self {
            RuntimeXdmNode::Document(doc) => {
                vec![RuntimeXdmNode::element(doc.root.clone())].into_iter()
            }
            RuntimeXdmNode::Element(elem) => {
                let mut results = Vec::new();
                for child in elem.node.children() {
                    results.push(RuntimeXdmNode::element(child));
                }
                results.into_iter()
            }
            RuntimeXdmNode::Attribute(_) => Vec::new().into_iter(),
        }
    }

    fn attributes(&self) -> Self::Attributes<'_> {
        match self {
            RuntimeXdmNode::Element(elem) => collect_attribute_nodes(&elem.node).into_iter(),
            _ => Vec::new().into_iter(),
        }
    }

    fn namespaces(&self) -> Self::Namespaces<'_> {
        Vec::new().into_iter()
    }
}

#[derive(Clone)]
struct DocumentData {
    root: Arc<dyn UiNode>,
    runtime_id: String,
}

#[derive(Clone)]
struct ElementData {
    node: Arc<dyn UiNode>,
    runtime_id: String,
    namespace: UiNamespace,
    role: String,
}

#[derive(Clone)]
struct AttributeData {
    owner: Arc<dyn UiNode>,
    owner_runtime_id: String,
    namespace: UiNamespace,
    name: String,
    value: UiValue,
    text: String,
}

impl AttributeData {
    fn to_evaluated(&self) -> EvaluatedAttribute {
        EvaluatedAttribute {
            owner: self.owner.clone(),
            namespace: self.namespace,
            name: self.name.clone(),
            value: self.value.clone(),
        }
    }
}

fn collect_attribute_nodes(node: &Arc<dyn UiNode>) -> Vec<RuntimeXdmNode> {
    let mut nodes = Vec::new();

    for attribute in node.attributes() {
        let namespace = attribute.namespace();
        let base_name = attribute.name().to_string();
        let value = attribute.value();

        if let Some(text) = ui_value_to_string(&value) {
            nodes.push(RuntimeXdmNode::attribute(
                Arc::clone(node),
                namespace,
                base_name.clone(),
                value.clone(),
                text,
            ));
        }

        for (derived_name, derived_value) in expand_structured_attribute(&base_name, &value) {
            if let Some(text) = ui_value_to_string(&derived_value) {
                nodes.push(RuntimeXdmNode::attribute(
                    Arc::clone(node),
                    namespace,
                    derived_name,
                    derived_value,
                    text,
                ));
            }
        }
    }

    nodes
}

fn expand_structured_attribute(base_name: &str, value: &UiValue) -> Vec<(String, UiValue)> {
    match value {
        UiValue::Rect(rect) => vec![
            (component_attribute_name(base_name, "X"), UiValue::from(rect.x())),
            (component_attribute_name(base_name, "Y"), UiValue::from(rect.y())),
            (component_attribute_name(base_name, "Width"), UiValue::from(rect.width())),
            (component_attribute_name(base_name, "Height"), UiValue::from(rect.height())),
        ],
        UiValue::Point(point) => vec![
            (component_attribute_name(base_name, "X"), UiValue::from(point.x())),
            (component_attribute_name(base_name, "Y"), UiValue::from(point.y())),
        ],
        UiValue::Size(size) => vec![
            (component_attribute_name(base_name, "Width"), UiValue::from(size.width())),
            (component_attribute_name(base_name, "Height"), UiValue::from(size.height())),
        ],
        _ => Vec::new(),
    }
}

fn component_attribute_name(base: &str, suffix: &str) -> String {
    format!("{}.{}", base, suffix)
}

fn ui_value_to_string(value: &UiValue) -> Option<String> {
    match value {
        UiValue::Null => None,
        UiValue::Bool(b) => Some(b.to_string()),
        UiValue::Integer(i) => Some(i.to_string()),
        UiValue::Number(n) => Some(trim_float(*n)),
        UiValue::String(s) => Some(s.clone()),
        UiValue::Array(items) => serde_json::to_string(items).ok(),
        UiValue::Object(map) => serde_json::to_string(map).ok(),
        UiValue::Point(p) => serde_json::to_string(p).ok(),
        UiValue::Size(s) => serde_json::to_string(s).ok(),
        UiValue::Rect(r) => serde_json::to_string(r).ok(),
    }
}

fn trim_float(value: f64) -> String {
    let s = format!("{}", value);
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

fn element_qname(ns: UiNamespace, role: &str) -> QName {
    QName {
        prefix: namespace_prefix(ns).map(|p| p.to_string()),
        local: role.to_string(),
        ns_uri: Some(namespace_uri(ns).to_string()),
    }
}

fn attribute_qname(ns: UiNamespace, name: &str) -> QName {
    QName {
        prefix: attribute_prefix(ns).map(|p| p.to_string()),
        local: name.to_string(),
        ns_uri: attribute_namespace(ns).map(|uri| uri.to_string()),
    }
}

fn namespace_prefix(ns: UiNamespace) -> Option<&'static str> {
    match ns {
        UiNamespace::Control => None,
        UiNamespace::Item => Some("item"),
        UiNamespace::App => Some("app"),
        UiNamespace::Native => Some("native"),
    }
}

fn namespace_uri(ns: UiNamespace) -> &'static str {
    match ns {
        UiNamespace::Control => CONTROL_NS_URI,
        UiNamespace::Item => ITEM_NS_URI,
        UiNamespace::App => APP_NS_URI,
        UiNamespace::Native => NATIVE_NS_URI,
    }
}

fn attribute_prefix(ns: UiNamespace) -> Option<&'static str> {
    match ns {
        UiNamespace::Control => None,
        UiNamespace::Item => Some("item"),
        UiNamespace::App => Some("app"),
        UiNamespace::Native => Some("native"),
    }
}

fn attribute_namespace(ns: UiNamespace) -> Option<&'static str> {
    match ns {
        UiNamespace::Control => None,
        UiNamespace::Item => Some(ITEM_NS_URI),
        UiNamespace::App => Some(APP_NS_URI),
        UiNamespace::Native => Some(NATIVE_NS_URI),
    }
}

fn atomic_to_ui_value(value: &XdmAtomicValue) -> UiValue {
    use XdmAtomicValue::*;
    match value {
        Boolean(b) => UiValue::Bool(*b),
        String(s) | UntypedAtomic(s) | AnyUri(s) | NormalizedString(s) | Token(s) | Language(s)
        | Name(s) | NCName(s) | NMTOKEN(s) | Id(s) | IdRef(s) | Entity(s) | Notation(s) => {
            UiValue::String(s.clone())
        }
        Integer(i) | Long(i) | NonPositiveInteger(i) | NegativeInteger(i) => UiValue::Integer(*i),
        Decimal(d) | Double(d) => UiValue::Number(*d),
        Float(f) => UiValue::Number(*f as f64),
        UnsignedLong(u) => UiValue::Integer(*u as i64),
        NonNegativeInteger(u) => UiValue::Integer(*u as i64),
        PositiveInteger(u) => UiValue::Integer(*u as i64),
        UnsignedInt(u) => UiValue::Integer(*u as i64),
        UnsignedShort(u) => UiValue::Integer(*u as i64),
        UnsignedByte(u) => UiValue::Integer(*u as i64),
        Int(i) => UiValue::Integer(*i as i64),
        Short(i) => UiValue::Integer(*i as i64),
        Byte(i) => UiValue::Integer(*i as i64),
        QName { ns_uri, prefix, local } => {
            let mut map = std::collections::BTreeMap::new();
            if let Some(ns) = ns_uri {
                map.insert("ns_uri".to_string(), UiValue::String(ns.clone()));
            }
            if let Some(pref) = prefix {
                map.insert("prefix".to_string(), UiValue::String(pref.clone()));
            }
            map.insert("local".to_string(), UiValue::String(local.clone()));
            UiValue::Object(map)
        }
        DateTime(dt) => UiValue::String(dt.to_rfc3339()),
        Date { date, tz } => UiValue::String(match tz {
            Some(offset) => format!("{}{}", date, offset),
            None => date.to_string(),
        }),
        Time { time, tz } => UiValue::String(match tz {
            Some(offset) => format!("{}{}", time, offset),
            None => time.to_string(),
        }),
        YearMonthDuration(months) => UiValue::String(format!("P{}M", months)),
        DayTimeDuration(secs) => UiValue::String(format!("PT{}S", secs)),
        Base64Binary(data) | HexBinary(data) => UiValue::String(data.clone()),
        GYear { year, tz } => {
            UiValue::String(format!("{}{}", year, tz.map_or("".to_string(), |o| o.to_string())))
        }
        GYearMonth { year, month, tz } => UiValue::String(format!(
            "{}-{:02}{}",
            year,
            month,
            tz.map_or("".to_string(), |o| o.to_string())
        )),
        GMonth { month, tz } => {
            UiValue::String(format!("{:02}{}", month, tz.map_or("".to_string(), |o| o.to_string())))
        }
        GMonthDay { month, day, tz } => UiValue::String(format!(
            "{:02}-{:02}{}",
            month,
            day,
            tz.map_or("".to_string(), |o| o.to_string())
        )),
        GDay { day, tz } => {
            UiValue::String(format!("{:02}{}", day, tz.map_or("".to_string(), |o| o.to_string())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::provider::{ProviderError, ProviderErrorKind};
    use platynui_core::types::Rect;
    use platynui_core::ui::{
        PatternId, RuntimeId, UiAttribute, UiNode, attribute_names, supported_patterns_value,
    };
    use rstest::rstest;
    use std::sync::{Arc, Mutex, Weak};

    struct StaticAttribute {
        namespace: UiNamespace,
        name: String,
        value: UiValue,
    }

    impl StaticAttribute {
        fn new(namespace: UiNamespace, name: &str, value: UiValue) -> Self {
            Self { namespace, name: name.to_string(), value }
        }
    }

    impl UiAttribute for StaticAttribute {
        fn namespace(&self) -> UiNamespace {
            self.namespace
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn value(&self) -> UiValue {
            self.value.clone()
        }
    }

    struct StaticNode {
        namespace: UiNamespace,
        role: &'static str,
        name: String,
        runtime_id: RuntimeId,
        attributes: Vec<Arc<dyn UiAttribute>>,
        patterns: Vec<PatternId>,
        children: Mutex<Vec<Arc<dyn UiNode>>>,
        parent: Mutex<Option<Weak<dyn UiNode>>>,
    }

    impl StaticNode {
        fn new(
            namespace: UiNamespace,
            runtime_id: &str,
            role: &'static str,
            name: &str,
            bounds: Rect,
            patterns: Vec<&str>,
        ) -> Arc<Self> {
            let runtime_id = RuntimeId::from(runtime_id);
            let patterns_vec: Vec<PatternId> = patterns.into_iter().map(PatternId::from).collect();
            let supported = supported_patterns_value(&patterns_vec);

            let mut attributes: Vec<Arc<dyn UiAttribute>> = vec![
                Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::element::BOUNDS,
                    UiValue::Rect(bounds),
                )) as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::common::ROLE,
                    UiValue::from(role),
                )) as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::common::NAME,
                    UiValue::from(name),
                )) as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::element::IS_VISIBLE,
                    UiValue::from(true),
                )) as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::element::IS_ENABLED,
                    UiValue::from(true),
                )) as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::common::RUNTIME_ID,
                    UiValue::from(runtime_id.as_str().to_owned()),
                )) as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::common::TECHNOLOGY,
                    UiValue::from("Mock"),
                )) as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::common::SUPPORTED_PATTERNS,
                    supported,
                )) as Arc<dyn UiAttribute>,
            ];

            if role == "Desktop" {
                attributes.push(Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::desktop::DISPLAY_COUNT,
                    UiValue::from(1_i64),
                )) as Arc<dyn UiAttribute>);
                attributes.push(Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::desktop::OS_NAME,
                    UiValue::from("Test OS"),
                )) as Arc<dyn UiAttribute>);
                attributes.push(Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::desktop::OS_VERSION,
                    UiValue::from("1.0"),
                )) as Arc<dyn UiAttribute>);
                let mut monitor = std::collections::BTreeMap::new();
                monitor.insert("Name".to_string(), UiValue::from("Display 1"));
                monitor.insert("Bounds".to_string(), UiValue::Rect(bounds));
                attributes.push(Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::desktop::MONITORS,
                    UiValue::Array(vec![UiValue::Object(monitor)]),
                )) as Arc<dyn UiAttribute>);
            }

            let node = Arc::new(Self {
                namespace,
                role,
                name: name.to_string(),
                runtime_id,
                attributes,
                patterns: patterns_vec,
                children: Mutex::new(Vec::new()),
                parent: Mutex::new(None),
            });

            if matches!(namespace, UiNamespace::Control | UiNamespace::Item) {
                platynui_core::ui::validate_control_or_item(node.as_ref())
                    .expect("StaticNode violates UiNode contract");
            }

            node
        }

        fn to_ref(this: &Arc<Self>) -> Arc<dyn UiNode> {
            Arc::clone(this) as Arc<dyn UiNode>
        }

        fn add_child(parent: &Arc<Self>, child: &Arc<Self>) {
            *child.parent.lock().unwrap() =
                Some(Arc::downgrade(&(Arc::clone(parent) as Arc<dyn UiNode>)));
            parent.children.lock().unwrap().push(Self::to_ref(child));
        }
    }

    impl UiNode for StaticNode {
        fn namespace(&self) -> UiNamespace {
            self.namespace
        }

        fn role(&self) -> &str {
            self.role
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
            Box::new(snapshot.into_iter())
        }

        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_> {
            Box::new(self.attributes.clone().into_iter())
        }

        fn supported_patterns(&self) -> &[PatternId] {
            &self.patterns
        }

        fn invalidate(&self) {}
    }

    fn sample_tree() -> Arc<dyn UiNode> {
        let window = StaticNode::new(
            UiNamespace::Control,
            "window-1",
            "Window",
            "Main",
            Rect::new(0.0, 0.0, 800.0, 600.0),
            vec![],
        );
        let desktop = StaticNode::new(
            UiNamespace::Control,
            "desktop",
            "Desktop",
            "Desktop",
            Rect::new(0.0, 0.0, 1920.0, 1080.0),
            vec![],
        );

        StaticNode::add_child(&desktop, &window);

        StaticNode::to_ref(&desktop)
    }

    #[rstest]
    fn evaluates_node_selection() {
        let tree = sample_tree();
        let items = evaluate(None, "//Window", EvaluateOptions::new(tree.clone())).unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            EvaluationItem::Node(node) => {
                assert_eq!(node.runtime_id().as_str(), "window-1");
            }
            other => panic!("unexpected evaluation result: {:?}", other),
        }
    }

    #[rstest]
    fn evaluates_count_function() {
        let tree = sample_tree();
        let items = evaluate(None, "count(//Window)", EvaluateOptions::new(tree.clone())).unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            EvaluationItem::Value(value) => assert_eq!(value, &UiValue::Integer(1)),
            other => panic!("unexpected evaluation result: {:?}", other),
        }
    }

    #[rstest]
    fn absolute_path_from_context_root() {
        let tree = sample_tree();
        let items = evaluate(None, "/control:Desktop", EvaluateOptions::new(tree.clone())).unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            EvaluationItem::Node(node) => {
                assert_eq!(node.runtime_id().as_str(), "desktop");
            }
            other => panic!("unexpected evaluation result: {:?}", other),
        }
    }

    #[rstest]
    fn desktop_bounds_alias_attributes_are_available() {
        let tree = sample_tree();
        let items =
            evaluate(None, "/control:Desktop/@Bounds.X", EvaluateOptions::new(tree.clone()))
                .unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            EvaluationItem::Attribute(attr) => {
                assert_eq!(attr.name, "Bounds.X");
                assert_eq!(attr.value, UiValue::Number(0.0));
            }
            other => panic!("unexpected attribute result: {:?}", other),
        }
    }

    #[rstest]
    fn desktop_monitors_attribute_is_exposed() {
        let tree = sample_tree();
        let items =
            evaluate(None, "/control:Desktop/@Monitors", EvaluateOptions::new(tree.clone()))
                .unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            EvaluationItem::Attribute(attr) => {
                assert_eq!(attr.name, "Monitors");
                match &attr.value {
                    UiValue::Array(monitors) => {
                        assert_eq!(monitors.len(), 1);
                    }
                    other => panic!("unexpected attribute type: {:?}", other),
                }
            }
            other => panic!("unexpected monitors result: {:?}", other),
        }
    }

    struct ResolverOk {
        node: Arc<dyn UiNode>,
    }

    impl NodeResolver for ResolverOk {
        fn resolve(
            &self,
            _runtime_id: &RuntimeId,
        ) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
            Ok(Some(self.node.clone()))
        }
    }

    struct ResolverMissing;

    impl NodeResolver for ResolverMissing {
        fn resolve(
            &self,
            _runtime_id: &RuntimeId,
        ) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
            Ok(None)
        }
    }

    struct ResolverError;

    impl NodeResolver for ResolverError {
        fn resolve(
            &self,
            _runtime_id: &RuntimeId,
        ) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
            Err(ProviderError::simple(ProviderErrorKind::TreeUnavailable))
        }
    }

    #[rstest]
    fn context_is_re_resolved_via_resolver() {
        let tree = sample_tree();
        let stale = StaticNode::new(
            UiNamespace::Control,
            "stale-window",
            "Window",
            "Old",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            vec![],
        );
        let fresh = StaticNode::new(
            UiNamespace::Control,
            "stale-window",
            "Window",
            "New",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            vec![],
        );
        let stale_node = StaticNode::to_ref(&stale);
        let fresh_node = StaticNode::to_ref(&fresh);
        let resolver = Arc::new(ResolverOk { node: fresh_node.clone() });

        let items = evaluate(
            Some(stale_node.clone()),
            ".",
            EvaluateOptions::new(tree.clone()).with_node_resolver(resolver),
        )
        .unwrap();

        match &items[0] {
            EvaluationItem::Node(node) => {
                assert!(Arc::ptr_eq(node, &fresh_node));
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }

    #[rstest]
    fn context_missing_yields_error() {
        let tree = sample_tree();
        let stale = StaticNode::new(
            UiNamespace::Control,
            "missing-window",
            "Window",
            "Old",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            vec![],
        );
        let stale_node = StaticNode::to_ref(&stale);
        let runtime_id = stale_node.runtime_id().as_str().to_string();
        let resolver = Arc::new(ResolverMissing);

        let result = evaluate(
            Some(stale_node.clone()),
            ".",
            EvaluateOptions::new(tree.clone()).with_node_resolver(resolver),
        );

        match result {
            Err(EvaluateError::ContextNodeUnknown(id)) => assert_eq!(id, runtime_id),
            other => panic!("unexpected result: {:?}", other),
        }
    }

    #[rstest]
    fn resolver_error_is_propagated() {
        let tree = sample_tree();
        let stale = StaticNode::new(
            UiNamespace::Control,
            "errored-window",
            "Window",
            "Old",
            Rect::new(0.0, 0.0, 100.0, 100.0),
            vec![],
        );
        let stale_node = StaticNode::to_ref(&stale);
        let resolver = Arc::new(ResolverError);

        let result = evaluate(
            Some(stale_node.clone()),
            ".",
            EvaluateOptions::new(tree.clone()).with_node_resolver(resolver),
        );

        match result {
            Err(EvaluateError::Provider(err)) => {
                assert_eq!(err.kind, ProviderErrorKind::TreeUnavailable);
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }
}
