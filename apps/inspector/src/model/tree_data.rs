//! Model layer: data structures wrapping `UiNode` for the inspector.
//!
//! This module provides cached wrappers and display-ready types that bridge
//! the PlatynUI runtime (`UiNode`, `UiAttribute`, `UiValue`) to the inspector
//! UI without coupling to any GUI framework.

use platynui_core::ui::{Namespace, UiNode, UiValue};
use platynui_runtime::EvaluationItem;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

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
                Self::Attribute {
                    label: format!("@{}:{} = {}", attr.namespace, attr.name, val_str),
                    node: Arc::clone(&attr.owner),
                }
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
}

/// A single attribute as displayed in the properties table.
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
    id_cache: Mutex<Option<String>>,
    label_cache: Mutex<Option<String>>,
    has_children_cache: Mutex<Option<bool>>,
    children_cache: Mutex<Option<Vec<Arc<UiNodeData>>>>,
}

impl UiNodeData {
    /// Wrap a `UiNode` in a new `UiNodeData` with empty caches.
    pub fn new(node: Arc<dyn UiNode>) -> Self {
        Self {
            node,
            id_cache: Mutex::new(None),
            label_cache: Mutex::new(None),
            has_children_cache: Mutex::new(None),
            children_cache: Mutex::new(None),
        }
    }

    /// Runtime ID string (cached).
    pub fn id(&self) -> String {
        if let Some(v) = self.id_cache.lock().unwrap().as_ref() {
            return v.clone();
        }
        let v = self.node.runtime_id().as_str().to_string();
        *self.id_cache.lock().unwrap() = Some(v.clone());
        v
    }

    /// Display label: `Role "Name"` (cached).
    pub fn label(&self) -> String {
        if let Some(v) = self.label_cache.lock().unwrap().as_ref() {
            return v.clone();
        }
        let name_str = self.node.name();
        let escaped = escape_control_chars(&name_str);
        let label = if escaped.is_empty() {
            self.node.role().to_string()
        } else {
            format!("{} \"{}\"", self.node.role(), escaped)
        };
        *self.label_cache.lock().unwrap() = Some(label.clone());
        label
    }

    /// Whether the node has children (uses cache, falls back to `has_children()`).
    pub fn has_children(&self) -> bool {
        if let Some(children) = self.children_cache.lock().unwrap().as_ref() {
            return !children.is_empty();
        }
        if let Some(hc) = *self.has_children_cache.lock().unwrap() {
            return hc;
        }
        let has = self.node.has_children();
        *self.has_children_cache.lock().unwrap() = Some(has);
        has
    }

    /// Children as `UiNodeData` wrappers (cached; triggers lazy load on first call).
    pub fn children(&self) -> Vec<Arc<UiNodeData>> {
        if let Some(v) = self.children_cache.lock().unwrap().as_ref() {
            return v.clone();
        }
        let list: Vec<Arc<UiNodeData>> =
            self.node.children().map(|child_node| Arc::new(UiNodeData::new(child_node))).collect();
        *self.has_children_cache.lock().unwrap() = Some(!list.is_empty());
        *self.children_cache.lock().unwrap() = Some(list.clone());
        list
    }

    /// Return already-cached children without triggering any I/O.
    ///
    /// Returns `None` if children have not been loaded yet.
    pub fn cached_children(&self) -> Option<Vec<Arc<UiNodeData>>> {
        self.children_cache.lock().unwrap().clone()
    }

    /// Whether the underlying node is still valid (not destroyed).
    pub fn is_valid(&self) -> bool {
        self.node.is_valid()
    }

    /// Whether this node has a parent (false for the desktop root).
    pub fn has_parent(&self) -> bool {
        self.node.parent().is_some()
    }

    /// Collect all attributes formatted for the properties table.
    pub fn display_attributes(&self) -> Vec<DisplayAttribute> {
        let mut attrs = Vec::new();
        for attr in self.node.attributes() {
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

    /// Get the Bounds rect if available (for highlighting).
    pub fn bounds_rect(&self) -> Option<platynui_core::types::Rect> {
        for attr in self.node.attributes() {
            if let (Namespace::Control, "Bounds") = (attr.namespace(), attr.name())
                && let UiValue::Rect(r) = attr.value()
                && !r.is_empty()
            {
                return Some(r);
            }
        }
        None
    }

    /// Invalidate all caches so values are re-queried on next access.
    pub fn refresh(&self) {
        self.node.invalidate();
        *self.id_cache.lock().unwrap() = None;
        *self.label_cache.lock().unwrap() = None;
        *self.has_children_cache.lock().unwrap() = None;
        *self.children_cache.lock().unwrap() = None;
    }

    /// Recursively refresh this node and all cached children.
    pub fn refresh_recursive(&self) {
        self.refresh();
        if let Some(children) = self.children_cache.lock().unwrap().as_ref() {
            for child in children {
                child.refresh_recursive();
            }
        }
    }
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
