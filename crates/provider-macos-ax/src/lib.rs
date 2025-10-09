//! macOS Accessibility (AX) UiTree provider (stub).
//!
//! This crate exposes a minimal provider factory so tests and consumers can
//! construct a `Runtime` with a macOS AX provider via
//! `Runtime::new_with_factories(&[&MACOS_AX_FACTORY])`. The actual
//! AXUIElement-backed implementation will be added incrementally.

use once_cell::sync::Lazy;
use platynui_core::provider::{ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory};
use platynui_core::ui::{TechnologyId, UiNode};
use std::sync::Arc;

pub const PROVIDER_ID: &str = "macos-ax";
pub const PROVIDER_NAME: &str = "macOS Accessibility";
pub static TECHNOLOGY: Lazy<TechnologyId> = Lazy::new(|| TechnologyId::from("AX"));

pub struct MacOsAxFactory;

impl UiTreeProviderFactory for MacOsAxFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        static DESCRIPTOR: Lazy<ProviderDescriptor> = Lazy::new(|| {
            ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from("AX"), ProviderKind::Native)
        });
        &DESCRIPTOR
    }

    fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        Ok(Arc::new(MacOsAxProvider::new()))
    }
}

struct MacOsAxProvider {
    descriptor: &'static ProviderDescriptor,
}

impl MacOsAxProvider {
    fn new() -> Self {
        static DESCRIPTOR: Lazy<ProviderDescriptor> = Lazy::new(|| {
            ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from("AX"), ProviderKind::Native)
        });
        Self { descriptor: &DESCRIPTOR }
    }
}

impl UiTreeProvider for MacOsAxProvider {
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

pub static MACOS_AX_FACTORY: MacOsAxFactory = MacOsAxFactory;

// Auto-register the macOS AX provider when linked
platynui_core::register_provider!(&MACOS_AX_FACTORY);
