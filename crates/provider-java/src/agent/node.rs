//! `UiNode` over an agent element — the mapping layer proper.
//!
//! # What the provider adds
//!
//! The agent answers about *its* model; a node has to answer about PlatynUI's.
//! Three of those translations carry the change:
//!
//! - **Identity.** The agent holds real object references, so its element ids are
//!   identity-based and stable for as long as the element lives. That is what
//!   makes a `RuntimeId` here mean "this element" rather than "whatever is
//!   currently in that position" — the fundamental limit of the Access Bridge's
//!   enumeration-index scheme. The id is scoped to the tree the node was
//!   reached through (see [`View`]).
//! - **Validity.** `UiNode::is_valid` is load-bearing: the Robot Framework
//!   library reuses the element a scoped root resolved to for exactly as long as
//!   it answers `true`, so the trait's `true` default would pin a dead root
//!   forever. It is answered from the agent's registry, and answered `false` when
//!   the agent cannot be reached at all.
//! - **Window delegation.** The window capability patterns do not go through the
//!   agent; they go through the runtime's `WindowManager`, exactly as the JAB
//!   backend's do. What the agent contributes is the native handle that makes
//!   that resolution exact instead of a PID guess.

use super::element::{
    Cell, ColumnHeader, Element, FRAME_ICONIFIED, FRAME_MAXIMIZED_BOTH, Kind, Table, TableRow, map_role,
};
use super::session::AgentSession;
use platynui_core::platform::{WindowId, WindowManager};
use platynui_core::types::{Point, Size};
use platynui_core::ui::attribute_names::{
    activation_target, application, closeable, common, element as element_attrs, expandable, focusable, maximizable,
    minimizable, movable, resizable, selectable, selection_provider, stateful_value, text_content, text_editable,
    toggleable, window_state,
};
use platynui_core::ui::{
    ActivatableAction, CloseableAction, FocusableAction, MaximizableAction, MinimizableAction, MovableAction,
    Namespace, PatternError, PatternName, ResizableAction, ResponsiveAction, RestorableAction, RuntimeId, UiAttribute,
    UiNode, UiPattern, UiValue, pattern_names, supported_patterns_value,
};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tracing::debug;

/// `@Technology` of every node this backend produces. Distinct from `"JAB"` on
/// purpose: which channel served a node is a fact a user comparing two runs
/// needs to be able to see.
pub(crate) const TECHNOLOGY: &str = "JavaAgent";

/// The tree a node was reached through, which scopes its runtime id, as UI
/// Automation and the Access Bridge scope theirs: a window listed for the
/// desktop and the same window under its `app:Application` are two nodes, each
/// at its own place in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    /// The top-levels the desktop lists, and everything below them.
    Flat,
    /// Everything below a JVM's `app:Application` node.
    Application,
}

impl View {
    /// The runtime id of the agent element `id` of the JVM `pid` in this view.
    pub(crate) fn runtime_id(self, pid: u32, id: u64) -> RuntimeId {
        match self {
            View::Flat => RuntimeId::from(format!("agent/{pid}/{id}")),
            View::Application => RuntimeId::from(format!("agent/app/{pid}/{id}")),
        }
    }
}

/// One element of one JVM.
pub(crate) struct AgentNode {
    session: Arc<AgentSession>,
    /// Fixed where the node is created, and passed to every child.
    view: View,
    /// The last payload read for this element. Refreshed on
    /// [`UiNode::invalidate`] rather than per access — an XPath step reads a
    /// dozen attributes off one node, and a call per attribute is what makes
    /// out-of-process accessibility slow.
    snapshot: Mutex<Element>,
    stale: AtomicBool,
    /// The parent's raw agent role, which two role promotions depend on (a list
    /// entry and a tree row both report their renderer's role).
    parent_role: Option<String>,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    /// Keeps the parent alive, so that a node keeps its chain of ancestors for
    /// as long as it lives (the rule of [`UiNode::parent`]). Set for every node
    /// a child listing produces, and for a hit-test chain, which has no owner
    /// other than the node it was returned as. Top-level windows listed for
    /// the desktop leave it `None`: they reach the runtime's desktop only
    /// through the `parent` `Weak`.
    held_parent: Mutex<Option<Arc<dyn UiNode>>>,
    self_weak: OnceLock<Weak<dyn UiNode>>,
    runtime_id: OnceLock<RuntimeId>,
    role: OnceLock<(Namespace, String)>,
    window_manager: Option<Arc<dyn WindowManager>>,
    /// Native handle of a top-level, resolved once: the agent's answer when it
    /// has one, else the PID+geometry match.
    native_handle: OnceLock<Option<u64>>,
}

impl AgentNode {
    pub(crate) fn new(
        session: Arc<AgentSession>,
        view: View,
        element: Element,
        parent_role: Option<String>,
        window_manager: Option<Arc<dyn WindowManager>>,
        parent: Option<&Arc<dyn UiNode>>,
    ) -> Arc<Self> {
        let node = Arc::new(Self {
            session,
            view,
            snapshot: Mutex::new(element),
            stale: AtomicBool::new(false),
            parent_role,
            parent: Mutex::new(parent.map(Arc::downgrade)),
            held_parent: Mutex::new(None),
            self_weak: OnceLock::new(),
            runtime_id: OnceLock::new(),
            role: OnceLock::new(),
            window_manager,
            native_handle: OnceLock::new(),
        });
        let erased: Arc<dyn UiNode> = node.clone();
        let _ = node.self_weak.set(Arc::downgrade(&erased));
        node
    }

    /// Keeps `parent` alive for as long as this node lives.
    pub(crate) fn hold_parent(&self, parent: Arc<dyn UiNode>) {
        *self.held_parent.lock().unwrap_or_else(|poisoned| {
            self.held_parent.clear_poison();
            poisoned.into_inner()
        }) = Some(parent);
    }

    /// The current payload, re-read from the agent when this node was
    /// invalidated. A failed refresh keeps the previous snapshot: stale
    /// attributes beat no attributes, and `is_valid` is where "this element is
    /// gone" is reported.
    fn current(&self) -> Element {
        let mut snapshot = self.snapshot.lock().unwrap_or_else(|poisoned| {
            self.snapshot.clear_poison();
            poisoned.into_inner()
        });
        if self.stale.swap(false, Ordering::AcqRel) {
            match self.session.call("ui/element", json!({ "id": snapshot.id })) {
                Ok(result) => match serde_json::from_value::<Element>(result["element"].clone()) {
                    Ok(fresh) => *snapshot = fresh,
                    Err(error) => debug!(element = snapshot.id, %error, "unreadable element payload; keeping the last"),
                },
                Err(error) => debug!(element = snapshot.id, %error, "element refresh failed; keeping the last"),
            }
        }
        snapshot.clone()
    }

    fn element_id(&self) -> u64 {
        self.snapshot
            .lock()
            .unwrap_or_else(|poisoned| {
                self.snapshot.clear_poison();
                poisoned.into_inner()
            })
            .id
    }

    fn resolved_role(&self) -> &(Namespace, String) {
        self.role.get_or_init(|| {
            let element = self.current();
            map_role(&element, self.parent_role.as_deref())
        })
    }

    /// The native handle of the window this node is (design 5).
    ///
    /// The agent's in-JVM answer first — exact by construction. Failing that, a
    /// PID plus geometry match against the platform's window list: still a
    /// match rather than a guess, because the geometry has to agree, which is
    /// what separates this from the platform layer's PID-only fallback that
    /// "can target the wrong sibling window".
    fn resolve_native_handle(&self) -> Option<u64> {
        *self.native_handle.get_or_init(|| {
            let element = self.current();
            let window = element.window.as_ref()?;
            if let Some(handle) = window.handle.filter(|handle| *handle != 0) {
                return Some(handle);
            }
            let bounds = element.rect();
            let resolved = super::window_handle::match_by_pid_and_geometry(self.session.pid(), bounds);
            debug!(
                pid = self.session.pid(),
                agent_source = %window.handle_source,
                matched = ?resolved,
                "no in-JVM window handle; fell back to a PID plus geometry match"
            );
            resolved
        })
    }

    fn advertised_patterns(element: &Element) -> Vec<PatternName> {
        let mut patterns = Vec::new();
        if element.focusable || element.focused {
            patterns.push(PatternName::from(pattern_names::FOCUSABLE));
        }
        // A capability marker only: `pattern_by_name` returns no instance for it,
        // because per `text-input-policy` text is typed with synthesized keyboard
        // input. A programmatic setter would bypass the validation, listeners and
        // input masks the application relies on.
        if element.editable == Some(true) {
            patterns.push(PatternName::from(pattern_names::TEXT_EDITABLE));
        }
        if element.is_top_level() {
            patterns.extend(
                [
                    pattern_names::ACTIVATABLE,
                    pattern_names::MINIMIZABLE,
                    pattern_names::MAXIMIZABLE,
                    pattern_names::RESTORABLE,
                    pattern_names::CLOSEABLE,
                    pattern_names::MOVABLE,
                    pattern_names::RESIZABLE,
                    pattern_names::RESPONSIVE,
                ]
                .map(PatternName::from),
            );
        }
        patterns
    }
}

impl UiNode for AgentNode {
    fn namespace(&self) -> Namespace {
        self.resolved_role().0
    }

    fn role(&self) -> &str {
        &self.resolved_role().1
    }

    fn name(&self) -> String {
        self.current().display_name()
    }

    fn runtime_id(&self) -> &RuntimeId {
        // Identity-based and therefore stable: the agent's id stands for the
        // object, not for a position, so a relayout that reorders siblings does
        // not rename anything. Scoped to the node's view.
        self.runtime_id.get_or_init(|| self.view.runtime_id(self.session.pid(), self.element_id()))
    }

    fn id(&self) -> Option<String> {
        self.current().stable_id()
    }

    fn description(&self) -> Option<String> {
        self.current().accessible_description.filter(|text| !text.is_empty())
    }

    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent
            .lock()
            .unwrap_or_else(|poisoned| {
                self.parent.clear_poison();
                poisoned.into_inner()
            })
            .clone()
    }

    fn has_children(&self) -> bool {
        // The payload carries the count, so this costs nothing beyond what the
        // node already holds.
        self.current().child_count > 0
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        let parent = self.self_weak.get().and_then(Weak::upgrade);
        let element = self.current();
        let result = match self.session.call("ui/children", json!({ "id": element.id })) {
            Ok(result) => result,
            Err(error) => {
                debug!(element = element.id, %error, "children unavailable");
                return Box::new(std::iter::empty());
            }
        };
        let payloads: Vec<Element> = result
            .get("children")
            .and_then(|children| serde_json::from_value(children.clone()).ok())
            .unwrap_or_default();
        let session = Arc::clone(&self.session);
        let view = self.view;
        let window_manager = self.window_manager.clone();
        let parent_role = element.role.clone();
        Box::new(payloads.into_iter().map(move |child| {
            let node = AgentNode::new(
                Arc::clone(&session),
                view,
                child,
                Some(parent_role.clone()),
                window_manager.clone(),
                parent.as_ref(),
            );
            if let Some(parent) = &parent {
                node.hold_parent(Arc::clone(parent));
            }
            node as Arc<dyn UiNode>
        }))
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        let element = self.current();
        let (namespace, role) = self.resolved_role().clone();
        let mut attrs: Vec<Arc<dyn UiAttribute>> = Vec::with_capacity(32);

        attrs.push(literal(Namespace::Control, common::ROLE, UiValue::from(role)));
        attrs.push(literal(Namespace::Control, common::NAME, UiValue::from(element.display_name())));
        attrs.push(literal(Namespace::Control, common::RUNTIME_ID, UiValue::from(self.runtime_id().as_str())));
        attrs.push(literal(Namespace::Control, common::TECHNOLOGY, UiValue::from(TECHNOLOGY)));
        // Required by the node contract, and the reason a consumer can ask what a
        // node does before trying it. Derived from the very function that answers
        // `supported_patterns()`, so the attribute and the trait cannot drift.
        attrs.push(literal(
            Namespace::Control,
            common::SUPPORTED_PATTERNS,
            supported_patterns_value(&Self::advertised_patterns(&element)),
        ));
        if let Some(id) = element.stable_id() {
            attrs.push(literal(Namespace::Control, common::ID, UiValue::from(id)));
        }
        if let Some(description) = element.accessible_description.clone().filter(|text| !text.is_empty()) {
            attrs.push(literal(Namespace::Control, common::DESCRIPTION, UiValue::from(description)));
        }

        // Element geometry and state. `Bounds` is absent rather than zeroed when
        // the element is off screen, so the pointer input and the highlighter
        // cannot act on a rectangle that does not exist.
        if let Some(rect) = element.rect() {
            attrs.push(literal(Namespace::Control, element_attrs::BOUNDS, UiValue::Rect(rect)));
            // Where pointer input aims, published next to the bounds exactly as the
            // JAB backend does, so a consumer never has to know which backend served
            // a node in order to click it.
            attrs.push(literal(Namespace::Control, activation_target::ACTIVATION_POINT, UiValue::Point(rect.center())));
        }
        attrs.push(literal(Namespace::Control, element_attrs::IS_ENABLED, UiValue::from(element.enabled)));
        attrs.push(literal(Namespace::Control, element_attrs::IS_VISIBLE, UiValue::from(element.visible)));
        attrs.push(literal(Namespace::Control, element_attrs::IS_IN_VIEW, UiValue::from(element.showing)));
        attrs.push(literal(Namespace::Control, focusable::IS_FOCUSED, UiValue::from(element.focused)));

        let states = element.state_flags();
        if states.selectable || states.selected {
            attrs.push(literal(namespace, selectable::IS_SELECTED, UiValue::from(states.selected)));
        }
        // Toggle state for the three roles that have one. Reported as an attribute
        // and *not* as a Toggleable pattern instance — matching the JAB backend and
        // the input philosophy behind it: a toggle is flipped by clicking it, so a
        // programmatic setter would take a different path through the application
        // than a user does.
        if matches!(element.role.as_str(), "check box" | "radio button" | "toggle button") {
            let toggle_state = if states.indeterminate {
                "Indeterminate"
            } else if states.checked {
                "On"
            } else {
                "Off"
            };
            attrs.push(literal(namespace, toggleable::TOGGLE_STATE, UiValue::from(toggle_state)));
        }
        if states.expandable || states.expanded {
            attrs.push(literal(namespace, expandable::IS_EXPANDED, UiValue::from(states.expanded)));
            attrs.push(literal(namespace, expandable::CAN_EXPAND, UiValue::from(states.expandable)));
        }
        if let Some(text) = element.text.clone() {
            attrs.push(literal(Namespace::Control, text_content::TEXT, UiValue::from(text)));
        }
        if let Some(editable) = element.editable {
            attrs.push(literal(Namespace::Control, text_editable::IS_READ_ONLY, UiValue::from(!editable)));
        }
        if let Some(value) = element.value {
            push_optional_number(&mut attrs, stateful_value::VALUE, value.current);
            push_optional_number(&mut attrs, stateful_value::MIN_VALUE, value.minimum);
            push_optional_number(&mut attrs, stateful_value::MAX_VALUE, value.maximum);
        }
        if let Some(selection) = element.selection.as_ref() {
            attrs.push(literal(
                Namespace::Control,
                selection_provider::CAN_SELECT_MULTIPLE,
                UiValue::from(states.multiselectable),
            ));
            // RuntimeIds of the selected children — the *same* ids the child nodes
            // carry in this node's view, so a consumer can match them against
            // nodes it already holds.
            // Emitted only when the agent could name the children exactly; a list
            // of ids that resolve to nothing is worse than no list, because it
            // looks like an answer.
            if let Some(ids) = selection.ids.as_ref() {
                let runtime_ids: Vec<UiValue> = ids
                    .iter()
                    .map(|id| UiValue::from(self.view.runtime_id(self.session.pid(), *id).as_str()))
                    .collect();
                attrs.push(literal(
                    Namespace::Control,
                    selection_provider::SELECTED_ITEMS,
                    UiValue::Array(runtime_ids),
                ));
            }
        }

        push_native(&mut attrs, &element);
        if let Some(table) = element.table.as_ref() {
            push_table(&mut attrs, table);
        }
        if let Some(cell) = element.cell {
            push_cell(&mut attrs, cell);
        }
        if let Some(row) = element.table_row {
            push_table_row(&mut attrs, row);
        }
        if let Some(header) = element.column_header {
            push_column_header(&mut attrs, header);
        }
        if element.is_top_level() {
            push_window(&mut attrs, &element, self.resolve_native_handle());
            push_jvm_classification(&mut attrs, self.session.toolkits());
        }
        attrs.push(literal(Namespace::Control, application::PROCESS_ID, UiValue::from(i64::from(self.session.pid()))));

        Box::new(attrs.into_iter())
    }

    fn supported_patterns(&self) -> Vec<PatternName> {
        Self::advertised_patterns(&self.current())
    }

    fn pattern_by_name(&self, pattern: &PatternName) -> Option<Arc<dyn UiPattern>> {
        let element = self.current();
        if !Self::advertised_patterns(&element).contains(pattern) {
            return None;
        }
        let id = pattern.as_str();
        if id == pattern_names::FOCUSABLE {
            let session = Arc::clone(&self.session);
            let element_id = element.id;
            return Some(Arc::new(FocusableAction::new(move || {
                session
                    .call("ui/focus", json!({ "id": element_id }))
                    .map(|_| ())
                    .map_err(|error| PatternError::new(format!("focus failed: {error}")))
            })));
        }
        // TextEditable is advertised as a capability marker and deliberately has
        // no instance: text is typed, never written.
        if id == pattern_names::TEXT_EDITABLE {
            return None;
        }
        if !element.is_top_level() {
            return None;
        }
        let surface = Arc::new(WindowSurface {
            node: self.self_weak.get().cloned()?,
            session: Arc::clone(&self.session),
            window_manager: self.window_manager.clone(),
        });
        Some(make_window_pattern(id, &surface))
    }

    fn is_valid(&self) -> bool {
        self.session.is_element_live(self.element_id())
    }

    fn invalidate(&self) {
        // Only the payload is dropped. The native handle and the resolved role
        // belong to the element's identity, not to its state — re-deriving them
        // would change what a node *is*, not what it currently looks like.
        self.stale.store(true, Ordering::Release);
    }
}

// ---------------------------------------------------------------------------
// Attributes

fn literal(namespace: Namespace, name: &'static str, value: UiValue) -> Arc<dyn UiAttribute> {
    Arc::new(Literal { namespace, name, value })
}

struct Literal {
    namespace: Namespace,
    name: &'static str,
    value: UiValue,
}

impl UiAttribute for Literal {
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

/// An owned attribute name, for the `native:` passthrough where the names come
/// from the target rather than from a constant.
struct Dynamic {
    name: String,
    value: UiValue,
}

impl UiAttribute for Dynamic {
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

fn push_optional_number(attrs: &mut Vec<Arc<dyn UiAttribute>>, name: &'static str, value: Option<f64>) {
    if let Some(number) = value {
        attrs.push(literal(Namespace::Control, name, UiValue::Number(number)));
    }
}

fn dynamic(attrs: &mut Vec<Arc<dyn UiAttribute>>, name: impl Into<String>, value: UiValue) {
    attrs.push(Arc::new(Dynamic { name: name.into(), value }));
}

/// The `native:` passthrough — what the toolkit said, unnormalised.
///
/// Deliberately generous: this is where a user goes when the normalised surface
/// does not carry what their application encodes, and every one of these is
/// something no out-of-process bridge could report.
fn push_native(attrs: &mut Vec<Arc<dyn UiAttribute>>, element: &Element) {
    dynamic(attrs, "Role", UiValue::from(element.role.clone()));
    dynamic(attrs, "ClassName", UiValue::from(element.class_name.clone()));
    dynamic(attrs, "States", UiValue::from(element.states.join(",")));
    if let Some(selection) = element.selection.as_ref() {
        dynamic(attrs, "Selection.Count", UiValue::from(i64::try_from(selection.count).unwrap_or(0)));
        // The raw child indices, next to the resolved RuntimeIds in
        // `control:SelectedItems`. Kept because they survive the case the ids do
        // not: when the accessible order and the tree order cannot be shown to
        // agree, the indices are still exactly what the toolkit said. For a table
        // they are row indices, because a table's children are its rows.
        dynamic(
            attrs,
            "Selection.Indices",
            UiValue::Array(
                selection.indices.iter().map(|index| UiValue::from(i64::try_from(*index).unwrap_or(0))).collect(),
            ),
        );
    }
    dynamic(attrs, "ElementId", UiValue::from(i64::try_from(element.id).unwrap_or(i64::MAX)));
    dynamic(attrs, "Kind", UiValue::from(kind_label(element.kind)));
    if let Some(name) = element.component_name.as_ref() {
        // `Component.getName()` verbatim, next to the normalised `control:Id`.
        // Only components and windows have one.
        dynamic(attrs, "ComponentName", UiValue::from(name.clone()));
    }
    if let Some(accessible) = element.accessible_name.as_ref() {
        dynamic(attrs, "AccessibleName", UiValue::from(accessible.clone()));
    }
    if let Some(value) = element.model_value.as_ref().and_then(model_value) {
        // What a cell holds, next to what it displays, which is its `control:Name`.
        dynamic(attrs, "TableCell.ModelValue", value);
    }
    if let Some(tip) = element.tool_tip_text.as_ref() {
        dynamic(attrs, "ToolTipText", UiValue::from(tip.clone()));
    }
    for (key, value) in &element.client_properties {
        // Where enterprise Swing applications habitually keep their own
        // automation ids; there is no other way to see them.
        if let Some(scalar) = json_scalar(value) {
            dynamic(attrs, format!("ClientProperty.{key}"), scalar);
        }
    }
}

fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Window => "window",
        Kind::Component => "component",
        Kind::Cell => "cell",
        Kind::Accessible => "accessible",
    }
}

/// A cell's model value as the agent typed it: an integral number as an
/// integer — asked first, so `42` and `42.0` stay apart, which [`json_scalar`]
/// does not keep — any other number as a number, a boolean and a string as
/// themselves.
fn model_value(value: &serde_json::Value) -> Option<UiValue> {
    match value {
        serde_json::Value::Number(number) => {
            number.as_i64().map(UiValue::Integer).or_else(|| number.as_f64().map(UiValue::Number))
        }
        serde_json::Value::Bool(flag) => Some(UiValue::from(*flag)),
        serde_json::Value::String(text) => Some(UiValue::from(text.clone())),
        _ => None,
    }
}

fn json_scalar(value: &serde_json::Value) -> Option<UiValue> {
    match value {
        serde_json::Value::String(text) => Some(UiValue::from(text.clone())),
        serde_json::Value::Bool(flag) => Some(UiValue::from(*flag)),
        serde_json::Value::Number(number) => number.as_f64().map(UiValue::Number),
        _ => None,
    }
}

/// `native:Table.*`, matching the JAB backend's names so the same assertions hold.
fn push_table(attrs: &mut Vec<Arc<dyn UiAttribute>>, table: &Table) {
    dynamic(attrs, "Table.RowCount", UiValue::from(i64::try_from(table.rows).unwrap_or(i64::MAX)));
    dynamic(attrs, "Table.ColumnCount", UiValue::from(i64::try_from(table.columns).unwrap_or(i64::MAX)));
    dynamic(
        attrs,
        "Table.SelectedRows",
        UiValue::Array(table.selected_rows.iter().map(|row| UiValue::from(i64::try_from(*row).unwrap_or(0))).collect()),
    );
    dynamic(
        attrs,
        "Table.SelectedColumns",
        UiValue::Array(
            table.selected_columns.iter().map(|column| UiValue::from(i64::try_from(*column).unwrap_or(0))).collect(),
        ),
    );
}

/// `native:TableCell.*` — the same names the JAB backend publishes, on purpose:
/// the acceptance suite asserts on them, and a cell served through the agent
/// must answer the same questions, only correctly.
fn push_cell(attrs: &mut Vec<Arc<dyn UiAttribute>>, cell: Cell) {
    dynamic(attrs, "TableCell.Row", UiValue::from(i64::try_from(cell.row).unwrap_or(0)));
    dynamic(attrs, "TableCell.Column", UiValue::from(i64::try_from(cell.column).unwrap_or(0)));
    dynamic(attrs, "TableCell.RowExtent", UiValue::from(i64::try_from(cell.row_extent).unwrap_or(1)));
    dynamic(attrs, "TableCell.ColumnExtent", UiValue::from(i64::try_from(cell.column_extent).unwrap_or(1)));
    dynamic(attrs, "TableCell.IsSelected", UiValue::from(cell.selected));
    dynamic(attrs, "TableCell.IsEditable", UiValue::from(cell.editable));
}

/// `native:TableRow.*` — where a row sits and whether it is the selected one.
///
/// The index is free and is what a user reads off the screen; it duplicates what
/// the tree already encodes structurally, which is the point — a row should be
/// assertable both ways.
fn push_table_row(attrs: &mut Vec<Arc<dyn UiAttribute>>, row: TableRow) {
    dynamic(attrs, "TableRow.Index", UiValue::from(i64::try_from(row.row).unwrap_or(0)));
    dynamic(attrs, "TableRow.IsSelected", UiValue::from(row.selected));
}

/// `native:ColumnHeader.*` — which column a header belongs to.
///
/// Both indices travel, because they answer different questions: the view index
/// is where the header currently sits, the model index is which data it heads.
/// Only the second survives the user dragging columns around, so it is the one a
/// durable locator should use.
fn push_column_header(attrs: &mut Vec<Arc<dyn UiAttribute>>, header: ColumnHeader) {
    dynamic(attrs, "ColumnHeader.Column", UiValue::from(i64::try_from(header.column).unwrap_or(0)));
    dynamic(attrs, "ColumnHeader.ModelIndex", UiValue::from(i64::try_from(header.model_index).unwrap_or(0)));
    dynamic(attrs, "ColumnHeader.IsResizable", UiValue::from(header.resizable));
}

/// The JVM classification facts, on the same `native:` names every other
/// provider publishes them under.
///
/// Reported so the same window answers the same questions whichever backend
/// served it — a user comparing a bridge-served and an agent-served Java window
/// should not find the toolkit missing on one of them. Two of the three are
/// *certain* here rather than inferred: we are talking to this JVM, so it is one,
/// and we are its agent, so an agent is present. The toolkit is the authoritative
/// answer too, read from the loaded classes inside the process rather than
/// guessed from a window class — which is exactly what `JavaToolkit`'s own
/// documentation says an in-JVM agent would provide.
///
/// `JvmAccessibilityReachable` is deliberately **not** among them: it is about
/// the platform's native accessibility, which the agent is not, and it has no way
/// to find out. Absent means "not asserted", which beats a guess.
fn push_jvm_classification(attrs: &mut Vec<Arc<dyn UiAttribute>>, toolkits: &[String]) {
    use platynui_core::platform::java;

    attrs.push(literal(Namespace::Native, java::IS_JVM_ATTRIBUTE, UiValue::from(true)));
    attrs.push(literal(
        Namespace::Native,
        java::JVM_TOOLKIT_ATTRIBUTE,
        UiValue::from(crate::agent::element::map_toolkit(toolkits).label()),
    ));
    attrs.push(literal(Namespace::Native, java::JVM_AGENT_PRESENT_ATTRIBUTE, UiValue::from(true)));
    // The raw set alongside the single label: a JVM running more than one toolkit
    // is the deferred mixed case, and throwing the other names away here would
    // make it invisible.
    dynamic(attrs, "JvmToolkits", UiValue::from(toolkits.join(",")));
}

/// Top-level facts, including the handle the `WindowManager` delegation needs.
fn push_window(attrs: &mut Vec<Arc<dyn UiAttribute>>, element: &Element, handle: Option<u64>) {
    let Some(window) = element.window.as_ref() else {
        return;
    };
    if let Some(raw) = handle {
        // The name the platform's `resolve_window` looks for; without it the
        // window patterns fall back to a PID guess.
        dynamic(attrs, "NativeWindowHandle", UiValue::from(i64::try_from(raw).unwrap_or(i64::MAX)));
    }
    dynamic(attrs, "WindowHandleSource", UiValue::from(window.handle_source.clone()));
    if let Some(title) = window.title.as_ref() {
        // The window-manager title, next to `control:Name`: for a frame whose
        // accessible name was never set, this is the only human-readable label.
        dynamic(attrs, "WindowTitle", UiValue::from(title.clone()));
    }
    dynamic(attrs, "WindowIsFocused", UiValue::from(window.focused));
    attrs.push(literal(Namespace::Control, window_state::IS_ACTIVE, UiValue::from(window.active)));
    attrs.push(literal(Namespace::Control, window_state::IS_TOPMOST, UiValue::from(window.always_on_top)));
    attrs.push(literal(Namespace::Control, window_state::IS_MODAL, UiValue::from(element.state_flags().modal)));
    attrs.push(literal(
        Namespace::Control,
        maximizable::IS_MAXIMIZED,
        UiValue::from(window.extended_state & FRAME_MAXIMIZED_BOTH == FRAME_MAXIMIZED_BOTH),
    ));
    attrs.push(literal(
        Namespace::Control,
        minimizable::IS_MINIMIZED,
        UiValue::from(window.extended_state & FRAME_ICONIFIED == FRAME_ICONIFIED),
    ));
    attrs.push(literal(Namespace::Control, minimizable::CAN_MINIMIZE, UiValue::from(true)));
    attrs.push(literal(Namespace::Control, maximizable::CAN_MAXIMIZE, UiValue::from(window.resizable)));
    attrs.push(literal(Namespace::Control, closeable::CAN_CLOSE, UiValue::from(true)));
    attrs.push(literal(Namespace::Control, movable::CAN_MOVE, UiValue::from(true)));
    attrs.push(literal(Namespace::Control, resizable::CAN_RESIZE, UiValue::from(window.resizable)));
}

// ---------------------------------------------------------------------------
// Window capability patterns

/// The window patterns delegate to the runtime's `WindowManager`, not to the
/// agent — the same shape the JAB backend uses, and for the same reason: moving,
/// resizing and activating a window are the window system's operations, and a
/// toolkit-level imitation of them would behave differently from what a user
/// does.
struct WindowSurface {
    node: Weak<dyn UiNode>,
    session: Arc<AgentSession>,
    window_manager: Option<Arc<dyn WindowManager>>,
}

impl WindowSurface {
    fn resolve(&self) -> Result<(Arc<dyn WindowManager>, WindowId), PatternError> {
        let node = self.node.upgrade().ok_or_else(|| PatternError::new("the node is gone"))?;
        let manager = self.window_manager.clone().ok_or_else(|| PatternError::new("no window manager was injected"))?;
        let id = manager.resolve_window(node.as_ref())?;
        Ok((manager, id))
    }

    fn run(
        &self,
        op: impl FnOnce(&dyn WindowManager, WindowId) -> Result<(), platynui_core::platform::PlatformError>,
    ) -> Result<(), PatternError> {
        let (manager, id) = self.resolve()?;
        op(manager.as_ref(), id)?;
        Ok(())
    }
}

fn make_window_pattern(id: &str, surface: &Arc<WindowSurface>) -> Arc<dyn UiPattern> {
    match id {
        x if x == pattern_names::ACTIVATABLE => {
            let surface = Arc::clone(surface);
            Arc::new(ActivatableAction::new(move || surface.run(|manager, id| manager.activate(id))))
        }
        x if x == pattern_names::MINIMIZABLE => {
            let surface = Arc::clone(surface);
            Arc::new(MinimizableAction::new(move || surface.run(|manager, id| manager.minimize(id))))
        }
        x if x == pattern_names::MAXIMIZABLE => {
            let surface = Arc::clone(surface);
            Arc::new(MaximizableAction::new(move || surface.run(|manager, id| manager.maximize(id))))
        }
        x if x == pattern_names::RESTORABLE => {
            let surface = Arc::clone(surface);
            Arc::new(RestorableAction::new(move || surface.run(|manager, id| manager.restore(id))))
        }
        x if x == pattern_names::CLOSEABLE => {
            let surface = Arc::clone(surface);
            Arc::new(CloseableAction::new(move || surface.run(|manager, id| manager.close(id))))
        }
        x if x == pattern_names::MOVABLE => {
            let surface = Arc::clone(surface);
            Arc::new(MovableAction::new(move |point: Point| surface.run(|manager, id| manager.move_to(id, point))))
        }
        x if x == pattern_names::RESIZABLE => {
            let surface = Arc::clone(surface);
            Arc::new(ResizableAction::new(move |size: Size| surface.run(|manager, id| manager.resize(id, size))))
        }
        x if x == pattern_names::RESPONSIVE => {
            let surface = Arc::clone(surface);
            // A ping that comes back means the agent's RPC thread answers; the
            // toolkit thread behind it is covered by the call deadline, so a
            // wedged UI shows up as a degraded session rather than as
            // "responsive".
            Arc::new(ResponsiveAction::new(move || {
                Ok(Some(!surface.session.is_degraded() && surface.session.call("ping", json!({})).is_ok()))
            }))
        }
        _ => unreachable!("make_window_pattern called with a non-window pattern id"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node over a recorded payload. A component's attributes come from the
    /// payload alone, so the session never connects.
    fn node(payload: &str) -> Arc<AgentNode> {
        let element = serde_json::from_str(payload).expect("recorded payload must parse");
        AgentNode::new(Arc::new(AgentSession::unconnected(4242)), View::Flat, element, None, None, None)
    }

    fn attribute(node: &AgentNode, namespace: Namespace, name: &str) -> Option<UiValue> {
        node.attributes()
            .find(|attribute| attribute.namespace() == namespace && attribute.name() == name)
            .map(|attribute| attribute.value())
    }

    /// Spec `attribute-values`: *The Java agent passes an empty native value
    /// through*. The native attributes carry `""`, `Name` is `""`, and `Id`
    /// keeps its rule: an empty component name is no identifier.
    #[test]
    fn empty_strings_on_the_wire_are_listed_as_reported() {
        let button = node(
            r#"{"id": 4, "kind": "component", "role": "push button", "className": "javax.swing.JButton",
                "componentName": "", "accessibleName": "", "toolTipText": ""}"#,
        );
        assert_eq!(attribute(&button, Namespace::Native, "AccessibleName"), Some(UiValue::from("")));
        assert_eq!(attribute(&button, Namespace::Native, "ComponentName"), Some(UiValue::from("")));
        assert_eq!(attribute(&button, Namespace::Native, "ToolTipText"), Some(UiValue::from("")));
        assert_eq!(attribute(&button, Namespace::Control, common::NAME), Some(UiValue::from("")));
        assert_eq!(attribute(&button, Namespace::Control, common::ID), None);
        assert_eq!(button.id(), None);
    }

    /// Spec `description-attribute`: an empty description is no `Description`.
    #[test]
    fn an_empty_accessible_description_is_no_description() {
        let button = node(
            r#"{"id": 4, "kind": "component", "role": "push button", "className": "javax.swing.JButton",
                "accessibleDescription": ""}"#,
        );
        assert_eq!(attribute(&button, Namespace::Control, common::DESCRIPTION), None);
        assert_eq!(button.description(), None);
    }

    /// Spec `attribute-values`: a padded description is listed as reported.
    #[test]
    fn a_padded_description_is_listed_unchanged() {
        let button = node(
            r#"{"id": 4, "kind": "component", "role": "push button", "className": "javax.swing.JButton",
                "accessibleName": " Save ", "accessibleDescription": "  Closes the dialog  "}"#,
        );
        assert_eq!(attribute(&button, Namespace::Control, common::NAME), Some(UiValue::from(" Save ")));
        assert_eq!(
            attribute(&button, Namespace::Control, common::DESCRIPTION),
            Some(UiValue::from("  Closes the dialog  "))
        );
        assert_eq!(button.description().as_deref(), Some("  Closes the dialog  "));
    }

    /// Spec `name-attribute`: `native:ComponentName` is a component's name set
    /// through `setName`, and nothing else.
    #[test]
    fn only_a_component_name_is_listed_as_native_component_name() {
        let button = node(
            r#"{"id": 4, "kind": "component", "role": "push button", "className": "javax.swing.JButton",
                "componentName": "namesButton", "accessibleName": "names-button"}"#,
        );
        assert_eq!(attribute(&button, Namespace::Native, "ComponentName"), Some(UiValue::from("namesButton")));
        let cell = node(
            r#"{"id": 40, "kind": "cell", "role": "label", "className": "javax.swing.JTable",
                "accessibleName": "r2c0", "modelValue": "r2c0", "cell": {"row": 2, "column": 0}}"#,
        );
        assert_eq!(attribute(&cell, Namespace::Native, "ComponentName"), None, "a cell has no component name");
    }

    /// Spec `name-attribute`: `native:TableCell.ModelValue` keeps the model
    /// value typed — an integer, a number, a boolean, otherwise a string — and
    /// a cell without one lists none.
    #[test]
    fn a_cell_lists_its_model_value_typed() {
        let model_value = |value: &str| {
            let cell = node(&format!(
                r#"{{"id": 40, "kind": "cell", "role": "label", "className": "javax.swing.JTable",
                    {value} "cell": {{"row": 0, "column": 0}}}}"#
            ));
            attribute(&cell, Namespace::Native, "TableCell.ModelValue")
        };
        assert_eq!(model_value(r#""modelValue": 42,"#), Some(UiValue::Integer(42)));
        assert_eq!(model_value(r#""modelValue": 1234.5,"#), Some(UiValue::Number(1234.5)));
        assert_eq!(model_value(r#""modelValue": true,"#), Some(UiValue::Bool(true)));
        assert_eq!(model_value(r#""modelValue": "NaN","#), Some(UiValue::from("NaN")));
        assert_eq!(model_value(""), None, "a null model value lists none");
    }

    /// A list, agent element 42 of JVM 4711, whose selected child is element 43.
    fn list(view: View) -> Arc<AgentNode> {
        let element: Element = serde_json::from_value(json!({
            "id": 42,
            "kind": "component",
            "role": "list",
            "className": "javax.swing.JList",
            "selection": { "count": 1, "indices": [0], "ids": [43] },
        }))
        .expect("an element payload");
        AgentNode::new(Arc::new(AgentSession::unconnected(4711)), view, element, None, None, None)
    }

    #[test]
    fn runtime_ids_are_scoped_per_view() {
        for (view, prefix) in [(View::Flat, "agent/4711/"), (View::Application, "agent/app/4711/")] {
            let node = list(view);
            assert_eq!(node.runtime_id().as_str(), format!("{prefix}42"), "{view:?}");
            assert_eq!(
                attribute(&node, Namespace::Control, selection_provider::SELECTED_ITEMS),
                Some(UiValue::Array(vec![UiValue::from(format!("{prefix}43"))])),
                "{view:?}: the selected child's id in the list's own view"
            );
        }
    }
}
