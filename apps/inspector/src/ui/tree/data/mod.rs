use slint::SharedString;
use std::sync::Arc;

pub mod uinode;

#[derive(Debug)]
pub enum TreeDataError {
    Other,
}

/// Abstraction for tree data that can back our UI.
/// Provides cached identifiers/labels and navigation for a single node.
pub trait TreeData: Send + Sync {
    type Underlying;
    fn id(&self) -> SharedString;
    fn label(&self) -> Result<SharedString, TreeDataError>;
    fn has_children(&self) -> Result<bool, TreeDataError>;
    fn children(&self) -> Result<Vec<Arc<dyn TreeData<Underlying = Self::Underlying>>>, TreeDataError>;
    fn parent(&self) -> Result<Option<Arc<dyn TreeData<Underlying = Self::Underlying>>>, TreeDataError>;
    fn as_underlying_data(&self) -> Option<Self::Underlying>;

    /// Optional: refresh any internal caches for this node only.
    /// Default is no-op; providers with caches can override.
    fn refresh_self(&self) {}

    /// Optional: clear caches for this node and children to force re-query.
    fn refresh_recursive(&self) {
        self.refresh_self();
        if let Ok(children) = self.children() {
            for ch in children {
                ch.refresh_recursive();
            }
        }
    }
}
