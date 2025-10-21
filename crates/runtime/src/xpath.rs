//
use platynui_core::ui::PatternId;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use platynui_core::provider::ProviderError;
use platynui_core::ui::attribute_names;
use platynui_core::ui::identifiers::RuntimeId;
use platynui_core::ui::{Namespace as UiNamespace, UiAttribute, UiNode, UiValue};
use platynui_xpath::compiler;
use platynui_xpath::engine::evaluator;
use platynui_xpath::engine::runtime::{DynamicContextBuilder, StaticContextBuilder};
use platynui_xpath::model::{NodeKind, QName};
use platynui_xpath::xdm::XdmAtomicValue;
use platynui_xpath::{self, XdmNode};
use thiserror::Error;

const CONTROL_NS_URI: &str = "urn:platynui:control";
const ITEM_NS_URI: &str = "urn:platynui:item";
const APP_NS_URI: &str = "urn:platynui:app";
const NATIVE_NS_URI: &str = "urn:platynui:native";

// Type aliases for complex iterator types to satisfy clippy::type_complexity
type NodeIterator = Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>;
type AttributeIterator = Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send>;
type NodeIteratorCell = Rc<RefCell<Option<NodeIterator>>>;
type AttributeIteratorCell = Rc<RefCell<Option<AttributeIterator>>>;

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
            EvaluationItem::Node(node) => f.debug_tuple("Node").field(&node.runtime_id().as_str()).finish(),
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
    let iter = evaluate_iter(node, xpath, options)?;
    Ok(iter.collect())
}

/// Concrete, owned iterator over XPath evaluation results.
/// This type is stable across FFI boundaries and does not expose generics or lifetimes to callers.
pub struct EvaluationStream {
    inner: Box<dyn Iterator<Item = EvaluationItem>>,
}

impl Iterator for EvaluationStream {
    type Item = EvaluationItem;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl EvaluationStream {
    /// Build a new owned evaluation stream. Accepts an owned XPath string to avoid lifetime issues.
    pub fn new(
        node: Option<Arc<dyn UiNode>>,
        xpath: String,
        options: EvaluateOptions,
    ) -> Result<Self, EvaluateError> {
        let context = resolve_context(node.as_ref(), &options)?;

        if options.invalidate_before_eval() {
            context.invalidate();
        }

        let static_ctx = build_static_context();

        let compiled = compiler::compile_with_context(&xpath, &static_ctx)?;
        let mut dyn_builder = DynamicContextBuilder::new();
        let ctx_item = RuntimeXdmNode::from_node(context.clone());
        dyn_builder = dyn_builder.with_context_item(ctx_item);
        let dyn_ctx = dyn_builder.build();

        let stream = evaluator::evaluate_stream(&compiled, &dyn_ctx)?;
        let it = eval_stream_to_iter(stream);
        Ok(Self { inner: Box::new(it) })
    }
}

pub fn evaluate_iter(
    node: Option<Arc<dyn UiNode>>,
    xpath: &str,
    options: EvaluateOptions,
) -> Result<impl Iterator<Item = EvaluationItem>, EvaluateError> {
    let context = resolve_context(node.as_ref(), &options)?;

    if options.invalidate_before_eval() {
        context.invalidate();
    }

    let static_ctx = build_static_context();

    let compiled = compiler::compile_with_context(xpath, &static_ctx)?;
    let mut dyn_builder = DynamicContextBuilder::new();
    dyn_builder = dyn_builder.with_context_item(RuntimeXdmNode::from_node(context.clone()));
    let dyn_ctx = dyn_builder.build();

    let stream = evaluator::evaluate_stream(&compiled, &dyn_ctx)?;
    Ok(eval_stream_to_iter(stream))
}

/// Resolve the effective context node for evaluation based on input node and options.
fn resolve_context(
    node: Option<&Arc<dyn UiNode>>,
    options: &EvaluateOptions,
) -> Result<Arc<dyn UiNode>, EvaluateError> {
    let root = options.desktop();
    let context = if let Some(node_ref) = node {
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
    Ok(context)
}

/// Build the static context with PlatynUI namespaces configured.
fn build_static_context() -> platynui_xpath::engine::runtime::StaticContext {
    StaticContextBuilder::new()
        .with_default_element_namespace(CONTROL_NS_URI)
        .with_namespace("control", CONTROL_NS_URI)
        .with_namespace("item", ITEM_NS_URI)
        .with_namespace("app", APP_NS_URI)
        .with_namespace("native", NATIVE_NS_URI)
        .build()
}

/// Map a stream of XDM items to `EvaluationItem`s, skipping errors and non-mappable values.
fn eval_stream_to_iter<I>(
    iter: I,
) -> impl Iterator<Item = EvaluationItem>
where
    I: IntoIterator<
        Item = Result<platynui_xpath::xdm::XdmItem<RuntimeXdmNode>, platynui_xpath::engine::runtime::Error>,
    >,
{
    use platynui_xpath::xdm::XdmItem;
    iter.into_iter().filter_map(|res| match res.ok()? {
        XdmItem::Node(node) => match node {
            RuntimeXdmNode::Document(doc) => Some(EvaluationItem::Node(doc.root.clone())),
            RuntimeXdmNode::Element(elem) => Some(EvaluationItem::Node(elem.node.clone())),
            RuntimeXdmNode::Attribute(attr) => Some(EvaluationItem::Attribute(attr.to_evaluated())),
        },
        XdmItem::Atomic(atom) => Some(EvaluationItem::Value(atomic_to_ui_value(&atom))),
    })
}

#[derive(Clone)]
enum RuntimeXdmNode {
    Document(DocumentData),
    Element(ElementData),
    Attribute(AttributeData),
}

impl RuntimeXdmNode {
    fn document(root: Arc<dyn UiNode>) -> Self {
        let runtime_id = root.runtime_id().clone();
        RuntimeXdmNode::Document(DocumentData::new(root, runtime_id))
    }

    fn element(node: Arc<dyn UiNode>) -> Self {
        let runtime_id = node.runtime_id().clone();
        let namespace = node.namespace();
        let role = node.role().to_string();
        let order_key = node.doc_order_key();
        RuntimeXdmNode::Element(ElementData::new(node, runtime_id, namespace, role, order_key))
    }

    fn from_node(node: Arc<dyn UiNode>) -> Self {
        if node.parent().is_none() { RuntimeXdmNode::document(node) } else { RuntimeXdmNode::element(node) }
    }
}

impl PartialEq for RuntimeXdmNode {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RuntimeXdmNode::Document(a), RuntimeXdmNode::Document(b)) => a.runtime_id == b.runtime_id,
            (RuntimeXdmNode::Element(a), RuntimeXdmNode::Element(b)) => {
                a.runtime_id == b.runtime_id && a.order_key == b.order_key
            }
            (RuntimeXdmNode::Attribute(a), RuntimeXdmNode::Attribute(b)) => {
                a.owner_runtime_id == b.owner_runtime_id && a.namespace == b.namespace && a.name == b.name
            }
            _ => false,
        }
    }
}

impl Eq for RuntimeXdmNode {}

impl std::fmt::Debug for RuntimeXdmNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeXdmNode::Document(doc) => f.debug_struct("Document").field("runtime_id", &doc.runtime_id).finish(),
            RuntimeXdmNode::Element(elem) => f
                .debug_struct("Element")
                .field("runtime_id", &elem.runtime_id)
                .field("order_key", &elem.order_key)
                .field("role", &elem.role)
                .finish(),
            RuntimeXdmNode::Attribute(attr) => {
                f.debug_struct("Attribute").field("owner", &attr.owner_runtime_id).field("name", &attr.name).finish()
            }
        }
    }
}

impl XdmNode for RuntimeXdmNode {
    type Children<'a>
        = NodeChildrenIter<'a>
    where
        Self: 'a;
    type Attributes<'a>
        = NodeAttributeIter<'a>
    where
        Self: 'a;
    type Namespaces<'a>
        = std::iter::Empty<RuntimeXdmNode>
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

    fn typed_value(&self) -> Vec<XdmAtomicValue> {
        match self {
            RuntimeXdmNode::Document(_) | RuntimeXdmNode::Element(_) => Vec::new(),
            RuntimeXdmNode::Attribute(attr) => attr.typed().clone(),
        }
    }

    fn base_uri(&self) -> Option<String> {
        None
    }

    fn parent(&self) -> Option<Self> {
        match self {
            RuntimeXdmNode::Document(_) => None,
            RuntimeXdmNode::Element(elem) => {
                // Try cached parent first
                if let Some(cached) = elem.parent_cache.borrow().as_ref() {
                    return cached.clone();
                }
                // Compute once and cache the result
                let computed: Option<RuntimeXdmNode> = match elem.node.parent() {
                    Some(parent) => match parent.upgrade() {
                        Some(p) => Some(RuntimeXdmNode::from_node(p)),
                        None => Some(RuntimeXdmNode::document(elem.node.clone())),
                    },
                    None => Some(RuntimeXdmNode::document(elem.node.clone())),
                };
                *elem.parent_cache.borrow_mut() = Some(computed.clone());
                computed
            }
            RuntimeXdmNode::Attribute(attr) => Some(RuntimeXdmNode::from_node(attr.owner.clone())),
        }
    }

    fn children(&self) -> Self::Children<'_> {
        match self {
            RuntimeXdmNode::Document(doc) => {
                if doc.children_inner.borrow().is_none() && !doc.children_finished.get() {
                    *doc.children_inner.borrow_mut() = Some(doc.root.children());
                }
                NodeChildrenIter::from_shared(
                    Rc::clone(&doc.children_inner),
                    Rc::clone(&doc.children_cache),
                    Rc::clone(&doc.children_finished),
                )
                .with_parent_doc(self.clone())
            }
            RuntimeXdmNode::Element(elem) => {
                if elem.children_inner.borrow().is_none() && !elem.children_finished.get() {
                    *elem.children_inner.borrow_mut() = Some(elem.node.children());
                }
                NodeChildrenIter::from_shared(
                    Rc::clone(&elem.children_inner),
                    Rc::clone(&elem.children_cache),
                    Rc::clone(&elem.children_finished),
                )
            }
            RuntimeXdmNode::Attribute(_) => NodeChildrenIter::empty(),
        }
    }

    fn attributes(&self) -> Self::Attributes<'_> {
        match self {
            // In PlatynUI's model, the document wrapper exposes the root element's attributes
            // to allow queries like './@*' from the desktop context. This intentionally
            // diverges from the strict XPath document-node semantics to keep CLI UX simple.
            RuntimeXdmNode::Document(doc) => {
                if doc.attrs_inner.borrow().is_none() && !doc.attrs_finished.get() {
                    *doc.attrs_inner.borrow_mut() = Some(doc.root.attributes());
                }
                NodeAttributeIter::from_shared(
                    doc.root.clone(),
                    Rc::clone(&doc.attrs_inner),
                    Rc::clone(&doc.attrs_cache),
                    Rc::clone(&doc.attrs_finished),
                )
            }
            RuntimeXdmNode::Element(elem) => {
                if elem.attrs_inner.borrow().is_none() && !elem.attrs_finished.get() {
                    *elem.attrs_inner.borrow_mut() = Some(elem.node.attributes());
                }
                NodeAttributeIter::from_shared(
                    elem.node.clone(),
                    Rc::clone(&elem.attrs_inner),
                    Rc::clone(&elem.attrs_cache),
                    Rc::clone(&elem.attrs_finished),
                )
            }
            RuntimeXdmNode::Attribute(_) => NodeAttributeIter::empty(),
        }
    }

    fn namespaces(&self) -> Self::Namespaces<'_> {
        std::iter::empty()
    }

    fn doc_order_key(&self) -> Option<u64> {
        match self {
            RuntimeXdmNode::Element(elem) => elem.order_key,
            RuntimeXdmNode::Document(_) | RuntimeXdmNode::Attribute(_) => None,
        }
    }
}

#[derive(Clone)]
struct DocumentData {
    root: Arc<dyn UiNode>,
    runtime_id: RuntimeId,
    // Persistent inner iterators + caches
    children_inner: NodeIteratorCell,
    children_cache: Rc<RefCell<Vec<RuntimeXdmNode>>>,
    children_finished: Rc<Cell<bool>>,
    attrs_inner: AttributeIteratorCell,
    attrs_cache: Rc<RefCell<Vec<RuntimeXdmNode>>>,
    attrs_finished: Rc<Cell<bool>>,
}

impl DocumentData {
    fn new(root: Arc<dyn UiNode>, runtime_id: RuntimeId) -> Self {
        Self {
            root,
            runtime_id,
            children_inner: Rc::new(RefCell::new(None)),
            children_cache: Rc::new(RefCell::new(Vec::new())),
            children_finished: Rc::new(Cell::new(false)),
            attrs_inner: Rc::new(RefCell::new(None)),
            attrs_cache: Rc::new(RefCell::new(Vec::new())),
            attrs_finished: Rc::new(Cell::new(false)),
        }
    }
}

#[derive(Clone)]
struct ElementData {
    node: Arc<dyn UiNode>,
    runtime_id: RuntimeId,
    namespace: UiNamespace,
    role: String,
    order_key: Option<u64>,
    children_inner: NodeIteratorCell,
    children_cache: Rc<RefCell<Vec<RuntimeXdmNode>>>,
    children_finished: Rc<Cell<bool>>,
    attrs_inner: AttributeIteratorCell,
    attrs_cache: Rc<RefCell<Vec<RuntimeXdmNode>>>,
    attrs_finished: Rc<Cell<bool>>,
    parent_cache: Rc<RefCell<Option<Option<RuntimeXdmNode>>>>,
}

impl ElementData {
    fn new(
        node: Arc<dyn UiNode>,
        runtime_id: RuntimeId,
        namespace: UiNamespace,
        role: String,
        order_key: Option<u64>,
    ) -> Self {
        Self {
            node,
            runtime_id,
            namespace,
            role,
            order_key,
            children_inner: Rc::new(RefCell::new(None)),
            children_cache: Rc::new(RefCell::new(Vec::new())),
            children_finished: Rc::new(Cell::new(false)),
            attrs_inner: Rc::new(RefCell::new(None)),
            attrs_cache: Rc::new(RefCell::new(Vec::new())),
            attrs_finished: Rc::new(Cell::new(false)),
            parent_cache: Rc::new(RefCell::new(None)),
        }
    }
}

#[derive(Clone)]
struct AttributeData {
    owner: Arc<dyn UiNode>,
    owner_runtime_id: RuntimeId,
    namespace: UiNamespace,
    name: String,
    // Lazy value provider (either direct source or derived component)
    value_kind: ValueKind,
    value_cell: once_cell::sync::OnceCell<UiValue>,
    typed_cell: once_cell::sync::OnceCell<Vec<XdmAtomicValue>>,
}

// no StaticUiAttribute needed; attributes are sourced from provider or derived lazily

#[derive(Clone)]
enum ValueKind {
    Source(Arc<dyn UiAttribute>),
    RectComp { base: Arc<dyn UiAttribute>, comp: RectComp },
    PointComp { base: Arc<dyn UiAttribute>, comp: PointComp },
}

#[derive(Clone)]
enum RectComp {
    X,
    Y,
    Width,
    Height,
}
#[derive(Clone)]
enum PointComp {
    X,
    Y,
}

impl AttributeData {
    fn new_from_source(
        owner: Arc<dyn UiNode>,
        namespace: UiNamespace,
        name: String,
        source: Arc<dyn UiAttribute>,
    ) -> Self {
        let owner_runtime_id = owner.runtime_id().clone();
        Self {
            owner,
            owner_runtime_id,
            namespace,
            name,
            value_kind: ValueKind::Source(source),
            value_cell: once_cell::sync::OnceCell::new(),
            typed_cell: once_cell::sync::OnceCell::new(),
        }
    }
    fn new_rect_component(
        owner: Arc<dyn UiNode>,
        namespace: UiNamespace,
        base: Arc<dyn UiAttribute>,
        base_name: &str,
        comp: RectComp,
    ) -> Self {
        let name = component_attribute_name(
            base_name,
            match comp {
                RectComp::X => "X",
                RectComp::Y => "Y",
                RectComp::Width => "Width",
                RectComp::Height => "Height",
            },
        );
        let owner_runtime_id = owner.runtime_id().clone();
        Self {
            owner,
            owner_runtime_id,
            namespace,
            name,
            value_kind: ValueKind::RectComp { base, comp },
            value_cell: once_cell::sync::OnceCell::new(),
            typed_cell: once_cell::sync::OnceCell::new(),
        }
    }
    fn new_point_component(
        owner: Arc<dyn UiNode>,
        namespace: UiNamespace,
        base: Arc<dyn UiAttribute>,
        base_name: &str,
        comp: PointComp,
    ) -> Self {
        let name = component_attribute_name(
            base_name,
            match comp {
                PointComp::X => "X",
                PointComp::Y => "Y",
            },
        );
        let owner_runtime_id = owner.runtime_id().clone();
        Self {
            owner,
            owner_runtime_id,
            namespace,
            name,
            value_kind: ValueKind::PointComp { base, comp },
            value_cell: once_cell::sync::OnceCell::new(),
            typed_cell: once_cell::sync::OnceCell::new(),
        }
    }
    fn value(&self) -> UiValue {
        self.value_cell
            .get_or_init(|| match &self.value_kind {
                ValueKind::Source(src) => src.value(),
                ValueKind::RectComp { base, comp } => match base.value() {
                    UiValue::Rect(r) => match comp {
                        RectComp::X => UiValue::from(r.x()),
                        RectComp::Y => UiValue::from(r.y()),
                        RectComp::Width => UiValue::from(r.width()),
                        RectComp::Height => UiValue::from(r.height()),
                    },
                    _ => UiValue::Null,
                },
                ValueKind::PointComp { base, comp } => match base.value() {
                    UiValue::Point(p) => match comp {
                        PointComp::X => UiValue::from(p.x()),
                        PointComp::Y => UiValue::from(p.y()),
                    },
                    _ => UiValue::Null,
                },
            })
            .clone()
    }
    fn typed(&self) -> &Vec<XdmAtomicValue> {
        self.typed_cell.get_or_init(|| ui_value_to_atomic_values(&self.value()))
    }
    fn to_evaluated(&self) -> EvaluatedAttribute {
        EvaluatedAttribute {
            owner: self.owner.clone(),
            namespace: self.namespace,
            name: self.name.clone(),
            value: self.value(),
        }
    }
}

fn ui_value_to_atomic_values(value: &UiValue) -> Vec<XdmAtomicValue> {
    match value {
        UiValue::Null => Vec::new(),
        UiValue::Bool(b) => vec![XdmAtomicValue::Boolean(*b)],
        UiValue::Integer(i) => vec![XdmAtomicValue::Integer(*i)],
        UiValue::Number(n) => vec![XdmAtomicValue::Double(*n)],
        UiValue::String(s) => vec![XdmAtomicValue::String(s.clone())],
        UiValue::Array(items) => {
            serde_json::to_string(items).ok().map(|json| vec![XdmAtomicValue::String(json)]).unwrap_or_default()
        }
        UiValue::Object(map) => {
            serde_json::to_string(map).ok().map(|json| vec![XdmAtomicValue::String(json)]).unwrap_or_default()
        }
        UiValue::Point(point) => {
            serde_json::to_string(point).ok().map(|json| vec![XdmAtomicValue::String(json)]).unwrap_or_default()
        }
        UiValue::Size(size) => {
            serde_json::to_string(size).ok().map(|json| vec![XdmAtomicValue::String(json)]).unwrap_or_default()
        }
        UiValue::Rect(rect) => {
            serde_json::to_string(rect).ok().map(|json| vec![XdmAtomicValue::String(json)]).unwrap_or_default()
        }
    }
}

fn component_attribute_name(base: &str, suffix: &str) -> String {
    format!("{}.{}", base, suffix)
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
        String(s) | UntypedAtomic(s) | AnyUri(s) | NormalizedString(s) | Token(s) | Language(s) | Name(s)
        | NCName(s) | NMTOKEN(s) | Id(s) | IdRef(s) | Entity(s) | Notation(s) => UiValue::String(s.clone()),
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
        GYear { year, tz } => UiValue::String(format!("{}{}", year, tz.map_or("".to_string(), |o| o.to_string()))),
        GYearMonth { year, month, tz } => {
            UiValue::String(format!("{}-{:02}{}", year, month, tz.map_or("".to_string(), |o| o.to_string())))
        }
        GMonth { month, tz } => {
            UiValue::String(format!("{:02}{}", month, tz.map_or("".to_string(), |o| o.to_string())))
        }
        GMonthDay { month, day, tz } => {
            UiValue::String(format!("{:02}-{:02}{}", month, day, tz.map_or("".to_string(), |o| o.to_string())))
        }
        GDay { day, tz } => UiValue::String(format!("{:02}{}", day, tz.map_or("".to_string(), |o| o.to_string()))),
    }
}

struct NodeChildrenIter<'a> {
    inner: NodeIteratorCell,
    cache: Rc<RefCell<Vec<RuntimeXdmNode>>>,
    finished: Rc<Cell<bool>>,
    pos: usize,
    _marker: std::marker::PhantomData<&'a ()>,
    parent_doc: Option<RuntimeXdmNode>,
}
impl<'a> NodeChildrenIter<'a> {
    fn from_shared(inner: NodeIteratorCell, cache: Rc<RefCell<Vec<RuntimeXdmNode>>>, finished: Rc<Cell<bool>>) -> Self {
        Self { inner, cache, finished, pos: 0, _marker: std::marker::PhantomData, parent_doc: None }
    }
    fn with_parent_doc(mut self, doc: RuntimeXdmNode) -> Self {
        self.parent_doc = Some(doc);
        self
    }
    fn empty() -> Self {
        Self {
            inner: Rc::new(RefCell::new(None)),
            cache: Rc::new(RefCell::new(Vec::new())),
            finished: Rc::new(Cell::new(true)),
            pos: 0,
            _marker: std::marker::PhantomData,
            parent_doc: None,
        }
    }
}
impl<'a> Iterator for NodeChildrenIter<'a> {
    type Item = RuntimeXdmNode;
    fn next(&mut self) -> Option<Self::Item> {
        {
            let cache = self.cache.borrow();
            if self.pos < cache.len() {
                let item = cache[self.pos].clone();
                drop(cache);
                self.pos += 1;
                return Some(item);
            }
        }
        if self.finished.get() {
            return None;
        }
        let mut inner_borrow = self.inner.borrow_mut();
        match inner_borrow.as_mut() {
            Some(iter) => {
                if let Some(owner) = iter.next() {
                    let mut node = RuntimeXdmNode::from_node(owner);
                    if let (Some(RuntimeXdmNode::Document(doc_parent)), RuntimeXdmNode::Element(elem)) =
                        (self.parent_doc.as_ref(), &mut node)
                    {
                        // Link element's cached parent to the shared document wrapper
                        *elem.parent_cache.borrow_mut() = Some(Some(RuntimeXdmNode::Document(doc_parent.clone())));
                    }
                    self.cache.borrow_mut().push(node.clone());
                    self.pos += 1;
                    Some(node)
                } else {
                    self.finished.set(true);
                    None
                }
            }
            None => {
                self.finished.set(true);
                None
            }
        }
    }
}

struct NodeAttributeIter<'a> {
    owner: Arc<dyn UiNode>,
    inner: AttributeIteratorCell,
    cache: Rc<RefCell<Vec<RuntimeXdmNode>>>,
    finished: Rc<Cell<bool>>,
    pos: usize,
    _marker: std::marker::PhantomData<&'a ()>,
}
impl<'a> NodeAttributeIter<'a> {
    fn from_shared(
        owner: Arc<dyn UiNode>,
        inner: AttributeIteratorCell,
        cache: Rc<RefCell<Vec<RuntimeXdmNode>>>,
        finished: Rc<Cell<bool>>,
    ) -> Self {
        Self { owner, inner, cache, finished, pos: 0, _marker: std::marker::PhantomData }
    }
    fn empty() -> Self {
        Self {
            owner: Arc::new(DummyNode),
            inner: Rc::new(RefCell::new(None)),
            cache: Rc::new(RefCell::new(Vec::new())),
            finished: Rc::new(Cell::new(true)),
            pos: 0,
            _marker: std::marker::PhantomData,
        }
    }
}
impl<'a> Iterator for NodeAttributeIter<'a> {
    type Item = RuntimeXdmNode;
    fn next(&mut self) -> Option<Self::Item> {
        {
            let cache = self.cache.borrow();
            if self.pos < cache.len() {
                let item = cache[self.pos].clone();
                drop(cache);
                self.pos += 1;
                return Some(item);
            }
        }
        if self.finished.get() {
            return None;
        }
        let mut inner_borrow = self.inner.borrow_mut();
        let iter = inner_borrow.as_mut()?;
        if let Some(attr) = iter.next() {
            let ns = attr.namespace();
            let base_name = attr.name().to_string();
            let src = attr.clone();
            {
                let mut cache = self.cache.borrow_mut();
                cache.push(RuntimeXdmNode::Attribute(AttributeData::new_from_source(
                    self.owner.clone(),
                    ns,
                    base_name.clone(),
                    src.clone(),
                )));
                if base_name == attribute_names::element::BOUNDS {
                    cache.push(RuntimeXdmNode::Attribute(AttributeData::new_rect_component(
                        self.owner.clone(),
                        ns,
                        src.clone(),
                        attribute_names::element::BOUNDS,
                        RectComp::X,
                    )));
                    cache.push(RuntimeXdmNode::Attribute(AttributeData::new_rect_component(
                        self.owner.clone(),
                        ns,
                        src.clone(),
                        attribute_names::element::BOUNDS,
                        RectComp::Y,
                    )));
                    cache.push(RuntimeXdmNode::Attribute(AttributeData::new_rect_component(
                        self.owner.clone(),
                        ns,
                        src.clone(),
                        attribute_names::element::BOUNDS,
                        RectComp::Width,
                    )));
                    cache.push(RuntimeXdmNode::Attribute(AttributeData::new_rect_component(
                        self.owner.clone(),
                        ns,
                        src,
                        attribute_names::element::BOUNDS,
                        RectComp::Height,
                    )));
                } else if base_name == attribute_names::activation_target::ACTIVATION_POINT {
                    let src_point = attr.clone();
                    cache.push(RuntimeXdmNode::Attribute(AttributeData::new_point_component(
                        self.owner.clone(),
                        ns,
                        src_point.clone(),
                        attribute_names::activation_target::ACTIVATION_POINT,
                        PointComp::X,
                    )));
                    cache.push(RuntimeXdmNode::Attribute(AttributeData::new_point_component(
                        self.owner.clone(),
                        ns,
                        src_point,
                        attribute_names::activation_target::ACTIVATION_POINT,
                        PointComp::Y,
                    )));
                }
            }
            // Return the just-pushed item at current position (should exist)
            let idx = self.pos;
            {
                let cache = self.cache.borrow();
                if idx < cache.len() {
                    let it = cache[idx].clone();
                    self.pos += 1;
                    return Some(it);
                }
            }
            // Fallback: inconsistent state — mark finished
            self.finished.set(true);
            None
        } else {
            self.finished.set(true);
            None
        }
    }
}

struct DummyNode;
impl UiNode for DummyNode {
    fn namespace(&self) -> UiNamespace {
        UiNamespace::Control
    }
    fn role(&self) -> &str {
        ""
    }
    fn name(&self) -> String {
        String::new()
    }
    fn runtime_id(&self) -> &RuntimeId {
        static RID: once_cell::sync::OnceCell<RuntimeId> = once_cell::sync::OnceCell::new();
        RID.get_or_init(|| RuntimeId::from("dummy"))
    }
    fn parent(&self) -> Option<std::sync::Weak<dyn UiNode>> {
        None
    }
    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        Box::new(std::iter::empty())
    }
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        Box::new(std::iter::empty())
    }
    fn supported_patterns(&self) -> Vec<PatternId> {
        Vec::new()
    }
    fn invalidate(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::provider::{ProviderError, ProviderErrorKind};
    use platynui_core::types::Rect;
    use platynui_core::ui::{PatternId, RuntimeId, UiAttribute, UiNode, attribute_names, supported_patterns_value};
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
                Arc::new(StaticAttribute::new(namespace, attribute_names::element::BOUNDS, UiValue::Rect(bounds)))
                    as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(namespace, attribute_names::common::ROLE, UiValue::from(role)))
                    as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(namespace, attribute_names::common::NAME, UiValue::from(name)))
                    as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(namespace, attribute_names::element::IS_VISIBLE, UiValue::from(true)))
                    as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(namespace, attribute_names::element::IS_ENABLED, UiValue::from(true)))
                    as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(
                    namespace,
                    attribute_names::common::RUNTIME_ID,
                    UiValue::from(runtime_id.as_str().to_owned()),
                )) as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(namespace, attribute_names::common::TECHNOLOGY, UiValue::from("Mock")))
                    as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute::new(namespace, attribute_names::common::SUPPORTED_PATTERNS, supported))
                    as Arc<dyn UiAttribute>,
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
            *child.parent.lock().unwrap() = Some(Arc::downgrade(&(Arc::clone(parent) as Arc<dyn UiNode>)));
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

        fn name(&self) -> String {
            self.name.clone()
        }

        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }

        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            self.parent.lock().unwrap().clone()
        }

        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
            let snapshot = self.children.lock().unwrap().clone();
            Box::new(snapshot.into_iter())
        }

        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
            Box::new(self.attributes.clone().into_iter())
        }

        fn supported_patterns(&self) -> Vec<PatternId> {
            self.patterns.clone()
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
    fn absolute_path_from_document_returns_children() {
        let tree = sample_tree();
        let items = evaluate(None, "/*", EvaluateOptions::new(tree.clone())).unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            EvaluationItem::Node(node) => {
                assert_eq!(node.runtime_id().as_str(), "window-1");
            }
            other => panic!("unexpected evaluation result: {:?}", other),
        }
    }

    #[rstest]
    fn desktop_bounds_alias_attributes_are_available() {
        let tree = sample_tree();
        let items = evaluate(None, "./@Bounds.X", EvaluateOptions::new(tree.clone())).unwrap();
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
    fn bounds_width_data_returns_numbers() {
        let tree = sample_tree();
        let attrs = evaluate(None, "//@*:Bounds.Width", EvaluateOptions::new(tree.clone())).unwrap();
        assert!(!attrs.is_empty());
        for item in &attrs {
            match item {
                EvaluationItem::Attribute(attr) => {
                    assert!(matches!(attr.value, UiValue::Number(_)));
                }
                other => panic!("expected attribute node, got {:?}", other),
            }
        }
        let items = evaluate(None, "data(//@*:Bounds.Width)", EvaluateOptions::new(tree.clone())).unwrap();
        assert!(!items.is_empty());
        let mut widths = Vec::new();
        for item in items {
            match item {
                EvaluationItem::Value(UiValue::Number(n)) => widths.push(n),
                other => panic!("expected numeric UiValue, got {:?}", other),
            }
        }
        assert!(widths.contains(&1920.0));
        assert!(widths.contains(&800.0));
    }

    #[rstest]
    fn boolean_attributes_atomize_to_bools() {
        let tree = sample_tree();
        let items = evaluate(None, "data(//@*:IsVisible)", EvaluateOptions::new(tree.clone())).unwrap();
        assert!(!items.is_empty());
        for item in items {
            match item {
                EvaluationItem::Value(UiValue::Bool(value)) => assert!(value),
                other => panic!("expected boolean UiValue, got {:?}", other),
            }
        }
    }

    #[rstest]
    fn bounds_base_attribute_remains_json_string() {
        let tree = sample_tree();
        let items = evaluate(None, "data(./@Bounds)", EvaluateOptions::new(tree.clone())).unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            EvaluationItem::Value(UiValue::String(json)) => {
                assert!(json.contains("\"width\""));
                assert!(json.contains("\"height\""));
            }
            other => panic!("expected serialized bounds string, got {:?}", other),
        }
    }

    #[rstest]
    fn desktop_monitors_attribute_is_exposed() {
        let tree = sample_tree();
        let items = evaluate(None, "./@Monitors", EvaluateOptions::new(tree.clone())).unwrap();
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
        fn resolve(&self, _runtime_id: &RuntimeId) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
            Ok(Some(self.node.clone()))
        }
    }

    struct ResolverMissing;

    impl NodeResolver for ResolverMissing {
        fn resolve(&self, _runtime_id: &RuntimeId) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
            Ok(None)
        }
    }

    struct ResolverError;

    impl NodeResolver for ResolverError {
        fn resolve(&self, _runtime_id: &RuntimeId) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
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

        let items =
            evaluate(Some(stale_node.clone()), ".", EvaluateOptions::new(tree.clone()).with_node_resolver(resolver))
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

        let result =
            evaluate(Some(stale_node.clone()), ".", EvaluateOptions::new(tree.clone()).with_node_resolver(resolver));

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

        let result =
            evaluate(Some(stale_node.clone()), ".", EvaluateOptions::new(tree.clone()).with_node_resolver(resolver));

        match result {
            Err(EvaluateError::Provider(err)) => match err {
                ProviderError::TreeUnavailable { .. } => {}
                other => panic!("unexpected provider error: {other}"),
            },
            other => panic!("unexpected result: {:?}", other),
        }
    }
}
