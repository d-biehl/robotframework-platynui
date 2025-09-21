use super::identifiers::{PatternId, RuntimeId};
use super::namespace::Namespace;
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
    /// Invalidates cached state. Provider können den nächsten Zugriff nutzen,
    /// um Werte neu zu laden.
    fn invalidate(&self);
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
        patterns: Vec<PatternId>,
        children: Mutex<Vec<Arc<dyn UiNode>>>,
    }

    impl TestNode {
        fn new() -> Arc<Self> {
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
                patterns: vec![PatternId::from("Activatable")],
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
            &self.patterns
        }

        fn invalidate(&self) {}
    }

    #[test]
    fn attribute_lookup_uses_namespace_and_name() {
        let node = TestNode::new();
        let attr = node.attribute(Namespace::Control, "Bounds");
        assert!(attr.is_some());
        assert!(node.attribute(Namespace::Control, "Missing").is_none());
    }
}
