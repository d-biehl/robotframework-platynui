//! Node testing and name matching for XPath axis evaluation.

use crate::compiler::ir::{NameOrWildcard, NodeTestIR};
use crate::model::XdmNode;
use string_cache::DefaultAtom;

use super::Vm;

impl<N: 'static + XdmNode + Clone> Vm<N> {
    #[inline]
    pub(crate) fn matches_interned_name(&self, node: &N, expected: &crate::compiler::ir::InternedQName) -> bool {
        let node_name = match node.name() {
            Some(n) => n,
            None => return false,
        };

        if expected.local.as_ref() != node_name.local.as_str() {
            return false;
        }

        let node_ns = node_name.ns_uri.as_ref().map(|ns| DefaultAtom::from(ns.as_str()));

        let effective_ns = match node_ns {
            Some(atom) => Some(atom),
            None => match node_name.prefix.as_deref() {
                Some(prefix) => self.resolve_prefix_namespace(node, prefix),
                None if matches!(node.kind(), crate::model::NodeKind::Attribute) => None,
                None => self.resolve_prefix_namespace(node, ""),
            },
        };

        match (&effective_ns, &expected.ns_uri) {
            (None, None) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    pub(crate) fn node_test(&self, node: &N, test: &NodeTestIR) -> bool {
        use NodeTestIR::*;
        match test {
            AnyKind => true,
            Name(q) => {
                // For namespace nodes, the NameTest matches by prefix (local) only.
                if matches!(node.kind(), crate::model::NodeKind::Namespace) {
                    return node.name().map(|n| n.local == q.original.local).unwrap_or(false);
                }
                // Use fast path for name comparison
                self.matches_interned_name(node, q)
            }
            WildcardAny => true,
            NsWildcard(ns) => node
                .name()
                .map(|n| {
                    let eff = if let Some(uri) = n.ns_uri.as_ref() {
                        Some(DefaultAtom::from(uri.as_str()))
                    } else if let Some(pref) = &n.prefix {
                        self.resolve_prefix_namespace(node, pref)
                    } else if matches!(node.kind(), crate::model::NodeKind::Element | crate::model::NodeKind::Namespace)
                    {
                        self.resolve_prefix_namespace(node, "")
                    } else {
                        None
                    };
                    eff.is_some_and(|atom| atom == *ns)
                })
                .unwrap_or(false),
            LocalWildcard(local) => {
                // Use interned comparison for local names
                node.name().map(|n| n.local.as_str() == local.as_ref()).unwrap_or(false)
            }
            KindText => matches!(node.kind(), crate::model::NodeKind::Text),
            KindComment => matches!(node.kind(), crate::model::NodeKind::Comment),
            KindProcessingInstruction(target_opt) => {
                if !matches!(node.kind(), crate::model::NodeKind::ProcessingInstruction) {
                    return false;
                }
                if let Some(target) = target_opt {
                    if let Some(nm) = node.name() { nm.local == *target } else { false }
                } else {
                    true
                }
            }
            KindDocument(inner_opt) => {
                if !matches!(node.kind(), crate::model::NodeKind::Document) {
                    return false;
                }
                if let Some(inner) = inner_opt {
                    for c in node.children() {
                        if self.node_test(&c, inner) {
                            return true;
                        }
                    }
                    false
                } else {
                    true
                }
            }
            KindElement { name, .. } => {
                if !matches!(node.kind(), crate::model::NodeKind::Element) {
                    return false;
                }
                match name {
                    None => true,
                    Some(NameOrWildcard::Any) => true,
                    Some(NameOrWildcard::Name(exp)) => self.matches_interned_name(node, exp),
                }
            }
            KindAttribute { name, .. } => {
                if !matches!(node.kind(), crate::model::NodeKind::Attribute) {
                    return false;
                }
                match name {
                    None => true,
                    Some(NameOrWildcard::Any) => true,
                    Some(NameOrWildcard::Name(exp)) => self.matches_interned_name(node, exp),
                }
            }
            KindSchemaElement(_) | KindSchemaAttribute(_) => true, // simplified
        }
    }

    /// Resolve a namespace prefix to its in-scope namespace URI for the given node by walking
    /// up the ancestor chain and inspecting declared namespace nodes. Honors the implicit `xml`
    /// binding. Returns `None` when no binding is found.
    pub(crate) fn resolve_prefix_namespace(&self, node: &N, prefix: &str) -> Option<DefaultAtom> {
        if prefix == "xml" {
            return Some(DefaultAtom::from(crate::consts::XML_URI));
        }
        use crate::model::NodeKind;
        let mut cur = Some(node.clone());
        while let Some(n) = cur {
            if matches!(n.kind(), NodeKind::Element) {
                for ns in n.namespaces() {
                    if let Some(q) = ns.name() {
                        let p = q.prefix.unwrap_or_default();
                        if p == prefix {
                            return Some(DefaultAtom::from(ns.string_value().as_str()));
                        }
                    }
                }
            }
            cur = n.parent();
        }
        None
    }
}
