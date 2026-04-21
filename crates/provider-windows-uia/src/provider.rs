use std::sync::Arc;

// Windows UIAutomation provider: registers the UIA technology and streams root
// children via the RawView walker (no FindAll/CacheRequest).

use platynui_core::provider::{ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory};
use platynui_core::register_provider;
use platynui_core::ui::{TechnologyId, UiNode};
use std::collections::HashSet;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub const PROVIDER_ID: &str = "windows-uia";
pub const PROVIDER_NAME: &str = "Windows UIAutomation";
pub static TECHNOLOGY: LazyLock<TechnologyId> = LazyLock::new(|| TechnologyId::from("UIAutomation"));
// Cache current process id once for the entire module; stable for process lifetime.
static SELF_PID: LazyLock<i32> = LazyLock::new(|| std::process::id() as i32);

// Streams root children first, then app:Application nodes.
struct ElementAndAppIter {
    parent_elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
    current: Option<windows::Win32::UI::Accessibility::IUIAutomationElement>,
    first: bool,
    parent: Arc<dyn UiNode>,
    seen: HashSet<i32>,
    apps_phase: bool,
    app_order: Vec<i32>,
    app_index: usize,
    windows_by_pid: crate::node::AppWindowBuckets,
}

impl ElementAndAppIter {
    fn new(parent_elem: windows::Win32::UI::Accessibility::IUIAutomationElement, parent: Arc<dyn UiNode>) -> Self {
        Self {
            parent_elem,
            current: None,
            first: true,
            parent,
            seen: HashSet::new(),
            apps_phase: false,
            app_order: Vec::new(),
            app_index: 0,
            windows_by_pid: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
}

impl Iterator for ElementAndAppIter {
    type Item = Arc<dyn UiNode>;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.apps_phase {
                while self.app_index < self.app_order.len() {
                    let pid = self.app_order[self.app_index];
                    self.app_index += 1;
                    if pid > 0 && pid != *SELF_PID {
                        let app =
                            crate::node::ApplicationNode::new(pid, &self.parent, Arc::clone(&self.windows_by_pid));
                        return Some(app as Arc<dyn UiNode>);
                    }
                }
                return None;
            }

            let walker = match crate::com::raw_walker() {
                Ok(w) => w,
                Err(err) => {
                    tracing::warn!(%err, "UIA raw_walker failed, skipping children");
                    return None;
                }
            };
            loop {
                if self.first {
                    self.first = false;
                    self.current = walker.GetFirstChildElement(&self.parent_elem).ok();
                    if self.current.is_none() {
                        self.apps_phase = true;
                        self.app_index = 0;
                        return None;
                    }
                } else if let Some(ref e) = self.current {
                    let cur = e.clone();
                    self.current = walker.GetNextSiblingElement(&cur).ok();
                    if self.current.is_none() {
                        self.apps_phase = true;
                        self.app_index = 0;
                        return self.next();
                    }
                }

                let elem = self.current.as_ref()?.clone();
                let pid = crate::map::get_process_id(&elem).unwrap_or(-1);
                if pid > 0 && pid != *SELF_PID {
                    // Create and return the desktop-scoped window node.
                    let desktop_node =
                        crate::node::UiaNode::from_elem_with_scope(elem.clone(), crate::map::UiaIdScope::Desktop);
                    desktop_node.set_parent(&self.parent);
                    crate::node::UiaNode::init_self(&desktop_node);

                    // Store raw element in pid bucket; app-scoped nodes are
                    // created lazily in ApplicationNode::children().
                    if let Ok(mut grouped) = self.windows_by_pid.lock() {
                        grouped.entry(pid).or_default().push(crate::node::AppWindowElem(elem));
                    }

                    // Record app order by first appearance; app nodes are streamed
                    // only after all top-level nodes have been emitted.
                    if self.seen.insert(pid) {
                        self.app_order.push(pid);
                    }

                    return Some(desktop_node as Arc<dyn UiNode>);
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
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
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

/// Windows UIAutomation provider.
///
/// COM objects live in thread-local storage (see [`crate::com`]).  The
/// `is_shutdown` flag prevents new queries after [`UiTreeProvider::shutdown`]
/// has been called and triggers cleanup of the thread-local singletons on
/// the calling thread.
pub struct WindowsUiaProvider {
    descriptor: &'static ProviderDescriptor,
    is_shutdown: AtomicBool,
}

impl WindowsUiaProvider {
    fn new() -> Self {
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
            ProviderDescriptor::new(
                PROVIDER_ID,
                PROVIDER_NAME,
                TechnologyId::from("UIAutomation"),
                ProviderKind::Native,
            )
        });

        Self { descriptor: &DESCRIPTOR, is_shutdown: AtomicBool::new(false) }
    }
}

impl UiTreeProvider for WindowsUiaProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
    }

    fn shutdown(&self) {
        if self.is_shutdown.swap(true, Ordering::AcqRel) {
            return; // already shut down
        }
        tracing::info!("Windows UIAutomation provider shutting down");
        crate::com::clear_thread_local_singletons();
    }

    fn get_nodes(
        &self,
        parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
        if self.is_shutdown.load(Ordering::Acquire) {
            return Err(ProviderError::CommunicationFailure {
                channel: "windows uia",
                details: Some(crate::error::UiaError::Shutdown.to_string()),
            });
        }
        let uia = crate::com::uia().map_err(|e| ProviderError::CommunicationFailure {
            channel: "windows uia",
            details: Some(e.to_string()),
        })?;

        let root = unsafe {
            uia.GetRootElement().map_err(|e| ProviderError::CommunicationFailure {
                channel: "windows uia",
                details: Some(e.to_string()),
            })?
        };
        // Stream: first raw desktop children (excluding own process), then one app:Application per PID.
        let it = ElementAndAppIter::new(root, parent);
        Ok(Box::new(it))
    }
}

// Register the factory with the global inventory when this crate is linked.
pub static WINDOWS_UIA_FACTORY: WindowsUiaFactory = WindowsUiaFactory;
register_provider!(&WINDOWS_UIA_FACTORY);

// (no second specialized impl; Windows path handled above with cfg guards inside)
