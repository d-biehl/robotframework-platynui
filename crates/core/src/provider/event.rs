use crate::ui::{RuntimeId, UiNode};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

/// Event emitted by a provider to keep the runtime tree in sync.
#[derive(Clone)]
pub struct ProviderEvent {
    pub kind: ProviderEventKind,
}

/// Listener interface for components interested in provider events.
pub trait ProviderEventListener: Send + Sync {
    fn on_event(&self, event: ProviderEvent);
}

#[derive(Clone)]
pub enum ProviderEventKind {
    NodeAdded { parent: Option<RuntimeId>, node: Arc<dyn UiNode> },
    NodeUpdated { node: Arc<dyn UiNode> },
    NodeRemoved { runtime_id: RuntimeId },
    TreeInvalidated,
}

impl Debug for ProviderEventKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderEventKind::NodeAdded { parent, node } => {
                f.debug_struct("NodeAdded").field("parent", parent).field("runtime_id", &node.runtime_id()).finish()
            }
            ProviderEventKind::NodeUpdated { node } => {
                f.debug_struct("NodeUpdated").field("runtime_id", &node.runtime_id()).finish()
            }
            ProviderEventKind::NodeRemoved { runtime_id } => {
                f.debug_struct("NodeRemoved").field("runtime_id", runtime_id).finish()
            }
            ProviderEventKind::TreeInvalidated => f.write_str("TreeInvalidated"),
        }
    }
}

impl Debug for ProviderEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderEvent").field("kind", &self.kind).finish()
    }
}
