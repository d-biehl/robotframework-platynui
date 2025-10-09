use crate::events;
use crate::provider::MockProvider;
use platynui_core::provider::{
    ProviderDescriptor, ProviderError, ProviderEventCapabilities, ProviderKind, UiTreeProvider, UiTreeProviderFactory,
};
use platynui_core::ui::identifiers::TechnologyId;
use std::sync::{Arc, LazyLock};

pub const PROVIDER_ID: &str = "mock";
pub const PROVIDER_NAME: &str = "PlatynUI Mock Provider";
pub const TECHNOLOGY: &str = "Mock";

#[cfg(test)]
pub const APP_RUNTIME_ID: &str = "mock://app/main";
#[cfg(test)]
pub const WINDOW_RUNTIME_ID: &str = "mock://window/main";
#[cfg(test)]
pub const BUTTON_RUNTIME_ID: &str = "mock://button/ok";

pub static MOCK_PROVIDER_FACTORY: MockProviderFactory = MockProviderFactory;

pub struct MockProviderFactory;

impl MockProviderFactory {
    pub fn descriptor_static() -> &'static ProviderDescriptor {
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
            ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from(TECHNOLOGY), ProviderKind::Native)
                .with_event_capabilities(ProviderEventCapabilities::STRUCTURE_WITH_PROPERTIES)
        });
        &DESCRIPTOR
    }
}

impl UiTreeProviderFactory for MockProviderFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        Self::descriptor_static()
    }

    fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        let provider: Arc<MockProvider> = Arc::new(MockProvider::new(Self::descriptor_static()));
        events::register_active_instance(&provider);
        Ok(provider)
    }
}
