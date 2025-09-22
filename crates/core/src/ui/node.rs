use super::identifiers::{PatternId, RuntimeId};
use super::namespace::Namespace;
use super::pattern::{UiPattern, downcast_pattern_arc};
use super::value::UiValue;
use std::sync::{Arc, Weak};

/// Trait representing a UI node surfaced by a provider.
pub trait UiNode: Send + Sync {
    /// Namespace of the node (control/item/app/native).
    fn namespace(&self) -> Namespace;
    /// Normalised PascalCase role (used as local-name in XPath).
    fn role(&self) -> &str;
    /// Human readable name.
    fn name(&self) -> &str;
    /// Platform specific runtime identifier.
    fn runtime_id(&self) -> &RuntimeId;
    /// Weak reference to the parent node, if available.
    fn parent(&self) -> Option<Weak<dyn UiNode>>;
    /// Child nodes. Providers können Iteratoren über vorbereitetes oder lazily erzeugtes Material liefern.
    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_>;
    /// Alle Attribute dieses Knotens; Iterator darf Werte lazy erzeugen.
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_>;
    /// Returns a matching attribute for the given namespace/name pair.
    fn attribute(&self, namespace: Namespace, name: &str) -> Option<Arc<dyn UiAttribute>> {
        self.attributes().find(|attr| attr.namespace() == namespace && attr.name() == name)
    }
    /// Capability patterns implemented by the node.
    fn supported_patterns(&self) -> &[PatternId];
    /// Retrieves a pattern instance by identifier. Default implementation
    /// returns `None`; providers override this to surface concrete pattern
    /// objects.
    fn pattern_by_id(&self, _pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
        None
    }
    /// Optional hint for Dokumentordnungs-Vergleiche. Wenn vorhanden, muss der Wert
    /// für jeden Knoten eindeutig sein.
    fn doc_order_key(&self) -> Option<u64> {
        None
    }
    /// Invalidates cached state. Provider können den nächsten Zugriff nutzen,
    /// um Werte neu zu laden.
    fn invalidate(&self);
}

impl dyn UiNode {
    /// Typed convenience accessor for pattern instances. Returns `Some(Arc<T>)`
    /// if the pattern is available on this node.
    pub fn pattern<T>(&self) -> Option<Arc<T>>
    where
        T: UiPattern + 'static,
    {
        let id = T::static_id();
        let pattern = self.pattern_by_id(&id)?;
        downcast_pattern_arc::<T>(pattern)
    }
}

/// Trait describing a lazily computed attribute of a UI node.
pub trait UiAttribute: Send + Sync {
    /// Namespace of the attribute (control/item/app/native/... ).
    fn namespace(&self) -> Namespace;
    /// PascalCase attribute name (without namespace prefix).
    fn name(&self) -> &str;
    /// Current value. Implementationen können hier neue `UiValue`-Instanzen
    /// erzeugen oder gecachte Werte zurückgeben.
    fn value(&self) -> UiValue;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Rect;
    use crate::ui::PatternRegistry;
    use rstest::rstest;
    use std::sync::{Arc, Mutex, Weak};

    struct TestAttribute {
        namespace: Namespace,
        name: &'static str,
        value: UiValue,
    }

    impl UiAttribute for TestAttribute {
        fn namespace(&self) -> Namespace {
            self.namespace
        }

        fn name(&self) -> &str {
            self.name
        }

        fn value(&self) -> UiValue {
            self.value.clone()
        }
    }

    struct TestNode {
        namespace: Namespace,
        role: &'static str,
        name: &'static str,
        runtime_id: RuntimeId,
        attributes: Vec<Arc<dyn UiAttribute>>,
        patterns: PatternRegistry,
        children: Mutex<Vec<Arc<dyn UiNode>>>,
    }

    impl TestNode {
        fn new_with_pattern(pattern: Arc<dyn UiPattern>) -> Arc<Self> {
            Arc::new(Self {
                namespace: Namespace::Control,
                role: "Button",
                name: "OK",
                runtime_id: RuntimeId::from("node-1"),
                attributes: vec![Arc::new(TestAttribute {
                    namespace: Namespace::Control,
                    name: "Bounds",
                    value: UiValue::Rect(Rect::new(0.0, 0.0, 10.0, 5.0)),
                }) as Arc<dyn UiAttribute>],
                patterns: {
                    let mut registry = PatternRegistry::new();
                    registry.register_dyn(pattern);
                    registry
                },
                children: Mutex::new(Vec::new()),
            })
        }

        fn new_without_pattern() -> Arc<Self> {
            Arc::new(Self {
                namespace: Namespace::Control,
                role: "Button",
                name: "OK",
                runtime_id: RuntimeId::from("node-1"),
                attributes: vec![Arc::new(TestAttribute {
                    namespace: Namespace::Control,
                    name: "Bounds",
                    value: UiValue::Rect(Rect::new(0.0, 0.0, 10.0, 5.0)),
                }) as Arc<dyn UiAttribute>],
                patterns: PatternRegistry::new(),
                children: Mutex::new(Vec::new()),
            })
        }
    }

    impl UiNode for TestNode {
        fn namespace(&self) -> Namespace {
            self.namespace
        }

        fn role(&self) -> &str {
            self.role
        }

        fn name(&self) -> &str {
            self.name
        }

        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }

        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            None
        }

        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_> {
            let snapshot = self.children.lock().unwrap().clone();
            Box::new(snapshot.into_iter())
        }

        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_> {
            Box::new(self.attributes.clone().into_iter())
        }

        fn supported_patterns(&self) -> &[PatternId] {
            self.patterns.supported()
        }

        fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
            self.patterns.get(pattern)
        }

        fn invalidate(&self) {}
    }

    struct ActivatablePattern;

    impl UiPattern for ActivatablePattern {
        fn id(&self) -> PatternId {
            Self::static_id()
        }

        fn static_id() -> PatternId
        where
            Self: Sized,
        {
            PatternId::from("Activatable")
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[test]
    fn attribute_lookup_uses_namespace_and_name() {
        let node = TestNode::new_without_pattern();
        let attr = node.attribute(Namespace::Control, "Bounds");
        assert!(attr.is_some());
        assert!(node.attribute(Namespace::Control, "Missing").is_none());
    }

    #[rstest]
    #[case(true)]
    #[case(false)]
    fn pattern_lookup_respects_registry(#[case] register_pattern: bool) {
        let node = if register_pattern {
            TestNode::new_with_pattern(Arc::new(ActivatablePattern) as Arc<dyn UiPattern>)
        } else {
            TestNode::new_without_pattern()
        };

        let ui_node: &dyn UiNode = &*node;
        let pattern = ui_node.pattern::<ActivatablePattern>();

        if register_pattern {
            assert!(pattern.is_some());
            assert_eq!(node.supported_patterns()[0], ActivatablePattern::static_id());
            assert_eq!(node.supported_patterns(), node.patterns.supported());
            for id in node.supported_patterns() {
                assert!(node.pattern_by_id(id).is_some(), "pattern {id:?} missing instance");
            }
        } else {
            assert!(pattern.is_none());
            assert!(node.supported_patterns().is_empty());
            assert_eq!(node.supported_patterns(), node.patterns.supported());
        }
    }
}
