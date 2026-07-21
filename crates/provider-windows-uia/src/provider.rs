use std::sync::Arc;

// Windows UIAutomation provider: registers the UIA technology and streams root
// children via the RawView walker.

use platynui_core::config::RuntimeConfig;
use platynui_core::platform::JavaClassifier;
use platynui_core::provider::{ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory};
use platynui_core::register_provider;
use platynui_core::types::Point;
use platynui_core::ui::{TechnologyId, UiNode};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

pub const PROVIDER_ID: &str = "windows-uia";
pub const PROVIDER_NAME: &str = "Windows UIAutomation";
pub static TECHNOLOGY: LazyLock<TechnologyId> = LazyLock::new(|| TechnologyId::from("UIAutomation"));
// Cache current process id once for the entire module; stable for process lifetime.
static SELF_PID: LazyLock<i32> = LazyLock::new(|| std::process::id() as i32);

// The provider enumerates the desktop's top-level windows via `EnumWindows`
// rather than the UIA RawView `TreeWalker`. Navigating the raw sibling chain
// forces UIA to materialise every top-level window, and for a window without a
// *native* UIA provider that goes through the MSAA→UIA bridge
// (`AccessibleObjectFromWindow`), which blocks on a fixed ~10 s
// `SendMessageTimeout(WM_GETOBJECT)` when the window does not answer promptly.
// That happens routinely against a just-launched accessibility app
// (e.g. egui/AccessKit) that has not finished registering its provider, stalling
// *every* query for ~10 s. To avoid it, each window is gated before UIA touches
// it (see [`window_is_ready`]); a momentarily unresponsive window is skipped for
// this pass and picked up on a later poll instead of stalling on OLEACC's timeout.

/// Bounded timeout for the per-window `WM_GETOBJECT` liveness probe.
const WINDOW_PROBE_TIMEOUT_MS: u32 = 300;

// Streams desktop windows first, followed by synthetic app:Application nodes.
struct ElementAndAppIter {
    elements: std::vec::IntoIter<windows::Win32::UI::Accessibility::IUIAutomationElement>,
    parent: Arc<dyn UiNode>,
    seen: HashSet<i32>,
    pending_apps: VecDeque<i32>,
    raw_phase_complete: bool,
    honor_window_claims: bool,
    java_classifier: Option<Arc<dyn JavaClassifier>>,
}

/// `EnumWindows` callback: collects each top-level `HWND` into the `Vec` passed
/// through `lparam`.
unsafe extern "system" fn collect_hwnd(
    hwnd: windows::Win32::Foundation::HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::core::BOOL {
    // SAFETY: `lparam` carries the `&mut Vec<HWND>` supplied to `EnumWindows`.
    let hwnds = unsafe { &mut *(lparam.0 as *mut Vec<windows::Win32::Foundation::HWND>) };
    hwnds.push(hwnd);
    windows::core::BOOL(1)
}

/// Returns whether `hwnd` can be materialised by UIA without risking the OLEACC
/// 10 s `WM_GETOBJECT` stall: either it exposes a native UIA provider, or it
/// answers a short, bounded `WM_GETOBJECT` probe.
fn window_is_ready(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Accessibility::{IAccessible, ObjectFromLresult, UiaHasServerSideProvider};
    use windows::Win32::UI::WindowsAndMessaging::{SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_GETOBJECT};
    use windows::core::Interface;

    // Fast path: a native UIA provider never needs the MSAA bridge.
    if unsafe { UiaHasServerSideProvider(hwnd) }.as_bool() {
        return true;
    }

    // Fallback for MSAA-only windows: keep them, but only if they answer a
    // bounded WM_GETOBJECT probe. `OBJID_CLIENT` is what the MSAA bridge queries.
    const OBJID_CLIENT: isize = -4;
    let mut cookie: usize = 0;
    let sent = unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_GETOBJECT,
            WPARAM(0),
            LPARAM(OBJID_CLIENT),
            SMTO_ABORTIFHUNG,
            WINDOW_PROBE_TIMEOUT_MS,
            Some(std::ptr::addr_of_mut!(cookie)),
        )
    };
    if sent.0 == 0 {
        return false; // timed out / not answering -> skip this pass
    }
    // Release the accessible object the window handed back so the probe cannot leak.
    if cookie != 0 {
        let mut obj: *mut core::ffi::c_void = std::ptr::null_mut();
        // SAFETY: `cookie` is the LRESULT returned by the WM_GETOBJECT above;
        // ObjectFromLresult retrieves the accessible object so we can release it.
        let hr = unsafe {
            ObjectFromLresult(LRESULT(cookie.cast_signed()), &IAccessible::IID, WPARAM(0), std::ptr::addr_of_mut!(obj))
        };
        if hr.is_ok() && !obj.is_null() {
            // SAFETY: ObjectFromLresult handed back an owning IAccessible reference.
            drop(unsafe { IAccessible::from_raw(obj) });
        }
    }
    true
}

/// Whether `hwnd` is on-screen: visible and not DWM-cloaked (cloaking hides
/// virtual-desktop / UWP "ghost" windows that are composed elsewhere or suspended).
fn is_visible_uncloaked(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
    use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;

    // SAFETY: `hwnd` is a valid window handle for these read-only queries.
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }
        let mut cloaked: u32 = 0;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            std::ptr::addr_of_mut!(cloaked).cast(),
            u32::try_from(std::mem::size_of::<u32>()).unwrap_or(4),
        );
        cloaked == 0
    }
}

/// Whether `hwnd` has a non-empty on-screen rectangle. Some immersive shell
/// windows linger as visible, uncloaked, but zero-size (`0×0`, `w×0`, `1×1`)
/// placeholders; those must not surface as phantom "invisible" windows in the
/// tree.
fn window_has_area(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let mut rect = RECT::default();
    // SAFETY: `hwnd` is valid; `rect` is written by GetWindowRect.
    if unsafe { GetWindowRect(hwnd, std::ptr::addr_of_mut!(rect)) }.is_err() {
        return false;
    }
    rect.right > rect.left && rect.bottom > rect.top
}

/// Whether `hwnd` is a real top-level window worth surfacing from the general
/// `EnumWindows` population: on-screen (see [`is_visible_uncloaked`]) and not a
/// non-activating helper/overlay window (`WS_EX_NOACTIVATE` — ConPTY console,
/// winit event target, Narrator helper, ...). The `WS_EX_NOACTIVATE` heuristic
/// excludes *unknown* helper windows; it is deliberately **not** applied to the
/// curated immersive shell classes (see [`immersive_shell_windows`]), which are
/// legitimately non-activating (menus, flyouts).
fn is_candidate_top_level(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GetWindowLongPtrW, WS_EX_NOACTIVATE};

    if !is_visible_uncloaked(hwnd) || !window_has_area(hwnd) {
        return false;
    }
    // SAFETY: `hwnd` is a valid window handle for this read-only query.
    let exstyle = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    exstyle & isize::try_from(WS_EX_NOACTIVATE.0).unwrap_or(0) == 0
}

/// Top-level window classes for the Win11 shell's **immersive** surfaces that
/// `EnumWindows` does not return: the Start menu / search (`CoreWindow`) and the
/// taskbar / jump-list / settings flyouts (`Xaml_WindowedPopupClass`). Without
/// these the desktop tree — and therefore the live picker's reveal — cannot reach
/// them even though `ElementFromPoint` resolves them, and Accessibility Insights
/// (which walks the UIA tree) can.
const IMMERSIVE_SHELL_CLASSES: [windows::core::PCWSTR; 2] =
    [windows::core::w!("Windows.UI.Core.CoreWindow"), windows::core::w!("Xaml_WindowedPopupClass")];

/// Enumerates top-level windows of the [`IMMERSIVE_SHELL_CLASSES`] via
/// `FindWindowExW` (cheap, by class name). These carry native UIA providers, so
/// materialising them later does not risk the OLEACC bridge stall.
fn immersive_shell_windows() -> Vec<windows::Win32::Foundation::HWND> {
    use windows::Win32::UI::WindowsAndMessaging::FindWindowExW;
    use windows::core::PCWSTR;

    let mut out = Vec::new();
    for class in IMMERSIVE_SHELL_CLASSES {
        let mut prev: Option<windows::Win32::Foundation::HWND> = None;
        loop {
            // SAFETY: read-only enumeration of top-level windows by class name.
            match unsafe { FindWindowExW(None, prev, class, PCWSTR::null()) } {
                Ok(hwnd) if !hwnd.is_invalid() => {
                    out.push(hwnd);
                    prev = Some(hwnd);
                }
                _ => break,
            }
        }
    }
    out
}

/// Enumerates the desktop's top-level application windows and returns UIA elements
/// for those ready to be materialised (see [`window_is_ready`]). When `pid_filter`
/// is set, only windows owned by that process are returned — used to list one
/// application's top-level windows without navigating UIA's tree (which would risk
/// the OLEACC stall described above).
///
/// With `honor_window_claims` set (the `providers.windows-uia.honor_window_claims`
/// config, default true), windows another provider has claimed in the process-wide
/// registry (e.g. Java windows fully represented by the JAB provider) are skipped,
/// so each window appears exactly once in the merged tree.
pub(crate) fn ready_top_level_elements(
    pid_filter: Option<i32>,
    honor_window_claims: bool,
) -> Vec<windows::Win32::UI::Accessibility::IUIAutomationElement> {
    use std::collections::HashSet;
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId};

    let mut elements = Vec::new();
    let Ok(uia) = crate::com::uia() else {
        return elements;
    };

    let mut raw_hwnds: Vec<windows::Win32::Foundation::HWND> = Vec::new();
    // SAFETY: `collect_hwnd` only pushes into the `Vec` pointed to by `lparam`.
    unsafe {
        let _ = EnumWindows(
            Some(collect_hwnd),
            windows::Win32::Foundation::LPARAM(std::ptr::addr_of_mut!(raw_hwnds) as isize),
        );
    }

    // Build the candidate set from two sources with source-appropriate gates:
    //  1. the general `EnumWindows` population — full filter (incl. the
    //     `WS_EX_NOACTIVATE` helper-window heuristic);
    //  2. the curated immersive shell classes `EnumWindows` omits — gated only on
    //     being on-screen, since they are legitimately non-activating.
    let mut seen: HashSet<isize> = HashSet::new();
    let mut candidates: Vec<windows::Win32::Foundation::HWND> = Vec::new();
    for hwnd in raw_hwnds {
        if is_candidate_top_level(hwnd) && seen.insert(hwnd.0 as isize) {
            candidates.push(hwnd);
        }
    }
    for hwnd in immersive_shell_windows() {
        if is_visible_uncloaked(hwnd) && window_has_area(hwnd) && seen.insert(hwnd.0 as isize) {
            candidates.push(hwnd);
        }
    }

    for hwnd in candidates {
        if let Some(target) = pid_filter {
            let mut wpid: u32 = 0;
            // SAFETY: `hwnd` is valid; `wpid` receives the owning process id.
            unsafe { GetWindowThreadProcessId(hwnd, Some(std::ptr::addr_of_mut!(wpid))) };
            if u32::try_from(target).ok() != Some(wpid) {
                continue;
            }
        }
        if honor_window_claims
            && platynui_core::platform::window_claims::is_claimed_by_other(hwnd.0 as u64, PROVIDER_ID)
        {
            continue;
        }
        if !window_is_ready(hwnd) {
            continue;
        }
        if let Ok(element) = unsafe { uia.ElementFromHandle(hwnd) } {
            elements.push(element);
        }
    }
    elements
}

impl ElementAndAppIter {
    fn new(
        parent: Arc<dyn UiNode>,
        honor_window_claims: bool,
        java_classifier: Option<Arc<dyn JavaClassifier>>,
    ) -> Self {
        Self {
            elements: ready_top_level_elements(None, honor_window_claims).into_iter(),
            parent,
            seen: HashSet::new(),
            pending_apps: VecDeque::new(),
            raw_phase_complete: false,
            honor_window_claims,
            java_classifier,
        }
    }

    fn stream_next_pending_app(&mut self) -> Option<Arc<dyn UiNode>> {
        while let Some(pid) = self.pending_apps.pop_front() {
            if pid > 0 && pid != *SELF_PID {
                let app = crate::node::ApplicationNode::new(
                    pid,
                    &self.parent,
                    self.honor_window_claims,
                    self.java_classifier.clone(),
                );
                return Some(app as Arc<dyn UiNode>);
            }
        }
        None
    }
}

impl Iterator for ElementAndAppIter {
    type Item = Arc<dyn UiNode>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.raw_phase_complete {
            return self.stream_next_pending_app();
        }
        for element in self.elements.by_ref() {
            let pid = crate::map::get_process_id(&element).unwrap_or(-1);
            if pid <= 0 || pid == *SELF_PID {
                continue;
            }
            // Create and return the desktop-scoped window node.
            let desktop_node = crate::node::UiaNode::from_elem_with_scope(element, crate::map::UiaIdScope::Desktop);
            desktop_node.set_parent(&self.parent);
            crate::node::UiaNode::init_self(&desktop_node);
            if let Some(classifier) = &self.java_classifier {
                desktop_node.set_java_classifier(Arc::clone(classifier));
            }

            // Queue the synthetic application node once per process; queued
            // applications are emitted after all top-level windows.
            if self.seen.insert(pid) {
                self.pending_apps.push_back(pid);
            }
            return Some(desktop_node as Arc<dyn UiNode>);
        }
        self.raw_phase_complete = true;
        self.stream_next_pending_app()
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

    fn create(&self, config: &RuntimeConfig) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        Ok(Arc::new(self.build(config)))
    }
}

impl WindowsUiaFactory {
    /// Build a concrete provider from `config` — split out from `create` so
    /// the config wiring is unit-testable without a live UIA session.
    fn build(&self, config: &RuntimeConfig) -> WindowsUiaProvider {
        // Kill switch for the window-claims cooperation (see
        // `platynui_core::platform::window_claims`): with `false`, windows
        // claimed by other providers (e.g. JAB) reappear as UIA shells.
        let honor_window_claims =
            config.provider(PROVIDER_ID).and_then(|uia| uia.get_bool("honor_window_claims")).unwrap_or(true);
        WindowsUiaProvider::new(honor_window_claims)
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
    /// `providers.windows-uia.honor_window_claims` (default true).
    honor_window_claims: bool,
    /// Java-app classifier injected via [`UiTreeProvider::set_java_classifier`]
    /// (from the runtime's platform bundle); `None` until injected — nodes then
    /// simply carry no JVM classification attributes.
    java_classifier: Mutex<Option<Arc<dyn JavaClassifier>>>,
}

impl WindowsUiaProvider {
    fn new(honor_window_claims: bool) -> Self {
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
            ProviderDescriptor::new(
                PROVIDER_ID,
                PROVIDER_NAME,
                TechnologyId::from("UIAutomation"),
                ProviderKind::Native,
            )
        });

        Self {
            descriptor: &DESCRIPTOR,
            is_shutdown: AtomicBool::new(false),
            honor_window_claims,
            java_classifier: Mutex::new(None),
        }
    }

    fn java_classifier(&self) -> Option<Arc<dyn JavaClassifier>> {
        self.java_classifier.lock().expect("java classifier mutex poisoned").clone()
    }
}

impl UiTreeProvider for WindowsUiaProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
    }

    fn set_java_classifier(&self, classifier: Arc<dyn JavaClassifier>) {
        let mut slot = self.java_classifier.lock().expect("java classifier mutex poisoned");
        if slot.is_none() {
            *slot = Some(classifier);
        }
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
        // Fail fast if the UIA client is unavailable (e.g. after shutdown).
        crate::com::uia().map_err(|e| ProviderError::CommunicationFailure {
            channel: "windows uia",
            details: Some(e.to_string()),
        })?;

        // Stream: desktop top-level windows (excluding own process), then one app:Application per PID.
        let it = ElementAndAppIter::new(parent, self.honor_window_claims, self.java_classifier());
        Ok(Box::new(it))
    }

    fn element_at_point(&self, point: Point) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
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

        // UIA `ElementFromPoint` resolves window- and in-window z-order natively.
        #[expect(clippy::cast_possible_truncation, reason = "desktop coordinates fit in i32")]
        let pt = windows::Win32::Foundation::POINT { x: point.x().round() as i32, y: point.y().round() as i32 };

        // Order-independent hit-test arbitration (add-jab-hit-test): a window
        // another provider claims (e.g. a Java window fully represented by the
        // JAB provider) must resolve to that provider's node, not the UIA
        // shell — so UIA abstains for the point *before* `ElementFromPoint`
        // and the runtime falls through to the claiming provider, regardless
        // of provider registration order. Mirrors the root-streaming skip in
        // `ready_top_level_elements`, including its kill switch.
        if let Some(hwnd) = top_level_window_at(pt)
            && abstains_from_claimed_window(self.honor_window_claims, hwnd)
        {
            return Err(ProviderError::UnsupportedOperation {
                operation: "element_at_point",
                details: Some("window at point is claimed by another provider".into()),
            });
        }
        let elem = match unsafe { uia.ElementFromPoint(pt) } {
            Ok(elem) => elem,
            Err(err) => {
                tracing::debug!(%err, ?point, "UIA ElementFromPoint returned no element");
                return Ok(None);
            }
        };

        // Never resolve the host process's own UI. `ElementFromPoint` returns the
        // top-most element, so a point the Inspector's own window covers resolves
        // to the Inspector — return nothing there (the picker is meant to inspect
        // *other* windows; keep the Inspector off the target). The highlight
        // overlay is `WS_EX_TRANSPARENT`, so `ElementFromPoint` passes through it
        // to the target beneath rather than resolving the overlay.
        let pid = crate::map::get_process_id(&elem).ok();
        if pid == Some(*SELF_PID) {
            return Ok(None);
        }
        // Scope the node to its owning process so its runtime id matches the id
        // top-down traversal produces for the same element (app:Application/PID).
        let scope = match pid {
            Some(pid) => crate::map::UiaIdScope::App { pid },
            None => crate::map::UiaIdScope::Desktop,
        };
        let node = crate::node::UiaNode::from_elem_with_scope(elem, scope);
        crate::node::UiaNode::init_self(&node);
        // A point hit-test resolves a node out of tree order, so it has no
        // parent chain — the Inspector's reveal-and-select walks `parent()` up to
        // match tree nodes and would find nothing. Wire the app-scoped ancestor
        // chain (matching runtime ids) so the picked element is actually selected
        // in the tree. (Desktop-scoped fallback has no application subtree to
        // reveal into, so it is left chainless.)
        if let crate::map::UiaIdScope::App { pid } = scope {
            crate::node::UiaNode::attach_ancestor_chain(&node, pid);
        }
        Ok(Some(node as Arc<dyn UiNode>))
    }
}

/// Top-level window under `pt` (`WindowFromPoint` → `GetAncestor(GA_ROOT)`),
/// as the raw claim key used by `platynui_core::platform::window_claims`.
fn top_level_window_at(pt: windows::Win32::Foundation::POINT) -> Option<u64> {
    use windows::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, WindowFromPoint};

    // SAFETY: read-only point/window queries.
    unsafe {
        let hwnd = WindowFromPoint(pt);
        if hwnd.is_invalid() {
            return None;
        }
        let root = GetAncestor(hwnd, GA_ROOT);
        let top_level = if root.is_invalid() { hwnd } else { root };
        Some(top_level.0 as u64)
    }
}

/// The claim gate of [`WindowsUiaProvider::element_at_point`], split out so the
/// config/registry interplay is unit-testable without a desktop: abstain
/// exactly when claims are honored and another provider owns the window.
fn abstains_from_claimed_window(honor_window_claims: bool, hwnd: u64) -> bool {
    honor_window_claims && platynui_core::platform::window_claims::is_claimed_by_other(hwnd, PROVIDER_ID)
}

// Register the factory with the global inventory when this crate is linked.
pub static WINDOWS_UIA_FACTORY: WindowsUiaFactory = WindowsUiaFactory;
register_provider!(&WINDOWS_UIA_FACTORY);

// (no second specialized impl; Windows path handled above with cfg guards inside)

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::config::ConfigMap;

    #[test]
    fn honor_window_claims_defaults_to_true() {
        let provider = WindowsUiaFactory.build(&RuntimeConfig::default());
        assert!(provider.honor_window_claims);
    }

    #[test]
    fn honor_window_claims_can_be_disabled() {
        let providers = ConfigMap::new().with(PROVIDER_ID, ConfigMap::new().with("honor_window_claims", false));
        let config = RuntimeConfig::new(ConfigMap::new(), providers);
        let provider = WindowsUiaFactory.build(&config);
        assert!(!provider.honor_window_claims);
    }

    #[test]
    fn hit_test_abstains_from_claimed_windows_only_while_honoring_claims() {
        use platynui_core::platform::window_claims;

        // Process-global registry: use an hwnd value no other test claims.
        const HWND: u64 = 0xA7_0001;
        window_claims::claim_window(HWND, "jab");
        assert!(abstains_from_claimed_window(true, HWND), "claimed window must abstain while claims are honored");
        assert!(!abstains_from_claimed_window(false, HWND), "kill switch off must resolve the shell again");
        window_claims::release_window(HWND, "jab");
        assert!(!abstains_from_claimed_window(true, HWND), "released window must resolve normally");
    }

    #[test]
    fn hit_test_does_not_abstain_from_own_claims() {
        use platynui_core::platform::window_claims;

        const HWND: u64 = 0xA7_0002;
        window_claims::claim_window(HWND, PROVIDER_ID);
        assert!(!abstains_from_claimed_window(true, HWND), "own claims must not gate the hit-test");
        window_claims::release_window(HWND, PROVIDER_ID);
    }
}
