use super::identifiers::{PatternId, RuntimeId};
use super::namespace::Namespace;
use super::pattern::{UiPattern, downcast_pattern_arc};
use super::value::UiValue;
use crate::ui::DESKTOP_RUNTIME_ID;
use std::sync::{Arc, Weak};

/// Trait representing a UI node surfaced by a provider.
pub trait UiNode: Send + Sync {
    /// Namespace of the node (control/item/app/native).
    fn namespace(&self) -> Namespace;
    /// Normalised PascalCase role (used as local-name in XPath).
    fn role(&self) -> &str;
    /// Human readable name (owned string). Providers may compute this on each call
    /// or cache internally. For up-to-date values prefer the Control/Name attribute.
    fn name(&self) -> String;
    /// Platform specific runtime identifier.
    fn runtime_id(&self) -> &RuntimeId;
    /// Weak reference to the parent node, if available.
    fn parent(&self) -> Option<Weak<dyn UiNode>>;
    /// Child nodes. Providers may return iterators over prepared or lazily produced material.
    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static>;
    /// All attributes of this node; the iterator may produce values lazily.
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static>;

    /// Returns a matching attribute for the given namespace/name pair.
    fn attribute(&self, namespace: Namespace, name: &str) -> Option<Arc<dyn UiAttribute>> {
        self.attributes().find(|attr| attr.namespace() == namespace && attr.name() == name)
    }
    /// Capability patterns implemented by the node.
    fn supported_patterns(&self) -> Vec<PatternId>;
    /// Retrieves a pattern instance by identifier. Default implementation
    /// returns `None`; providers override this to surface concrete pattern
    /// objects.
    fn pattern_by_id(&self, _pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
        None
    }
    /// Optional hint for document-order comparisons. If present, the value must
    /// be unique per node.
    fn doc_order_key(&self) -> Option<u64> {
        None
    }
    /// Returns whether the underlying platform node is still valid/available.
    /// Default returns true; providers may override with a cheap liveness check
    /// (e.g., a lightweight property call that fails with a platform-specific
    /// "element not available" error when the node is stale).
    fn is_valid(&self) -> bool {
        true
    }
    /// Invalidates cached state. Providers may reload values on the next access.
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

/// Iterator over ancestor nodes.
pub struct UiNodeAncestorIter {
    next: Option<Arc<dyn UiNode>>,
}

impl Iterator for UiNodeAncestorIter {
    type Item = Arc<dyn UiNode>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next.take()?;
        let parent = current.parent().and_then(|weak| weak.upgrade());
        self.next = parent;
        Some(current)
    }
}

/// Convenience helpers for working with `Arc<dyn UiNode>`.
pub trait UiNodeExt {
    /// Returns the parent as `Arc`, if available.
    fn parent_arc(&self) -> Option<Arc<dyn UiNode>>;
    /// Iterator over all ancestors, starting with the immediate parent.
    fn ancestors(&self) -> UiNodeAncestorIter;
    /// Iterator over the node itself followed by all ancestors.
    fn ancestors_including_self(&self) -> UiNodeAncestorIter;
    /// Top-level ancestor (or `self` if no parent exists).
    fn top_level_or_self(&self) -> Arc<dyn UiNode>;
    /// First ancestor (including self) that exposes the requested pattern.
    fn ancestor_pattern<P>(&self) -> Option<Arc<P>>
    where
        P: UiPattern + 'static;
    /// Pattern of the top-level node, if available.
    fn top_level_pattern<P>(&self) -> Option<Arc<P>>
    where
        P: UiPattern + 'static;
}

impl UiNodeExt for Arc<dyn UiNode> {
    fn parent_arc(&self) -> Option<Arc<dyn UiNode>> {
        self.parent()
            .and_then(|weak| weak.upgrade())
            .and_then(|parent| if is_desktop(&parent) { None } else { Some(parent) })
    }

    fn ancestors(&self) -> UiNodeAncestorIter {
        UiNodeAncestorIter { next: self.parent_arc() }
    }

    fn ancestors_including_self(&self) -> UiNodeAncestorIter {
        UiNodeAncestorIter { next: Some(self.clone()) }
    }

    fn top_level_or_self(&self) -> Arc<dyn UiNode> {
        let mut current = self.clone();
        while let Some(parent) = current.parent_arc() {
            current = parent;
        }
        current
    }

    fn ancestor_pattern<P>(&self) -> Option<Arc<P>>
    where
        P: UiPattern + 'static,
    {
        for node in self.ancestors_including_self() {
            if let Some(pattern) = node.pattern::<P>() {
                return Some(pattern);
            }
        }
        None
    }

    fn top_level_pattern<P>(&self) -> Option<Arc<P>>
    where
        P: UiPattern + 'static,
    {
        self.top_level_or_self().pattern::<P>()
    }
}

fn is_desktop(node: &Arc<dyn UiNode>) -> bool {
    node.parent().is_none() && node.role() == "Desktop" && node.runtime_id().as_str() == DESKTOP_RUNTIME_ID
}

/// Trait describing a lazily computed attribute of a UI node.
pub trait UiAttribute: Send + Sync {
    /// Namespace of the attribute (control/item/app/native/... ).
    fn namespace(&self) -> Namespace;
    /// PascalCase attribute name (without namespace prefix).
    fn name(&self) -> &str;
    /// Current value. Implementations may construct fresh `UiValue`s or return
    /// cached values.
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
                    let registry = PatternRegistry::new();
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

        fn name(&self) -> String {
            self.name.to_string()
        }

        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }

        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            None
        }

        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
            let snapshot = self.children.lock().unwrap().clone();
            Box::new(snapshot.into_iter())
        }

        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
            Box::new(self.attributes.clone().into_iter())
        }

        fn supported_patterns(&self) -> Vec<PatternId> {
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

    #[test]
    fn ancestor_helpers_resolve_top_level_and_patterns() {
        struct StubNode {
            name: &'static str,
            runtime_id: RuntimeId,
            pattern_registry: PatternRegistry,
            parent: Mutex<Option<Weak<dyn UiNode>>>,
        }

        impl StubNode {
            fn new(name: &'static str, runtime_id: &'static str) -> Arc<Self> {
                Arc::new(Self {
                    name,
                    runtime_id: RuntimeId::from(runtime_id),
                    pattern_registry: PatternRegistry::new(),
                    parent: Mutex::new(None),
                })
            }

            fn with_pattern(name: &'static str, runtime_id: &'static str, pattern: Arc<dyn UiPattern>) -> Arc<Self> {
                let registry = {
                    let r = PatternRegistry::new();
                    r.register_dyn(pattern);
                    r
                };
                Arc::new(Self {
                    name,
                    runtime_id: RuntimeId::from(runtime_id),
                    pattern_registry: registry,
                    parent: Mutex::new(None),
                })
            }

            fn set_parent(child: &Arc<Self>, parent: &Arc<dyn UiNode>) {
                *child.parent.lock().unwrap() = Some(Arc::downgrade(parent));
            }
        }

        impl UiNode for StubNode {
            fn namespace(&self) -> Namespace {
                Namespace::Control
            }

            fn role(&self) -> &str {
                self.name
            }

            fn name(&self) -> String {
                self.name.to_string()
            }

            fn runtime_id(&self) -> &RuntimeId {
                &self.runtime_id
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

            fn supported_patterns(&self) -> Vec<PatternId> {
                self.pattern_registry.supported()
            }

            fn pattern_by_id(&self, id: &PatternId) -> Option<Arc<dyn UiPattern>> {
                self.pattern_registry.get(id)
            }

            fn invalidate(&self) {}
        }

        struct WindowPattern;

        impl UiPattern for WindowPattern {
            fn id(&self) -> PatternId {
                Self::static_id()
            }

            fn static_id() -> PatternId
            where
                Self: Sized,
            {
                PatternId::from("WindowSurface")
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let root = StubNode::with_pattern("Window", "root", Arc::new(WindowPattern));
        let panel = StubNode::new("Panel", "panel");
        let button = StubNode::new("Button", "button");

        let root_arc: Arc<dyn UiNode> = root.clone();
        let panel_arc: Arc<dyn UiNode> = panel.clone();
        let button_arc: Arc<dyn UiNode> = button.clone();

        StubNode::set_parent(&panel, &root_arc);
        StubNode::set_parent(&button, &panel_arc);

        // parent_arc
        let parent = button_arc.parent_arc().expect("parent present");
        assert_eq!(parent.runtime_id(), &RuntimeId::from("panel"));

        // ancestors iterator order: parent, then root
        let ancestors: Vec<String> = button_arc.ancestors().map(|node| node.runtime_id().to_string()).collect();
        assert_eq!(ancestors, vec!["panel", "root"]);

        // top level
        let top = button_arc.top_level_or_self();
        assert_eq!(top.runtime_id(), &RuntimeId::from("root"));

        // pattern resolution (ancestor incl. self)
        let pattern = button_arc.ancestor_pattern::<WindowPattern>().expect("window pattern via ancestor");
        assert_eq!(pattern.id(), WindowPattern::static_id());

        // top-level pattern
        assert!(button_arc.top_level_pattern::<WindowPattern>().is_some());
    }

    #[test]
    fn parent_arc_skips_desktop_node() {
        struct DesktopStub {
            runtime_id: RuntimeId,
        }

        impl DesktopStub {
            fn new() -> Arc<Self> {
                Arc::new(Self { runtime_id: RuntimeId::from(DESKTOP_RUNTIME_ID) })
            }
        }

        impl UiNode for DesktopStub {
            fn namespace(&self) -> Namespace {
                Namespace::Control
            }

            fn role(&self) -> &str {
                "Desktop"
            }

            fn name(&self) -> String {
                "Desktop".to_string()
            }

            fn runtime_id(&self) -> &RuntimeId {
                &self.runtime_id
            }

            fn parent(&self) -> Option<Weak<dyn UiNode>> {
                None
            }

            fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
                Box::new(std::iter::empty())
            }

            fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
                Box::new(std::iter::empty())
            }

            fn supported_patterns(&self) -> Vec<PatternId> {
                vec![]
            }

            fn pattern_by_id(&self, _pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
                None
            }

            fn invalidate(&self) {}
        }

        struct ChildNode {
            parent: Mutex<Option<Weak<dyn UiNode>>>,
            runtime_id: RuntimeId,
        }

        impl ChildNode {
            fn new(parent: &Arc<dyn UiNode>) -> Arc<Self> {
                Arc::new(Self {
                    parent: Mutex::new(Some(Arc::downgrade(parent))),
                    runtime_id: RuntimeId::from("child"),
                })
            }
        }

        impl UiNode for ChildNode {
            fn namespace(&self) -> Namespace {
                Namespace::Control
            }

            fn role(&self) -> &str {
                "Button"
            }

            fn name(&self) -> String {
                "Button".to_string()
            }

            fn runtime_id(&self) -> &RuntimeId {
                &self.runtime_id
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

            fn supported_patterns(&self) -> Vec<PatternId> {
                vec![]
            }

            fn pattern_by_id(&self, _pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
                None
            }

            fn invalidate(&self) {}
        }

        let desktop: Arc<dyn UiNode> = DesktopStub::new();
        let child: Arc<dyn UiNode> = ChildNode::new(&desktop);
        assert!(child.parent_arc().is_none());
        assert!(child.ancestors().all(|node| node.runtime_id() != &RuntimeId::from("desktop")));
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
            let supported = node.supported_patterns();
            assert_eq!(supported[0], ActivatablePattern::static_id());
            assert_eq!(supported, node.patterns.supported());
            for id in supported {
                assert!(node.pattern_by_id(&id).is_some(), "pattern {id:?} missing instance");
            }
        } else {
            assert!(pattern.is_none());
            let supported = node.supported_patterns();
            assert!(supported.is_empty());
            assert_eq!(supported, node.patterns.supported());
        }
    }
}
