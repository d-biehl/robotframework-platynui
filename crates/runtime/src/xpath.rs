//
use platynui_core::ui::PatternName;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

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
use std::sync::LazyLock;
use thiserror::Error;

const CONTROL_NS_URI: &str = "urn:platynui:control";
const ITEM_NS_URI: &str = "urn:platynui:item";
const APP_NS_URI: &str = "urn:platynui:app";
const NATIVE_NS_URI: &str = "urn:platynui:native";

type NodeIterator = Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>;
type AttributeIterator = Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send>;
type NodeIteratorCell = Arc<Mutex<Option<NodeIterator>>>;
type AttributeIteratorCell = Arc<Mutex<Option<AttributeIterator>>>;
type NodeCacheCell = Arc<Mutex<Vec<RuntimeXdmNode>>>;
type SharedFlag = Arc<AtomicBool>;

/// Locks `mutex` even if a panic poisoned it. A provider that panics during a
/// query must fail only that query: every later use of the snapshot, above all
/// its release, which runs in destructors, has to work, and a second panic in
/// a destructor would abort the process.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Cross-evaluation XDM tree cache: the snapshot the next query may reuse
/// (`dev-docs/architecture.md` §9.3).
///
/// `Clone + Send + Sync` so a single runtime-owned cache can be shared across
/// threads while still preserving explicit invalidation semantics.
#[derive(Clone)]
pub struct XdmCache {
    inner: Arc<Mutex<Option<(RuntimeId, RuntimeXdmNode)>>>,
}

impl XdmCache {
    #[must_use]
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(None)) }
    }

    /// Discards the cached snapshot, so the next evaluation reads the current
    /// UI, and releases it together with every provider node that only the
    /// snapshot held.
    pub fn clear(&self) {
        let discarded = lock(&self.inner).take();
        drop(discarded);
    }
}

impl Default for XdmCache {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for XdmCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let has_entry = lock(&self.inner).is_some();
        f.debug_struct("XdmCache").field("cached", &has_entry).finish()
    }
}

pub trait NodeResolver: Send + Sync {
    /// Looks up the current node for `runtime_id`; `Ok(None)` means the node no longer exists.
    ///
    /// # Errors
    ///
    /// Returns a [`ProviderError`] if the lookup itself fails.
    fn resolve(&self, runtime_id: &RuntimeId) -> Result<Option<Arc<dyn UiNode>>, ProviderError>;
}

#[derive(Clone)]
#[must_use]
pub struct EvaluateOptions {
    desktop: Arc<dyn UiNode>,
    invalidate_before_eval: bool,
    resolver: Option<Arc<dyn NodeResolver>>,
    cache: Option<XdmCache>,
    cancel_flag: Option<Arc<AtomicBool>>,
}

impl EvaluateOptions {
    pub fn new(desktop: Arc<dyn UiNode>) -> Self {
        Self { desktop, invalidate_before_eval: false, resolver: None, cache: None, cancel_flag: None }
    }

    #[must_use]
    pub fn desktop(&self) -> Arc<dyn UiNode> {
        Arc::clone(&self.desktop)
    }

    pub fn with_invalidation(mut self, invalidate: bool) -> Self {
        self.invalidate_before_eval = invalidate;
        self
    }

    #[must_use]
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

    pub fn with_cache(mut self, cache: XdmCache) -> Self {
        self.cache = Some(cache);
        self
    }

    pub fn without_cache(mut self) -> Self {
        self.cache = None;
        self
    }

    #[must_use]
    pub fn cache(&self) -> Option<&XdmCache> {
        self.cache.as_ref()
    }

    pub fn with_cancel_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancel_flag = Some(flag);
        self
    }

    #[must_use]
    pub fn cancel_flag(&self) -> Option<&Arc<AtomicBool>> {
        self.cancel_flag.as_ref()
    }
}

#[derive(Debug, Error)]
pub enum EvaluateError {
    #[error("XPath evaluation failed {0}")]
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

// `owner` is a `dyn UiNode` without a `Debug` impl; it is left out on purpose.
#[allow(clippy::missing_fields_in_debug)]
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

/// Evaluates `xpath` against `node` (or the desktop from `options`) and collects all results.
///
/// # Errors
///
/// Returns [`EvaluateError::ContextNodeUnknown`] if a node resolver is configured and cannot find
/// `node`, [`EvaluateError::Provider`] if that resolver fails, and [`EvaluateError::XPath`] if
/// `xpath` does not compile or its evaluation fails.
pub fn evaluate(
    node: Option<Arc<dyn UiNode>>,
    xpath: &str,
    options: EvaluateOptions,
) -> Result<Vec<EvaluationItem>, EvaluateError> {
    // XPath evaluation's own records exist in debug builds only (dev-docs/logging.md §12).
    #[cfg(debug_assertions)]
    tracing::debug!(xpath, cached = options.cache().is_some(), "collecting XPath results");
    let iter = evaluate_iter(node, xpath, options)?;
    iter.collect()
}

/// Returns whether evaluating an `XPath` expression reads its context node, so that its result
/// depends on which node it is evaluated against: through a relative path (`.//x`, `child::x`), the
/// context item (`.`) or a function that falls back to the context (`name()`, `position()`),
/// anywhere in its operands, conditions, bindings and function arguments (`count(.//x)`). Absolute
/// paths (`/x`, `//x`), predicates and the later steps of a path, which have their own focus, do
/// not count, so `count(//x)` and `//x[.='y']` are independent. Parses only; no context node or
/// backend is required.
///
/// # Errors
///
/// Returns [`EvaluateError::XPath`] if `xpath` does not parse.
pub fn is_context_dependent(xpath: &str) -> Result<bool, EvaluateError> {
    Ok(platynui_xpath::parser::parse(xpath)?.is_context_dependent())
}

pub struct EvaluationStream {
    inner: Box<dyn Iterator<Item = Result<EvaluationItem, EvaluateError>>>,
}

impl Iterator for EvaluationStream {
    type Item = Result<EvaluationItem, EvaluateError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl EvaluationStream {
    /// Starts a lazy evaluation of `xpath` against `node` (or the desktop from `options`).
    ///
    /// # Errors
    ///
    /// Returns [`EvaluateError::ContextNodeUnknown`] if a node resolver is configured and cannot
    /// find `node`, [`EvaluateError::Provider`] if that resolver fails, and
    /// [`EvaluateError::XPath`] if `xpath` does not compile or the evaluation cannot start. Errors
    /// raised while items are produced are yielded by the stream.
    // Public API signature; borrowing the arguments instead would break callers.
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(node: Option<Arc<dyn UiNode>>, xpath: String, options: EvaluateOptions) -> Result<Self, EvaluateError> {
        let context = resolve_context(node.as_ref(), &options)?;

        if options.invalidate_before_eval() {
            context.invalidate();
        }

        let xdm_root = get_or_create_xdm_root(&context, options.invalidate_before_eval(), options.cache());

        let static_ctx = build_static_context();
        let compiled = compiler::compile_with_context(&xpath, static_ctx)?;
        let mut dyn_builder = DynamicContextBuilder::new();
        dyn_builder = dyn_builder.with_context_item(xdm_root);
        if let Some(flag) = options.cancel_flag() {
            dyn_builder = dyn_builder.with_cancel_flag(Arc::clone(flag));
        }
        let dyn_ctx = dyn_builder.build();

        let stream = evaluator::evaluate_stream(&compiled, &dyn_ctx)?;
        let it = eval_stream_to_iter(stream);
        Ok(Self { inner: Box::new(it) })
    }
}

// Owned arguments mirror the exported `evaluate` / `Runtime::evaluate_iter*` signatures that
// forward here; borrowing would move the lint onto those public functions.
#[allow(clippy::needless_pass_by_value)]
pub fn evaluate_iter(
    node: Option<Arc<dyn UiNode>>,
    xpath: &str,
    options: EvaluateOptions,
) -> Result<impl Iterator<Item = Result<EvaluationItem, EvaluateError>>, EvaluateError> {
    // XPath evaluation's own records exist in debug builds only (dev-docs/logging.md §12).
    #[cfg(debug_assertions)]
    tracing::debug!(xpath, cached = options.cache().is_some(), "evaluating XPath expression");
    let context = resolve_context(node.as_ref(), &options)?;

    if options.invalidate_before_eval() {
        context.invalidate();
    }

    let xdm_root = get_or_create_xdm_root(&context, options.invalidate_before_eval(), options.cache());

    let static_ctx = build_static_context();
    let compiled = compiler::compile_with_context(xpath, static_ctx)?;
    let mut dyn_builder = DynamicContextBuilder::new();
    dyn_builder = dyn_builder.with_context_item(xdm_root);
    if let Some(flag) = options.cancel_flag() {
        dyn_builder = dyn_builder.with_cancel_flag(Arc::clone(flag));
    }
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

fn get_or_create_xdm_root(context: &Arc<dyn UiNode>, force_rebuild: bool, cache: Option<&XdmCache>) -> RuntimeXdmNode {
    let Some(cache) = cache else {
        return RuntimeXdmNode::from_node(Arc::clone(context));
    };

    if !force_rebuild {
        // Validity is asked outside the lock: `is_valid` calls the provider.
        let cached = lock(&cache.inner)
            .as_ref()
            .filter(|(cached_id, _)| cached_id == context.runtime_id())
            .map(|(_, node)| node.clone());
        if let Some(node) = cached
            && node.is_valid()
        {
            node.prepare_for_evaluation();
            return node;
        }
    }

    let context_id = context.runtime_id().clone();
    let node = RuntimeXdmNode::from_node(Arc::clone(context));
    // The snapshot this one replaces is released outside the lock.
    let replaced = lock(&cache.inner).replace((context_id, node.clone()));
    drop(replaced);
    node
}

/// Build the static context with `PlatynUI` namespaces configured.
fn build_static_context() -> &'static platynui_xpath::engine::runtime::StaticContext {
    static STATIC_CTX: LazyLock<platynui_xpath::engine::runtime::StaticContext> = LazyLock::new(|| {
        StaticContextBuilder::new()
            .with_default_element_namespace(CONTROL_NS_URI)
            .with_namespace("control", CONTROL_NS_URI)
            .with_namespace("item", ITEM_NS_URI)
            .with_namespace("app", APP_NS_URI)
            .with_namespace("native", NATIVE_NS_URI)
            .build()
    });
    &STATIC_CTX
}

/// Map a stream of XDM items to `EvaluationItem`s, propagating evaluation errors.
fn eval_stream_to_iter<I>(iter: I) -> impl Iterator<Item = Result<EvaluationItem, EvaluateError>>
where
    I: IntoIterator<
        Item = Result<platynui_xpath::xdm::XdmItem<RuntimeXdmNode>, platynui_xpath::engine::runtime::Error>,
    >,
{
    use platynui_xpath::xdm::XdmItem;
    iter.into_iter().map(|res| match res {
        Err(e) => Err(EvaluateError::XPath(e)),
        Ok(item) => match item {
            XdmItem::Node(node) => match node {
                RuntimeXdmNode::Document(doc) => Ok(EvaluationItem::Node(doc.root.clone())),
                RuntimeXdmNode::Element(elem) => Ok(EvaluationItem::Node(elem.node.clone())),
                RuntimeXdmNode::Attribute(attr) => Ok(EvaluationItem::Attribute(attr.to_evaluated())),
            },
            XdmItem::Atomic(atom) => Ok(EvaluationItem::Value(atomic_to_ui_value(&atom))),
        },
    })
}

/// A node of a snapshot: a provider node wrapped for the `XPath` engine.
///
/// A document or an element owns the wrappers of the children it has read,
/// and an element links back to the wrapper whose list of children holds it
/// only weakly (see [`ParentLink`]), so the wrappers of a snapshot never form a
/// cycle and a snapshot is released as soon as nothing holds it.
#[derive(Clone)]
enum RuntimeXdmNode {
    Document(Arc<DocumentData>),
    Element(Arc<ElementData>),
    Attribute(Arc<AttributeData>),
}

/// A weak reference to the wrapper whose list of children holds an element.
enum WeakXdmNode {
    Document(Weak<DocumentData>),
    Element(Weak<ElementData>),
}

impl WeakXdmNode {
    fn upgrade(&self) -> Option<RuntimeXdmNode> {
        match self {
            WeakXdmNode::Document(weak) => weak.upgrade().map(RuntimeXdmNode::Document),
            WeakXdmNode::Element(weak) => weak.upgrade().map(RuntimeXdmNode::Element),
        }
    }
}

/// How an element reaches the wrapper of its parent.
///
/// Invariant: `Owned` holds only a wrapper that the `parent()` call storing it
/// has just built, never an existing one. Every strong reference between
/// wrappers therefore points from an older wrapper to a newer one — a child is
/// built by its parent's list of children and links back weakly, an owned
/// parent is built by the child that owns it, and attributes hold only provider
/// nodes — so the wrappers of a snapshot never form a cycle. The drop-count
/// tests in `crates/runtime/tests/xdm_release.rs` fail if one comes back.
enum ParentLink {
    /// Not looked up yet.
    Unresolved,
    /// The wrapper whose list of children holds this element. Sharing it keeps
    /// a sibling walk at one provider enumeration per list.
    Linked(WeakXdmNode),
    /// A parent built upward from this element, which only this element holds:
    /// for an element that no list of children produced — the context node of
    /// a query, the ancestors built from it, an attribute's owner — or whose
    /// list is gone.
    Owned(RuntimeXdmNode),
}

impl RuntimeXdmNode {
    fn attribute(data: AttributeData) -> Self {
        RuntimeXdmNode::Attribute(Arc::new(data))
    }

    fn document(root: Arc<dyn UiNode>) -> Self {
        let runtime_id = root.runtime_id().clone();
        RuntimeXdmNode::Document(Arc::new(DocumentData::new(root, runtime_id)))
    }

    fn element(node: Arc<dyn UiNode>) -> Self {
        let runtime_id = node.runtime_id().clone();
        // XPath evaluation's own records exist in debug builds only (dev-docs/logging.md §12).
        #[cfg(debug_assertions)]
        tracing::trace!(runtime_id = %runtime_id, "wrapping element");
        let namespace = node.namespace();
        let role = node.role().to_string();
        let order_key = node.doc_order_key();
        #[cfg(debug_assertions)]
        tracing::trace!(runtime_id = %runtime_id, role = %role, "element wrapped");
        RuntimeXdmNode::Element(Arc::new(ElementData::new(node, runtime_id, namespace, role, order_key)))
    }

    fn from_node(node: Arc<dyn UiNode>) -> Self {
        if node.parent().is_none() { RuntimeXdmNode::document(node) } else { RuntimeXdmNode::element(node) }
    }

    fn downgrade(&self) -> Option<WeakXdmNode> {
        match self {
            RuntimeXdmNode::Document(doc) => Some(WeakXdmNode::Document(Arc::downgrade(doc))),
            RuntimeXdmNode::Element(elem) => Some(WeakXdmNode::Element(Arc::downgrade(elem))),
            RuntimeXdmNode::Attribute(_) => None,
        }
    }

    /// The provider node and the lazily read content of a document or element.
    fn parts(&self) -> Option<(&Arc<dyn UiNode>, &LazyContent)> {
        match self {
            RuntimeXdmNode::Document(doc) => Some((&doc.root, &doc.content)),
            RuntimeXdmNode::Element(elem) => Some((&elem.node, &elem.content)),
            RuntimeXdmNode::Attribute(_) => None,
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            RuntimeXdmNode::Document(doc) => doc.root.is_valid(),
            RuntimeXdmNode::Element(elem) => elem.node.is_valid(),
            RuntimeXdmNode::Attribute(attr) => attr.owner.is_valid(),
        }
    }

    /// Prepares a retained snapshot for the next query: forgets the attributes
    /// read so far and marks every list of children for revalidation — the
    /// ancestors built upward from a held element included, which no list of
    /// children reaches. Iterative, so a deep snapshot cannot overflow the
    /// stack; it ends because an owned parent's children link back weakly.
    fn prepare_for_evaluation(&self) {
        let mut pending = vec![self.clone()];
        while let Some(node) = pending.pop() {
            let Some((_, content)) = node.parts() else { continue };
            content.reset();
            pending.extend(lock(&content.children_cache).iter().cloned());
            if let RuntimeXdmNode::Element(elem) = &node {
                let owned = match &*lock(&elem.parent) {
                    ParentLink::Owned(parent) => Some(parent.clone()),
                    ParentLink::Unresolved | ParentLink::Linked(_) => None,
                };
                pending.extend(owned);
            }
        }
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
            RuntimeXdmNode::Element(elem) => Some(elem.qname.clone()),
            RuntimeXdmNode::Attribute(attr) => Some(attr.qname.clone()),
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
            RuntimeXdmNode::Element(elem) => Some(elem.parent_wrapper()),
            RuntimeXdmNode::Attribute(attr) => Some(RuntimeXdmNode::from_node(attr.owner.clone())),
        }
    }

    fn children(&self) -> Self::Children<'_> {
        match self.parts() {
            Some((node, content)) => content.children(node, self),
            None => NodeChildrenIter::empty(),
        }
    }

    fn attributes(&self) -> Self::Attributes<'_> {
        match self.parts() {
            Some((node, content)) => content.attributes(node),
            None => NodeAttributeIter::empty(),
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

    /// O(1) attribute lookup via the provider's `UiNode::attribute()` method,
    /// bypassing the full attribute iterator. This avoids materialising
    /// expensive native properties (≈1 050 COM calls per element on Windows)
    /// when the requested attribute lives in a cheaper namespace.
    fn attribute_by_name(&self, name: &QName) -> Option<Self> {
        let node: &Arc<dyn UiNode> = match self {
            RuntimeXdmNode::Document(doc) => &doc.root,
            RuntimeXdmNode::Element(elem) => &elem.node,
            RuntimeXdmNode::Attribute(_) => return None,
        };

        // Map XPath namespace URI to provider UiNamespace.
        let ui_ns = match &name.ns_uri {
            None => UiNamespace::Control,
            Some(uri) => match uri.as_str() {
                CONTROL_NS_URI => UiNamespace::Control,
                ITEM_NS_URI => UiNamespace::Item,
                APP_NS_URI => UiNamespace::App,
                NATIVE_NS_URI => UiNamespace::Native,
                _ => return None,
            },
        };

        // Fast path: direct provider lookup (handles Role, Name, Id, Bounds, …)
        if let Some(attr) = node.attribute(ui_ns, &name.local) {
            return Some(RuntimeXdmNode::attribute(AttributeData::new_from_source(
                node.clone(),
                attr.namespace(),
                attr.name().to_string(),
                attr,
            )));
        }

        // Handle virtual component attributes (e.g. Bounds.X, ActivationPoint.Y)
        // that only exist inside the `NodeAttributeIter` expansion.
        if ui_ns == UiNamespace::Control {
            if let Some(suffix) = name.local.strip_prefix("Bounds.") {
                let comp = match suffix {
                    "X" => Some(RectComp::X),
                    "Y" => Some(RectComp::Y),
                    "Width" => Some(RectComp::Width),
                    "Height" => Some(RectComp::Height),
                    _ => None,
                };
                if let Some(comp) = comp
                    && let Some(base) = node.attribute(ui_ns, attribute_names::element::BOUNDS)
                {
                    return Some(RuntimeXdmNode::attribute(AttributeData::new_rect_component(
                        node.clone(),
                        ui_ns,
                        base,
                        attribute_names::element::BOUNDS,
                        comp,
                    )));
                }
            }
            if let Some(suffix) = name.local.strip_prefix("ActivationPoint.") {
                let comp = match suffix {
                    "X" => Some(PointComp::X),
                    "Y" => Some(PointComp::Y),
                    _ => None,
                };
                if let Some(comp) = comp
                    && let Some(base) = node.attribute(ui_ns, attribute_names::activation_target::ACTIVATION_POINT)
                {
                    return Some(RuntimeXdmNode::attribute(AttributeData::new_point_component(
                        node.clone(),
                        ui_ns,
                        base,
                        attribute_names::activation_target::ACTIVATION_POINT,
                        comp,
                    )));
                }
            }
        }

        None
    }
}

/// The children and attributes of a document or element, read lazily from the
/// provider and kept for as long as the snapshot lives. The cells are shared
/// with the iterators the engine holds.
struct LazyContent {
    children_inner: NodeIteratorCell,
    children_cache: NodeCacheCell,
    children_finished: SharedFlag,
    children_validated: SharedFlag,
    attrs_inner: AttributeIteratorCell,
    attrs_cache: NodeCacheCell,
    attrs_finished: SharedFlag,
}

impl LazyContent {
    fn new() -> Self {
        Self {
            children_inner: Arc::new(Mutex::new(None)),
            children_cache: Arc::new(Mutex::new(Vec::new())),
            children_finished: Arc::new(AtomicBool::new(false)),
            children_validated: Arc::new(AtomicBool::new(false)),
            attrs_inner: Arc::new(Mutex::new(None)),
            attrs_cache: Arc::new(Mutex::new(Vec::new())),
            attrs_finished: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Forgets the attributes read so far and marks the children for
    /// revalidation, before a query that reuses the snapshot.
    fn reset(&self) {
        let attributes = std::mem::take(&mut *lock(&self.attrs_cache));
        self.attrs_finished.store(false, Ordering::Release);
        let iterator = lock(&self.attrs_inner).take();
        self.children_validated.store(false, Ordering::Release);
        drop((attributes, iterator));
    }

    /// The children of `node`, revalidated once per query: a list that holds a
    /// node that is no longer valid is read again from the provider, and the
    /// wrappers it held are released, outside the locks.
    fn children<'a>(&self, node: &Arc<dyn UiNode>, owner: &RuntimeXdmNode) -> NodeChildrenIter<'a> {
        if !self.children_validated.load(Ordering::Acquire) {
            // Validity is asked outside the lock: `is_valid` calls the provider.
            let cached = lock(&self.children_cache).clone();
            if cached.iter().any(|child| !child.is_valid()) {
                let fresh = node.children();
                let stale = std::mem::take(&mut *lock(&self.children_cache));
                self.children_finished.store(false, Ordering::Release);
                let replaced = lock(&self.children_inner).replace(fresh);
                drop((stale, replaced));
            }
            drop(cached);
            self.children_validated.store(true, Ordering::Release);
        }
        if !self.children_finished.load(Ordering::Acquire) && lock(&self.children_inner).is_none() {
            let fresh = node.children();
            let unused = {
                // Another iterator may have finished the list and dropped its
                // provider iterator meanwhile; it does both under this lock.
                let mut inner = lock(&self.children_inner);
                if inner.is_none() && !self.children_finished.load(Ordering::Acquire) {
                    *inner = Some(fresh);
                    None
                } else {
                    Some(fresh)
                }
            };
            drop(unused);
        }
        NodeChildrenIter::from_shared(
            Arc::clone(&self.children_inner),
            Arc::clone(&self.children_cache),
            Arc::clone(&self.children_finished),
        )
        .with_parent_node(owner.clone())
    }

    fn attributes<'a>(&self, node: &Arc<dyn UiNode>) -> NodeAttributeIter<'a> {
        if !self.attrs_finished.load(Ordering::Acquire) && lock(&self.attrs_inner).is_none() {
            let fresh = node.attributes();
            let unused = {
                let mut inner = lock(&self.attrs_inner);
                if inner.is_none() {
                    *inner = Some(fresh);
                    None
                } else {
                    Some(fresh)
                }
            };
            drop(unused);
        }
        NodeAttributeIter::from_shared(
            Arc::clone(node),
            Arc::clone(&self.attrs_inner),
            Arc::clone(&self.attrs_cache),
            Arc::clone(&self.attrs_finished),
        )
    }

    /// Moves the wrappers of the children read so far to `release`, one level
    /// deeper than `rank`, and drops the rest outside the locks. What it drops
    /// holds only provider nodes and attributes.
    fn detach(&self, rank: isize, release: &mut Release) {
        let children = std::mem::take(&mut *lock(&self.children_cache));
        release.pending.extend(children.into_iter().map(|child| (rank + 1, child)));
        let attributes = std::mem::take(&mut *lock(&self.attrs_cache));
        let iterators = (lock(&self.children_inner).take(), lock(&self.attrs_inner).take());
        drop((attributes, iterators));
    }
}

/// The work of releasing a snapshot, or the part of one that a dropped wrapper
/// held, without recursion (see [`Release::run`]).
#[derive(Default)]
struct Release {
    /// Wrappers still to take apart, with their depth relative to the first.
    pending: Vec<(isize, RuntimeXdmNode)>,
    /// The provider nodes of the wrappers taken apart, with their depth.
    nodes: Vec<(isize, Arc<dyn UiNode>)>,
}

impl Release {
    /// Takes the pending wrappers apart one by one, where the last reference
    /// to them is this one, and then drops the provider nodes they held, the
    /// deepest first.
    ///
    /// A provider node keeps its parent alive (`UiNode::parent`), so dropping
    /// a parent before its children would release a whole chain of ancestors
    /// recursively as the last child goes. Deepest first, every node's parent
    /// is still held here when the node is released. The depth is relative:
    /// a child is one level below its parent's wrapper and an owned parent one
    /// level above its element, so it follows the provider's tree in both
    /// directions.
    fn run(mut self) {
        while let Some((rank, wrapper)) = self.pending.pop() {
            match wrapper {
                RuntimeXdmNode::Document(doc) => {
                    if let Some(mut doc) = Arc::into_inner(doc) {
                        doc.detach(rank, &mut self);
                    }
                }
                RuntimeXdmNode::Element(elem) => {
                    if let Some(mut elem) = Arc::into_inner(elem) {
                        elem.detach(rank, &mut self);
                    }
                }
                RuntimeXdmNode::Attribute(_) => {}
            }
            // A wrapper taken apart is dropped here; its own `Drop` finds
            // nothing left to release.
        }
        self.nodes.sort_by_key(|(rank, _)| *rank);
        while let Some(node) = self.nodes.pop() {
            drop(node);
        }
    }
}

/// What a document or element holds in place of its provider node once a
/// release has taken the node: a placeholder shared by all of them.
fn released_node() -> Arc<dyn UiNode> {
    static RELEASED: LazyLock<Arc<dyn UiNode>> = LazyLock::new(|| Arc::new(DummyNode));
    Arc::clone(&RELEASED)
}

struct DocumentData {
    root: Arc<dyn UiNode>,
    runtime_id: RuntimeId,
    content: LazyContent,
}

impl DocumentData {
    fn new(root: Arc<dyn UiNode>, runtime_id: RuntimeId) -> Self {
        Self { root, runtime_id, content: LazyContent::new() }
    }

    /// Moves this document's children to `release`, and its provider node to
    /// the nodes it drops.
    fn detach(&mut self, rank: isize, release: &mut Release) {
        self.content.detach(rank, release);
        release.nodes.push((rank, std::mem::replace(&mut self.root, released_node())));
    }
}

impl Drop for DocumentData {
    fn drop(&mut self) {
        let mut release = Release::default();
        self.detach(0, &mut release);
        release.run();
    }
}

struct ElementData {
    node: Arc<dyn UiNode>,
    runtime_id: RuntimeId,
    role: String,
    qname: QName,
    order_key: Option<u64>,
    content: LazyContent,
    parent: Mutex<ParentLink>,
}

impl ElementData {
    fn new(
        node: Arc<dyn UiNode>,
        runtime_id: RuntimeId,
        namespace: UiNamespace,
        role: String,
        order_key: Option<u64>,
    ) -> Self {
        let qname = element_qname(namespace, &role);
        Self {
            node,
            runtime_id,
            role,
            qname,
            order_key,
            content: LazyContent::new(),
            parent: Mutex::new(ParentLink::Unresolved),
        }
    }

    /// The wrapper of this element's parent: the wrapper whose list of
    /// children holds this element, while it lives; otherwise one built from
    /// the provider's parent and owned here. If the provider's parent is gone,
    /// that is a document that wraps the element itself.
    fn parent_wrapper(&self) -> RuntimeXdmNode {
        {
            let link = lock(&self.parent);
            match &*link {
                ParentLink::Owned(parent) => return parent.clone(),
                ParentLink::Linked(parent) => {
                    if let Some(parent) = parent.upgrade() {
                        return parent;
                    }
                }
                ParentLink::Unresolved => {}
            }
        }
        // Built outside the lock: this calls the provider.
        let built = match self.node.parent().and_then(|parent| parent.upgrade()) {
            Some(parent) => RuntimeXdmNode::from_node(parent),
            None => RuntimeXdmNode::document(Arc::clone(&self.node)),
        };
        let replaced = std::mem::replace(&mut *lock(&self.parent), ParentLink::Owned(built.clone()));
        drop(replaced);
        built
    }

    /// Moves this element's children and owned parent to `release`, and its
    /// provider node to the nodes it drops. The node itself moves, rather than
    /// a copy of it: kept here until after the release, it would be the last
    /// reference to a chain of owned ancestors that the release has already let
    /// go of, and dropping it would then release that chain recursively.
    fn detach(&mut self, rank: isize, release: &mut Release) {
        self.content.detach(rank, release);
        // A linked parent belongs to the list that holds this element; only an
        // owned one is this element's to release.
        let link = std::mem::replace(&mut *lock(&self.parent), ParentLink::Unresolved);
        if let ParentLink::Owned(parent) = link {
            release.pending.push((rank - 1, parent));
        }
        release.nodes.push((rank, std::mem::replace(&mut self.node, released_node())));
    }
}

impl Drop for ElementData {
    fn drop(&mut self) {
        let mut release = Release::default();
        self.detach(0, &mut release);
        release.run();
    }
}

struct AttributeData {
    owner: Arc<dyn UiNode>,
    owner_runtime_id: RuntimeId,
    namespace: UiNamespace,
    name: String,
    qname: QName,
    // Lazy value provider (either direct source or derived component)
    value_kind: ValueKind,
    value_cell: std::sync::OnceLock<UiValue>,
    typed_cell: std::sync::OnceLock<Vec<XdmAtomicValue>>,
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
        let qname = attribute_qname(namespace, &name);
        Self {
            owner,
            owner_runtime_id,
            namespace,
            name,
            qname,
            value_kind: ValueKind::Source(source),
            value_cell: std::sync::OnceLock::new(),
            typed_cell: std::sync::OnceLock::new(),
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
        let qname = attribute_qname(namespace, &name);
        Self {
            owner,
            owner_runtime_id,
            namespace,
            name,
            qname,
            value_kind: ValueKind::RectComp { base, comp },
            value_cell: std::sync::OnceLock::new(),
            typed_cell: std::sync::OnceLock::new(),
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
        let qname = attribute_qname(namespace, &name);
        Self {
            owner,
            owner_runtime_id,
            namespace,
            name,
            qname,
            value_kind: ValueKind::PointComp { base, comp },
            value_cell: std::sync::OnceLock::new(),
            typed_cell: std::sync::OnceLock::new(),
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
    format!("{base}.{suffix}")
}

fn element_qname(ns: UiNamespace, role: &str) -> QName {
    QName {
        prefix: namespace_prefix(ns).map(std::string::ToString::to_string),
        local: role.to_string(),
        ns_uri: Some(namespace_uri(ns).to_string()),
    }
}

fn attribute_qname(ns: UiNamespace, name: &str) -> QName {
    QName {
        prefix: attribute_prefix(ns).map(std::string::ToString::to_string),
        local: name.to_string(),
        ns_uri: attribute_namespace(ns).map(std::string::ToString::to_string),
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
    use XdmAtomicValue::{
        AnyUri, Base64Binary, Boolean, Byte, Date, DateTime, DayTimeDuration, Decimal, Double, Entity, Float, GDay,
        GMonth, GMonthDay, GYear, GYearMonth, HexBinary, Id, IdRef, Int, Integer, Language, Long, NCName, NMTOKEN,
        Name, NegativeInteger, NonNegativeInteger, NonPositiveInteger, NormalizedString, Notation, PositiveInteger,
        QName, Short, String, Time, Token, UnsignedByte, UnsignedInt, UnsignedLong, UnsignedShort, UntypedAtomic,
        YearMonthDuration,
    };
    match value {
        Boolean(b) => UiValue::Bool(*b),
        String(s) | UntypedAtomic(s) | AnyUri(s) | NormalizedString(s) | Token(s) | Language(s) | Name(s)
        | NCName(s) | NMTOKEN(s) | Id(s) | IdRef(s) | Entity(s) | Notation(s) => UiValue::String(s.clone()),
        Integer(i) | Long(i) | NonPositiveInteger(i) | NegativeInteger(i) => UiValue::Integer(*i),
        Decimal(d) => {
            use rust_decimal::prelude::ToPrimitive;
            UiValue::Number(d.to_f64().unwrap_or(f64::NAN))
        }
        Double(d) => UiValue::Number(*d),
        Float(f) => UiValue::Number(f64::from(*f)),
        // Values above `i64::MAX` wrap to negative integers (bit-for-bit reinterpretation).
        UnsignedLong(u) | NonNegativeInteger(u) | PositiveInteger(u) => UiValue::Integer(u.cast_signed()),
        UnsignedInt(u) => UiValue::Integer(i64::from(*u)),
        UnsignedShort(u) => UiValue::Integer(i64::from(*u)),
        UnsignedByte(u) => UiValue::Integer(i64::from(*u)),
        Int(i) => UiValue::Integer(i64::from(*i)),
        Short(i) => UiValue::Integer(i64::from(*i)),
        Byte(i) => UiValue::Integer(i64::from(*i)),
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
            Some(offset) => format!("{date}{offset}"),
            None => date.to_string(),
        }),
        Time { time, tz } => UiValue::String(match tz {
            Some(offset) => format!("{time}{offset}"),
            None => time.to_string(),
        }),
        YearMonthDuration(months) => UiValue::String(format!("P{months}M")),
        DayTimeDuration(secs) => UiValue::String(format!("PT{secs}S")),
        Base64Binary(data) | HexBinary(data) => UiValue::String(data.clone()),
        GYear { year, tz } => UiValue::String(format!("{}{}", year, tz.map(|o| o.to_string()).unwrap_or_default())),
        GYearMonth { year, month, tz } => {
            UiValue::String(format!("{}-{:02}{}", year, month, tz.map(|o| o.to_string()).unwrap_or_default()))
        }
        GMonth { month, tz } => {
            UiValue::String(format!("{:02}{}", month, tz.map(|o| o.to_string()).unwrap_or_default()))
        }
        GMonthDay { month, day, tz } => {
            UiValue::String(format!("{:02}-{:02}{}", month, day, tz.map(|o| o.to_string()).unwrap_or_default()))
        }
        GDay { day, tz } => UiValue::String(format!("{:02}{}", day, tz.map(|o| o.to_string()).unwrap_or_default())),
    }
}

struct NodeChildrenIter<'a> {
    inner: NodeIteratorCell,
    cache: NodeCacheCell,
    finished: SharedFlag,
    pos: usize,
    _marker: std::marker::PhantomData<&'a ()>,
    parent_node: Option<RuntimeXdmNode>,
}
impl NodeChildrenIter<'_> {
    fn from_shared(inner: NodeIteratorCell, cache: NodeCacheCell, finished: SharedFlag) -> Self {
        Self { inner, cache, finished, pos: 0, _marker: std::marker::PhantomData, parent_node: None }
    }
    fn with_parent_node(mut self, parent: RuntimeXdmNode) -> Self {
        self.parent_node = Some(parent);
        self
    }
    fn empty() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            cache: Arc::new(Mutex::new(Vec::new())),
            finished: Arc::new(AtomicBool::new(true)),
            pos: 0,
            _marker: std::marker::PhantomData,
            parent_node: None,
        }
    }
}
/// The child at `pos` if the shared list already holds it, advancing `pos`.
fn cached_child(cache: &NodeCacheCell, pos: &mut usize) -> Option<RuntimeXdmNode> {
    let item = lock(cache).get(*pos).cloned();
    if item.is_some() {
        *pos += 1;
    }
    item
}

impl Iterator for NodeChildrenIter<'_> {
    type Item = RuntimeXdmNode;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = cached_child(&self.cache, &mut self.pos) {
            return Some(item);
        }
        // Several iterators may read one list, and children are appended under
        // `inner`, so each check of the shared list is repeated where another
        // iterator may have appended meanwhile: the list is marked finished
        // only after its last child was appended.
        if self.finished.load(Ordering::Acquire) {
            return cached_child(&self.cache, &mut self.pos);
        }
        let mut inner = lock(&self.inner);
        if let Some(item) = cached_child(&self.cache, &mut self.pos) {
            return Some(item);
        }
        let Some(owner) = inner.as_mut().and_then(Iterator::next) else {
            self.finished.store(true, Ordering::Release);
            // Release the provider's exhausted iterator (for UI Automation its
            // tree walker and cache request) outside the lock.
            let exhausted = inner.take();
            drop(inner);
            drop(exhausted);
            return None;
        };
        let node = RuntimeXdmNode::from_node(owner);
        // Link the child to the parent wrapper whose list holds it (Document or
        // Element), so that cursor helpers like next_sibling_in_doc() get the
        // SAME parent wrapper, with its shared list of children, instead of
        // re-enumerating the provider (O(N²) COM tree walks) per sibling lookup.
        // The link is weak: the parent owns the child, not the other way round.
        if let RuntimeXdmNode::Element(elem) = &node
            && let Some(parent) = self.parent_node.as_ref().and_then(RuntimeXdmNode::downgrade)
        {
            *lock(&elem.parent) = ParentLink::Linked(parent);
        }
        lock(&self.cache).push(node.clone());
        self.pos += 1;
        Some(node)
    }
}

struct NodeAttributeIter<'a> {
    owner: Arc<dyn UiNode>,
    inner: AttributeIteratorCell,
    cache: NodeCacheCell,
    finished: SharedFlag,
    pos: usize,
    _marker: std::marker::PhantomData<&'a ()>,
}
impl NodeAttributeIter<'_> {
    fn from_shared(
        owner: Arc<dyn UiNode>,
        inner: AttributeIteratorCell,
        cache: NodeCacheCell,
        finished: SharedFlag,
    ) -> Self {
        Self { owner, inner, cache, finished, pos: 0, _marker: std::marker::PhantomData }
    }
    fn empty() -> Self {
        Self {
            owner: Arc::new(DummyNode),
            inner: Arc::new(Mutex::new(None)),
            cache: Arc::new(Mutex::new(Vec::new())),
            finished: Arc::new(AtomicBool::new(true)),
            pos: 0,
            _marker: std::marker::PhantomData,
        }
    }
}
impl Iterator for NodeAttributeIter<'_> {
    type Item = RuntimeXdmNode;
    fn next(&mut self) -> Option<Self::Item> {
        {
            let cache = lock(&self.cache);
            if self.pos < cache.len() {
                let item = cache[self.pos].clone();
                drop(cache);
                self.pos += 1;
                return Some(item);
            }
        }
        if self.finished.load(Ordering::Acquire) {
            return None;
        }
        let mut inner_borrow = lock(&self.inner);
        let iter = inner_borrow.as_mut()?;
        if let Some(attr) = iter.next() {
            let ns = attr.namespace();
            let base_name = attr.name().to_string();
            let src = attr.clone();
            {
                let mut cache = lock(&self.cache);
                cache.push(RuntimeXdmNode::attribute(AttributeData::new_from_source(
                    self.owner.clone(),
                    ns,
                    base_name.clone(),
                    src.clone(),
                )));
                if base_name == attribute_names::element::BOUNDS {
                    cache.push(RuntimeXdmNode::attribute(AttributeData::new_rect_component(
                        self.owner.clone(),
                        ns,
                        src.clone(),
                        attribute_names::element::BOUNDS,
                        RectComp::X,
                    )));
                    cache.push(RuntimeXdmNode::attribute(AttributeData::new_rect_component(
                        self.owner.clone(),
                        ns,
                        src.clone(),
                        attribute_names::element::BOUNDS,
                        RectComp::Y,
                    )));
                    cache.push(RuntimeXdmNode::attribute(AttributeData::new_rect_component(
                        self.owner.clone(),
                        ns,
                        src.clone(),
                        attribute_names::element::BOUNDS,
                        RectComp::Width,
                    )));
                    cache.push(RuntimeXdmNode::attribute(AttributeData::new_rect_component(
                        self.owner.clone(),
                        ns,
                        src,
                        attribute_names::element::BOUNDS,
                        RectComp::Height,
                    )));
                } else if base_name == attribute_names::activation_target::ACTIVATION_POINT {
                    let src_point = attr.clone();
                    cache.push(RuntimeXdmNode::attribute(AttributeData::new_point_component(
                        self.owner.clone(),
                        ns,
                        src_point.clone(),
                        attribute_names::activation_target::ACTIVATION_POINT,
                        PointComp::X,
                    )));
                    cache.push(RuntimeXdmNode::attribute(AttributeData::new_point_component(
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
                let cache = lock(&self.cache);
                if idx < cache.len() {
                    let it = cache[idx].clone();
                    self.pos += 1;
                    return Some(it);
                }
            }
            // Fallback: inconsistent state — mark finished
            self.finished.store(true, Ordering::Release);
            None
        } else {
            self.finished.store(true, Ordering::Release);
            None
        }
    }
}

struct DummyNode;
impl UiNode for DummyNode {
    fn namespace(&self) -> UiNamespace {
        UiNamespace::Control
    }
    fn role(&self) -> &'static str {
        ""
    }
    fn name(&self) -> String {
        String::new()
    }
    fn runtime_id(&self) -> &RuntimeId {
        static RID: std::sync::OnceLock<RuntimeId> = std::sync::OnceLock::new();
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
    fn supported_patterns(&self) -> Vec<PatternName> {
        Vec::new()
    }
    fn invalidate(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{logged, records};
    use platynui_core::provider::ProviderError;
    use platynui_core::types::Rect;
    use platynui_core::ui::{PatternName, RuntimeId, UiAttribute, UiNode, attribute_names, supported_patterns_value};
    use platynui_xpath::engine::runtime::ErrorCode;
    use rstest::rstest;
    use std::sync::{Arc, Mutex, Weak};

    #[rstest]
    #[case("//x", false)]
    #[case("/Window", false)]
    #[case("(//x)[1]", false)]
    #[case("//x[.='y']", false)]
    #[case(".//x", true)]
    #[case(".", true)]
    #[case("count(//x)", false)]
    #[case("count(.//x)", true)]
    #[case("name()", true)]
    fn is_context_dependent_classifies(#[case] expr: &str, #[case] expected: bool) {
        assert_eq!(is_context_dependent(expr).expect("should parse"), expected, "for {expr}");
    }

    #[test]
    fn is_context_dependent_rejects_malformed_xpath() {
        assert!(matches!(is_context_dependent("//x[broken"), Err(EvaluateError::XPath(_))));
    }

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
        patterns: Vec<PatternName>,
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
            let patterns_vec: Vec<PatternName> = patterns.into_iter().map(PatternName::from).collect();
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

        fn supported_patterns(&self) -> Vec<PatternName> {
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
            other => panic!("unexpected evaluation result: {other:?}"),
        }
    }

    #[rstest]
    fn evaluates_count_function() {
        let tree = sample_tree();
        let items = evaluate(None, "count(//Window)", EvaluateOptions::new(tree.clone())).unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            EvaluationItem::Value(value) => assert_eq!(value, &UiValue::Integer(1)),
            other => panic!("unexpected evaluation result: {other:?}"),
        }
    }

    #[rstest]
    fn xpath_records_exist_in_debug_builds_only() {
        let tree = sample_tree();
        let (result, log) = logged(|| evaluate(None, "trace(count(//Window), 'windows')", EvaluateOptions::new(tree)));
        result.expect("the expression evaluates");

        let (trace, own): (Vec<&str>, Vec<&str>) = log.lines().partition(|line| line.contains("fn:trace"));
        assert!(
            matches!(trace.as_slice(), [line] if line.trim_start().starts_with("DEBUG")
                && line.contains("label=windows")
                && line.contains("value=1")),
            "exactly one debug record of fn:trace() with its label and value: {log}"
        );
        assert_eq!(
            own.is_empty(),
            !cfg!(debug_assertions),
            "the evaluation's own records exist in debug builds only; found: {own:#?}"
        );
        if cfg!(debug_assertions) {
            for level in ["DEBUG", "TRACE"] {
                assert!(
                    own.iter().any(|line| line.trim_start().starts_with(level)),
                    "no {level} record of the evaluation: {log}"
                );
            }
            for line in &own {
                let target = line.split_whitespace().nth(1).unwrap_or_default();
                assert!(target.starts_with("platynui"), "a record outside PlatynUI's modules: {line}");
            }
        }
    }

    #[rstest]
    #[case::does_not_compile("//Window[", ErrorCode::XPST0003)]
    #[case::fails_while_running("xs:integer(//Window/@Name)", ErrorCode::FORG0001)]
    #[case::calls_error("error()", ErrorCode::FOER0000)]
    fn a_failing_expression_is_returned_not_logged(#[case] xpath: &str, #[case] code: ErrorCode) {
        let tree = sample_tree();
        let (result, log) = logged(|| evaluate(None, xpath, EvaluateOptions::new(tree)));
        match result {
            Err(EvaluateError::XPath(err)) => assert_eq!(err.code_enum(), code, "for {xpath}: {err}"),
            other => panic!("expected an XPath error for {xpath}, got {other:?}"),
        }
        assert!(records(&log, "WARN").is_empty(), "a returned failure is not logged as a warning: {log}");
        assert!(records(&log, "ERROR").is_empty(), "a returned failure is not logged as an error: {log}");
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
            other => panic!("unexpected evaluation result: {other:?}"),
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
            other => panic!("unexpected attribute result: {other:?}"),
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
                other => panic!("expected attribute node, got {other:?}"),
            }
        }
        let items = evaluate(None, "data(//@*:Bounds.Width)", EvaluateOptions::new(tree.clone())).unwrap();
        assert!(!items.is_empty());
        let mut widths = Vec::new();
        for item in items {
            match item {
                EvaluationItem::Value(UiValue::Number(n)) => widths.push(n),
                other => panic!("expected numeric UiValue, got {other:?}"),
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
                other => panic!("expected boolean UiValue, got {other:?}"),
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
            other => panic!("expected serialized bounds string, got {other:?}"),
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
                    other => panic!("unexpected attribute type: {other:?}"),
                }
            }
            other => panic!("unexpected monitors result: {other:?}"),
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
            Err(ProviderError::TreeUnavailable { provider: "resolver", details: None })
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
            other => panic!("unexpected result: {other:?}"),
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
            other => panic!("unexpected result: {other:?}"),
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
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
