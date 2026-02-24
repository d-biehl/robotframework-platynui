//! ViewModel: Flattened tree with expand/collapse state and keyboard navigation.

use crate::model::tree_data::UiNodeData;
use crate::view::tree_view::TreeRowData;
use std::collections::HashSet;
use std::sync::Arc;

/// A single visible row in the flattened tree.
#[derive(Clone)]
pub struct VisibleRow {
    /// Display label (role + name).
    pub label: String,
    /// Nesting depth (0 = root).
    pub depth: usize,
    /// Whether this node has children (for disclosure triangle).
    pub has_children: bool,
    /// Whether this node is currently expanded.
    pub is_expanded: bool,
    /// Whether the underlying `UiNode` is still valid.
    pub is_valid: bool,
    /// Reference to the underlying node data.
    pub data: Arc<UiNodeData>,
}

impl TreeRowData for VisibleRow {
    fn label(&self) -> &str {
        &self.label
    }

    fn depth(&self) -> usize {
        self.depth
    }

    fn has_children(&self) -> bool {
        self.has_children
    }

    fn is_expanded(&self) -> bool {
        self.is_expanded
    }

    fn is_valid(&self) -> bool {
        self.is_valid
    }
}

/// ViewModel that maintains a flattened list of visible rows based on expansion state.
pub struct TreeViewModel {
    root: Arc<UiNodeData>,
    expanded: HashSet<String>,
    visible_rows: Vec<VisibleRow>,
}

impl TreeViewModel {
    /// Create a new tree ViewModel rooted at the given node.
    ///
    /// The root node is auto-expanded.
    pub fn new(root: Arc<UiNodeData>) -> Self {
        let mut vm = Self { root, expanded: HashSet::new(), visible_rows: Vec::new() };
        // Auto-expand the root Desktop node
        vm.expanded.insert(vm.root.id());
        vm.rebuild();
        vm
    }

    /// Snapshot of currently visible rows.
    pub fn rows(&self) -> &[VisibleRow] {
        &self.visible_rows
    }

    /// The root `UiNodeData` of the tree (for background operations).
    pub fn root(&self) -> &Arc<UiNodeData> {
        &self.root
    }

    /// Number of currently visible rows.
    pub fn row_count(&self) -> usize {
        self.visible_rows.len()
    }

    /// Toggle expand/collapse for the node at `index`.
    pub fn toggle(&mut self, index: usize) {
        if let Some(row) = self.visible_rows.get(index)
            && row.has_children
        {
            let id = row.data.id();
            if self.expanded.contains(&id) {
                self.expanded.remove(&id);
            } else {
                self.expanded.insert(id);
            }
            self.rebuild();
        }
    }

    /// Expand the node at `index`.
    pub fn expand(&mut self, index: usize) {
        if let Some(row) = self.visible_rows.get(index)
            && row.has_children
            && !row.is_expanded
        {
            self.expanded.insert(row.data.id());
            self.rebuild();
        }
    }

    /// Collapse the node at `index`.
    pub fn collapse(&mut self, index: usize) {
        if let Some(row) = self.visible_rows.get(index)
            && row.is_expanded
        {
            self.expanded.remove(&row.data.id());
            self.rebuild();
        }
    }

    /// Find the parent's visible index by walking backwards to `depth - 1`.
    pub fn parent_index(&self, index: usize) -> Option<usize> {
        let row = self.visible_rows.get(index)?;
        if row.depth == 0 {
            return None;
        }
        let target_depth = row.depth - 1;
        (0..index).rev().find(|&i| self.visible_rows[i].depth == target_depth)
    }

    /// Refresh a single row's cached data and rebuild.
    pub fn refresh_row(&mut self, index: usize) {
        if let Some(row) = self.visible_rows.get(index) {
            row.data.refresh();
        }
        self.rebuild();
    }

    /// Refresh a row and all its descendants recursively, then rebuild.
    pub fn refresh_subtree(&mut self, index: usize) {
        if let Some(row) = self.visible_rows.get(index) {
            row.data.refresh_recursive();
        }
        self.rebuild();
    }

    /// Force rebuild (e.g. after external data changes).
    #[allow(dead_code)]
    pub fn force_rebuild(&mut self) {
        self.rebuild();
    }

    /// Reveal a node by runtime ID using only already-cached children.
    ///
    /// Returns `true` if the node was found and all ancestors expanded.
    /// This is the cheap counterpart to [`reveal_node`] — it never triggers
    /// I/O.  Use it after a background thread has pre-populated the caches.
    pub fn reveal_node_cached(&mut self, target_id: &str) -> bool {
        let root = Arc::clone(&self.root);
        if self.find_and_expand(&root, target_id) {
            self.rebuild();
            true
        } else {
            false
        }
    }

    /// DFS through the `UiNodeData` tree using only cached children (no I/O).
    ///
    /// If the target is found among already-loaded descendants, this node is
    /// expanded and `true` is returned.  Returns `false` immediately when a
    /// node's children have not been loaded yet.
    fn find_and_expand(&mut self, node: &Arc<UiNodeData>, target_id: &str) -> bool {
        if node.id() == target_id {
            return true;
        }

        // Only walk children that are already cached — never trigger I/O.
        let Some(children) = node.cached_children() else {
            return false;
        };

        for child in children {
            if self.find_and_expand(&child, target_id) {
                self.expanded.insert(node.id());
                return true;
            }
        }

        false
    }

    /// Rebuild the flattened visible row list from the current expansion state.
    fn rebuild(&mut self) {
        self.visible_rows.clear();
        Self::flatten(Arc::clone(&self.root), 0, &self.expanded, &mut self.visible_rows);
    }

    /// Recursively flatten the tree into visible rows.
    fn flatten(node: Arc<UiNodeData>, depth: usize, expanded: &HashSet<String>, out: &mut Vec<VisibleRow>) {
        let id = node.id();
        let is_expanded = expanded.contains(&id);
        let has_children = node.has_children();
        let label = node.label();
        let is_valid = node.is_valid();

        out.push(VisibleRow { label, depth, has_children, is_expanded, is_valid, data: Arc::clone(&node) });

        if has_children && is_expanded {
            for child in node.children() {
                Self::flatten(child, depth + 1, expanded, out);
            }
        }
    }
}
