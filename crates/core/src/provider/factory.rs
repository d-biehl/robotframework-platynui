use super::{ProviderDescriptor, ProviderError, UiTreeProvider};
use crate::config::RuntimeConfig;
use std::sync::Arc;

/// Factory trait used by inventory registrations.
pub trait UiTreeProviderFactory: Send + Sync {
    fn descriptor(&self) -> &ProviderDescriptor;

    /// Build a provider instance for one runtime, bound to the session named by
    /// `config`. The factory reads its own settings sub-map via
    /// `config.provider(self.descriptor().id)`, falling back to the environment
    /// for any value it does not find.
    fn create(&self, config: &RuntimeConfig) -> Result<Arc<dyn UiTreeProvider>, ProviderError>;
}
