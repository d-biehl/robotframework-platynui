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
    crate::factory::MOCK_PROVIDER_FACTORY.create().expect("mock provider instantiation")
}
