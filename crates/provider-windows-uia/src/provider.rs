use std::sync::Arc;

// Windows UIAutomation provider factory and top‑level traversal.
//
// - Registers a native provider with Technology `UIAutomation`.
// - `get_nodes(desktop)` returns the children of the UIA Root's first Desktop
//   element using the shared RawView walker and the common `ElementChildrenIter`.
// - No `FindAll`, no UIA `CacheRequest`.

use once_cell::sync::Lazy;
use platynui_core::provider::ProviderErrorKind;
use platynui_core::provider::{
    ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory,
};
use platynui_core::register_provider;
use platynui_core::ui::{TechnologyId, UiNode};

pub const PROVIDER_ID: &str = "windows-uia";
pub const PROVIDER_NAME: &str = "Windows UIAutomation";
pub static TECHNOLOGY: Lazy<TechnologyId> = Lazy::new(|| TechnologyId::from("UIAutomation"));

/// Factory for the UIAutomation provider.
pub struct WindowsUiaFactory;

impl UiTreeProviderFactory for WindowsUiaFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        static DESCRIPTOR: Lazy<ProviderDescriptor> = Lazy::new(|| {
            ProviderDescriptor::new(
                PROVIDER_ID,
                PROVIDER_NAME,
                TechnologyId::from("UIAutomation"),
                ProviderKind::Native,
            )
        });
        &DESCRIPTOR
    }

    fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        Ok(Arc::new(WindowsUiaProvider::new()))
    }
}

/// Minimal provider skeleton. On Windows this will drive a UIA actor. For now
/// we return an empty iterator so the crate compiles across targets.
pub struct WindowsUiaProvider {
    descriptor: &'static ProviderDescriptor,
}

impl WindowsUiaProvider {
    fn new() -> Self {
        static DESCRIPTOR: Lazy<ProviderDescriptor> = Lazy::new(|| {
            ProviderDescriptor::new(
                PROVIDER_ID,
                PROVIDER_NAME,
                TechnologyId::from("UIAutomation"),
                ProviderKind::Native,
            )
        });
        // Warm up COM + UIA singletons on the current thread to avoid first-use latency
        {
            let _ = crate::com::uia();
            let _ = crate::com::raw_walker();
            // Touch root once; ignore errors to keep construction infallible
            if let Ok(uia) = crate::com::uia() {
                let _ = unsafe { uia.GetRootElement() };
            }
        }
        Self { descriptor: &DESCRIPTOR }
    }
}

impl UiTreeProvider for WindowsUiaProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
    }

    fn get_nodes(
        &self,
        parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
        let uia = crate::com::uia().map_err(|e| {
            ProviderError::new(ProviderErrorKind::CommunicationFailure, e.to_string())
        })?;

        let root = unsafe {
            uia.GetRootElement()
                .map_err(|e| ProviderError::new(ProviderErrorKind::CommunicationFailure, e.to_string()))?
        };

        let it = crate::node::ElementChildrenIter::new(root, parent);
        Ok(Box::new(it))
    }
}

// Register the factory with the global inventory when this crate is linked.
pub static WINDOWS_UIA_FACTORY: WindowsUiaFactory = WindowsUiaFactory;
register_provider!(&WINDOWS_UIA_FACTORY);

// (no second specialized impl; Windows path handled above with cfg guards inside)
