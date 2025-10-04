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
use std::collections::HashSet;

pub const PROVIDER_ID: &str = "windows-uia";
pub const PROVIDER_NAME: &str = "Windows UIAutomation";
pub static TECHNOLOGY: Lazy<TechnologyId> = Lazy::new(|| TechnologyId::from("UIAutomation"));

// Iterator similar to ElementChildrenIter: streams root's immediate children first (skipping
// elements from our own process), then one synthetic app:Application node per seen PID.
struct ElementAndAppIter {
    parent_elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
    current: Option<windows::Win32::UI::Accessibility::IUIAutomationElement>,
    first: bool,
    parent: Arc<dyn UiNode>,
    seen: HashSet<i32>,
    self_pid: i32,
    apps_phase: bool,
    app_order: Vec<i32>,
    app_index: usize,
}

impl ElementAndAppIter {
    fn new(parent_elem: windows::Win32::UI::Accessibility::IUIAutomationElement, parent: Arc<dyn UiNode>, self_pid: i32) -> Self {
        Self {
            parent_elem,
            current: None,
            first: true,
            parent,
            seen: HashSet::new(),
            self_pid,
            apps_phase: false,
            app_order: Vec::new(),
            app_index: 0,
        }
    }
}

impl Iterator for ElementAndAppIter {
    type Item = Arc<dyn UiNode>;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            // Phase 2: emit apps in stable PID order
            if self.apps_phase {
                while self.app_index < self.app_order.len() {
                    let pid = self.app_order[self.app_index];
                    self.app_index += 1;
                    if pid > 0 && pid != self.self_pid {
                        let app = crate::node::ApplicationNode::new(pid, self.parent_elem.clone(), &self.parent);
                        return Some(app as Arc<dyn UiNode>);
                    }
                }
                return None;
            }

            let walker = match crate::com::raw_walker() { Ok(w) => w, Err(_) => return None };
            loop {
                if self.first {
                    self.first = false;
                    self.current = walker.GetFirstChildElement(&self.parent_elem).ok();
                    if self.current.is_none() {
                        // Switch to apps phase
                        self.apps_phase = true;
                        let mut ordered: Vec<i32> = self.seen.iter().copied().collect();
                        ordered.sort_unstable();
                        self.app_order = ordered;
                        self.app_index = 0;
                        return self.next();
                    }
                } else if let Some(ref e) = self.current {
                    let cur = e.clone();
                    self.current = walker.GetNextSiblingElement(&cur).ok();
                    if self.current.is_none() {
                        // finished elements, switch to apps phase
                        self.apps_phase = true;
                        let mut ordered: Vec<i32> = self.seen.iter().copied().collect();
                        ordered.sort_unstable();
                        self.app_order = ordered;
                        self.app_index = 0;
                        return self.next();
                    }
                }

                let elem = self.current.as_ref()?.clone();
                let pid = crate::map::get_process_id(&elem).unwrap_or(-1);
                if pid > 0 && pid != self.self_pid {
                    self.seen.insert(pid);
                    let node = crate::node::UiaNode::from_elem(elem);
                    node.set_parent(&self.parent);
                    crate::node::UiaNode::init_self(&node);
                    return Some(node as Arc<dyn UiNode>);
                } else {
                    continue;
                }
            }
        }
    }
}

unsafe impl Send for ElementAndAppIter {}

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
        // Stream: first raw desktop children (excluding own process), then one app:Application per PID.
        let self_pid: i32 = std::process::id() as i32;
        let it = ElementAndAppIter::new(root, parent, self_pid);
        Ok(Box::new(it))
    }
}

// Register the factory with the global inventory when this crate is linked.
pub static WINDOWS_UIA_FACTORY: WindowsUiaFactory = WindowsUiaFactory;
register_provider!(&WINDOWS_UIA_FACTORY);

// (no second specialized impl; Windows path handled above with cfg guards inside)
