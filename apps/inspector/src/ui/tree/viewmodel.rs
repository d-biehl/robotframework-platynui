use slint::{ModelRc, SharedString, VecModel};
use std::collections::HashSet;

use super::data::TreeData;
use crate::TreeNodeVM;
use platynui_core::ui::{RuntimeId, UiNode};
use std::rc::Rc;
use std::sync::Arc;

/// Visible row item used by the TreeView (flat model)
#[derive(Clone, Default)]
pub struct VisibleRow {
    pub id: SharedString,
    pub label: SharedString,
    pub depth: i32,
    pub has_children: bool,
    pub is_expanded: bool,
    pub is_valid: bool,
    pub data: Option<Arc<dyn TreeData<Underlying = Arc<dyn UiNode>>>>,
}

impl From<&VisibleRow> for TreeNodeVM {
    fn from(v: &VisibleRow) -> Self {
        TreeNodeVM {
            id: v.id.clone(),
            label: v.label.clone(),
            has_children: v.has_children,
            icon_name: SharedString::from(""),
            depth: v.depth,
            is_expanded: v.is_expanded,
            is_valid: v.is_valid,
        }
    }
}

/// A simple viewmodel that maintains a flattened list of visible rows based on expansion state.
pub struct ViewModel {
    root: Arc<dyn TreeData<Underlying = Arc<dyn UiNode>>>,
    expanded: HashSet<RuntimeId>,
    model: Rc<VecModel<TreeNodeVM>>,
    visible_rows: Vec<VisibleRow>,
}

impl ViewModel {
    pub fn new(root: Arc<dyn TreeData<Underlying = Arc<dyn UiNode>>>) -> Self {
        let mut vm =
            Self { root, expanded: Default::default(), model: Rc::new(VecModel::default()), visible_rows: Vec::new() };
        vm.rebuild_visible();
        vm
    }

    pub fn model_rc(&self) -> ModelRc<TreeNodeVM> {
        ModelRc::from(self.model.clone())
    }

    fn set_expanded_key(&mut self, key: &RuntimeId, expand: bool) {
        if expand {
            self.expanded.insert(key.clone());
        } else {
            self.expanded.remove(key);
        }
        self.rebuild_visible();
    }

    fn rebuild_visible(&mut self) {
        // Build a temporary list including UiNode handles
        let mut out: Vec<VisibleRow> = Vec::new();
        Self::flatten_node_static(Arc::clone(&self.root), 0, &mut out, &self.expanded);
        // push into VecModel for Slint
        let rows: Vec<TreeNodeVM> = out.iter().map(TreeNodeVM::from).collect();
        self.model.set_vec(rows);
        // keep the full rows including UiNode for fast resolution
        self.visible_rows = out;
    }

    /// Public: force a rebuild of visible rows, useful after external refresh actions.
    pub fn force_rebuild(&mut self) {
        self.rebuild_visible();
    }

    fn flatten_node_static(
        node: Arc<dyn TreeData<Underlying = Arc<dyn UiNode>>>,
        depth: i32,
        out: &mut Vec<VisibleRow>,
        expanded: &HashSet<RuntimeId>,
    ) {
        let id = node.id();
        let has_children = node.has_children().unwrap_or(false);
        // Prefer a stable key from the underlying UiNode
        let key_opt: Option<RuntimeId> = node.as_underlying_data().as_ref().map(|u| u.runtime_id().clone());
        let is_expanded = key_opt.as_ref().map(|k| expanded.contains(k)).unwrap_or(false);
        let label = node.label().unwrap_or_else(|_| format!("Error loading node {}", id.as_str()).into());

        // We don't own an Arc here; rebuild will call with Arc roots/children.
        // Fallback: do not store if we cannot produce an Arc (children/parent provide Arcs).
        // In practice, root and children are always Arc-backed in our UiNodeData provider.
        // So we only push None here and let callers pass Arcs.
        // Determine validity via underlying UiNode if available; default true
        let is_valid = node.as_underlying_data().map(|u| u.is_valid()).unwrap_or(true);
        out.push(VisibleRow {
            id: id.clone(),
            label,
            depth,
            has_children,
            is_expanded,
            is_valid,
            data: Some(Arc::clone(&node)),
        });

        if has_children
            && is_expanded
            && let Ok(children) = node.children()
        {
            for child in children {
                Self::flatten_node_static(child, depth + 1, out, expanded);
            }
        }
    }

    // id-based helpers removed in index-only inspector
}

impl ViewModel {
    pub fn visible_model(&self) -> ModelRc<TreeNodeVM> {
        self.model_rc()
    }
    pub fn toggle_index(&mut self, index: usize, expand: bool) {
        if let Some(key) = self
            .visible_rows
            .get(index)
            .and_then(|vr| vr.data.as_ref())
            .and_then(|d| d.as_underlying_data())
            .map(|u| u.runtime_id().clone())
        {
            self.set_expanded_key(&key, expand)
        }
    }
    pub fn request_children_index(&mut self, _index: usize) { /* read-only demo: no-op */
    }
    pub fn resolve_node_by_index(&self, index: usize) -> Option<Arc<dyn UiNode>> {
        self.visible_rows.get(index).and_then(|vr| vr.data.as_ref().and_then(|d| d.as_underlying_data()))
    }

    /// Refresh the TreeData caches for a specific row, if available.
    pub fn refresh_row(&mut self, index: usize) {
        if let Some(Some(data)) = self.visible_rows.get(index).map(|vr| vr.data.as_ref()) {
            data.refresh_self();
        }
    }

    /// Refresh recursively from a given row.
    pub fn refresh_row_recursive(&mut self, index: usize) {
        if let Some(Some(data)) = self.visible_rows.get(index).map(|vr| vr.data.as_ref()) {
            data.refresh_recursive();
        }
    }
}
