//! ViewModel: Flattened tree with expand/collapse state and keyboard navigation.

use crate::model::tree_data::UiNodeData;
use platynui_core::ui::UiNode;
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

    /// Reveal a node by `UiNode` reference: search the `UiNodeData` tree,
    /// expand all ancestors along the path, rebuild, and return its visible index.
    ///
    /// If the node isn't found in our cached tree, we walk up the result
    /// node's parent chain to find the nearest cached ancestor, refresh that
    /// ancestor's children, and retry.
    pub fn reveal_node(&mut self, node: &Arc<dyn UiNode>) -> Option<usize> {
        let target_id = node.runtime_id().as_str().to_string();

        // 1st attempt: DFS through our cached UiNodeData tree.
        let root = Arc::clone(&self.root);
        if self.find_and_expand(&root, &target_id) {
            self.rebuild();
            return self.visible_rows.iter().position(|row| row.data.id() == target_id);
        }

        // 2nd attempt: Walk up the result node's parent chain to find the
        // nearest ancestor that exists in our tree. Refresh that ancestor so
        // its children get reloaded, then search again.
        tracing::debug!(target_id, "reveal_node: not in cached tree, walking parent chain to refresh");

        let mut ancestor_ids = Vec::new();
        let mut current: Option<Arc<dyn UiNode>> = Some(Arc::clone(node));
        while let Some(n) = current {
            if let Some(parent_weak) = n.parent() {
                if let Some(parent) = parent_weak.upgrade() {
                    ancestor_ids.push(parent.runtime_id().as_str().to_string());
                    current = Some(parent);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Walk ancestor_ids from bottom to top (closest parent first).
        let root2 = Arc::clone(&self.root);
        for ancestor_id in &ancestor_ids {
            if let Some(tree_node) = Self::find_node(&root2, ancestor_id) {
                tracing::debug!(ancestor_id, "reveal_node: found ancestor in tree, refreshing subtree");
                tree_node.refresh_recursive();

                let root3 = Arc::clone(&self.root);
                if self.find_and_expand(&root3, &target_id) {
                    self.rebuild();
                    return self.visible_rows.iter().position(|row| row.data.id() == target_id);
                }
            }
        }

        tracing::warn!(target_id, "reveal_node: not found even after refreshing ancestors");
        None
    }

    /// Find a `UiNodeData` in the tree by runtime ID (DFS, read-only — only
    /// searches already-cached children).
    fn find_node(node: &Arc<UiNodeData>, target_id: &str) -> Option<Arc<UiNodeData>> {
        if node.id() == target_id {
            return Some(Arc::clone(node));
        }
        for child in node.cached_children() {
            if let Some(found) = Self::find_node(&child, target_id) {
                return Some(found);
            }
        }
        None
    }

    /// DFS through the `UiNodeData` tree. If the target is found among the
    /// descendants, this node is expanded and `true` is returned.
    fn find_and_expand(&mut self, node: &Arc<UiNodeData>, target_id: &str) -> bool {
        if node.id() == target_id {
            return true;
        }

        for child in node.children() {
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
