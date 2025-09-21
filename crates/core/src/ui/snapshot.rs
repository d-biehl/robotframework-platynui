use super::node::UiNode;
use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq)]
pub struct UiSnapshot {
    root: UiNode,
    captured_at: SystemTime,
}

impl UiSnapshot {
    pub fn new(root: UiNode) -> Self {
        Self { root, captured_at: SystemTime::now() }
    }

    pub fn with_timestamp(root: UiNode, captured_at: SystemTime) -> Self {
        Self { root, captured_at }
    }

    pub fn root(&self) -> &UiNode {
        &self.root
    }

    pub fn captured_at(&self) -> SystemTime {
        self.captured_at
    }

    pub fn into_root(self) -> UiNode {
        self.root
    }
}
