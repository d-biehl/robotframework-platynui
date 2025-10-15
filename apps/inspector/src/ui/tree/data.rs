use slint::SharedString;
use thiserror::Error;

/// Errors that can occur during TreeData operations
#[derive(Error, Debug, Clone)]
pub enum TreeDataError {
    #[error("Tree operation failed: {0}")]
    OperationFailed(String),
}

/// Read-only data provider for a single tree node. Each TreeData represents one node.
/// Methods return other TreeData instances for navigation, eliminating the need for ID-based searches.
pub trait TreeData {
    fn id(&self) -> SharedString;
    fn label(&self) -> Result<SharedString, TreeDataError>;
    fn has_children(&self) -> Result<bool, TreeDataError>;
    fn children(&self) -> Result<Vec<Box<dyn TreeData>>, TreeDataError>;
    fn parent(&self) -> Result<Option<Box<dyn TreeData>>, TreeDataError>;
}

pub mod uinode;
