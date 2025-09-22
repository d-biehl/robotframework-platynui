mod descriptor;
mod error;
mod event;
mod factory;
mod registration;
mod tree_provider;

pub use descriptor::{ProviderDescriptor, ProviderEventCapabilities, ProviderKind};
pub use error::{ProviderError, ProviderErrorKind};
pub use event::{ProviderEvent, ProviderEventKind, ProviderEventListener};
pub use factory::UiTreeProviderFactory;
pub use registration::{ProviderRegistration, provider_factories};
pub use tree_provider::UiTreeProvider;

#[macro_export]
macro_rules! register_provider {
    ($factory:expr) => {
        inventory::submit! {
            $crate::provider::ProviderRegistration { factory: $factory }
        }
    };
}

pub use register_provider;
