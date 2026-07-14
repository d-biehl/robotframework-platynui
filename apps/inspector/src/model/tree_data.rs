//! Model layer: data structures wrapping `UiNode` for the inspector.
//!
//! This module provides cached wrappers and display-ready types that bridge
//! the PlatynUI runtime (`UiNode`, `UiAttribute`, `UiValue`) to the inspector
//! UI without coupling to any GUI framework.

use platynui_core::ui::{Namespace, UiNode, UiValue};
use platynui_runtime::EvaluationItem;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A single XPath search result, ready for display.
#[derive(Clone)]
pub enum SearchResultItem {
    /// Result is a UI node (clickable — reveals in tree).
    Node {
        /// Human-readable label (role + name).
        label: String,
        /// Reference to the underlying node.
        node: Arc<dyn UiNode>,
    },
    /// Result is an attribute value (clickable — reveals owner node in tree).
    Attribute {
        /// Display label showing namespace:name = value.
        label: String,
        /// Raw formatted attribute value for copy actions.
        value: String,
        /// Owner node for tree reveal.
        node: Arc<dyn UiNode>,
    },
    /// Result is a plain value (string, number, etc.).
    Value {
        /// Display label with value and type.
        label: String,
    },
}

impl SearchResultItem {
    /// Create from a PlatynUI `EvaluationItem`.
    pub fn from_evaluation_item(item: &EvaluationItem) -> Self {
        match item {
            EvaluationItem::Node(node) => {
                let name = node.name();
                let escaped = escape_control_chars(&name);
                let label = if escaped.is_empty() {
                    node.role().to_string()
                } else {
                    format!("{} \"{}\"", node.role(), escaped)
                };
                Self::Node { label, node: Arc::clone(node) }
            }
            EvaluationItem::Attribute(attr) => {
                let (val_str, _) = format_ui_value(&attr.value);
                let label = format!("@{} = {}", xpath_attribute_name(attr.namespace.as_str(), &attr.name), val_str);
                Self::Attribute { label, value: val_str, node: Arc::clone(&attr.owner) }
            }
            EvaluationItem::Value(val) => {
                let (val_str, ty_str) = format_ui_value(val);
                Self::Value { label: format!("{val_str} ({ty_str})") }
            }
        }
    }

    /// Human-readable label for rendering.
    pub fn display_label(&self) -> &str {
        match self {
            Self::Node { label, .. } | Self::Attribute { label, .. } | Self::Value { label } => label,
        }
    }

    /// Returns true if this result can be revealed in the tree (Node or Attribute).
    pub fn is_node(&self) -> bool {
        matches!(self, Self::Node { .. } | Self::Attribute { .. })
    }

    /// Get the `UiNode` reference for tree reveal (works for Node and Attribute).
    pub fn ui_node(&self) -> Option<&Arc<dyn UiNode>> {
        match self {
            Self::Node { node, .. } | Self::Attribute { node, .. } => Some(node),
            Self::Value { .. } => None,
        }
    }

    /// Owner runtime id for node-backed results.
    pub fn runtime_id(&self) -> Option<String> {
        self.ui_node().map(|node| node.runtime_id().as_str().to_string())
    }

    /// Raw attribute value for attribute results.
    pub fn attribute_value(&self) -> Option<&str> {
        match self {
            Self::Attribute { value, .. } => Some(value),
            Self::Node { .. } | Self::Value { .. } => None,
        }
    }

    /// A fuller copy representation than the plain display label.
    pub fn full_copy_text(&self) -> String {
        match self {
            Self::Node { label, node } | Self::Attribute { label, node, .. } => {
                format!("{label} [{}]", node.runtime_id().as_str())
            }
            Self::Value { label } => label.clone(),
        }
    }
}

/// A single attribute as displayed in the attributes table.
#[derive(Clone, Debug)]
pub struct DisplayAttribute {
    /// Namespace prefix (control, item, app, native).
    pub namespace: String,
    /// Attribute name (PascalCase).
    pub name: String,
    /// Formatted value string.
    pub value: String,
    /// Type label (bool, string, Rect, etc.).
    pub value_type: String,
}

/// Cached wrapper around a `UiNode` for the inspector tree.
///
/// All cached fields are protected by `Mutex` for `Send + Sync` compatibility.
/// Call `refresh()` to invalidate caches so values are re-queried from the
/// native accessibility API on next access.
pub struct UiNodeData {
    node: Arc<dyn UiNode>,
    namespace_cache: Mutex<Option<Namespace>>,
    id_cache: Mutex<Option<String>>,
    label_cache: Mutex<Option<String>>,
    has_children_cache: Mutex<Option<bool>>,
    children_cache: Mutex<Option<Vec<Arc<UiNodeData>>>>,
    is_valid_cache: Mutex<Option<bool>>,
}

impl UiNodeData {
    /// Wrap a `UiNode` in a new `UiNodeData` with empty caches.
    pub fn new(node: Arc<dyn UiNode>) -> Self {
        Self {
            node,
            namespace_cache: Mutex::new(None),
            id_cache: Mutex::new(None),
            label_cache: Mutex::new(None),
            has_children_cache: Mutex::new(None),
            children_cache: Mutex::new(None),
            is_valid_cache: Mutex::new(None),
        }
    }

    /// Runtime ID string (cached).
    /// Note: No warning here since we pre-fill cache during initial load.
    pub fn id(&self) -> String {
        if let Some(v) = self.id_cache.lock().unwrap().as_ref() {
            return v.clone();
        }
        let v = self.node.runtime_id().as_str().to_string();
        *self.id_cache.lock().unwrap() = Some(v.clone());
        v
    }

    /// Cached runtime ID without triggering provider I/O.
    pub fn cached_id(&self) -> Option<String> {
        self.id_cache.lock().unwrap().clone()
    }

    /// Preload the node namespace.
    pub fn preload_namespace(&self) {
        let namespace = self.node.namespace();
        *self.namespace_cache.lock().unwrap() = Some(namespace);
    }

    /// Preload the complete display label, including the node name.
    pub fn preload_label(&self) {
        let label = format_node_label(self.node.as_ref());
        *self.label_cache.lock().unwrap() = Some(label);
    }

    /// Cached display label without triggering provider I/O.
    pub fn cached_label(&self) -> Option<String> {
        self.label_cache.lock().unwrap().clone()
    }

    /// Children as `UiNodeData` wrappers (cached; triggers lazy load on first call).
    pub fn children(&self) -> Vec<Arc<UiNodeData>> {
        if let Some(v) = self.children_cache.lock().unwrap().as_ref() {
            return v.clone();
        }
        let _ = self.load_children_incremental(|_, _| {}, |_, _| {});
        self.cached_children().unwrap_or_default()
    }

    /// Load children on the current thread, publishing each child to the cache as it arrives.
    ///
    /// Use this only from background task contexts where blocking provider calls are acceptable.
    pub fn load_children_incremental<F, G>(&self, mut on_child: F, mut on_child_updated: G) -> usize
    where
        F: FnMut(Arc<UiNodeData>, usize),
        G: FnMut(Arc<UiNodeData>, usize),
    {
        *self.children_cache.lock().unwrap() = Some(Vec::new());
        *self.has_children_cache.lock().unwrap() = Some(false);

        let streaming_start = Instant::now();
        let mut child_count = 0;
        let mut loaded_children = Vec::new();
        let mut child_nodes = self.node.children();
        loop {
            let next_start = Instant::now();
            let Some(child_node) = child_nodes.next() else {
                log_child_load_timing("iterator-complete", child_count, next_start.elapsed());
                break;
            };
            log_child_load_timing("iterator-next", child_count + 1, next_start.elapsed());

            let data = Arc::new(UiNodeData::new(child_node));
            let row_preload_start = Instant::now();
            data.preload_row_caches();
            log_child_load_timing("row-preload", child_count + 1, row_preload_start.elapsed());
            child_count += 1;

            let mut cache = self.children_cache.lock().unwrap();
            let children = cache.get_or_insert_with(Vec::new);
            children.push(Arc::clone(&data));
            drop(cache);

            *self.has_children_cache.lock().unwrap() = Some(true);
            on_child(Arc::clone(&data), child_count);
            loaded_children.push((child_count, data));
        }

        log_child_load_timing("children-streamed", child_count, streaming_start.elapsed());
        *self.has_children_cache.lock().unwrap() = Some(child_count > 0);

        let metadata_start = Instant::now();
        for (child_index, data) in loaded_children {
            let metadata_preload_start = Instant::now();
            data.preload_deferred_row_caches();
            log_child_load_timing("deferred-row-preload", child_index, metadata_preload_start.elapsed());
            on_child_updated(data, child_index);
        }
        log_child_load_timing("deferred-row-preload-complete", child_count, metadata_start.elapsed());

        child_count
    }

    /// Return already-cached children without triggering any I/O.
    ///
    /// Returns `None` if children have not been loaded yet.
    pub fn cached_children(&self) -> Option<Vec<Arc<UiNodeData>>> {
        self.children_cache.lock().unwrap().clone()
    }

    /// Return cached knowledge about whether this node has children.
    ///
    /// Returns `None` when child presence has not been probed yet.
    pub fn cached_has_children(&self) -> Option<bool> {
        if let Some(children) = self.children_cache.lock().unwrap().as_ref() {
            return Some(!children.is_empty());
        }
        *self.has_children_cache.lock().unwrap()
    }

    /// Preload row data that is needed before a child is first streamed to the UI.
    pub fn preload_row_caches(&self) {
        let _ = self.id();
        self.preload_namespace();
        self.preload_label();
    }

    /// Preload row data that can be updated after a child is already visible.
    pub fn preload_deferred_row_caches(&self) {
        let _ = self.is_valid();
        if self.has_children_cache.lock().unwrap().is_none() {
            let has_ch = self.node.has_children();
            *self.has_children_cache.lock().unwrap() = Some(has_ch);
        }
    }

    /// Whether the underlying node is still valid (not destroyed).
    /// Note: No warning here since we pre-fill cache during initial load.
    pub fn is_valid(&self) -> bool {
        if let Some(v) = self.is_valid_cache.lock().unwrap().as_ref() {
            return *v;
        }
        let is_valid = self.node.is_valid();
        *self.is_valid_cache.lock().unwrap() = Some(is_valid);
        is_valid
    }

    /// Cached validity without triggering provider I/O.
    pub fn cached_is_valid(&self) -> Option<bool> {
        *self.is_valid_cache.lock().unwrap()
    }

    /// Whether this node has a parent, evaluated on the current thread.
    ///
    /// Use this only from background task contexts where blocking provider calls
    /// are acceptable.
    pub fn has_parent_direct(&self) -> bool {
        self.node.parent().is_some()
    }

    /// Collect all attributes on the current thread.
    ///
    /// Use this only from background task contexts where blocking provider calls
    /// are acceptable.
    pub fn display_attributes_direct(&self) -> Vec<DisplayAttribute> {
        collect_display_attributes(self.node.as_ref())
    }

    /// Get the Bounds rect on the current thread.
    ///
    /// Use this only from background task contexts where blocking provider calls
    /// are acceptable.
    pub fn bounds_rect_direct(&self) -> Option<platynui_core::types::Rect> {
        extract_bounds_rect(self.node.as_ref())
    }

    /// Clear cached child data without querying the provider.
    pub fn clear_children_cache(&self) {
        *self.children_cache.lock().unwrap() = None;
        *self.has_children_cache.lock().unwrap() = None;
    }

    /// Graft a picker-resolved live subtree under this node so the reveal can
    /// select it even when the tree's own top-down enumeration cannot reach it.
    /// Dynamic XAML / Chromium menus hand out UIA RuntimeIds that differ between
    /// the hit-test's `GetParentElement` walk and a `GetChildren` enumeration, so
    /// matching the resolved node's ancestor chain against re-enumerated children
    /// diverges. `chain` is top-down — the direct child of `self` first, down to
    /// the resolved target; each node becomes a nested `UiNodeData`, and the top
    /// is appended to this node's cached children (deduplicated by runtime id).
    pub fn graft_chain(&self, chain: &[Arc<dyn UiNode>]) {
        let mut built: Option<Arc<UiNodeData>> = None;
        for node in chain.iter().rev() {
            let data = Arc::new(UiNodeData::new(Arc::clone(node)));
            data.preload_row_caches();
            if let Some(child) = built.take() {
                *data.children_cache.lock().unwrap() = Some(vec![child]);
                *data.has_children_cache.lock().unwrap() = Some(true);
            }
            built = Some(data);
        }
        let Some(top) = built else {
            return;
        };
        let mut cache = self.children_cache.lock().unwrap();
        let children = cache.get_or_insert_with(Vec::new);
        if !children.iter().any(|c| c.id() == top.id()) {
            children.push(top);
        }
        *self.has_children_cache.lock().unwrap() = Some(true);
    }

    /// Clear cached descendants without querying the provider.
    pub fn clear_children_cache_recursive(&self) {
        let cached_children = self.children_cache.lock().unwrap().clone();
        self.clear_children_cache();
        if let Some(children) = cached_children.as_ref() {
            for child in children {
                child.clear_children_cache_recursive();
            }
        }
    }

    /// Invalidate all caches so values are re-queried on next access.
    pub fn refresh(&self) {
        self.node.invalidate();
        *self.namespace_cache.lock().unwrap() = None;
        *self.id_cache.lock().unwrap() = None;
        *self.label_cache.lock().unwrap() = None;
        *self.has_children_cache.lock().unwrap() = None;
        *self.children_cache.lock().unwrap() = None;
        *self.is_valid_cache.lock().unwrap() = None;
    }

    /// Recursively refresh this node and all cached children.
    pub fn refresh_recursive(&self) {
        let cached_children = self.children_cache.lock().unwrap().clone();
        self.refresh();
        if let Some(children) = cached_children.as_ref() {
            for child in children {
                child.refresh_recursive();
            }
        }
    }
}

fn format_node_label(node: &dyn UiNode) -> String {
    let name_str = node.name();
    let escaped = escape_control_chars(&name_str);
    if escaped.is_empty() { node.role().to_string() } else { format!("{} \"{}\"", node.role(), escaped) }
}

fn log_child_load_timing(stage: &'static str, child_count: usize, elapsed: Duration) {
    if elapsed > Duration::from_millis(200) {
        let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
        tracing::warn!(stage, child_count, elapsed_ms, "slow inspector tree child loading stage");
    }
}

/// XPath-conformant rendering of an attribute's qualified name.
///
/// `control` is the default attribute namespace in PlatynUI XPath: unprefixed
/// `@Foo` matches control attributes, while `@control:Foo` does not (the engine
/// follows standard XPath, where unprefixed attributes are in no namespace).
/// User-facing strings (display + clipboard) therefore drop the `control:`
/// prefix so what the user sees and copies works when pasted into a query.
pub fn xpath_attribute_name(namespace: &str, name: &str) -> String {
    if namespace == Namespace::Control.as_str() { name.to_string() } else { format!("{namespace}:{name}") }
}

fn collect_display_attributes(node: &dyn UiNode) -> Vec<DisplayAttribute> {
    let mut attrs = Vec::new();
    for attr in node.attributes() {
        let ns = attr.namespace();
        let name = attr.name().to_string();
        let value = attr.value();

        let (val_str, ty_str) = format_ui_value(&value);
        let ns_name = match ns {
            Namespace::Control => "control",
            Namespace::Item => "item",
            Namespace::App => "app",
            Namespace::Native => "native",
        };

        attrs.push(DisplayAttribute { namespace: ns_name.to_string(), name, value: val_str, value_type: ty_str });
    }
    attrs
}

fn extract_bounds_rect(node: &dyn UiNode) -> Option<platynui_core::types::Rect> {
    for attr in node.attributes() {
        if let (Namespace::Control, "Bounds") = (attr.namespace(), attr.name())
            && let UiValue::Rect(r) = attr.value()
            && !r.is_empty()
        {
            return Some(r);
        }
    }
    None
}

/// Format a `UiValue` for display as `(value_string, type_label)`.
fn format_ui_value(value: &UiValue) -> (String, String) {
    match value {
        UiValue::Null => ("<null>".to_string(), "null".to_string()),
        UiValue::Bool(b) => (b.to_string(), "bool".to_string()),
        UiValue::Integer(i) => (i.to_string(), "integer".to_string()),
        UiValue::Number(n) => (n.to_string(), "number".to_string()),
        UiValue::String(s) => (s.clone(), "string".to_string()),
        UiValue::Point(p) => (format!("{:.0}, {:.0}", p.x(), p.y()), "Point".to_string()),
        UiValue::Size(s) => (format!("{:.0} x {:.0}", s.width(), s.height()), "Size".to_string()),
        UiValue::Rect(r) => {
            (format!("{:.0}, {:.0}, {:.0}, {:.0}", r.x(), r.y(), r.width(), r.height()), "Rect".to_string())
        }
        UiValue::Array(a) => {
            let mut s = String::from("[");
            for (i, it) in a.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                let _ = match it {
                    UiValue::String(st) => write!(&mut s, "{st}"),
                    _ => write!(&mut s, "{it:?}"),
                };
            }
            s.push(']');
            (s, "array".to_string())
        }
        UiValue::Object(o) => {
            let mut s = String::from("{");
            for (i, (k, v)) in o.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                let _ = write!(&mut s, "{k}: {v:?}");
            }
            s.push('}');
            (s, "object".to_string())
        }
    }
}

/// Escape control characters in a label for display.
///
/// Collapses consecutive `\r` and `\n` into a single space, and renders other
/// control characters as `\xNN` or `\u{NNNN}`.
fn escape_control_chars(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut it = input.chars().peekable();
    while let Some(ch) = it.next() {
        match ch {
            '\r' => {
                while let Some('\r') = it.peek() {
                    let _ = it.next();
                }
            }
            '\n' => {
                while let Some('\n' | '\r') = it.peek() {
                    let _ = it.next();
                }
                out.push(' ');
            }
            _ if ch.is_control() => {
                let code = ch as u32;
                if code <= 0xFF {
                    let _ = write!(&mut out, "\\x{code:02X}");
                } else {
                    let _ = write!(&mut out, "\\u{{{code:X}}}");
                }
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Minimal in-memory `UiNode` mock for exercising tree/reveal helpers
/// (`graft_chain`, `resolved_chain_below`) without a real provider. Shared by the
/// `tree_data` and `async_tasks` test modules.
#[cfg(test)]
pub(crate) mod test_mock {
    use platynui_core::ui::{Namespace, PatternName, RuntimeId, UiAttribute, UiNode};
    use std::sync::{Arc, Mutex, Weak};

    pub(crate) struct MockNode {
        rid: RuntimeId,
        name: String,
        parent: Mutex<Option<Weak<dyn UiNode>>>,
    }

    impl MockNode {
        pub(crate) fn new(id: &str) -> Arc<Self> {
            Arc::new(Self { rid: RuntimeId::from(id), name: id.to_string(), parent: Mutex::new(None) })
        }

        pub(crate) fn set_parent(&self, parent: &Arc<dyn UiNode>) {
            *self.parent.lock().unwrap() = Some(Arc::downgrade(parent));
        }
    }

    impl UiNode for MockNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &str {
            "Mock"
        }
        fn name(&self) -> String {
            self.name.clone()
        }
        fn runtime_id(&self) -> &RuntimeId {
            &self.rid
        }
        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            self.parent.lock().unwrap().clone()
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
}

#[cfg(test)]
mod tests {
    use super::UiNodeData;
    use super::test_mock::MockNode;
    use platynui_core::ui::UiNode;
    use std::sync::Arc;

    #[test]
    fn graft_chain_nests_the_subtree_and_dedupes_the_top() {
        let root = Arc::new(UiNodeData::new(MockNode::new("root") as Arc<dyn UiNode>));
        let chain: Vec<Arc<dyn UiNode>> = vec![
            MockNode::new("child") as Arc<dyn UiNode>,
            MockNode::new("mid") as Arc<dyn UiNode>,
            MockNode::new("leaf") as Arc<dyn UiNode>,
        ];

        root.graft_chain(&chain);

        // Top of the chain is appended as root's (only) child.
        let kids = root.cached_children().expect("children cached after graft");
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].id(), "child");

        // Nested down: child -> mid -> leaf.
        let mids = kids[0].cached_children().expect("child has cached children");
        assert_eq!(mids.iter().map(|n| n.id()).collect::<Vec<_>>(), vec!["mid".to_string()]);
        let leaves = mids[0].cached_children().expect("mid has cached children");
        assert_eq!(leaves.iter().map(|n| n.id()).collect::<Vec<_>>(), vec!["leaf".to_string()]);

        // The leaf is left lazy (no children cached) so the user can still expand it.
        assert!(leaves[0].cached_children().is_none());

        // Grafting the same chain again does not duplicate the top.
        root.graft_chain(&chain);
        assert_eq!(root.cached_children().unwrap().len(), 1);
    }

    #[test]
    fn graft_chain_of_empty_slice_is_a_noop() {
        let root = Arc::new(UiNodeData::new(MockNode::new("root") as Arc<dyn UiNode>));
        root.graft_chain(&[]);
        assert!(root.cached_children().is_none());
    }
}
