//! Windows UIAutomation node wrapper and iterators (no provider-side caching).
//!
//! UiaNode reflects the current UIA state; no heavy provider‑side caches.

use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex, Weak};
// no name cache atomics needed

use platynui_core::types::{Point as UiPoint, Rect};
use platynui_core::ui::pattern::{FocusableAction, PatternError, UiPattern, WindowSurfaceActions};
use platynui_core::ui::{Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiValue};
use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, WaitForInputIdle};
use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, IUIAutomationTransformPattern, IUIAutomationVirtualizedItemPattern,
    IUIAutomationWindowPattern, WindowVisualState_Maximized, WindowVisualState_Minimized,
};
use windows::core::Interface;

/// Thread-safe checker for WaitForInputIdle using process ID
#[derive(Clone)]
struct WaitForInputIdleChecker {
    pid: i32,
}

impl WaitForInputIdleChecker {
    fn new(pid: i32) -> Self {
        Self { pid }
    }

    fn check_input_idle(&self) -> Result<Option<bool>, PatternError> {
        if self.pid <= 0 {
            return Ok(Some(false)); // Invalid process ID
        }

        // Open process handle with query rights
        let process_handle = match unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, false, self.pid as u32) } {
            Ok(handle) => handle,
            Err(_) => {
                // Process might not be accessible or doesn't exist anymore
                return Ok(Some(false));
            }
        };

        // Call WaitForInputIdle with a short timeout (100ms)
        let result = unsafe { WaitForInputIdle(process_handle, 100) };
        unsafe {
            let _ = CloseHandle(process_handle);
        };

        if result == WAIT_OBJECT_0.0 { Ok(Some(true)) } else { Ok(Some(false)) }
    }
}

unsafe impl Send for WaitForInputIdleChecker {}
unsafe impl Sync for WaitForInputIdleChecker {}

pub struct UiaNode {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    self_weak: once_cell::sync::OnceCell<Weak<dyn UiNode>>,
    // Minimal identity caches required by trait return types
    rid_cell: once_cell::sync::OnceCell<RuntimeId>,
    id_scope: crate::map::UiaIdScope,
}
unsafe impl Send for UiaNode {}
unsafe impl Sync for UiaNode {}

impl UiaNode {
    pub fn from_elem_with_scope(
        elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
        scope: crate::map::UiaIdScope,
    ) -> Arc<Self> {
        Arc::new(Self {
            elem,
            parent: Mutex::new(None),
            self_weak: once_cell::sync::OnceCell::new(),
            rid_cell: once_cell::sync::OnceCell::new(),
            id_scope: scope,
        })
    }
    pub fn set_parent(&self, parent: &Arc<dyn UiNode>) {
        if let Ok(mut guard) = self.parent.lock() {
            *guard = Some(Arc::downgrade(parent));
        }
    }
    pub fn init_self(this: &Arc<Self>) {
        let arc: Arc<dyn UiNode> = this.clone();
        let _ = this.self_weak.set(Arc::downgrade(&arc));
    }
    fn as_ui_node(&self) -> Option<Arc<dyn UiNode>> {
        self.self_weak.get().and_then(|w| w.upgrade())
    }

    /// Best-effort realization for virtualized items.
    /// If the element supports `VirtualizedItem` pattern, call `Realize()`.
    /// Errors are ignored to keep traversal non-panicking.
    fn try_realize(&self) {
        let unk = unsafe {
            self.elem
                .GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                    windows::Win32::UI::Accessibility::UIA_VirtualizedItemPatternId.0,
                ))
                .ok()
        };
        if let Some(unk) = unk
            && let Ok(pat) = unk.cast::<IUIAutomationVirtualizedItemPattern>()
        {
            let _ = unsafe { pat.Realize() };
        }
    }
}

impl UiNode for UiaNode {
    fn namespace(&self) -> Namespace {
        unsafe {
            let is_control = self.elem.CurrentIsControlElement().map(|b| b.as_bool()).unwrap_or(true);
            if is_control {
                return Namespace::Control;
            }
            let is_content = self.elem.CurrentIsContentElement().map(|b| b.as_bool()).unwrap_or(false);
            if is_content { Namespace::Item } else { Namespace::Control }
        }
    }
    fn role(&self) -> &str {
        let ct = crate::map::get_control_type(&self.elem).unwrap_or(0);
        crate::map::control_type_to_role(ct)
    }
    fn name(&self) -> String {
        crate::map::get_name(&self.elem).unwrap_or_default()
    }
    fn runtime_id(&self) -> &RuntimeId {
        self.rid_cell.get_or_init(|| {
            let s =
                crate::map::format_scoped_runtime_id(&self.elem, self.id_scope).unwrap_or_else(|_| "uia://temp".into());
            RuntimeId::from(s)
        })
    }
    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        match self.parent.lock() {
            Ok(g) => g.clone(),
            Err(_) => None,
        }
    }
    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        // Ensure a virtualized item is realized before enumerating its children.
        self.try_realize();
        match self.as_ui_node() {
            Some(parent_arc) => Box::new(ElementChildrenIter::new(self.elem.clone(), Some(parent_arc), self.id_scope)),
            None => Box::new(std::iter::empty::<Arc<dyn UiNode>>()),
        }
    }
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        let rid_str = self.runtime_id().as_str().to_string();
        Box::new(AttrsIter::new(self.elem.clone(), rid_str))
    }

    fn supported_patterns(&self) -> Vec<PatternId> {
        use windows::Win32::UI::Accessibility::*;
        let mut out = vec![FocusableAction::static_id()];
        let (has_window, has_transform) = unsafe {
            let has_window = self.elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_WindowPatternId.0)).is_ok();
            let has_transform = self.elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_TransformPatternId.0)).is_ok();
            (has_window, has_transform)
        };
        if has_window || has_transform {
            out.push(WindowSurfaceActions::static_id());
        }
        out
    }
    fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
        use windows::Win32::UI::Accessibility::*;
        use windows::core::Interface;
        let pid = pattern.as_str();
        if pid == FocusableAction::static_id().as_str() {
            #[derive(Clone)]
            struct ElemSend {
                elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
            }
            unsafe impl Send for ElemSend {}
            unsafe impl Sync for ElemSend {}
            impl ElemSend {
                unsafe fn set_focus(&self) -> Result<(), crate::error::UiaError> {
                    crate::error::uia_api("IUIAutomationElement::SetFocus", unsafe { self.elem.SetFocus() })
                }
            }
            let es = ElemSend { elem: self.elem.clone() };
            let action = FocusableAction::new(move || unsafe {
                // If the element is virtualized, try to realize before focusing.
                if let Ok(unk) = es.elem.GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                    windows::Win32::UI::Accessibility::UIA_VirtualizedItemPatternId.0,
                )) && let Ok(vpat) = unk.cast::<IUIAutomationVirtualizedItemPattern>()
                {
                    let _ = vpat.Realize();
                }
                es.set_focus().map_err(|e| PatternError::new(e.to_string()))
            });
            return Some(Arc::new(action) as Arc<dyn UiPattern>);
        }
        if pid == WindowSurfaceActions::static_id().as_str() {
            #[derive(Clone)]
            struct ElemSend {
                elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
            }
            unsafe impl Send for ElemSend {}
            unsafe impl Sync for ElemSend {}
            impl ElemSend {
                unsafe fn set_focus(&self) -> Result<(), crate::error::UiaError> {
                    crate::error::uia_api("IUIAutomationElement::SetFocus", unsafe { self.elem.SetFocus() })
                }
                unsafe fn window_set_state(&self, state: WindowVisualState) -> Result<(), crate::error::UiaError> {
                    let unk = crate::error::uia_api("IUIAutomationElement::GetCurrentPattern(Window)", unsafe {
                        self.elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_WindowPatternId.0))
                    })?;
                    let pat: IUIAutomationWindowPattern =
                        crate::error::uia_api("IUnknown::cast(WindowPattern)", unk.cast())?;
                    crate::error::uia_api("IUIAutomationWindowPattern::SetWindowVisualState", unsafe {
                        pat.SetWindowVisualState(state)
                    })
                }
                unsafe fn window_close(&self) -> Result<(), crate::error::UiaError> {
                    let unk = crate::error::uia_api("IUIAutomationElement::GetCurrentPattern(Window)", unsafe {
                        self.elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_WindowPatternId.0))
                    })?;
                    let pat: IUIAutomationWindowPattern =
                        crate::error::uia_api("IUnknown::cast(WindowPattern)", unk.cast())?;
                    crate::error::uia_api("IUIAutomationWindowPattern::Close", unsafe { pat.Close() })
                }
                unsafe fn transform_move(&self, x: f64, y: f64) -> Result<(), crate::error::UiaError> {
                    let unk = crate::error::uia_api("IUIAutomationElement::GetCurrentPattern(Transform)", unsafe {
                        self.elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_TransformPatternId.0))
                    })?;
                    let pat: IUIAutomationTransformPattern =
                        crate::error::uia_api("IUnknown::cast(TransformPattern)", unk.cast())?;
                    crate::error::uia_api("IUIAutomationTransformPattern::Move", unsafe { pat.Move(x, y) })
                }
                unsafe fn transform_resize(&self, w: f64, h: f64) -> Result<(), crate::error::UiaError> {
                    let unk = crate::error::uia_api("IUIAutomationElement::GetCurrentPattern(Transform)", unsafe {
                        self.elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_TransformPatternId.0))
                    })?;
                    let pat: IUIAutomationTransformPattern =
                        crate::error::uia_api("IUnknown::cast(TransformPattern)", unk.cast())?;
                    crate::error::uia_api("IUIAutomationTransformPattern::Resize", unsafe { pat.Resize(w, h) })
                }
            }
            let e1 = ElemSend { elem: self.elem.clone() };
            let e2 = e1.clone();
            let e3 = e1.clone();
            let e4 = e1.clone();
            let e5 = e1.clone();
            let e_move = e1.clone();
            let e_resize = e1.clone();

            // Get process ID for WaitForInputIdle checking
            let pid = crate::map::get_process_id(&self.elem).unwrap_or(-1);
            let input_checker = WaitForInputIdleChecker::new(pid);

            let check_input_idle = move || -> Result<Option<bool>, PatternError> {
                // Use WaitForInputIdle to check if process is ready for input
                let idle_result = input_checker.check_input_idle()?;

                // If process is not idle, return false immediately
                if Some(false) == idle_result {
                    return Ok(Some(false));
                }

                // Process is idle or check failed - combine with basic enabled/visible check
                // Note: We can't access UIA element here due to thread safety, so we return
                // None to indicate that the check should be performed dynamically when needed
                Ok(None)
            };
            let actions = WindowSurfaceActions::new()
                .with_activate(move || unsafe { e1.set_focus().map_err(|e| PatternError::new(e.to_string())) })
                .with_minimize(move || unsafe {
                    e2.window_set_state(WindowVisualState_Minimized).map_err(|e| PatternError::new(e.to_string()))
                })
                .with_maximize(move || unsafe {
                    e3.window_set_state(WindowVisualState_Maximized).map_err(|e| PatternError::new(e.to_string()))
                })
                .with_restore(move || unsafe {
                    e4.window_set_state(WindowVisualState_Normal).map_err(|e| PatternError::new(e.to_string()))
                })
                .with_close(move || unsafe { e5.window_close().map_err(|e| PatternError::new(e.to_string())) })
                .with_move_to(move |p| unsafe {
                    e_move.transform_move(p.x(), p.y()).map_err(|e| PatternError::new(e.to_string()))
                })
                .with_resize(move |s| unsafe {
                    e_resize.transform_resize(s.width(), s.height()).map_err(|e| PatternError::new(e.to_string()))
                })
                .with_accepts_user_input(check_input_idle);
            return Some(Arc::new(actions) as Arc<dyn UiPattern>);
        }
        None
    }

    fn invalidate(&self) {}

    fn is_valid(&self) -> bool {
        // Try a very cheap property read that should succeed for live elements.
        // If the element is stale (e.g., UIA_E_ELEMENTNOTAVAILABLE), windows::core::Result will be Err.
        unsafe {
            // CurrentControlType is a minimal call and doesn't alter state.
            self.elem.CurrentProcessId().is_ok()
        }
    }
}

pub(crate) struct ElementChildrenIter {
    walker: Option<windows::Win32::UI::Accessibility::IUIAutomationTreeWalker>,
    parent_elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
    current: Option<windows::Win32::UI::Accessibility::IUIAutomationElement>,
    first: bool,
    parent: Option<Arc<dyn UiNode>>,
    scope: crate::map::UiaIdScope,
}
impl ElementChildrenIter {
    pub fn new(
        parent_elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
        parent_node: Option<Arc<dyn UiNode>>,
        scope: crate::map::UiaIdScope,
    ) -> Self {
        let walker = crate::com::raw_walker().ok();
        Self { walker, parent_elem, current: None, first: true, parent: parent_node, scope }
    }

    fn next_internal(&mut self) -> Option<IUIAutomationElement> {
        let Some(walker) = &self.walker else { return None };
        if self.first {
            self.first = false;
            self.current = unsafe { walker.GetFirstChildElement(&self.parent_elem).ok() };
            self.current.as_ref()?;
        } else if let Some(ref elem) = self.current {
            let cur = elem.clone();
            self.current = unsafe { walker.GetNextSiblingElement(&cur).ok() };
        } else {
            return None;
        }
        let elem = self.current.as_ref()?.clone();
        // Best-effort: if the child is virtualized, realize it before wrapping.
        unsafe {
            if let Ok(unk) = elem.GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                windows::Win32::UI::Accessibility::UIA_VirtualizedItemPatternId.0,
            )) && let Ok(vpat) = unk.cast::<IUIAutomationVirtualizedItemPattern>()
            {
                let _ = vpat.Realize();
            }
        }
        Some(elem)
    }
}

unsafe impl Send for ElementChildrenIter {}
impl Iterator for ElementChildrenIter {
    type Item = Arc<dyn UiNode>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let elem = self.next_internal()?;
            let pid = crate::map::get_process_id(&elem).unwrap_or(-1);
            if pid == *SELF_PID {
                // Skip elements from the same process to avoid infinite loops.
                continue;
            }

            let node = UiaNode::from_elem_with_scope(elem, self.scope);
            if let Some(ref parent) = self.parent {
                node.set_parent(parent);
            }
            UiaNode::init_self(&node);
            return Some(node as Arc<dyn UiNode>);
        }
    }
}

// Cache current process id once for the entire module; process id is stable for the process lifetime.
static SELF_PID: Lazy<i32> = Lazy::new(|| std::process::id() as i32);

struct RoleAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for RoleAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "Role"
    }
    fn value(&self) -> UiValue {
        UiValue::from(crate::map::control_type_to_role(crate::map::get_control_type(&self.elem).unwrap_or(0)))
    }
}
unsafe impl Send for RoleAttr {}
unsafe impl Sync for RoleAttr {}

struct NameAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for NameAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "Name"
    }
    fn value(&self) -> UiValue {
        UiValue::from(crate::map::get_name(&self.elem).unwrap_or_default())
    }
}
unsafe impl Send for NameAttr {}
unsafe impl Sync for NameAttr {}

struct RuntimeIdAttr {
    rid: String,
}
impl UiAttribute for RuntimeIdAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "RuntimeId"
    }
    fn value(&self) -> UiValue {
        UiValue::from(self.rid.clone())
    }
}
unsafe impl Send for RuntimeIdAttr {}
unsafe impl Sync for RuntimeIdAttr {}

struct BoundsAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for BoundsAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "Bounds"
    }
    fn value(&self) -> UiValue {
        UiValue::from(crate::map::get_bounding_rect(&self.elem).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)))
    }
}
unsafe impl Send for BoundsAttr {}
unsafe impl Sync for BoundsAttr {}

struct IsEnabledAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for IsEnabledAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "IsEnabled"
    }
    fn value(&self) -> UiValue {
        UiValue::from(crate::map::get_is_enabled(&self.elem).unwrap_or(false))
    }
}
unsafe impl Send for IsEnabledAttr {}
unsafe impl Sync for IsEnabledAttr {}

struct IsOffscreenAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for IsOffscreenAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "IsOffscreen"
    }
    fn value(&self) -> UiValue {
        UiValue::from(crate::map::get_is_offscreen(&self.elem).unwrap_or(false))
    }
}
unsafe impl Send for IsOffscreenAttr {}
unsafe impl Sync for IsOffscreenAttr {}

struct ActivationPointAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for ActivationPointAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "ActivationPoint"
    }
    fn value(&self) -> UiValue {
        let p = crate::map::get_clickable_point(&self.elem).ok().unwrap_or_else(|| {
            let r = crate::map::get_bounding_rect(&self.elem).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
            UiPoint::new(r.x() + r.width() / 2.0, r.y() + r.height() / 2.0)
        });
        UiValue::from(p)
    }
}
unsafe impl Send for ActivationPointAttr {}
unsafe impl Sync for ActivationPointAttr {}

struct IsVisibleAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for IsVisibleAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "IsVisible"
    }
    fn value(&self) -> UiValue {
        let off = crate::map::get_is_offscreen(&self.elem).unwrap_or(false);
        let r = crate::map::get_bounding_rect(&self.elem).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        UiValue::from(!off && r.width() > 0.0 && r.height() > 0.0)
    }
}
unsafe impl Send for IsVisibleAttr {}
unsafe impl Sync for IsVisibleAttr {}

struct AcceptsUserInputAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for AcceptsUserInputAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "AcceptsUserInput"
    }
    fn value(&self) -> UiValue {
        // Get process ID for WaitForInputIdle checking
        let pid = crate::map::get_process_id(&self.elem).unwrap_or(-1);

        // First check if the process is ready for input using WaitForInputIdle
        let input_checker = WaitForInputIdleChecker::new(pid);
        if let Ok(Some(idle_ready)) = input_checker.check_input_idle()
            && !idle_ready
        {
            // Process is not ready for input (busy or error)
            return UiValue::from(false);
        }

        // Process is idle or check failed - fall back to basic enabled/visible check
        let enabled = crate::map::get_is_enabled(&self.elem).unwrap_or(false);
        let off = crate::map::get_is_offscreen(&self.elem).unwrap_or(false);
        UiValue::from(enabled && !off)
    }
}
unsafe impl Send for AcceptsUserInputAttr {}
unsafe impl Sync for AcceptsUserInputAttr {}

struct AttrsIter {
    idx: u8,
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
    has_window_surface: bool,
    native_cache: Option<Vec<Arc<dyn UiAttribute>>>,
    native_pos: usize,
    rid_str: String,
}
impl AttrsIter {
    fn new(elem: windows::Win32::UI::Accessibility::IUIAutomationElement, rid_str: String) -> Self {
        use windows::Win32::UI::Accessibility::*;
        let has_window_surface = unsafe {
            let has_window = elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_WindowPatternId.0)).is_ok();
            let has_transform = elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_TransformPatternId.0)).is_ok();
            has_window || has_transform
        };
        Self { idx: 0, elem, has_window_surface, native_cache: None, native_pos: 0, rid_str }
    }
}
impl Iterator for AttrsIter {
    type Item = Arc<dyn UiAttribute>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let elem = self.elem.clone();
            let item = match self.idx {
                0 => Some(Arc::new(RoleAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>),
                1 => Some(Arc::new(NameAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>),
                2 => Some(Arc::new(RuntimeIdAttr { rid: self.rid_str.clone() }) as Arc<dyn UiAttribute>),
                3 => Some(Arc::new(BoundsAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>),
                4 => Some(Arc::new(ActivationPointAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>),
                5 => Some(Arc::new(IsEnabledAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>),
                6 => Some(Arc::new(IsOffscreenAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>),
                7 => Some(Arc::new(IsVisibleAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>),
                8 => Some(Arc::new(IsFocusedAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>),
                9 => {
                    if self.has_window_surface {
                        Some(Arc::new(IsMinimizedAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                10 => {
                    if self.has_window_surface {
                        Some(Arc::new(IsMaximizedAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                11 => {
                    if self.has_window_surface {
                        Some(Arc::new(IsTopmostAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                12 => {
                    if self.has_window_surface {
                        Some(Arc::new(SupportsMoveAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                13 => {
                    if self.has_window_surface {
                        Some(Arc::new(SupportsResizeAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                14 => {
                    if self.has_window_surface {
                        Some(Arc::new(AcceptsUserInputAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                // Native property attributes (dynamic): build once, then stream
                15 => {
                    if self.native_cache.is_none() {
                        let pairs = crate::map::collect_native_properties(&elem);
                        let attrs: Vec<Arc<dyn UiAttribute>> = pairs
                            .into_iter()
                            .map(|(name, value)| Arc::new(NativePropAttr { name, value }) as Arc<dyn UiAttribute>)
                            .collect();
                        self.native_cache = Some(attrs);
                        self.native_pos = 0;
                    }
                    match self.native_cache.as_ref().and_then(|v| v.get(self.native_pos)).cloned() {
                        Some(attr) => {
                            self.native_pos += 1;
                            Some(attr)
                        }
                        None => None,
                    }
                }
                _ => None,
            };
            self.idx = self.idx.saturating_add(1);
            match item {
                Some(attr) => return Some(attr),
                None => {
                    if self.idx > 15 && self.native_cache.is_some() {
                        // Continue streaming native cache until exhausted
                        if let Some(list) = self.native_cache.as_ref()
                            && self.native_pos < list.len()
                        {
                            // compensate index bump and yield next from cache
                            self.idx -= 1;
                            let attr = list[self.native_pos].clone();
                            self.native_pos += 1;
                            return Some(attr);
                        }
                    }
                    if self.idx > 15 && self.native_cache.is_none() {
                        // No native props at all
                        return None;
                    }
                    if self.idx > 15 && self.native_cache.as_ref().map(|v| self.native_pos >= v.len()).unwrap_or(false)
                    {
                        return None;
                    }
                    continue;
                }
            }
        }
    }
}
unsafe impl Send for AttrsIter {}

struct IsFocusedAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for IsFocusedAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "IsFocused"
    }
    fn value(&self) -> UiValue {
        let result = (|| -> Result<bool, crate::error::UiaError> {
            let v = crate::error::uia_api("IUIAutomationElement::CurrentHasKeyboardFocus", unsafe {
                self.elem.CurrentHasKeyboardFocus()
            })?;
            Ok(v.as_bool())
        })();
        UiValue::from(result.unwrap_or(false))
    }
}
unsafe impl Send for IsFocusedAttr {}
unsafe impl Sync for IsFocusedAttr {}

struct NativePropAttr {
    name: String,
    value: UiValue,
}
impl UiAttribute for NativePropAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Native
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn value(&self) -> UiValue {
        self.value.clone()
    }
}

// ---------------------------------------------------------------------------
// Application attribute types and iterator (module-level, lazy values)

struct AppRuntimeIdAttr {
    rid: String,
}
impl UiAttribute for AppRuntimeIdAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        platynui_core::ui::attribute_names::common::RUNTIME_ID
    }
    fn value(&self) -> UiValue {
        UiValue::from(self.rid.clone())
    }
}

struct AppProcessIdAttr {
    pid: i32,
}
impl UiAttribute for AppProcessIdAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }
    fn name(&self) -> &str {
        platynui_core::ui::attribute_names::application::PROCESS_ID
    }
    fn value(&self) -> UiValue {
        UiValue::from(self.pid as i64)
    }
}

struct AppNameAttr {
    pid: i32,
}
impl UiAttribute for AppNameAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        platynui_core::ui::attribute_names::application::NAME
    }
    fn value(&self) -> UiValue {
        if let Some(handle) = crate::map::open_process_query(self.pid) {
            if let Some(path) = crate::map::query_executable_path(handle) {
                let _ = unsafe { CloseHandle(handle) };
                if let Some(stem) = std::path::Path::new(&path).file_stem() {
                    return UiValue::from(stem.to_string_lossy().to_string());
                }
            } else {
                let _ = unsafe { CloseHandle(handle) };
            }
        }
        UiValue::from("")
    }
}

struct AppExecutablePathAttr {
    pid: i32,
}
impl UiAttribute for AppExecutablePathAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }
    fn name(&self) -> &str {
        platynui_core::ui::attribute_names::application::EXECUTABLE_PATH
    }
    fn value(&self) -> UiValue {
        if let Some(h) = crate::map::open_process_query(self.pid) {
            let out = crate::map::query_executable_path(h).map(UiValue::from).unwrap_or(UiValue::from(""));
            unsafe {
                let _ = CloseHandle(h);
            }
            return out;
        }
        UiValue::from("")
    }
}

struct AppCommandLineAttr {
    pid: i32,
}
impl UiAttribute for AppCommandLineAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }
    fn name(&self) -> &str {
        platynui_core::ui::attribute_names::application::COMMAND_LINE
    }
    fn value(&self) -> UiValue {
        if let Some(h) = crate::map::open_process_query(self.pid) {
            let out = crate::map::query_process_command_line(h).map(UiValue::from).unwrap_or(UiValue::Null);
            unsafe {
                let _ = CloseHandle(h);
            }
            return out;
        }
        UiValue::Null
    }
}

struct AppUserNameAttr {
    pid: i32,
}
impl UiAttribute for AppUserNameAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }
    fn name(&self) -> &str {
        platynui_core::ui::attribute_names::application::USER_NAME
    }
    fn value(&self) -> UiValue {
        if let Some(h) = crate::map::open_process_query(self.pid) {
            let out = crate::map::query_process_username(h).map(UiValue::from).unwrap_or(UiValue::from(""));
            unsafe {
                let _ = CloseHandle(h);
            }
            return out;
        }
        UiValue::from("")
    }
}

struct AppStartTimeAttr {
    pid: i32,
}
impl UiAttribute for AppStartTimeAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }
    fn name(&self) -> &str {
        platynui_core::ui::attribute_names::application::START_TIME
    }
    fn value(&self) -> UiValue {
        if let Some(h) = crate::map::open_process_query(self.pid) {
            let out = crate::map::query_process_start_time_iso8601(h).map(UiValue::from).unwrap_or(UiValue::from(""));
            unsafe {
                let _ = CloseHandle(h);
            }
            return out;
        }
        UiValue::from("")
    }
}

struct AppArchitectureAttr {
    pid: i32,
}
impl UiAttribute for AppArchitectureAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }
    fn name(&self) -> &str {
        platynui_core::ui::attribute_names::application::ARCHITECTURE
    }
    fn value(&self) -> UiValue {
        if let Some(h) = crate::map::open_process_query(self.pid) {
            if let Some(path) = crate::map::query_executable_path(h)
                && let Some(a) = crate::map::process_architecture_from_path(&path)
            {
                unsafe {
                    let _ = CloseHandle(h);
                }
                return UiValue::from(a);
            }
            let out = crate::map::process_architecture(h).map(UiValue::from).unwrap_or(UiValue::from("unknown"));
            unsafe {
                let _ = CloseHandle(h);
            }
            return out;
        }
        UiValue::from("unknown")
    }
}

struct AppAttrsIter {
    pid: i32,
    rid: String,
    idx: u8,
}
impl AppAttrsIter {
    fn new(pid: i32, rid: &str) -> Self {
        Self { pid, rid: rid.to_owned(), idx: 0 }
    }
}
impl Iterator for AppAttrsIter {
    type Item = Arc<dyn UiAttribute>;
    fn next(&mut self) -> Option<Self::Item> {
        let item = match self.idx {
            0 => Some(Arc::new(AppRuntimeIdAttr { rid: self.rid.clone() }) as Arc<dyn UiAttribute>),
            1 => Some(Arc::new(AppProcessIdAttr { pid: self.pid }) as Arc<dyn UiAttribute>),
            2 => Some(Arc::new(AppNameAttr { pid: self.pid }) as Arc<dyn UiAttribute>),
            3 => Some(Arc::new(AppExecutablePathAttr { pid: self.pid }) as Arc<dyn UiAttribute>),
            4 => Some(Arc::new(AppCommandLineAttr { pid: self.pid }) as Arc<dyn UiAttribute>),
            5 => Some(Arc::new(AppUserNameAttr { pid: self.pid }) as Arc<dyn UiAttribute>),
            6 => Some(Arc::new(AppStartTimeAttr { pid: self.pid }) as Arc<dyn UiAttribute>),
            7 => Some(Arc::new(AppArchitectureAttr { pid: self.pid }) as Arc<dyn UiAttribute>),
            _ => None,
        };
        self.idx = self.idx.saturating_add(1);
        item
    }
}
unsafe impl Send for AppAttrsIter {}

struct IsMinimizedAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for IsMinimizedAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "IsMinimized"
    }
    fn value(&self) -> UiValue {
        // Default false on errors/missing pattern
        let result = (|| -> Result<bool, crate::error::UiaError> {
            let unk = crate::error::uia_api("IUIAutomationElement::GetCurrentPattern(Window)", unsafe {
                self.elem.GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                    windows::Win32::UI::Accessibility::UIA_WindowPatternId.0,
                ))
            })?;
            let pat: IUIAutomationWindowPattern = crate::error::uia_api("IUnknown::cast(WindowPattern)", unk.cast())?;
            let state = crate::error::uia_api("IUIAutomationWindowPattern::CurrentWindowVisualState", unsafe {
                pat.CurrentWindowVisualState()
            })?;
            Ok(state == WindowVisualState_Minimized)
        })();
        UiValue::from(result.unwrap_or(false))
    }
}
unsafe impl Send for IsMinimizedAttr {}
unsafe impl Sync for IsMinimizedAttr {}

struct IsMaximizedAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for IsMaximizedAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "IsMaximized"
    }
    fn value(&self) -> UiValue {
        let result = (|| -> Result<bool, crate::error::UiaError> {
            let unk = crate::error::uia_api("IUIAutomationElement::GetCurrentPattern(Window)", unsafe {
                self.elem.GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                    windows::Win32::UI::Accessibility::UIA_WindowPatternId.0,
                ))
            })?;
            let pat: IUIAutomationWindowPattern = crate::error::uia_api("IUnknown::cast(WindowPattern)", unk.cast())?;
            let state = crate::error::uia_api("IUIAutomationWindowPattern::CurrentWindowVisualState", unsafe {
                pat.CurrentWindowVisualState()
            })?;
            Ok(state == WindowVisualState_Maximized)
        })();
        UiValue::from(result.unwrap_or(false))
    }
}
unsafe impl Send for IsMaximizedAttr {}
unsafe impl Sync for IsMaximizedAttr {}

struct IsTopmostAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for IsTopmostAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "IsTopmost"
    }
    fn value(&self) -> UiValue {
        let result = (|| -> Result<bool, crate::error::UiaError> {
            let unk = crate::error::uia_api("IUIAutomationElement::GetCurrentPattern(Window)", unsafe {
                self.elem.GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                    windows::Win32::UI::Accessibility::UIA_WindowPatternId.0,
                ))
            })?;
            let pat: IUIAutomationWindowPattern = crate::error::uia_api("IUnknown::cast(WindowPattern)", unk.cast())?;
            let v = crate::error::uia_api("IUIAutomationWindowPattern::CurrentIsTopmost", unsafe {
                pat.CurrentIsTopmost()
            })?;
            Ok(v.as_bool())
        })();
        UiValue::from(result.unwrap_or(false))
    }
}
unsafe impl Send for IsTopmostAttr {}
unsafe impl Sync for IsTopmostAttr {}

struct SupportsMoveAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for SupportsMoveAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "SupportsMove"
    }
    fn value(&self) -> UiValue {
        let result = (|| -> Result<bool, crate::error::UiaError> {
            let unk = crate::error::uia_api("IUIAutomationElement::GetCurrentPattern(Transform)", unsafe {
                self.elem.GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                    windows::Win32::UI::Accessibility::UIA_TransformPatternId.0,
                ))
            })?;
            let pat: IUIAutomationTransformPattern =
                crate::error::uia_api("IUnknown::cast(TransformPattern)", unk.cast())?;
            let v = crate::error::uia_api("IUIAutomationTransformPattern::CurrentCanMove", unsafe {
                pat.CurrentCanMove()
            })?;
            Ok(v.as_bool())
        })();
        UiValue::from(result.unwrap_or(false))
    }
}
unsafe impl Send for SupportsMoveAttr {}
unsafe impl Sync for SupportsMoveAttr {}

struct SupportsResizeAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for SupportsResizeAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "SupportsResize"
    }
    fn value(&self) -> UiValue {
        let result = (|| -> Result<bool, crate::error::UiaError> {
            let unk = crate::error::uia_api("IUIAutomationElement::GetCurrentPattern(Transform)", unsafe {
                self.elem.GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                    windows::Win32::UI::Accessibility::UIA_TransformPatternId.0,
                ))
            })?;
            let pat: IUIAutomationTransformPattern =
                crate::error::uia_api("IUnknown::cast(TransformPattern)", unk.cast())?;
            let v = crate::error::uia_api("IUIAutomationTransformPattern::CurrentCanResize", unsafe {
                pat.CurrentCanResize()
            })?;
            Ok(v.as_bool())
        })();
        UiValue::from(result.unwrap_or(false))
    }
}
unsafe impl Send for SupportsResizeAttr {}
unsafe impl Sync for SupportsResizeAttr {}

// ---------------------------------------------------------------------------
// Synthetic Application node for grouped view (Application -> Window)

pub struct ApplicationNode {
    pid: i32,
    root: windows::Win32::UI::Accessibility::IUIAutomationElement,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    self_weak: once_cell::sync::OnceCell<Weak<dyn UiNode>>,
    rid_cell: once_cell::sync::OnceCell<RuntimeId>,
    // no cached name required; compute on demand
}
unsafe impl Send for ApplicationNode {}
unsafe impl Sync for ApplicationNode {}

impl ApplicationNode {
    pub fn new(
        pid: i32,
        root: windows::Win32::UI::Accessibility::IUIAutomationElement,
        parent: &Arc<dyn UiNode>,
    ) -> Arc<Self> {
        let node = Arc::new(Self {
            pid,
            root,
            parent: Mutex::new(Some(Arc::downgrade(parent))),
            self_weak: once_cell::sync::OnceCell::new(),
            rid_cell: once_cell::sync::OnceCell::new(),
        });
        let arc: Arc<dyn UiNode> = node.clone();
        let _ = node.self_weak.set(Arc::downgrade(&arc));
        node
    }
}

impl UiNode for ApplicationNode {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }
    fn role(&self) -> &str {
        "Application"
    }
    fn name(&self) -> String {
        // Derive process name from executable path (file name)
        if let Some(handle) = crate::map::open_process_query(self.pid) {
            if let Some(path) = crate::map::query_executable_path(handle) {
                let _ = unsafe { CloseHandle(handle) };
                if let Some(stem) = std::path::Path::new(&path).file_stem() {
                    return stem.to_string_lossy().to_string();
                }
            } else {
                let _ = unsafe { CloseHandle(handle) };
            }
        }
        String::new()
    }
    fn runtime_id(&self) -> &RuntimeId {
        self.rid_cell.get_or_init(|| RuntimeId::from(format!("uia://app/{}", self.pid)))
    }
    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        match self.parent.lock() {
            Ok(g) => g.clone(),
            Err(_) => None,
        }
    }
    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        struct AppWindowsIter {
            // Hold no COM interfaces directly to keep the iterator Send; fetch walker on demand
            root: windows::Win32::UI::Accessibility::IUIAutomationElement,
            current: Option<windows::Win32::UI::Accessibility::IUIAutomationElement>,
            first: bool,
            parent: Option<Arc<dyn UiNode>>,
            pid: i32,
        }
        impl Iterator for AppWindowsIter {
            type Item = Arc<dyn UiNode>;
            fn next(&mut self) -> Option<Self::Item> {
                let walker = crate::com::raw_walker().ok()?;
                loop {
                    if self.first {
                        self.first = false;
                        self.current = unsafe { walker.GetFirstChildElement(&self.root).ok() };
                        self.current.as_ref()?;
                    } else if let Some(ref elem) = self.current {
                        let cur = elem.clone();
                        self.current = unsafe { walker.GetNextSiblingElement(&cur).ok() };
                        self.current.as_ref()?;
                    }
                    let elem = self.current.as_ref()?.clone();
                    let pid = crate::map::get_process_id(&elem).unwrap_or(-1);
                    if pid == self.pid {
                        let node = UiaNode::from_elem_with_scope(elem, crate::map::UiaIdScope::App { pid: self.pid });
                        if let Some(ref parent) = self.parent {
                            node.set_parent(parent);
                        }
                        UiaNode::init_self(&node);
                        return Some(node as Arc<dyn UiNode>);
                    }
                }
            }
        }
        unsafe impl Send for AppWindowsIter {}
        let parent = self.self_weak.get().and_then(|w| w.upgrade());
        Box::new(AppWindowsIter { root: self.root.clone(), current: None, first: true, parent, pid: self.pid })
    }
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        Box::new(AppAttrsIter::new(self.pid, self.runtime_id().as_str()))
    }
    fn supported_patterns(&self) -> Vec<PatternId> {
        Vec::new()
    }
    fn invalidate(&self) {
        // No-op: see UiaNode comment. Use "Refresh" via attributes for fresh values.
    }
    fn doc_order_key(&self) -> Option<u64> {
        Some(self.pid as u64)
    }
}
