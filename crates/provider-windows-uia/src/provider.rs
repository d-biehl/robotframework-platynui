use std::sync::Arc;

// Windows UIAutomation provider: registers the UIA technology and streams root
// children via the RawView walker.

use platynui_core::provider::{ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory};
use platynui_core::register_provider;
use platynui_core::ui::{TechnologyId, UiNode};
use std::collections::{HashSet, VecDeque};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub const PROVIDER_ID: &str = "windows-uia";
pub const PROVIDER_NAME: &str = "Windows UIAutomation";
pub static TECHNOLOGY: LazyLock<TechnologyId> = LazyLock::new(|| TechnologyId::from("UIAutomation"));
// Cache current process id once for the entire module; stable for process lifetime.
static SELF_PID: LazyLock<i32> = LazyLock::new(|| std::process::id() as i32);

// Streams root children first, followed by synthetic app:Application nodes.
struct ElementAndAppIter {
    walker: Option<windows::Win32::UI::Accessibility::IUIAutomationTreeWalker>,
    cache_req: Option<windows::Win32::UI::Accessibility::IUIAutomationCacheRequest>,
    parent_elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
    current: Option<windows::Win32::UI::Accessibility::IUIAutomationElement>,
    first: bool,
    parent: Arc<dyn UiNode>,
    seen: HashSet<i32>,
    pending_apps: VecDeque<i32>,
    raw_phase_complete: bool,
    raw_count: usize,
}

impl ElementAndAppIter {
    fn new(parent_elem: windows::Win32::UI::Accessibility::IUIAutomationElement, parent: Arc<dyn UiNode>) -> Self {
        let walker = match crate::com::raw_walker() {
            Ok(walker) => Some(walker),
            Err(err) => {
                tracing::warn!(%err, "UIA raw_walker failed, skipping desktop children");
                None
            }
        };
        let cache_req = crate::com::traversal_cache_request().ok();
        Self {
            walker,
            cache_req,
            parent_elem,
            current: None,
            first: true,
            parent,
            seen: HashSet::new(),
            pending_apps: VecDeque::new(),
            raw_phase_complete: false,
            raw_count: 0,
        }
    }

    fn stream_next_pending_app(&mut self) -> Option<Arc<dyn UiNode>> {
        while let Some(pid) = self.pending_apps.pop_front() {
            if pid > 0 && pid != *SELF_PID {
                let app = crate::node::ApplicationNode::new(pid, &self.parent);
                return Some(app as Arc<dyn UiNode>);
            }
        }
        None
    }
}

impl Iterator for ElementAndAppIter {
    type Item = Arc<dyn UiNode>;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.raw_phase_complete {
                return self.stream_next_pending_app();
            }

            let Some(walker) = &self.walker else { return None };
            let has_cache = self.cache_req.is_some();
            loop {
                if self.first {
                    self.first = false;
                    self.current = if let Some(ref req) = self.cache_req {
                        walker.GetFirstChildElementBuildCache(&self.parent_elem, req).ok()
                    } else {
                        walker.GetFirstChildElement(&self.parent_elem).ok()
                    };
                    if self.current.is_none() {
                        self.raw_phase_complete = true;
                        return None;
                    }
                } else if let Some(ref e) = self.current {
                    let cur = e.clone();
                    self.current = if let Some(ref req) = self.cache_req {
                        walker.GetNextSiblingElementBuildCache(&cur, req).ok()
                    } else {
                        walker.GetNextSiblingElement(&cur).ok()
                    };
                    if self.current.is_none() {
                        self.raw_phase_complete = true;
                        return self.stream_next_pending_app();
                    }
                }

                let elem = self.current.as_ref()?.clone();
                let pid_start = Instant::now();
                let pid = if has_cache {
                    elem.CachedProcessId().unwrap_or(-1)
                } else {
                    crate::map::get_process_id(&elem).unwrap_or(-1)
                };
                let pid_elapsed = pid_start.elapsed();
                if pid_elapsed > Duration::from_millis(200) {
                    tracing::warn!(
                        pid,
                        has_cache,
                        raw_count = self.raw_count,
                        elapsed_ms = pid_elapsed.as_secs_f64() * 1000.0,
                        "slow Windows UIA root process id lookup"
                    );
                }
                if pid > 0 && pid != *SELF_PID {
                    // Create and return the desktop-scoped window node.
                    let desktop_node =
                        crate::node::UiaNode::from_elem_with_scope(elem.clone(), crate::map::UiaIdScope::Desktop);
                    let cache_start = Instant::now();
                    if has_cache {
                        desktop_node.populate_cached_properties();
                    }
                    let cache_elapsed = cache_start.elapsed();
                    if cache_elapsed > Duration::from_millis(200) {
                        tracing::warn!(
                            pid,
                            raw_count = self.raw_count,
                            elapsed_ms = cache_elapsed.as_secs_f64() * 1000.0,
                            "slow Windows UIA cached property population"
                        );
                    }
                    desktop_node.set_parent(&self.parent);
                    crate::node::UiaNode::init_self(&desktop_node);

                    // Queue the synthetic application node once per process;
                    // queued applications are emitted after all top-level nodes.
                    if self.seen.insert(pid) {
                        self.pending_apps.push_back(pid);
                    }

                    self.raw_count += 1;
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
