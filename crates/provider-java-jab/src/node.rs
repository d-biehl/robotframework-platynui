//! Lazy `UiNode` implementation over JAB accessible contexts.
//!
//! Each standard attribute derives from a `getAccessibleContextInfo` snapshot
//! read **live** per access (see [`JabNode::info_opt`]) — never cached across
//! calls — so a state change (a click, an edit) shows on the next read, the
//! way the UIA and AT-SPI providers behave. One logical `attributes()` or
//! `children()` access costs a single round-trip because it derives everything
//! from one snapshot. Children are reached lazily via
//! `getAccessibleChildFromContext(parent, i)` — the enumeration index also
//! builds the RuntimeId child path, because the bridge's own `indexInParent`
//! is unreliable (spike finding: combo popups report `-1`, spinner editors
//! report shifted indices).

use crate::client::{ContextInfo, JabClient};
use crate::error::JabError;
use crate::ffi::{self, VmId};
use crate::handle::JabObject;
use crate::interfaces;
use crate::map;
use platynui_core::platform::{WindowId, WindowManager, java};
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::attribute_names::{
    activation_target, application, common, element, expandable, focusable, selectable, selection_provider,
    stateful_value, text_content, text_editable, toggleable, window_state,
};
use platynui_core::ui::{
    ActivatableAction, CloseableAction, FocusableAction, MaximizableAction, MinimizableAction, MovableAction,
    Namespace, PatternError, PatternName, ResizableAction, ResponsiveAction, RestorableAction, RuntimeId, UiAttribute,
    UiNode, UiPattern, UiValue, pattern_names, supported_patterns_value,
};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tracing::{debug, trace};

pub(crate) const TECHNOLOGY: &str = "JAB";

/// Attribute name the Windows `WindowManager` resolves window handles from
/// (`native:NativeWindowHandle`, same contract as the UIA provider).
pub(crate) const NATIVE_WINDOW_HANDLE: &str = "NativeWindowHandle";

/// Upper bound for the `SelectedItems` scan so a huge list cannot turn one
/// attribute read into thousands of bridge calls.
const SELECTED_ITEMS_SCAN_LIMIT: i32 = 512;

/// RuntimeId scope, mirroring the UIA provider: the same Java window appears
/// under the desktop and under its `app:Application` node, and ids must stay
/// unique across the two views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IdScope {
    Desktop,
    App { pid: u32 },
}

pub(crate) fn format_runtime_id(scope: IdScope, vm: VmId, hwnd: isize, index_path: &[i32]) -> String {
    let mut id = match scope {
        IdScope::Desktop => format!("jab://{vm}/0x{hwnd:X}"),
        IdScope::App { pid } => format!("jab://app/{pid}/{vm}/0x{hwnd:X}"),
    };
    for index in index_path {
        id.push('/');
        id.push_str(&index.to_string());
    }
    id
}

pub(crate) fn format_app_runtime_id(pid: u32) -> String {
    format!("jab://app/{pid}")
}

// ---------------------------------------------------------------------------
// Per-window DPI calibration (design decision 13)

/// Affine per-window transform from JAB's coordinate space into physical
/// desktop pixels. Java 8 is system-DPI-aware, so its coordinates are
/// DPI-virtualized whenever the window sits on a monitor whose scale differs
/// from the system DPI; PlatynUI runs Per-Monitor-V2 and works in physical
/// pixels. Comparing the window's JAB frame rect with `GetWindowRect` (both
/// describe the same rectangle) yields the correction — identity at 100 %
/// scaling (spike-verified byte-identical).
pub(crate) struct Calibration {
    client: Arc<JabClient>,
    window_ctx: Arc<JabObject>,
    hwnd: isize,
    transform: Mutex<Option<Transform>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Transform {
    scale_x: f64,
    scale_y: f64,
    offset_x: f64,
    offset_y: f64,
}

impl Transform {
    const IDENTITY: Self = Self { scale_x: 1.0, scale_y: 1.0, offset_x: 0.0, offset_y: 0.0 };

    fn apply(&self, (x, y, w, h): (i32, i32, i32, i32)) -> Rect {
        Rect::new(
            f64::from(x) * self.scale_x + self.offset_x,
            f64::from(y) * self.scale_y + self.offset_y,
            f64::from(w) * self.scale_x,
            f64::from(h) * self.scale_y,
        )
    }

    fn derive(jab: (i32, i32, i32, i32), physical: (i32, i32, i32, i32)) -> Self {
        if jab == physical {
            return Self::IDENTITY;
        }
        let (jx, jy, jw, jh) = jab;
        let (px, py, pw, ph) = physical;
        let scale_x = if jw > 0 { f64::from(pw) / f64::from(jw) } else { 1.0 };
        let scale_y = if jh > 0 { f64::from(ph) / f64::from(jh) } else { 1.0 };
        Self {
            scale_x,
            scale_y,
            offset_x: f64::from(px) - f64::from(jx) * scale_x,
            offset_y: f64::from(py) - f64::from(jy) * scale_y,
        }
    }
}

impl Calibration {
    fn new(client: Arc<JabClient>, window_ctx: Arc<JabObject>, hwnd: isize) -> Self {
        Self { client, window_ctx, hwnd, transform: Mutex::new(None) }
    }

    fn invalidate(&self) {
        *self.transform.lock().expect("calibration mutex poisoned") = None;
    }

    fn resolve(&self) -> Transform {
        if let Some(cached) = *self.transform.lock().expect("calibration mutex poisoned") {
            return cached;
        }
        let computed = self.compute();
        *self.transform.lock().expect("calibration mutex poisoned") = Some(computed);
        computed
    }

    fn compute(&self) -> Transform {
        let Some(jab_bounds) = self.client.context_info(&self.window_ctx).ok().and_then(|info| info.bounds) else {
            return Transform::IDENTITY;
        };
        let Some(physical) = window_rect(self.hwnd) else {
            return Transform::IDENTITY;
        };
        let transform = Transform::derive(jab_bounds, physical);
        if transform != Transform::IDENTITY {
            debug!(
                hwnd = format!("0x{:X}", self.hwnd),
                ?jab_bounds,
                ?physical,
                "JAB bounds are DPI-virtualized; applying per-window calibration"
            );
        }
        transform
    }

    fn apply(&self, bounds: (i32, i32, i32, i32)) -> Rect {
        self.resolve().apply(bounds)
    }
}

#[allow(unsafe_code)]
fn window_rect(hwnd: isize) -> Option<(i32, i32, i32, i32)> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let mut rect = RECT::default();
    // SAFETY: read-only query with a valid out-parameter; a stale HWND makes
    // the call fail, which maps to `None`.
    unsafe { GetWindowRect(HWND(hwnd as *mut core::ffi::c_void), &raw mut rect) }.ok()?;
    Some((rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top))
}

// ---------------------------------------------------------------------------
// JabNode

pub(crate) struct JabNode {
    client: Arc<JabClient>,
    window_manager: Option<Arc<dyn WindowManager>>,
    ctx: Arc<JabObject>,
    vm: VmId,
    hwnd: isize,
    scope: IdScope,
    /// Enumeration-index path from the top-level window down to this node;
    /// empty for the window itself.
    index_path: Arc<[i32]>,
    /// The parent's `role_en_US`, captured at construction (feeds the JList
    /// `label` → `item:ListItem` promotion without re-querying the parent).
    parent_role_en: Option<Arc<str>>,
    /// The tree parent's own context, captured at construction; feeds the
    /// per-cell `TableCell.*` resolution. The *bridge* parent cannot be used
    /// there: for JTable cells the JDK's AccessBridge answers the shared
    /// cell-renderer component, whose accessible parent is the
    /// `CellRendererPane`, not the table.
    parent_ctx: Option<Arc<JabObject>>,
    calibration: Arc<Calibration>,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    /// Strong ref to the parent that roots an off-tree chain (the live
    /// picker's hit-test result — see [`hit_test_node`]). Normal tree nodes
    /// leave this `None`; their parents are kept alive by the tree/consumer
    /// and only the `parent` `Weak` is used.
    parent_keepalive: Mutex<Option<Arc<dyn UiNode>>>,
    self_weak: OnceLock<Weak<dyn UiNode>>,
    runtime_id: OnceLock<RuntimeId>,
    role: OnceLock<(Namespace, String)>,
}

impl JabNode {
    /// Node for a Java top-level window (root of a JAB subtree).
    pub(crate) fn new_window(
        client: Arc<JabClient>,
        window_manager: Option<Arc<dyn WindowManager>>,
        vm: VmId,
        ctx: JabObject,
        hwnd: isize,
        scope: IdScope,
        parent: Option<&Arc<dyn UiNode>>,
    ) -> Arc<Self> {
        let ctx = Arc::new(ctx);
        let calibration = Arc::new(Calibration::new(Arc::clone(&client), Arc::clone(&ctx), hwnd));
        Self::build(
            client,
            window_manager,
            vm,
            ctx,
            hwnd,
            scope,
            Arc::from([].as_slice()),
            None,
            None,
            calibration,
            parent,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        client: Arc<JabClient>,
        window_manager: Option<Arc<dyn WindowManager>>,
        vm: VmId,
        ctx: Arc<JabObject>,
        hwnd: isize,
        scope: IdScope,
        index_path: Arc<[i32]>,
        parent_role_en: Option<Arc<str>>,
        parent_ctx: Option<Arc<JabObject>>,
        calibration: Arc<Calibration>,
        parent: Option<&Arc<dyn UiNode>>,
    ) -> Arc<Self> {
        let node = Arc::new(Self {
            client,
            window_manager,
            ctx,
            vm,
            hwnd,
            scope,
            index_path,
            parent_role_en,
            parent_ctx,
            calibration,
            parent: Mutex::new(parent.map(Arc::downgrade)),
            parent_keepalive: Mutex::new(None),
            self_weak: OnceLock::new(),
            runtime_id: OnceLock::new(),
            role: OnceLock::new(),
        });
        let arc: Arc<dyn UiNode> = node.clone();
        let _ = node.self_weak.set(Arc::downgrade(&arc));
        node
    }

    fn is_top_level(&self) -> bool {
        self.index_path.is_empty()
    }

    /// Pins `parent` alive so an off-tree chain stays walkable via `parent()`
    /// (whose stored ref is only a `Weak`). Chaining this from the deepest
    /// node up roots the whole ancestor chain in the returned leaf.
    fn hold_parent(&self, parent: Arc<dyn UiNode>) {
        *self.parent_keepalive.lock().expect("parent keepalive mutex poisoned") = Some(parent);
    }

    /// Fetch this node's context info **fresh** from the bridge on every call
    /// — deliberately not cached across calls.
    ///
    /// The runtime reuses the XDM node tree across `evaluate()` calls and, for
    /// a reused node, clears only its own attribute cache (`prepare_for_
    /// evaluation`) — it does not call `UiNode::invalidate()`. So a node that
    /// cached its info would serve a stale snapshot forever (a click's effect
    /// would never show on a re-read). Reading live per call matches the UIA
    /// provider (whose attributes read from COM on demand) and the AT-SPI
    /// provider (fresh resolver per `attributes()` call). JAB calls are cheap
    /// (~ms), and one logical `attributes()`/`children()` access takes a
    /// single round-trip because it derives everything from one snapshot.
    fn info_opt(&self) -> Option<Arc<ContextInfo>> {
        match self.client.context_info(&self.ctx) {
            Ok(info) => Some(Arc::new(info)),
            Err(err) => {
                trace!(runtime_id = %self.runtime_id(), %err, "getAccessibleContextInfo failed");
                None
            }
        }
    }

    fn resolve_role(&self) -> &(Namespace, String) {
        self.role.get_or_init(|| match self.info_opt() {
            Some(info) => {
                map::map_role(&info.role_en_us, self.parent_role_en.as_deref(), info.states, self.is_top_level())
            }
            None => (Namespace::Control, "Unknown".to_string()),
        })
    }

    fn supported_patterns_for(&self, info: &ContextInfo) -> Vec<PatternName> {
        advertised_patterns(info, self.is_top_level())
    }
}

/// Patterns a node advertises, derived purely from its context info and
/// top-level status.
///
/// `TEXT_EDITABLE` is advertised as a capability marker (see
/// `pattern_names::TEXT_EDITABLE`) — `pattern_by_name` deliberately returns no
/// instance for it, because text is entered via synthesized keyboard input.
fn advertised_patterns(info: &ContextInfo, is_top_level: bool) -> Vec<PatternName> {
    let mut patterns = Vec::new();
    if info.states.focusable || info.states.focused {
        patterns.push(PatternName::from(pattern_names::FOCUSABLE));
    }
    if info.has_interface(ffi::INTERFACE_TEXT) && info.states.editable {
        patterns.push(PatternName::from(pattern_names::TEXT_EDITABLE));
    }
    if is_top_level {
        patterns.push(PatternName::from(pattern_names::ACTIVATABLE));
        patterns.push(PatternName::from(pattern_names::MINIMIZABLE));
        patterns.push(PatternName::from(pattern_names::MAXIMIZABLE));
        patterns.push(PatternName::from(pattern_names::RESTORABLE));
        patterns.push(PatternName::from(pattern_names::CLOSEABLE));
        patterns.push(PatternName::from(pattern_names::MOVABLE));
        patterns.push(PatternName::from(pattern_names::RESIZABLE));
        patterns.push(PatternName::from(pattern_names::RESPONSIVE));
    }
    patterns
}

impl UiNode for JabNode {
    fn namespace(&self) -> Namespace {
        self.resolve_role().0
    }

    fn role(&self) -> &str {
        &self.resolve_role().1
    }

    fn name(&self) -> String {
        self.info_opt().map(|info| info.name.clone()).unwrap_or_default()
    }

    fn runtime_id(&self) -> &RuntimeId {
        self.runtime_id
            .get_or_init(|| RuntimeId::from(format_runtime_id(self.scope, self.vm, self.hwnd, &self.index_path)))
    }

    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent.lock().ok()?.clone()
    }

    fn has_children(&self) -> bool {
        self.info_opt().is_some_and(|info| info.children_count > 0)
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        let Some(info) = self.info_opt() else {
            return Box::new(std::iter::empty());
        };
        let parent = self.self_weak.get().and_then(Weak::upgrade);
        Box::new(ChildIter {
            client: Arc::clone(&self.client),
            window_manager: self.window_manager.clone(),
            ctx: Arc::clone(&self.ctx),
            vm: self.vm,
            hwnd: self.hwnd,
            scope: self.scope,
            index_path: Arc::clone(&self.index_path),
            role_en: Arc::from(info.role_en_us.as_str()),
            calibration: Arc::clone(&self.calibration),
            parent,
            count: info.children_count.max(0),
            next_index: 0,
        })
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        let rid = self.runtime_id().as_str().to_string();
        let (namespace, role) = self.resolve_role().clone();
        let Some(info) = self.info_opt() else {
            // Degraded surface: identification only, so the node stays
            // addressable while the JVM is unresponsive.
            let attrs: Vec<Arc<dyn UiAttribute>> = vec![
                static_attr(namespace, common::ROLE, UiValue::from(role)),
                static_attr(namespace, common::RUNTIME_ID, UiValue::from(rid)),
                static_attr(namespace, common::TECHNOLOGY, UiValue::from(TECHNOLOGY)),
            ];
            return Box::new(attrs.into_iter());
        };

        let lazy = Arc::new(LazyAttrs {
            client: Arc::clone(&self.client),
            ctx: Arc::clone(&self.ctx),
            info: Arc::clone(&info),
            calibration: Arc::clone(&self.calibration),
            window_manager: self.window_manager.clone(),
            owner: self.self_weak.get().cloned(),
            is_top_level: self.is_top_level(),
        });
        let states = info.states;
        // Standard attributes live in the Control namespace regardless of the
        // node's own namespace (item: nodes included).
        let ns = Namespace::Control;

        let mut attrs: Vec<Arc<dyn UiAttribute>> = vec![
            static_attr(ns, common::ROLE, UiValue::from(role.clone())),
            static_attr(ns, common::NAME, UiValue::from(info.name.clone())),
            static_attr(ns, common::RUNTIME_ID, UiValue::from(rid)),
            static_attr(ns, common::TECHNOLOGY, UiValue::from(TECHNOLOGY)),
            static_attr(ns, element::IS_ENABLED, UiValue::from(states.enabled)),
            static_attr(ns, element::IS_VISIBLE, UiValue::from(states.visible && states.showing)),
            static_attr(ns, element::IS_IN_VIEW, UiValue::from(states.showing)),
            static_attr(ns, focusable::IS_FOCUSED, UiValue::from(states.focused)),
            static_attr(ns, common::SUPPORTED_PATTERNS, supported_patterns_value(&self.supported_patterns_for(&info))),
        ];

        // Descendants expose bounds only when JAB reports a real rect (the
        // hidden-element sentinel maps to "no Bounds"); top-level windows always
        // expose bounds — sourced from the window manager (see `bounds_rect`).
        if info.bounds.is_some() || self.is_top_level() {
            attrs.push(Arc::new(BoundsAttr { lazy: Arc::clone(&lazy), kind: BoundsKind::Bounds }));
            attrs.push(Arc::new(BoundsAttr { lazy: Arc::clone(&lazy), kind: BoundsKind::ActivationPoint }));
        }

        if info.has_interface(ffi::INTERFACE_TEXT) {
            attrs.push(Arc::new(TextAttr { lazy: Arc::clone(&lazy) }));
            attrs.push(static_attr(ns, text_editable::IS_READ_ONLY, UiValue::from(!states.editable)));
        }

        if matches!(info.role_en_us.as_str(), "check box" | "radio button" | "toggle button") {
            let toggle_state = if states.checked { "On" } else { "Off" };
            attrs.push(static_attr(ns, toggleable::TOGGLE_STATE, UiValue::from(toggle_state)));
        }

        if info.has_interface(ffi::INTERFACE_VALUE)
            && matches!(info.role_en_us.as_str(), "slider" | "scroll bar" | "spinbox" | "progress bar")
        {
            attrs.push(Arc::new(ValueAttr { lazy: Arc::clone(&lazy), kind: ValueKind::Current }));
            attrs.push(Arc::new(ValueAttr { lazy: Arc::clone(&lazy), kind: ValueKind::Minimum }));
            attrs.push(Arc::new(ValueAttr { lazy: Arc::clone(&lazy), kind: ValueKind::Maximum }));
        }

        if states.selectable || states.selected {
            attrs.push(static_attr(ns, selectable::IS_SELECTED, UiValue::from(states.selected)));
        }

        if info.has_interface(ffi::INTERFACE_SELECTION) {
            attrs.push(Arc::new(SelectedItemsAttr {
                lazy: Arc::clone(&lazy),
                scope: self.scope,
                vm: self.vm,
                hwnd: self.hwnd,
                index_path: Arc::clone(&self.index_path),
            }));
            attrs.push(static_attr(ns, selection_provider::CAN_SELECT_MULTIPLE, UiValue::from(states.multiselectable)));
        }

        if states.expandable || states.expanded {
            attrs.push(static_attr(ns, expandable::IS_EXPANDED, UiValue::from(states.expanded)));
            attrs.push(static_attr(ns, expandable::CAN_EXPAND, UiValue::from(states.expandable)));
        }

        if self.is_top_level() {
            attrs.push(Arc::new(IsActiveAttr { lazy: Arc::clone(&lazy) }));
            attrs.push(static_attr(ns, window_state::IS_MODAL, UiValue::from(states.modal)));
            attrs.push(static_attr(
                Namespace::Native,
                NATIVE_WINDOW_HANDLE,
                UiValue::from(i64::try_from(self.hwnd).unwrap_or_default()),
            ));
            push_jvm_classification_attrs(&mut attrs, self.hwnd);
        }

        // Raw originals for debugging and native-level selectors.
        attrs.push(static_attr(Namespace::Native, "Role", UiValue::from(info.role_en_us.clone())));
        attrs.push(static_attr(Namespace::Native, "LocalizedRole", UiValue::from(info.role_localized.clone())));
        attrs.push(static_attr(Namespace::Native, "States", UiValue::from(info.states_en_us.clone())));
        attrs.push(static_attr(Namespace::Native, "Description", UiValue::from(info.description.clone())));
        attrs.push(static_attr(Namespace::Native, "IndexInParent", UiValue::from(i64::from(info.index_in_parent))));
        attrs.push(static_attr(
            Namespace::Native,
            "Interfaces",
            UiValue::from(ffi::interface_names(info.interfaces).into_iter().map(String::from).collect::<Vec<_>>()),
        ));

        // Interface-property projection (jab-interface-attributes): the
        // container-level tier, gated by the interfaces bitfield.
        interfaces::append_interface_attributes(&mut attrs, &self.client, &self.ctx, info.interfaces);

        // Per-cell tier: listed only on children of a table so the Inspector's
        // attribute panel can show them. The captured parent role is a free
        // gate (no bridge call); values resolve lazily per read, so a walk
        // that does not read them still issues no per-cell calls.
        if self.parent_role_en.as_deref() == Some("table")
            && let (Some(parent_ctx), Some(&index)) = (&self.parent_ctx, self.index_path.last())
            && index >= 0
        {
            interfaces::append_cell_attributes(&mut attrs, &self.client, parent_ctx, index);
        }

        Box::new(attrs.into_iter())
    }

    fn attribute(&self, namespace: Namespace, name: &str) -> Option<Arc<dyn UiAttribute>> {
        if namespace == Namespace::Native {
            // Targeted per-cell lookup — the expensive tier, deliberately
            // absent from `attributes()` enumeration. Resolvability is checked
            // here so a node whose parent is not a table simply has no
            // `TableCell.*` attributes (documented fallback); the returned
            // attribute still re-reads live per `value()` access. The cell is
            // addressed through the *tree* parent's context plus this node's
            // enumeration index (see the `parent_ctx` field docs).
            if let Some(property) = interfaces::cell_property(name) {
                let index = self.index_path.last().copied()?;
                let parent_ctx = self.parent_ctx.clone()?;
                interfaces::resolve_cell_info(&self.client, &parent_ctx, index)?;
                return Some(Arc::new(interfaces::TableCellAttr {
                    client: Arc::clone(&self.client),
                    parent_ctx,
                    index,
                    property,
                }));
            }
            // Container-level interface property: honor the bitfield gate on a
            // live snapshot (degraded JVM ⇒ no info ⇒ no interface attribute).
            if let Some((bit, property)) = interfaces::container_property(name) {
                if bit != 0 && !self.info_opt()?.has_interface(bit) {
                    return None;
                }
                return Some(interfaces::interface_attr(&self.client, &self.ctx, property));
            }
        }
        // Everything else: the trait-default scan over the enumeration.
        self.attributes().find(|attr| attr.namespace() == namespace && attr.name() == name)
    }

    fn supported_patterns(&self) -> Vec<PatternName> {
        self.info_opt().map(|info| self.supported_patterns_for(&info)).unwrap_or_default()
    }

    fn pattern_by_name(&self, pattern: &PatternName) -> Option<Arc<dyn UiPattern>> {
        let id = pattern.as_str();
        let info = self.info_opt()?;

        if id == pattern_names::FOCUSABLE {
            if !(info.states.focusable || info.states.focused) {
                return None;
            }
            let client = Arc::clone(&self.client);
            let ctx = Arc::clone(&self.ctx);
            return Some(Arc::new(FocusableAction::new(move || {
                client.request_focus(&ctx).map_err(PatternError::from)
            })));
        }

        // `TEXT_EDITABLE` is deliberately absent here: it is a capability
        // marker only (see `pattern_names::TEXT_EDITABLE`). It stays advertised
        // in `supported_patterns_for`, but text is entered via synthesized
        // keyboard input — never through a JAB write.

        let is_window_pattern = matches!(
            id,
            x if x == pattern_names::ACTIVATABLE
                || x == pattern_names::MINIMIZABLE
                || x == pattern_names::MAXIMIZABLE
                || x == pattern_names::RESTORABLE
                || x == pattern_names::CLOSEABLE
                || x == pattern_names::MOVABLE
                || x == pattern_names::RESIZABLE
                || x == pattern_names::RESPONSIVE
        );
        if is_window_pattern {
            if !self.is_top_level() {
                return None;
            }
            let core = Arc::new(JabWindowSurface {
                node: self.self_weak.get().cloned()?,
                client: Arc::clone(&self.client),
                vm: self.vm,
                window_manager: self.window_manager.clone(),
            });
            return Some(make_window_pattern(id, &core));
        }

        None
    }

    fn is_valid(&self) -> bool {
        // Cheap liveness probe: `isSameObject(ctx, ctx)` answers TRUE exactly
        // while the JVM still holds the reference — much lighter than a full
        // 6 KB context-info fetch.
        self.client.is_same(&self.ctx, &self.ctx).unwrap_or(false)
    }

    fn invalidate(&self) {
        // Context info is read live per access, so there is no attribute cache
        // to clear here. The per-window DPI calibration is the only cached
        // state; drop it so a moved/rescaled window recalibrates on next read.
        if self.is_top_level() {
            self.calibration.invalidate();
        }
    }
}

struct ChildIter {
    client: Arc<JabClient>,
    window_manager: Option<Arc<dyn WindowManager>>,
    ctx: Arc<JabObject>,
    vm: VmId,
    hwnd: isize,
    scope: IdScope,
    index_path: Arc<[i32]>,
    role_en: Arc<str>,
    calibration: Arc<Calibration>,
    parent: Option<Arc<dyn UiNode>>,
    count: i32,
    next_index: i32,
}

impl Iterator for ChildIter {
    type Item = Arc<dyn UiNode>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.next_index < self.count {
            let index = self.next_index;
            self.next_index += 1;
            match self.client.child(&self.ctx, index) {
                Ok(Some(child_ctx)) => {
                    let mut path = Vec::with_capacity(self.index_path.len() + 1);
                    path.extend_from_slice(&self.index_path);
                    path.push(index);
                    let node = JabNode::build(
                        Arc::clone(&self.client),
                        self.window_manager.clone(),
                        self.vm,
                        Arc::new(child_ctx),
                        self.hwnd,
                        self.scope,
                        Arc::from(path.as_slice()),
                        Some(Arc::clone(&self.role_en)),
                        Some(Arc::clone(&self.ctx)),
                        Arc::clone(&self.calibration),
                        self.parent.as_ref(),
                    );
                    return Some(node as Arc<dyn UiNode>);
                }
                Ok(None) => {
                    trace!(index, "child came back null; skipped");
                }
                Err(err) => {
                    debug!(index, %err, "child enumeration aborted");
                    return None;
                }
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Hit-testing (element_at_point reveal chain)

/// Upper bound for the ancestor chain walked from a hit context up to its
/// window root (`getAccessibleParentFromContext` loop).
const HIT_CHAIN_MAX_DEPTH: usize = 64;
/// Upper bound on the per-level child scan while mapping a hit context's
/// ancestor chain to enumeration indices.
const HIT_CHAIN_CHILD_SCAN_LIMIT: i32 = 1024;

/// Geometric fallback for a `getAccessibleContextAt` that answers nothing.
///
/// The JDK implements the native hit-test for Swing/AWT apps via
/// `EventQueueMonitor.getAccessibleAt`, which returns null for **every** point
/// until the target JVM has seen at least one mouse event over one of its
/// windows (`currentMousePosition == null` — screen readers never notice
/// because they hit-test on mouse-move). For a fresh JVM the pointer never
/// touched, descend geometrically instead: from the window context, pick the
/// first showing child whose calibrated bounds contain the point (AWT child
/// order approximates z-order — index 0 is topmost, which also makes menu
/// overlays win over the content beneath them), until no child contains it.
/// `Ok(None)` means no child of the window contains the point (frame area).
pub(crate) fn geometric_hit(
    client: &Arc<JabClient>,
    window_ctx: &JabObject,
    hwnd: isize,
    point: Point,
) -> Option<JabObject> {
    let window_info = client.context_info(window_ctx).ok()?;
    let transform = match (window_info.bounds, window_rect(hwnd)) {
        (Some(jab), Some(physical)) => Transform::derive(jab, physical),
        _ => Transform::IDENTITY,
    };

    let mut current: Option<JabObject> = None;
    let mut children_count = window_info.children_count;
    for _ in 0..HIT_CHAIN_MAX_DEPTH {
        let parent: &JabObject = current.as_ref().unwrap_or(window_ctx);
        let count = children_count.clamp(0, HIT_CHAIN_CHILD_SCAN_LIMIT);
        let mut matched: Option<(JabObject, i32)> = None;
        for index in 0..count {
            let Ok(Some(child)) = client.child(parent, index) else { continue };
            let Ok(info) = client.context_info(&child) else { continue };
            let Some(bounds) = info.bounds else { continue };
            if !info.states.showing {
                continue;
            }
            if transform.apply(bounds).contains(point) {
                matched = Some((child, info.children_count));
                break;
            }
        }
        match matched {
            Some((child, count)) => {
                children_count = count;
                current = Some(child);
            }
            None => break,
        }
    }
    current
}

/// Wraps a hit-test result (`getAccessibleContextAt`) in a reveal-ready node.
///
/// The picked context becomes a `JabNode` scoped `IdScope::App { pid }` with a
/// strong parent chain up to `app:Application`, so a consumer can walk
/// `parent()` and every level carries the same RuntimeId top-down traversal
/// produces. JAB's own `indexInParent` is unreliable (see the module docs), so
/// the enumeration-index path is recovered differently: the hit's ancestor
/// chain (`getAccessibleParentFromContext`) is matched level by level against
/// one bounded top-down re-walk of the owning window using `isSameObject`.
///
/// Documented fallback: when the chain cannot be matched (the JVM mutated the
/// subtree between hit-test and re-walk), the result is a parentless node
/// scoped to the window with a best-effort id — the picker still highlights
/// it, only tree-reveal degrades.
pub(crate) fn hit_test_node(
    client: &Arc<JabClient>,
    window_manager: Option<Arc<dyn WindowManager>>,
    vm: VmId,
    window_ctx: JabObject,
    hwnd: isize,
    pid: u32,
    hit: JabObject,
    exclusions: Option<Arc<dyn crate::provider::WindowExclusions>>,
) -> Arc<dyn UiNode> {
    let scope = IdScope::App { pid };
    let app: Arc<dyn UiNode> = JabAppNode::orphan(pid, Arc::clone(client), window_manager.clone(), exclusions);
    let window =
        JabNode::new_window(Arc::clone(client), window_manager.clone(), vm, window_ctx, hwnd, scope, Some(&app));
    window.hold_parent(app);

    if client.is_same(&window.ctx, &hit).unwrap_or(false) {
        return window;
    }

    // The hit's ancestor chain, deepest first, up to (exclusive) the window
    // root. `chain[0]` stays the hit itself for the fallback path.
    let mut chain: Vec<JabObject> = vec![hit];
    let mut reached_window = false;
    for _ in 0..HIT_CHAIN_MAX_DEPTH {
        match client.parent(chain.last().expect("chain is never empty")) {
            Ok(Some(parent)) => {
                if client.is_same(&parent, &window.ctx).unwrap_or(false) {
                    reached_window = true;
                    break;
                }
                chain.push(parent);
            }
            _ => break,
        }
    }

    let resolved = if reached_window { descend_to_hit(client, &window, &chain) } else { None };
    resolved.map_or_else(
        || {
            debug!(
                hwnd = format!("0x{hwnd:X}"),
                reached_window, "hit context could not be mapped to an enumeration path; tree-reveal degrades"
            );
            hit_fallback_node(
                client,
                window_manager,
                vm,
                hwnd,
                scope,
                &window,
                chain.into_iter().next().expect("chain holds at least the hit"),
            )
        },
        |node| node as Arc<dyn UiNode>,
    )
}

/// Walk down from `window`, matching each level of `chain` (stored deepest
/// first, so iterated in reverse) to its enumeration index via `isSameObject`.
/// Every constructed node pins its parent, so the returned leaf roots the
/// whole ancestor chain.
fn descend_to_hit(client: &Arc<JabClient>, window: &Arc<JabNode>, chain: &[JabObject]) -> Option<Arc<JabNode>> {
    let mut cur = Arc::clone(window);
    for target in chain.iter().rev() {
        let info = client.context_info(&cur.ctx).ok()?;
        let count = info.children_count.clamp(0, HIT_CHAIN_CHILD_SCAN_LIMIT);
        let parent_role: Arc<str> = Arc::from(info.role_en_us.as_str());
        let mut matched: Option<Arc<JabNode>> = None;
        for index in 0..count {
            let child = match client.child(&cur.ctx, index) {
                Ok(Some(child)) => child,
                Ok(None) => continue,
                Err(_) => return None,
            };
            if client.is_same(&child, target).unwrap_or(false) {
                let mut path = Vec::with_capacity(cur.index_path.len() + 1);
                path.extend_from_slice(&cur.index_path);
                path.push(index);
                let parent_dyn: Arc<dyn UiNode> = Arc::clone(&cur) as Arc<dyn UiNode>;
                let node = JabNode::build(
                    Arc::clone(client),
                    cur.window_manager.clone(),
                    cur.vm,
                    Arc::new(child),
                    cur.hwnd,
                    cur.scope,
                    Arc::from(path.as_slice()),
                    Some(parent_role),
                    Some(Arc::clone(&cur.ctx)),
                    Arc::clone(&cur.calibration),
                    Some(&parent_dyn),
                );
                node.hold_parent(parent_dyn);
                matched = Some(node);
                break;
            }
        }
        cur = matched?;
    }
    Some(cur)
}

/// Documented fallback for an unmatched hit: a parentless node scoped to the
/// window with a best-effort id (built from the raw handle — meaningless for
/// identity, but distinct per pick). The picker can highlight it; tree-reveal
/// degrades.
fn hit_fallback_node(
    client: &Arc<JabClient>,
    window_manager: Option<Arc<dyn WindowManager>>,
    vm: VmId,
    hwnd: isize,
    scope: IdScope,
    window: &Arc<JabNode>,
    hit: JabObject,
) -> Arc<dyn UiNode> {
    let handle = hit.handle();
    // The placeholder index path keeps `is_top_level()` (and with it the
    // window-pattern surface) off; it never reaches the RuntimeId, which is
    // pre-seeded below.
    let node = JabNode::build(
        Arc::clone(client),
        window_manager,
        vm,
        Arc::new(hit),
        hwnd,
        scope,
        Arc::from([-1].as_slice()),
        None,
        None,
        Arc::clone(&window.calibration),
        None,
    );
    let _ =
        node.runtime_id.set(RuntimeId::from(format!("{}/hit/{handle:#X}", format_runtime_id(scope, vm, hwnd, &[]))));
    node
}

// ---------------------------------------------------------------------------
// Attributes

fn static_attr(namespace: Namespace, name: &'static str, value: UiValue) -> Arc<dyn UiAttribute> {
    Arc::new(StaticAttr { namespace, name, value })
}

/// JVM classification facts (java-app-classification) on a top-level window:
/// a window the bridge serves is by definition JVM-backed and reachable
/// through native accessibility; the toolkit comes from the window class (a
/// JavaFX window served through the bridge reports JavaFX).
///
/// The agent-presence fact is reported here too, so the same facts are
/// observable no matter which provider serves a JVM window — a user comparing
/// a JAB-served and a UIA-served Java window should not find the answer
/// missing on one of them. It is a single `stat` on one path and never a
/// connection: reporting that an agent exists must not be what puts one there.
fn push_jvm_classification_attrs(attrs: &mut Vec<Arc<dyn UiAttribute>>, hwnd: isize) {
    let toolkit = window_class_of(hwnd)
        .as_deref()
        .and_then(java::JavaToolkit::from_window_class)
        .unwrap_or(java::JavaToolkit::Unknown);
    attrs.push(static_attr(Namespace::Native, java::IS_JVM_ATTRIBUTE, UiValue::from(true)));
    attrs.push(static_attr(Namespace::Native, java::JVM_TOOLKIT_ATTRIBUTE, UiValue::from(toolkit.label())));
    attrs.push(static_attr(Namespace::Native, java::JVM_ACCESSIBILITY_REACHABLE_ATTRIBUTE, UiValue::from(true)));
    if let Some(pid) = process_id_of(hwnd) {
        attrs.push(static_attr(
            Namespace::Native,
            java::JVM_AGENT_PRESENT_ATTRIBUTE,
            UiValue::from(platynui_java_agent::handshake::agent_present(pid)),
        ));
    }
}

/// The owning process of a top-level window; `None` when the window is gone.
#[allow(unsafe_code)]
fn process_id_of(hwnd: isize) -> Option<u32> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let handle = HWND(hwnd as *mut core::ffi::c_void);
    let mut pid = 0u32;
    // SAFETY: read-only query; `pid` is a valid out-pointer.
    let thread = unsafe { GetWindowThreadProcessId(handle, Some(&raw mut pid)) };
    (thread != 0 && pid != 0).then_some(pid)
}

/// The top-level window's class name (`GetClassNameW`), the toolkit
/// discriminator of the JVM classification facts. `None` when the window is
/// gone or reports no class.
#[allow(unsafe_code)]
fn window_class_of(hwnd: isize) -> Option<String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;

    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    let mut buffer = [0u16; 256];
    // SAFETY: read-only query; `buffer` is a valid out-buffer of the given length.
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if len <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..usize::try_from(len).unwrap_or(0)]))
}

struct StaticAttr {
    namespace: Namespace,
    name: &'static str,
    value: UiValue,
}

impl UiAttribute for StaticAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        self.name
    }

    fn value(&self) -> UiValue {
        self.value.clone()
    }
}

/// Shared context for the attributes whose value still needs bridge (or
/// window-manager) round-trips at `value()` time.
struct LazyAttrs {
    client: Arc<JabClient>,
    ctx: Arc<JabObject>,
    info: Arc<ContextInfo>,
    calibration: Arc<Calibration>,
    window_manager: Option<Arc<dyn WindowManager>>,
    owner: Option<Weak<dyn UiNode>>,
    is_top_level: bool,
}

impl LazyAttrs {
    /// Desktop-coordinate bounds of this node.
    ///
    /// Top-level windows resolve through the injected `WindowManager` (live
    /// `GetWindowRect`), mirroring the AT-SPI provider: JAB's own frame bounds
    /// lag out-of-band `SetWindowPos` moves, and the window manager is the
    /// authoritative source for window position anyway. Everything below the
    /// window uses the JAB-reported, DPI-calibrated bounds.
    fn bounds_rect(&self) -> Option<Rect> {
        if self.is_top_level
            && let Some(wm) = &self.window_manager
            && let Some(node) = self.owner.as_ref().and_then(Weak::upgrade)
            && let Ok(wid) = wm.resolve_window(node.as_ref())
            && let Ok(rect) = wm.bounds(wid, None)
        {
            return Some(rect);
        }
        self.info.bounds.map(|bounds| self.calibration.apply(bounds))
    }
}

#[derive(Clone, Copy)]
enum BoundsKind {
    Bounds,
    ActivationPoint,
}

struct BoundsAttr {
    lazy: Arc<LazyAttrs>,
    kind: BoundsKind,
}

impl UiAttribute for BoundsAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }

    fn name(&self) -> &str {
        match self.kind {
            BoundsKind::Bounds => element::BOUNDS,
            BoundsKind::ActivationPoint => activation_target::ACTIVATION_POINT,
        }
    }

    fn value(&self) -> UiValue {
        let Some(rect) = self.lazy.bounds_rect() else {
            return UiValue::Null;
        };
        match self.kind {
            BoundsKind::Bounds => UiValue::from(rect),
            BoundsKind::ActivationPoint => UiValue::from(rect.center()),
        }
    }
}

struct TextAttr {
    lazy: Arc<LazyAttrs>,
}

impl TextAttr {
    /// Full text content via chunked `getAccessibleTextRange` reads. Empty
    /// fields yield `Some("")` (present-and-empty, not Null).
    fn read_text(&self) -> Option<String> {
        let client = &self.lazy.client;
        let count = client.text_char_count(&self.lazy.ctx).ok().flatten()?;
        if count <= 0 {
            return Some(String::new());
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let chunk = (ffi::MAX_BUFFER_SIZE - 1) as i32;
        let mut text = String::new();
        let mut start = 0i32;
        while start < count {
            let end = (start + chunk - 1).min(count - 1);
            let part = client.text_range(&self.lazy.ctx, start, end).ok().flatten()?;
            text.push_str(&part);
            start = end + 1;
        }
        Some(text)
    }
}

impl UiAttribute for TextAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }

    fn name(&self) -> &str {
        text_content::TEXT
    }

    fn value(&self) -> UiValue {
        self.read_text().map_or(UiValue::Null, UiValue::from)
    }
}

#[derive(Clone, Copy)]
enum ValueKind {
    Current,
    Minimum,
    Maximum,
}

struct ValueAttr {
    lazy: Arc<LazyAttrs>,
    kind: ValueKind,
}

impl UiAttribute for ValueAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }

    fn name(&self) -> &str {
        match self.kind {
            ValueKind::Current => stateful_value::VALUE,
            ValueKind::Minimum => stateful_value::MIN_VALUE,
            ValueKind::Maximum => stateful_value::MAX_VALUE,
        }
    }

    fn value(&self) -> UiValue {
        let raw = match self.kind {
            ValueKind::Current => self.lazy.client.current_value(&self.lazy.ctx),
            ValueKind::Minimum => self.lazy.client.minimum_value(&self.lazy.ctx),
            ValueKind::Maximum => self.lazy.client.maximum_value(&self.lazy.ctx),
        };
        match raw {
            Ok(Some(text)) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    UiValue::Null
                } else if let Ok(number) = trimmed.parse::<f64>() {
                    UiValue::from(number)
                } else {
                    UiValue::from(trimmed.to_string())
                }
            }
            _ => UiValue::Null,
        }
    }
}

struct SelectedItemsAttr {
    lazy: Arc<LazyAttrs>,
    scope: IdScope,
    vm: VmId,
    hwnd: isize,
    index_path: Arc<[i32]>,
}

impl UiAttribute for SelectedItemsAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }

    fn name(&self) -> &str {
        selection_provider::SELECTED_ITEMS
    }

    fn value(&self) -> UiValue {
        let selected_count = self.lazy.client.selected_children_count(&self.lazy.ctx).unwrap_or(0);
        if selected_count <= 0 {
            return UiValue::Array(Vec::new());
        }
        // Selected child contexts do not reveal their enumeration index, so
        // scan the children (bounded) and collect the RuntimeIds of those the
        // container reports as selected.
        let scan = self.lazy.info.children_count.clamp(0, SELECTED_ITEMS_SCAN_LIMIT);
        let mut ids = Vec::new();
        for index in 0..scan {
            if self.lazy.client.is_child_selected(&self.lazy.ctx, index).unwrap_or(false) {
                let mut path = Vec::with_capacity(self.index_path.len() + 1);
                path.extend_from_slice(&self.index_path);
                path.push(index);
                ids.push(UiValue::from(format_runtime_id(self.scope, self.vm, self.hwnd, &path)));
                if ids.len() >= usize::try_from(selected_count).unwrap_or(usize::MAX) {
                    break;
                }
            }
        }
        UiValue::Array(ids)
    }
}

struct IsActiveAttr {
    lazy: Arc<LazyAttrs>,
}

impl UiAttribute for IsActiveAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }

    fn name(&self) -> &str {
        window_state::IS_ACTIVE
    }

    fn value(&self) -> UiValue {
        let active = (|| {
            let node = self.lazy.owner.as_ref()?.upgrade()?;
            let wm = self.lazy.window_manager.clone()?;
            let wid = wm.resolve_window(node.as_ref()).ok()?;
            wm.is_active(wid).ok()
        })()
        .unwrap_or(false);
        UiValue::from(active)
    }
}

// ---------------------------------------------------------------------------
// Window capability patterns (WindowManager delegation, the AT-SPI blueprint)

struct JabWindowSurface {
    node: Weak<dyn UiNode>,
    client: Arc<JabClient>,
    vm: VmId,
    window_manager: Option<Arc<dyn WindowManager>>,
}

impl JabWindowSurface {
    fn resolve(&self) -> Result<(Arc<dyn WindowManager>, WindowId), JabError> {
        let node = self.node.upgrade().ok_or(JabError::NodeDropped)?;
        let wm = self.window_manager.clone().ok_or(JabError::NoWindowManager)?;
        let wid = wm.resolve_window(node.as_ref()).map_err(|_| JabError::NoWindowManager)?;
        Ok((wm, wid))
    }

    fn run(
        &self,
        op: impl FnOnce(&dyn WindowManager, WindowId) -> Result<(), platynui_core::platform::PlatformError>,
    ) -> Result<(), PatternError> {
        let (wm, wid) = self.resolve()?;
        op(wm.as_ref(), wid)?;
        Ok(())
    }
}

fn make_window_pattern(id: &str, core: &Arc<JabWindowSurface>) -> Arc<dyn UiPattern> {
    match id {
        x if x == pattern_names::ACTIVATABLE => {
            let core = Arc::clone(core);
            Arc::new(ActivatableAction::new(move || core.run(|wm, wid| wm.activate(wid))))
        }
        x if x == pattern_names::MINIMIZABLE => {
            let core = Arc::clone(core);
            Arc::new(MinimizableAction::new(move || core.run(|wm, wid| wm.minimize(wid))))
        }
        x if x == pattern_names::MAXIMIZABLE => {
            let core = Arc::clone(core);
            Arc::new(MaximizableAction::new(move || core.run(|wm, wid| wm.maximize(wid))))
        }
        x if x == pattern_names::RESTORABLE => {
            let core = Arc::clone(core);
            Arc::new(RestorableAction::new(move || core.run(|wm, wid| wm.restore(wid))))
        }
        x if x == pattern_names::CLOSEABLE => {
            let core = Arc::clone(core);
            Arc::new(CloseableAction::new(move || core.run(|wm, wid| wm.close(wid))))
        }
        x if x == pattern_names::MOVABLE => {
            let core = Arc::clone(core);
            Arc::new(MovableAction::new(move |point: Point| core.run(|wm, wid| wm.move_to(wid, point))))
        }
        x if x == pattern_names::RESIZABLE => {
            let core = Arc::clone(core);
            Arc::new(ResizableAction::new(move |size: Size| core.run(|wm, wid| wm.resize(wid, size))))
        }
        x if x == pattern_names::RESPONSIVE => {
            let core = Arc::clone(core);
            // If the bridge answers a version query, the JVM's event pipeline
            // is alive — analogous to the AT-SPI state-query heuristic.
            Arc::new(ResponsiveAction::new(move || Ok(Some(core.client.version_info(core.vm).is_ok()))))
        }
        _ => unreachable!("make_window_pattern called with non-window pattern id"),
    }
}

// ---------------------------------------------------------------------------
// app:Application node

pub(crate) struct JabAppNode {
    pid: u32,
    client: Arc<JabClient>,
    window_manager: Option<Arc<dyn WindowManager>>,
    /// Carried rather than snapshot: [`Self::children`] runs long after the
    /// enumeration pass that produced this node, so the answer to "does another
    /// backend serve this window?" has to be asked fresh each time.
    exclusions: Option<Arc<dyn crate::provider::WindowExclusions>>,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    self_weak: OnceLock<Weak<dyn UiNode>>,
    runtime_id: OnceLock<RuntimeId>,
    name: OnceLock<String>,
}

impl JabAppNode {
    pub(crate) fn new(
        pid: u32,
        client: Arc<JabClient>,
        window_manager: Option<Arc<dyn WindowManager>>,
        exclusions: Option<Arc<dyn crate::provider::WindowExclusions>>,
        parent: &Arc<dyn UiNode>,
    ) -> Arc<Self> {
        Self::build(pid, client, window_manager, exclusions, Some(parent))
    }

    /// App node without a parent, capping an off-tree hit-test chain (see
    /// [`hit_test_node`]); the tree's reveal matches it by RuntimeId.
    pub(crate) fn orphan(
        pid: u32,
        client: Arc<JabClient>,
        window_manager: Option<Arc<dyn WindowManager>>,
        exclusions: Option<Arc<dyn crate::provider::WindowExclusions>>,
    ) -> Arc<Self> {
        Self::build(pid, client, window_manager, exclusions, None)
    }

    fn build(
        pid: u32,
        client: Arc<JabClient>,
        window_manager: Option<Arc<dyn WindowManager>>,
        exclusions: Option<Arc<dyn crate::provider::WindowExclusions>>,
        parent: Option<&Arc<dyn UiNode>>,
    ) -> Arc<Self> {
        let node = Arc::new(Self {
            pid,
            client,
            window_manager,
            exclusions,
            parent: Mutex::new(parent.map(Arc::downgrade)),
            self_weak: OnceLock::new(),
            runtime_id: OnceLock::new(),
            name: OnceLock::new(),
        });
        let arc: Arc<dyn UiNode> = node.clone();
        let _ = node.self_weak.set(Arc::downgrade(&arc));
        node
    }
}

impl UiNode for JabAppNode {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }

    #[allow(clippy::unnecessary_literal_bound)] // signature fixed by the UiNode trait
    fn role(&self) -> &str {
        "Application"
    }

    fn name(&self) -> String {
        self.name.get_or_init(|| crate::process::query_process_name(self.pid).unwrap_or_default()).clone()
    }

    fn runtime_id(&self) -> &RuntimeId {
        self.runtime_id.get_or_init(|| RuntimeId::from(format_app_runtime_id(self.pid)))
    }

    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent.lock().ok()?.clone()
    }

    fn has_children(&self) -> bool {
        // App nodes are emitted only after at least one Java window was seen
        // for the process.
        true
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        let parent = self.self_weak.get().and_then(Weak::upgrade);
        let scope = IdScope::App { pid: self.pid };
        let windows = crate::provider::java_windows(&self.client, Some(self.pid), self.exclusions.as_deref());
        let client = Arc::clone(&self.client);
        let window_manager = self.window_manager.clone();
        Box::new(windows.into_iter().map(move |window| {
            JabNode::new_window(
                Arc::clone(&client),
                window_manager.clone(),
                window.vm,
                window.ctx,
                window.hwnd,
                scope,
                parent.as_ref(),
            ) as Arc<dyn UiNode>
        }))
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        let pid = self.pid;
        let rid = self.runtime_id().as_str().to_string();
        let attrs: Vec<Arc<dyn UiAttribute>> = vec![
            static_attr(Namespace::Control, common::ROLE, UiValue::from("Application")),
            static_attr(Namespace::Control, common::NAME, UiValue::from(self.name())),
            static_attr(Namespace::Control, common::RUNTIME_ID, UiValue::from(rid)),
            static_attr(Namespace::Control, common::TECHNOLOGY, UiValue::from(TECHNOLOGY)),
            static_attr(Namespace::Control, application::PROCESS_ID, UiValue::from(i64::from(pid))),
            Arc::new(AppMetadataAttr { pid, kind: AppMetadataKind::ProcessName }),
            Arc::new(AppMetadataAttr { pid, kind: AppMetadataKind::ExecutablePath }),
            Arc::new(AppMetadataAttr { pid, kind: AppMetadataKind::CommandLine }),
            Arc::new(AppMetadataAttr { pid, kind: AppMetadataKind::UserName }),
            Arc::new(AppMetadataAttr { pid, kind: AppMetadataKind::StartTime }),
            Arc::new(AppMetadataAttr { pid, kind: AppMetadataKind::Architecture }),
        ];
        Box::new(attrs.into_iter())
    }

    fn supported_patterns(&self) -> Vec<PatternName> {
        Vec::new()
    }

    fn invalidate(&self) {}

    fn doc_order_key(&self) -> Option<u64> {
        Some(u64::from(self.pid))
    }
}

#[derive(Clone, Copy)]
enum AppMetadataKind {
    ProcessName,
    ExecutablePath,
    CommandLine,
    UserName,
    StartTime,
    Architecture,
}

struct AppMetadataAttr {
    pid: u32,
    kind: AppMetadataKind,
}

impl UiAttribute for AppMetadataAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }

    fn name(&self) -> &str {
        match self.kind {
            AppMetadataKind::ProcessName => application::PROCESS_NAME,
            AppMetadataKind::ExecutablePath => application::EXECUTABLE_PATH,
            AppMetadataKind::CommandLine => application::COMMAND_LINE,
            AppMetadataKind::UserName => application::USER_NAME,
            AppMetadataKind::StartTime => application::START_TIME,
            AppMetadataKind::Architecture => application::ARCHITECTURE,
        }
    }

    fn value(&self) -> UiValue {
        let value = match self.kind {
            AppMetadataKind::ProcessName => crate::process::query_process_name(self.pid),
            AppMetadataKind::ExecutablePath => crate::process::query_executable_path(self.pid),
            AppMetadataKind::CommandLine => crate::process::query_command_line(self.pid),
            AppMetadataKind::UserName => crate::process::query_user_name(self.pid),
            AppMetadataKind::StartTime => crate::process::query_start_time(self.pid),
            AppMetadataKind::Architecture => crate::process::query_architecture(self.pid),
        };
        value.map_or(UiValue::Null, UiValue::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_ids_follow_the_scoped_scheme() {
        assert_eq!(format_runtime_id(IdScope::Desktop, 123, 0x002A_0B3C, &[]), "jab://123/0x2A0B3C");
        assert_eq!(format_runtime_id(IdScope::Desktop, 123, 0x002A_0B3C, &[0, 3, 1]), "jab://123/0x2A0B3C/0/3/1");
        assert_eq!(
            format_runtime_id(IdScope::App { pid: 4711 }, 123, 0x002A_0B3C, &[2]),
            "jab://app/4711/123/0x2A0B3C/2"
        );
        assert_eq!(format_app_runtime_id(4711), "jab://app/4711");
    }

    /// Minimal `ContextInfo` for the pure advertisement logic.
    fn context_info(states: crate::map::StateFlags, interfaces: i32) -> ContextInfo {
        ContextInfo {
            name: String::new(),
            description: String::new(),
            role_localized: String::new(),
            role_en_us: "text".to_string(),
            states_en_us: String::new(),
            states,
            index_in_parent: 0,
            children_count: 0,
            bounds: Some((0, 0, 100, 20)),
            interfaces,
        }
    }

    #[test]
    fn editable_text_field_advertises_text_editable_marker() {
        let states = crate::map::StateFlags { enabled: true, focusable: true, editable: true, ..Default::default() };
        let patterns = advertised_patterns(&context_info(states, ffi::INTERFACE_TEXT), false);

        // The marker stays advertised even though no set-text action exists.
        assert!(patterns.iter().any(|p| p.as_str() == pattern_names::TEXT_EDITABLE));
        assert!(patterns.iter().any(|p| p.as_str() == pattern_names::FOCUSABLE));
    }

    #[test]
    fn read_only_or_interface_less_text_is_not_marked_editable() {
        let read_only = crate::map::StateFlags { enabled: true, focusable: true, ..Default::default() };
        let patterns = advertised_patterns(&context_info(read_only, ffi::INTERFACE_TEXT), false);
        assert!(patterns.iter().all(|p| p.as_str() != pattern_names::TEXT_EDITABLE));

        let editable_without_text = crate::map::StateFlags { editable: true, ..Default::default() };
        let patterns = advertised_patterns(&context_info(editable_without_text, 0), false);
        assert!(patterns.iter().all(|p| p.as_str() != pattern_names::TEXT_EDITABLE));
    }

    #[test]
    fn transform_identity_when_rects_match() {
        let rect = (10, 20, 300, 200);
        assert_eq!(Transform::derive(rect, rect), Transform::IDENTITY);
    }

    #[test]
    fn transform_scales_and_offsets_descendants() {
        // JAB (virtualized) window at (100, 50, 400x300); physical at
        // (150, 75, 600x450) — a 1.5x scale.
        let transform = Transform::derive((100, 50, 400, 300), (150, 75, 600, 450));
        let mapped = transform.apply((100, 50, 400, 300));
        assert_eq!(
            (mapped.x(), mapped.y(), mapped.width(), mapped.height()),
            (150.0, 75.0, 600.0, 450.0),
            "window maps onto its own physical rect"
        );
        // A child 40px right / 30px below the window origin in JAB space.
        let child = transform.apply((140, 80, 100, 20));
        assert_eq!((child.x(), child.y()), (210.0, 120.0));
        assert_eq!((child.width(), child.height()), (150.0, 30.0));
    }

    #[test]
    fn transform_survives_degenerate_window_sizes() {
        let transform = Transform::derive((0, 0, 0, 0), (10, 10, 100, 100));
        // Scale falls back to 1.0; offset still anchors to the physical origin.
        let mapped = transform.apply((0, 0, 50, 50));
        assert_eq!((mapped.x(), mapped.y()), (10.0, 10.0));
        assert_eq!((mapped.width(), mapped.height()), (50.0, 50.0));
    }
}
