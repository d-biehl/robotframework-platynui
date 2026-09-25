//! Node testing and name matching for `XPath` axis evaluation.

use crate::compiler::ir::{NameOrWildcard, NodeTestIR};
use crate::model::XdmNode;
use string_cache::DefaultAtom;

use super::Vm;

impl<N: 'static + XdmNode + Clone> Vm<N> {
    #[inline]
    pub(crate) fn matches_interned_name(node: &N, expected: &crate::compiler::ir::InternedQName) -> bool {
        let Some(node_name) = node.name() else {
            return false;
        };

        if expected.local.as_str() != node_name.local.as_str() {
            return false;
        }

        let node_ns = node_name.ns_uri.as_ref().map(|ns| DefaultAtom::from(ns.as_str()));

        let effective_ns = match node_ns {
            Some(atom) => Some(atom),
            None => match node_name.prefix.as_deref() {
                Some(prefix) => Self::resolve_prefix_namespace(node, prefix),
                None if matches!(node.kind(), crate::model::NodeKind::Attribute) => None,
                None => Self::resolve_prefix_namespace(node, ""),
            },
        };

        match (&effective_ns, &expected.ns_uri) {
            (None, None) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    pub(crate) fn node_test(node: &N, test: &NodeTestIR) -> bool {
        use NodeTestIR::{
            AnyKind, KindAttribute, KindComment, KindDocument, KindElement, KindProcessingInstruction,
            KindSchemaAttribute, KindSchemaElement, KindText, LocalWildcard, Name, NsWildcard, WildcardAny,
        };
        match test {
            // schema-element()/schema-attribute() are simplified to match everything.
            AnyKind | WildcardAny | KindSchemaElement(_) | KindSchemaAttribute(_) => true,
            Name(q) => {
                // For namespace nodes, the NameTest matches by prefix (local) only.
                if matches!(node.kind(), crate::model::NodeKind::Namespace) {
                    return node.name().is_some_and(|n| n.local == q.original.local);
                }
                // Use fast path for name comparison
                Self::matches_interned_name(node, q)
            }
            NsWildcard(ns) => node.name().is_some_and(|n| {
                let eff = if let Some(uri) = n.ns_uri.as_ref() {
                    Some(DefaultAtom::from(uri.as_str()))
                } else if let Some(pref) = &n.prefix {
                    Self::resolve_prefix_namespace(node, pref)
                } else if matches!(node.kind(), crate::model::NodeKind::Element | crate::model::NodeKind::Namespace) {
                    Self::resolve_prefix_namespace(node, "")
                } else {
                    None
                };
                eff.is_some_and(|atom| atom == *ns)
            }),
            LocalWildcard(local) => {
                // Use interned comparison for local names
                node.name().is_some_and(|n| n.local.as_str() == local.as_str())
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
                        if Self::node_test(&c, inner) {
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
                    None | Some(NameOrWildcard::Any) => true,
                    Some(NameOrWildcard::Name(exp)) => Self::matches_interned_name(node, exp),
                }
            }
            KindAttribute { name, .. } => {
                if !matches!(node.kind(), crate::model::NodeKind::Attribute) {
                    return false;
                }
                match name {
                    None | Some(NameOrWildcard::Any) => true,
                    Some(NameOrWildcard::Name(exp)) => Self::matches_interned_name(node, exp),
                }
            }
        }
    }

    /// Resolve a namespace prefix to its in-scope namespace URI for the given node by walking
    /// up the ancestor chain and inspecting declared namespace nodes. Honors the implicit `xml`
    /// binding. Returns `None` when no binding is found.
    pub(crate) fn resolve_prefix_namespace(node: &N, prefix: &str) -> Option<DefaultAtom> {
        use crate::model::NodeKind;
        if prefix == "xml" {
            return Some(DefaultAtom::from(crate::consts::XML_URI));
        }
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
