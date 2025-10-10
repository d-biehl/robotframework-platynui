use super::{ProviderDescriptor, ProviderError, ProviderEventListener};
use crate::ui::UiNode;
use std::sync::Arc;

/// Core interface implemented by every UI tree provider.
pub trait UiTreeProvider: Send + Sync {
    /// Returns static metadata describing this provider.
    fn descriptor(&self) -> &ProviderDescriptor;

    /// Returns an iterator over nodes that should be attached to the given
    /// parent (typically the runtime-managed desktop or an application node).
    fn get_nodes(
        &self,
        parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError>;

    /// Registers a listener for provider-originated events. The default
    /// implementation does nothing so providers without event support can
    /// ignore this call.
    fn subscribe_events(&self, _listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Allows the runtime to signal shutdown so the provider can release resources.
    fn shutdown(&self) {}
}
