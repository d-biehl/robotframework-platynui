mod descriptor;
mod error;
mod event;
mod factory;
mod provider;
mod registration;

pub use descriptor::{ProviderDescriptor, ProviderKind};
pub use error::{ProviderError, ProviderErrorKind};
pub use event::{ProviderEvent, ProviderEventKind, ProviderEventListener};
pub use factory::UiTreeProviderFactory;
pub use provider::UiTreeProvider;
pub use registration::{ProviderRegistration, provider_factories};

#[macro_export]
macro_rules! register_provider {
    ($factory:expr) => {
        inventory::submit! {
            $crate::provider::ProviderRegistration { factory: $factory }
        }
    };
}

pub use register_provider;
