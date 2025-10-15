use slint::{ModelRc, SharedString, VecModel};
use std::collections::HashSet;

use crate::TreeNodeVM;
use super::data::TreeData;
use std::rc::Rc;
use super::adapter::TreeViewAdapter;

/// Visible row item used by the TreeView (flat model)
#[derive(Clone, Default)]
pub struct VisibleRow {
    pub id: SharedString,
    pub label: SharedString,
    pub depth: i32,
    pub has_children: bool,
    pub is_expanded: bool,
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
        }
    }
}

/// A simple viewmodel that maintains a flattened list of visible rows based on expansion state.
pub struct ViewModel {
    root: Box<dyn TreeData>,
    expanded: HashSet<SharedString>,
    model: Rc<VecModel<TreeNodeVM>>,
}

impl ViewModel {
    pub fn new(root: Box<dyn TreeData>) -> Self {
        let mut vm = Self {
            root,
            expanded: Default::default(),
            model: Rc::new(VecModel::default()),
        };
        vm.rebuild_visible();
        vm
    }

    pub fn model_rc(&self) -> ModelRc<TreeNodeVM> { ModelRc::from(self.model.clone()) }

    fn set_expanded(&mut self, id: &str, expand: bool) {
        let id_ss: SharedString = id.into();
        if expand {
            self.expanded.insert(id_ss);
        } else {
            self.expanded.remove(&id_ss);
        }
        self.rebuild_visible();
    }

    // helper accessors can be added here when needed

    // previously exposed refresh() removed; internal rebuild handles changes

    fn rebuild_visible(&mut self) {
        // Build a temporary list
        let mut out: Vec<VisibleRow> = Vec::new();
        self.flatten_node(&*self.root, 0, &mut out);
        // push into VecModel
        let rows: Vec<TreeNodeVM> = out.iter().map(|v| TreeNodeVM::from(v)).collect();
        self.model.set_vec(rows);
    }

    fn flatten_node(&self, node: &dyn TreeData, depth: i32, out: &mut Vec<VisibleRow>) {
        let id = node.id();
        let has_children = node.has_children().unwrap_or(false);
        let is_expanded = self.expanded.contains(&id);
        let label = node.label().unwrap_or_else(|_| format!("Error loading node {}", id.as_str()).into());

        out.push(VisibleRow { id: id.clone(), label, depth, has_children, is_expanded });

        if has_children && is_expanded {
            if let Ok(children) = node.children() {
                for child in children {
                    self.flatten_node(&*child, depth + 1, out);
                }
            }
        }
    }

    /// Find parent id of a given node id by walking the tree recursively.
    pub fn find_parent_id(&self, id: &str) -> Option<SharedString> {
        self.find_parent_recursive(&*self.root, id)
    }

    /// Recursive helper to find parent of a node with given ID
    fn find_parent_recursive(&self, current: &dyn TreeData, target_id: &str) -> Option<SharedString> {
        // Check if any direct child has the target ID
        if let Ok(children) = current.children() {
            for child in children {
                if child.id().as_str() == target_id {
                    return Some(current.id());
                }
                // Recursively search in child subtrees
                if let Some(parent_id) = self.find_parent_recursive(&*child, target_id) {
                    return Some(parent_id);
                }
            }
        }
        None
    }
}

impl TreeViewAdapter for ViewModel {
    fn visible_model(&self) -> ModelRc<TreeNodeVM> { self.model_rc() }
    fn toggle(&mut self, id: &str, expand: bool) { self.set_expanded(id, expand) }
    fn request_children(&mut self, _id: &str) { /* read-only demo: no-op */ }
    fn parent_of(&self, id: &str) -> Option<SharedString> { self.find_parent_id(id) }
}
