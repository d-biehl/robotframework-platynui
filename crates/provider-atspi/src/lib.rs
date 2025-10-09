//! AT-SPI2 UiTree provider for Unix desktops (stub).
//!
//! This crate currently exposes a minimal provider factory so tests and
//! consumers can construct a `Runtime` with an AT-SPI2 provider via
//! `Runtime::new_with_factories(&[&ATSPI_FACTORY])`. The actual D-Bus backed
//! implementation will be added incrementally.

use once_cell::sync::Lazy;
use platynui_core::provider::{ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory};
use platynui_core::ui::{TechnologyId, UiNode};
use std::sync::Arc;

pub const PROVIDER_ID: &str = "atspi";
pub const PROVIDER_NAME: &str = "AT-SPI2";
pub static TECHNOLOGY: Lazy<TechnologyId> = Lazy::new(|| TechnologyId::from("AT-SPI2"));

pub struct AtspiFactory;

impl UiTreeProviderFactory for AtspiFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        static DESCRIPTOR: Lazy<ProviderDescriptor> = Lazy::new(|| {
            ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from("AT-SPI2"), ProviderKind::Native)
        });
        &DESCRIPTOR
    }

    fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        Ok(Arc::new(AtspiProvider::new()))
    }
}

struct AtspiProvider {
    descriptor: &'static ProviderDescriptor,
}

impl AtspiProvider {
    fn new() -> Self {
        static DESCRIPTOR: Lazy<ProviderDescriptor> = Lazy::new(|| {
            ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from("AT-SPI2"), ProviderKind::Native)
        });
        Self { descriptor: &DESCRIPTOR }
    }
}

impl UiTreeProvider for AtspiProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
    }
    fn get_nodes(
        &self,
        _parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
        Ok(Box::new(std::iter::empty()))
    }
}

pub static ATSPI_FACTORY: AtspiFactory = AtspiFactory;

// Auto-register the AT-SPI provider when linked
platynui_core::register_provider!(&ATSPI_FACTORY);
