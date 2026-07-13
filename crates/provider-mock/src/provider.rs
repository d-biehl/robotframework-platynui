use crate::focus;
use crate::node::MockNode;
use crate::tree;
use crate::window;
#[cfg(test)]
use platynui_core::provider::UiTreeProviderFactory;
use platynui_core::provider::{
    ProviderDescriptor, ProviderError, ProviderEvent, ProviderEventKind, ProviderEventListener, UiTreeProvider,
};
use platynui_core::ui::Namespace;
use platynui_core::ui::UiNode;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub(crate) struct MockProvider {
    descriptor: &'static ProviderDescriptor,
    roots: Vec<Arc<MockNode>>,
    flat_nodes: Vec<Arc<MockNode>>,
    nodes: HashMap<String, Arc<MockNode>>,
    listeners: RwLock<Vec<Arc<dyn ProviderEventListener>>>,
}

impl MockProvider {
    pub(crate) fn new(descriptor: &'static ProviderDescriptor) -> Self {
        focus::reset();
        window::reset();
        let (roots, flat_nodes, nodes) = tree::instantiate_nodes(descriptor);
        Self { descriptor, roots, flat_nodes, nodes, listeners: RwLock::new(Vec::new()) }
    }

    fn children_for_parent(&self, parent: &Arc<dyn UiNode>) -> Vec<Arc<MockNode>> {
        if let Some(node) = self.nodes.get(parent.runtime_id().as_str()) {
            node.children_snapshot()
        } else if parent.namespace() == Namespace::Control && parent.role() == "Desktop" {
            let mut nodes = self.roots.clone();
            for child in &self.flat_nodes {
                child.set_parent(parent);
                nodes.push(Arc::clone(child));
            }
            nodes
        } else {
            Vec::new()
        }
    }

    pub(crate) fn clone_node(&self, runtime_id: &str) -> Option<Arc<dyn UiNode>> {
        self.nodes.get(runtime_id).map(|node| {
            let cloned = Arc::clone(node);
            let trait_obj: Arc<dyn UiNode> = cloned;
            trait_obj
        })
    }

    /// Deepest, topmost mock node containing `point`, or `None`.
    ///
    /// Siblings are stacked by declaration order — a later child renders on top
    /// of an earlier one — so children are probed last-first and the first hit
    /// wins. A node without bounds is treated as a transparent container: its
    /// children are still probed, but it is never itself returned. A node that
    /// is not pickable (hidden — `IsVisible`/`IsInView` explicitly false) is
    /// likewise never returned, though its children are still probed.
    fn hit_test_node(node: &Arc<MockNode>, point: platynui_core::types::Point) -> Option<Arc<MockNode>> {
        let bounds = node.bounds();
        if let Some(bounds) = bounds
            && !bounds.contains(point)
        {
            return None;
        }

        for child in node.children_snapshot().iter().rev() {
            if let Some(hit) = Self::hit_test_node(child, point) {
                return Some(hit);
            }
        }

        if bounds.is_some() && node.is_pickable() { Some(Arc::clone(node)) } else { None }
    }

    pub(crate) fn notify_listeners(&self, event: ProviderEventKind) {
        let snapshot = {
            let listeners = self.listeners.read().unwrap();
            listeners.clone()
        };
        let event = ProviderEvent { kind: event };
        for listener in snapshot {
            listener.on_event(event.clone());
        }
    }
}

impl UiTreeProvider for MockProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
    }

    fn get_nodes(
        &self,
        parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
        let children = self.children_for_parent(&parent);

        for child in &children {
            child.set_parent(&parent);
        }

        Ok(Box::new(children.into_iter().map(|child| -> Arc<dyn UiNode> { child })))
    }

    fn element_at_point(&self, point: platynui_core::types::Point) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
        // Top-level nodes stacked by declaration order (later = on top); probe
        // last-first so the frontmost overlapping node wins.
        let top_level = self.roots.iter().chain(self.flat_nodes.iter());
        for node in top_level.collect::<Vec<_>>().into_iter().rev() {
            if let Some(hit) = Self::hit_test_node(node, point) {
                return Ok(Some(hit as Arc<dyn UiNode>));
            }
        }
        Ok(None)
    }

    fn subscribe_events(&self, listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
        listener.on_event(ProviderEvent { kind: ProviderEventKind::TreeInvalidated });
        self.listeners.write().unwrap().push(listener);
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn instantiate_test_provider() -> Arc<dyn UiTreeProvider> {
    crate::tree::reset_mock_tree();
    // Mock provider is no longer auto-registered; use factory directly for tests
    crate::factory::MOCK_PROVIDER_FACTORY
        .create(&platynui_core::config::RuntimeConfig::default())
        .expect("mock provider instantiation")
}
