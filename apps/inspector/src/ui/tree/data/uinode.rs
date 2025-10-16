#![allow(clippy::type_complexity)]
use slint::SharedString;
use std::sync::{Arc, Mutex};

use super::{TreeData, TreeDataError};
use platynui_core::ui::{Namespace, UiNode, UiNodeExt, UiValue};

/// TreeData implementation that wraps a single UiNode
/// Each UiNodeData represents exactly one node in the tree
pub struct UiNodeData {
    node: Arc<dyn UiNode>,
    id_cache: Mutex<Option<SharedString>>,
    label_cache: Mutex<Option<SharedString>>,
    has_children_cache: Mutex<Option<bool>>,
    children_cache: Mutex<Option<Vec<Arc<dyn super::TreeData<Underlying = Arc<dyn UiNode>>>>>>,
    parent_cache: Mutex<Option<Arc<dyn super::TreeData<Underlying = Arc<dyn UiNode>>>>>,
}

impl UiNodeData {
    pub fn new(node: Arc<dyn UiNode>) -> Self {
        Self {
            node,
            id_cache: Mutex::new(None),
            label_cache: Mutex::new(None),
            has_children_cache: Mutex::new(None),
            children_cache: Mutex::new(None),
            parent_cache: Mutex::new(None),
        }
    }
}

impl TreeData for UiNodeData {
    type Underlying = Arc<dyn UiNode>;
    fn id(&self) -> SharedString {
        if let Some(v) = self.id_cache.lock().unwrap().as_ref() {
            return v.clone();
        }
        let v: SharedString = self.node.runtime_id().as_str().into();
        *self.id_cache.lock().unwrap() = Some(v.clone());
        v
    }

    fn label(&self) -> Result<SharedString, TreeDataError> {
        if let Some(v) = self.label_cache.lock().unwrap().as_ref() {
            return Ok(v.clone());
        }
        // Prefer the dynamic Control/Name attribute (fresh each read); fall back to no name
        let name_str: String = self
            .node
            .attribute(Namespace::Control, "Name")
            .map(|attr| match attr.value() {
                UiValue::String(s) => s,
                _ => String::new(),
            })
            .unwrap_or_default();
        let escaped = escape_control_chars(&name_str);
        let label = if escaped.is_empty() {
            self.node.role().to_string()
        } else {
            format!("{} \"{}\"", self.node.role(), escaped)
        };
        let s: SharedString = label.into();
        *self.label_cache.lock().unwrap() = Some(s.clone());
        Ok(s)
    }

    fn has_children(&self) -> Result<bool, TreeDataError> {
        // Prefer cached children list if available
        if let Some(children) = self.children_cache.lock().unwrap().as_ref() {
            return Ok(!children.is_empty());
        }
        if let Some(hc) = *self.has_children_cache.lock().unwrap() {
            return Ok(hc);
        }
        // Probe a single child and cache the boolean
        let has = self.node.children().next().is_some();
        *self.has_children_cache.lock().unwrap() = Some(has);
        Ok(has)
    }

    fn children(&self) -> Result<Vec<Arc<dyn TreeData<Underlying = Self::Underlying>>>, TreeDataError> {
        if let Some(v) = self.children_cache.lock().unwrap().as_ref() {
            return Ok(v.clone());
        }
        let list: Vec<Arc<dyn TreeData<Underlying = Self::Underlying>>> = self
            .node
            .children()
            .map(|child_node| Arc::new(UiNodeData::new(child_node)) as Arc<dyn TreeData<Underlying = Self::Underlying>>)
            .collect();
        *self.has_children_cache.lock().unwrap() = Some(!list.is_empty());
        *self.children_cache.lock().unwrap() = Some(list.clone());
        Ok(list)
    }

    fn parent(&self) -> Result<Option<Arc<dyn TreeData<Underlying = Self::Underlying>>>, TreeDataError> {
        if let Some(v) = self.parent_cache.lock().unwrap().as_ref() {
            return Ok(Some(v.clone()));
        }
        let p = self.node.parent_arc().map(|parent_node| {
            Arc::new(UiNodeData::new(parent_node)) as Arc<dyn TreeData<Underlying = Self::Underlying>>
        });
        if let Some(ref arc) = p {
            *self.parent_cache.lock().unwrap() = Some(arc.clone());
        }
        Ok(p)
    }

    fn as_underlying_data(&self) -> Option<Self::Underlying> {
        Some(self.node.clone())
    }

    fn refresh_self(&self) {
        // Invalidate underlying provider caches so values (like name) are re-queried.
        self.node.invalidate();
        // Clear only our own caches; children are kept unless refresh_recursive is used.
        *self.id_cache.lock().unwrap() = None;
        *self.label_cache.lock().unwrap() = None;
        *self.has_children_cache.lock().unwrap() = None;
        *self.children_cache.lock().unwrap() = None;
        *self.parent_cache.lock().unwrap() = None;
    }
}

/// Escape control characters in a label string.
/// - Carriage Return (\r) and Line Feed (\n) are converted to a single space ' '.
/// - All other control characters are escaped as hex codes:
///   - ASCII control (<= 0xFF): \xHH
///   - Other Unicode control:   \u{XXXX}
fn escape_control_chars(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut it = input.chars().peekable();
    while let Some(ch) = it.next() {
        match ch {
            '\r' => {
                // Skip CR entirely (do not produce a space)
                while let Some(next) = it.peek() {
                    if *next == '\r' {
                        let _ = it.next();
                    } else {
                        break;
                    }
                }
            }
            '\n' => {
                // Collapse runs of LF (and any adjacent CR) into a single space
                while let Some(next) = it.peek() {
                    if *next == '\n' || *next == '\r' {
                        let _ = it.next();
                    } else {
                        break;
                    }
                }
                out.push(' ');
            }
            _ if ch.is_control() || is_format_char(ch) => {
                let code = ch as u32;
                if code <= 0xFF {
                    use std::fmt::Write as _;
                    let _ = write!(&mut out, "\\x{:02X}", code);
                } else {
                    use std::fmt::Write as _;
                    let _ = write!(&mut out, "\\u{{{:X}}}", code);
                }
            }
            _ => out.push(ch),
        }
    }
    out
}

// Minimal detection of Unicode Format (Cf) characters without external crates
fn is_format_char(ch: char) -> bool {
    let c = ch as u32;
    matches!(
        c,
        0x00AD
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x08E2
            | 0x180E
            | 0xFEFF
            | 0x110BD
            | 0x110CD
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
            | 0x0600..=0x0605
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFFF9..=0xFFFB
            | 0x13430..=0x13438
    )
}

#[cfg(test)]
mod tests {
    use super::escape_control_chars;

    #[test]
    fn cr_lf_to_space() {
        assert_eq!(escape_control_chars("a\r\nb"), "a b");
        assert_eq!(escape_control_chars("a\nb"), "a b");
        assert_eq!(escape_control_chars("a\rb"), "ab");
    }

    #[test]
    fn other_control_to_hex() {
        assert_eq!(escape_control_chars("a\t b"), "a\\x09 b");
        assert_eq!(escape_control_chars("\u{0000}"), "\\x00");
        assert_eq!(escape_control_chars("\u{007F}"), "\\x7F");
    }

    #[test]
    fn unicode_control_cf() {
        // ZERO WIDTH NO-BREAK SPACE (U+FEFF) is Cf
        assert_eq!(escape_control_chars("a\u{FEFF}b"), "a\\u{FEFF}b");
    }

    #[test]
    fn bidi_and_line_direction_marks_are_escaped() {
        // LRM (U+200E), RLM (U+200F), RLO (U+202E), LRO (U+202D), LRE (U+202A), RLE (U+202B), PDF (U+202C)
        assert_eq!(escape_control_chars("a\u{200E}b"), "a\\u{200E}b");
        assert_eq!(escape_control_chars("a\u{200F}b"), "a\\u{200F}b");
        assert_eq!(escape_control_chars("a\u{202E}b"), "a\\u{202E}b");
        assert_eq!(escape_control_chars("a\u{202D}b"), "a\\u{202D}b");
        assert_eq!(escape_control_chars("a\u{202A}b"), "a\\u{202A}b");
        assert_eq!(escape_control_chars("a\u{202B}b"), "a\\u{202B}b");
        assert_eq!(escape_control_chars("a\u{202C}b"), "a\\u{202C}b");
    }
}
