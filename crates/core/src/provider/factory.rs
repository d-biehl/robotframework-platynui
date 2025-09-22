use super::{ProviderDescriptor, ProviderError, UiTreeProvider};
use std::sync::Arc;

/// Factory trait used by inventory registrations.
pub trait UiTreeProviderFactory: Send + Sync {
    fn descriptor(&self) -> &ProviderDescriptor;
    fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError>;
}
