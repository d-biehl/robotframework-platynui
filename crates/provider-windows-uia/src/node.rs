//! Windows UIAutomation node wrapper and iterators (no provider-side caching).
//!
//! Philosophy
//! - UiaNode should reflect the current UIA state. Heavy caching is left to the
//!   Runtime/XPath adapter (RuntimeXdmNode). We keep only identity fields that
//!   require references (`name()`/`runtime_id()`) in small OnceCells.

use std::sync::{Arc, Mutex, Weak};

use platynui_core::types::{Point as UiPoint, Rect};
use platynui_core::ui::pattern::{FocusableAction, PatternError, UiPattern, WindowSurfaceActions};
use platynui_core::ui::{Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiValue};
use windows::Win32::UI::Accessibility::{
    IUIAutomationTransformPattern, IUIAutomationWindowPattern, WindowVisualState_Maximized,
    WindowVisualState_Minimized,
};
use windows::core::Interface;

pub struct UiaNode {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    self_weak: once_cell::sync::OnceCell<Weak<dyn UiNode>>,
    // Minimal identity caches required by trait return types
    name_cell: once_cell::sync::OnceCell<String>,
    rid_cell: once_cell::sync::OnceCell<RuntimeId>,
}
unsafe impl Send for UiaNode {}
unsafe impl Sync for UiaNode {}

impl UiaNode {
    pub fn from_elem(elem: windows::Win32::UI::Accessibility::IUIAutomationElement) -> Arc<Self> {
        Arc::new(Self {
            elem,
            parent: Mutex::new(None),
            self_weak: once_cell::sync::OnceCell::new(),
            name_cell: once_cell::sync::OnceCell::new(),
            rid_cell: once_cell::sync::OnceCell::new(),
        })
    }
    pub fn set_parent(&self, parent: &Arc<dyn UiNode>) {
        *self.parent.lock().unwrap() = Some(Arc::downgrade(parent));
    }
    pub fn init_self(this: &Arc<Self>) {
        let arc: Arc<dyn UiNode> = this.clone();
        let _ = this.self_weak.set(Arc::downgrade(&arc));
    }
    fn as_ui_node(&self) -> Arc<dyn UiNode> {
        self.self_weak.get().and_then(|w| w.upgrade()).expect("self weak set")
    }
}

impl UiNode for UiaNode {
    fn namespace(&self) -> Namespace {
        unsafe {
            let is_control =
                self.elem.CurrentIsControlElement().map(|b| b.as_bool()).unwrap_or(true);
            if is_control {
                return Namespace::Control;
            }
            let is_content =
                self.elem.CurrentIsContentElement().map(|b| b.as_bool()).unwrap_or(false);
            if is_content { Namespace::Item } else { Namespace::Control }
        }
    }
    fn role(&self) -> &str {
        let ct = crate::map::get_control_type(&self.elem).unwrap_or(0);
        crate::map::control_type_to_role(ct)
    }
    fn name(&self) -> &str {
        self.name_cell.get_or_init(|| crate::map::get_name(&self.elem).unwrap_or_default()).as_str()
    }
    fn runtime_id(&self) -> &RuntimeId {
        self.rid_cell.get_or_init(|| {
            let s =
                crate::map::format_runtime_id(&self.elem).unwrap_or_else(|_| "uia://temp".into());
            RuntimeId::from(s)
        })
    }
    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent.lock().unwrap().clone()
    }
    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_> {
        let parent_arc = self.as_ui_node();
        Box::new(ElementChildrenIter::new(self.elem.clone(), parent_arc))
    }
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_> {
        Box::new(AttrsIter::new(self))
    }

    fn supported_patterns(&self) -> Vec<PatternId> {
        use windows::Win32::UI::Accessibility::*;
        let mut out = vec![FocusableAction::static_id()];
        let (has_window, has_transform) = unsafe {
            let has_window =
                self.elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_WindowPatternId.0)).is_ok();
            let has_transform =
                self.elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_TransformPatternId.0)).is_ok();
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
                    crate::error::uia_api(
                        "IUIAutomationElement::SetFocus",
                        unsafe { self.elem.SetFocus() },
                    )
                }
            }
            let es = ElemSend { elem: self.elem.clone() };
            let action = FocusableAction::new(move || unsafe {
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
                unsafe fn window_set_state(&self, state: WindowVisualState) -> Result<(), crate::error::UiaError> {
                    let unk = crate::error::uia_api(
                        "IUIAutomationElement::GetCurrentPattern(Window)",
                        unsafe { self.elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_WindowPatternId.0)) },
                    )?;
                    let pat: IUIAutomationWindowPattern =
                        crate::error::uia_api("IUnknown::cast(WindowPattern)", unk.cast())?;
                    crate::error::uia_api(
                        "IUIAutomationWindowPattern::SetWindowVisualState",
                        unsafe { pat.SetWindowVisualState(state) },
                    )
                }
                unsafe fn window_close(&self) -> Result<(), crate::error::UiaError> {
                    let unk = crate::error::uia_api(
                        "IUIAutomationElement::GetCurrentPattern(Window)",
                        unsafe { self.elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_WindowPatternId.0)) },
                    )?;
                    let pat: IUIAutomationWindowPattern =
                        crate::error::uia_api("IUnknown::cast(WindowPattern)", unk.cast())?;
                    crate::error::uia_api("IUIAutomationWindowPattern::Close", unsafe { pat.Close() })
                }
                unsafe fn transform_move(&self, x: f64, y: f64) -> Result<(), crate::error::UiaError> {
                    let unk = crate::error::uia_api(
                        "IUIAutomationElement::GetCurrentPattern(Transform)",
                        unsafe { self.elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_TransformPatternId.0)) },
                    )?;
                    let pat: IUIAutomationTransformPattern =
                        crate::error::uia_api("IUnknown::cast(TransformPattern)", unk.cast())?;
                    crate::error::uia_api("IUIAutomationTransformPattern::Move", unsafe { pat.Move(x, y) })
                }
                unsafe fn transform_resize(&self, w: f64, h: f64) -> Result<(), crate::error::UiaError> {
                    let unk = crate::error::uia_api(
                        "IUIAutomationElement::GetCurrentPattern(Transform)",
                        unsafe { self.elem.GetCurrentPattern(UIA_PATTERN_ID(UIA_TransformPatternId.0)) },
                    )?;
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
            let accepts_now = {
                // Compute a best-effort static snapshot for accepts_user_input (lazy recomputation can be added later)
                let enabled = crate::map::get_is_enabled(&self.elem).unwrap_or(false);
                let off = crate::map::get_is_offscreen(&self.elem).unwrap_or(false);
                enabled && !off
            };
            let actions = WindowSurfaceActions::new()
                .with_activate(move || unsafe {
                    e1.window_set_state(WindowVisualState_Normal)
                        .map_err(|e| PatternError::new(e.to_string()))
                })
                .with_minimize(move || unsafe {
                    e2.window_set_state(WindowVisualState_Minimized)
                        .map_err(|e| PatternError::new(e.to_string()))
                })
                .with_maximize(move || unsafe {
                    e3.window_set_state(WindowVisualState_Maximized)
                        .map_err(|e| PatternError::new(e.to_string()))
                })
                .with_restore(move || unsafe {
                    e4.window_set_state(WindowVisualState_Normal)
                        .map_err(|e| PatternError::new(e.to_string()))
                })
                .with_close(move || unsafe {
                    e5.window_close().map_err(|e| PatternError::new(e.to_string()))
                })
                .with_move_to(move |p| unsafe {
                    e_move
                        .transform_move(p.x(), p.y())
                        .map_err(|e| PatternError::new(e.to_string()))
                })
                .with_resize(move |s| unsafe {
                    e_resize
                        .transform_resize(s.width(), s.height())
                        .map_err(|e| PatternError::new(e.to_string()))
                })
                .with_accepts_user_input(move || Ok(Some(accepts_now)));
            return Some(Arc::new(actions) as Arc<dyn UiPattern>);
        }
        None
    }
    fn invalidate(&self) {}
}

pub(crate) struct ElementChildrenIter {
    walker: windows::Win32::UI::Accessibility::IUIAutomationTreeWalker,
    parent_elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
    current: Option<windows::Win32::UI::Accessibility::IUIAutomationElement>,
    first: bool,
    parent: Arc<dyn UiNode>,
}
impl ElementChildrenIter {
    pub fn new(
        parent_elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
        parent_node: Arc<dyn UiNode>,
    ) -> Self {
        let walker = crate::com::raw_walker().expect("walker");
        Self { walker, parent_elem, current: None, first: true, parent: parent_node }
    }
}
unsafe impl Send for ElementChildrenIter {}
impl Iterator for ElementChildrenIter {
    type Item = Arc<dyn UiNode>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.first {
            self.first = false;
            self.current = unsafe { self.walker.GetFirstChildElement(&self.parent_elem).ok() };
            if self.current.is_none() {
                return None;
            }
        } else if let Some(ref elem) = self.current {
            let cur = elem.clone();
            self.current = unsafe { self.walker.GetNextSiblingElement(&cur).ok() };
        } else {
            return None;
        }
        let elem = self.current.as_ref()?.clone();
        let node = UiaNode::from_elem(elem);
        node.set_parent(&self.parent);
        UiaNode::init_self(&node);
        Some(node as Arc<dyn UiNode>)
    }
}

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
        UiValue::from(crate::map::control_type_to_role(
            crate::map::get_control_type(&self.elem).unwrap_or(0),
        ))
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
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for RuntimeIdAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        "RuntimeId"
    }
    fn value(&self) -> UiValue {
        UiValue::from(
            crate::map::format_runtime_id(&self.elem).unwrap_or_else(|_| "uia://temp".into()),
        )
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
        UiValue::from(
            crate::map::get_bounding_rect(&self.elem).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)),
        )
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
            let r =
                crate::map::get_bounding_rect(&self.elem).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
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

struct AttrsIter<'a> {
    idx: u8,
    node: &'a UiaNode,
    has_window_surface: bool,
    native_cache: Option<Vec<Arc<dyn UiAttribute>>>,
    native_pos: usize,
}
impl<'a> AttrsIter<'a> {
    fn new(node: &'a UiaNode) -> Self {
        use windows::Win32::UI::Accessibility::*;
        let has_window_surface = unsafe {
            let has_window = node
                .elem
                .GetCurrentPattern(UIA_PATTERN_ID(UIA_WindowPatternId.0))
                .is_ok();
            let has_transform = node
                .elem
                .GetCurrentPattern(UIA_PATTERN_ID(UIA_TransformPatternId.0))
                .is_ok();
            has_window || has_transform
        };
        Self { idx: 0, node, has_window_surface, native_cache: None, native_pos: 0 }
    }
}
impl<'a> Iterator for AttrsIter<'a> {
    type Item = Arc<dyn UiAttribute>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let elem = self.node.elem.clone();
            let item = match self.idx {
                0 => Some(Arc::new(RoleAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>),
                1 => Some(Arc::new(NameAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>),
                2 => Some(Arc::new(RuntimeIdAttr { elem: elem.clone() }) as Arc<dyn UiAttribute>),
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
                // Native property attributes (dynamic): build once, then stream
                14 => {
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
                    if self.idx > 14 && self.native_cache.is_some() {
                        // Continue streaming native cache until exhausted
                        if let Some(list) = self.native_cache.as_ref() {
                            if self.native_pos < list.len() {
                                // compensate index bump and yield next from cache
                                self.idx -= 1;
                                let attr = list[self.native_pos].clone();
                                self.native_pos += 1;
                                return Some(attr);
                            }
                        }
                    }
                    if self.idx > 14 && self.native_cache.is_none() {
                        // No native props at all
                        return None;
                    }
                    if self.idx > 14 && self.native_cache.as_ref().map(|v| self.native_pos >= v.len()).unwrap_or(false) {
                        return None;
                    }
                    continue;
                }
            }
        }
    }
}
unsafe impl<'a> Send for AttrsIter<'a> {}

struct IsFocusedAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for IsFocusedAttr {
    fn namespace(&self) -> Namespace { Namespace::Control }
    fn name(&self) -> &str { "IsFocused" }
    fn value(&self) -> UiValue {
        let result = (|| -> Result<bool, crate::error::UiaError> {
            let v = crate::error::uia_api(
                "IUIAutomationElement::CurrentHasKeyboardFocus",
                unsafe { self.elem.CurrentHasKeyboardFocus() },
            )?;
            Ok(v.as_bool())
        })();
        UiValue::from(result.unwrap_or(false))
    }
}
unsafe impl Send for IsFocusedAttr {}
unsafe impl Sync for IsFocusedAttr {}

struct NativePropAttr { name: String, value: UiValue }
impl UiAttribute for NativePropAttr {
    fn namespace(&self) -> Namespace { Namespace::Native }
    fn name(&self) -> &str { &self.name }
    fn value(&self) -> UiValue { self.value.clone() }
}

struct IsMinimizedAttr {
    elem: windows::Win32::UI::Accessibility::IUIAutomationElement,
}
impl UiAttribute for IsMinimizedAttr {
    fn namespace(&self) -> Namespace { Namespace::Control }
    fn name(&self) -> &str { "IsMinimized" }
    fn value(&self) -> UiValue {
        // Default false on errors/missing pattern
        let result = (|| -> Result<bool, crate::error::UiaError> {
            let unk = crate::error::uia_api(
                "IUIAutomationElement::GetCurrentPattern(Window)",
                unsafe {
                    self.elem
                        .GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                            windows::Win32::UI::Accessibility::UIA_WindowPatternId.0,
                        ))
                },
            )?;
            let pat: IUIAutomationWindowPattern =
                crate::error::uia_api("IUnknown::cast(WindowPattern)", unk.cast())?;
            let state = crate::error::uia_api(
                "IUIAutomationWindowPattern::CurrentWindowVisualState",
                unsafe { pat.CurrentWindowVisualState() },
            )?;
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
    fn namespace(&self) -> Namespace { Namespace::Control }
    fn name(&self) -> &str { "IsMaximized" }
    fn value(&self) -> UiValue {
        let result = (|| -> Result<bool, crate::error::UiaError> {
            let unk = crate::error::uia_api(
                "IUIAutomationElement::GetCurrentPattern(Window)",
                unsafe {
                    self.elem
                        .GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                            windows::Win32::UI::Accessibility::UIA_WindowPatternId.0,
                        ))
                },
            )?;
            let pat: IUIAutomationWindowPattern =
                crate::error::uia_api("IUnknown::cast(WindowPattern)", unk.cast())?;
            let state = crate::error::uia_api(
                "IUIAutomationWindowPattern::CurrentWindowVisualState",
                unsafe { pat.CurrentWindowVisualState() },
            )?;
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
    fn namespace(&self) -> Namespace { Namespace::Control }
    fn name(&self) -> &str { "IsTopmost" }
    fn value(&self) -> UiValue {
        let result = (|| -> Result<bool, crate::error::UiaError> {
            let unk = crate::error::uia_api(
                "IUIAutomationElement::GetCurrentPattern(Window)",
                unsafe {
                    self.elem
                        .GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                            windows::Win32::UI::Accessibility::UIA_WindowPatternId.0,
                        ))
                },
            )?;
            let pat: IUIAutomationWindowPattern =
                crate::error::uia_api("IUnknown::cast(WindowPattern)", unk.cast())?;
            let v = crate::error::uia_api(
                "IUIAutomationWindowPattern::CurrentIsTopmost",
                unsafe { pat.CurrentIsTopmost() },
            )?;
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
    fn namespace(&self) -> Namespace { Namespace::Control }
    fn name(&self) -> &str { "SupportsMove" }
    fn value(&self) -> UiValue {
        let result = (|| -> Result<bool, crate::error::UiaError> {
            let unk = crate::error::uia_api(
                "IUIAutomationElement::GetCurrentPattern(Transform)",
                unsafe {
                    self.elem
                        .GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                            windows::Win32::UI::Accessibility::UIA_TransformPatternId.0,
                        ))
                },
            )?;
            let pat: IUIAutomationTransformPattern =
                crate::error::uia_api("IUnknown::cast(TransformPattern)", unk.cast())?;
            let v = crate::error::uia_api(
                "IUIAutomationTransformPattern::CurrentCanMove",
                unsafe { pat.CurrentCanMove() },
            )?;
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
    fn namespace(&self) -> Namespace { Namespace::Control }
    fn name(&self) -> &str { "SupportsResize" }
    fn value(&self) -> UiValue {
        let result = (|| -> Result<bool, crate::error::UiaError> {
            let unk = crate::error::uia_api(
                "IUIAutomationElement::GetCurrentPattern(Transform)",
                unsafe {
                    self.elem
                        .GetCurrentPattern(windows::Win32::UI::Accessibility::UIA_PATTERN_ID(
                            windows::Win32::UI::Accessibility::UIA_TransformPatternId.0,
                        ))
                },
            )?;
            let pat: IUIAutomationTransformPattern =
                crate::error::uia_api("IUnknown::cast(TransformPattern)", unk.cast())?;
            let v = crate::error::uia_api(
                "IUIAutomationTransformPattern::CurrentCanResize",
                unsafe { pat.CurrentCanResize() },
            )?;
            Ok(v.as_bool())
        })();
        UiValue::from(result.unwrap_or(false))
    }
}
unsafe impl Send for SupportsResizeAttr {}
unsafe impl Sync for SupportsResizeAttr {}
