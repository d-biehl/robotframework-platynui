//! ViewModel: Flattened tree with expand/collapse state and keyboard navigation.

use crate::model::tree_data::UiNodeData;
use crate::view::tree_view::TreeRowData;
use std::collections::HashSet;
use std::sync::Arc;

/// A single visible row in the flattened tree.
#[derive(Clone)]
pub struct VisibleRow {
    /// Stable runtime ID for tree state and selection lookup.
    pub id: String,
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
    /// Whether this node is currently loading child data on a worker.
    pub is_loading: bool,
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

    fn is_loading(&self) -> bool {
        self.is_loading
    }
}

/// ViewModel that maintains a flattened list of visible rows based on expansion state.
pub struct TreeViewModel {
    root: Arc<UiNodeData>,
    expanded: HashSet<String>,
    loading: HashSet<String>,
    visible_rows: Vec<VisibleRow>,
}

impl TreeViewModel {
    /// Create a new tree ViewModel rooted at the given node.
    ///
    /// The tree starts empty; call [`expand_root`] once the initial background
    /// load has populated the root's children cache.
    pub fn new(root: Arc<UiNodeData>) -> Self {
        Self { root, expanded: HashSet::new(), loading: HashSet::new(), visible_rows: Vec::new() }
    }

    /// Expand the root node and rebuild the visible row list.
    ///
    /// Call this once after the initial background load completes so the
    /// rebuild is cheap (children are already cached).
    pub fn expand_root(&mut self) {
        if let Some(root_id) = self.root.cached_id() {
            self.expanded.insert(root_id);
        }
        self.rebuild();
    }

    /// Rebuild the visible row list from the current cache state.
    ///
    /// Call this after the background streaming thread has pushed new children
    /// into the root's cache so the tree view reflects the latest data.
    pub fn rebuild_rows(&mut self) {
        self.rebuild();
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

    /// Mark a node as loading or idle by runtime ID.
    pub fn set_loading(&mut self, node_id: &str, is_loading: bool) {
        if is_loading {
            self.loading.insert(node_id.to_string());
        } else {
            self.loading.remove(node_id);
        }
        self.rebuild();
    }

    /// Toggle expand/collapse for the node at `index`.
    pub fn toggle(&mut self, index: usize) -> Option<Arc<UiNodeData>> {
        let row = self.visible_rows.get(index)?.clone();
        if !row.has_children {
            return None;
        }

        let request = if self.expanded.contains(&row.id) {
            self.expanded.remove(&row.id);
            None
        } else {
            self.expanded.insert(row.id.clone());
            if row.data.cached_children().is_none() { Some(Arc::clone(&row.data)) } else { None }
        };
        self.rebuild();
        request
    }

    /// Expand the node at `index`.
    pub fn expand(&mut self, index: usize) -> Option<Arc<UiNodeData>> {
        let row = self.visible_rows.get(index)?.clone();
        if row.has_children && !row.is_expanded {
            self.expanded.insert(row.id.clone());
            let request = if row.data.cached_children().is_none() { Some(Arc::clone(&row.data)) } else { None };
            self.rebuild();
            return request;
        }
        None
    }

    /// Collapse the node at `index`.
    pub fn collapse(&mut self, index: usize) {
        if let Some(row) = self.visible_rows.get(index)
            && row.is_expanded
        {
            self.expanded.remove(&row.id);
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
    pub fn refresh_row(&mut self, index: usize) -> Option<Arc<UiNodeData>> {
        let row = self.visible_rows.get(index)?.clone();
        row.data.clear_children_cache();
        self.loading.insert(row.id);
        self.rebuild();
        Some(row.data)
    }

    /// Refresh a row and all its descendants recursively, then rebuild.
    pub fn refresh_subtree(&mut self, index: usize) -> Option<Arc<UiNodeData>> {
        let row = self.visible_rows.get(index)?.clone();
        row.data.clear_children_cache_recursive();
        self.loading.insert(row.id);
        self.rebuild();
        Some(row.data)
    }

    /// Expanded nodes whose children are not cached yet and are not already loading.
    pub fn expanded_nodes_missing_children(&self) -> Vec<Arc<UiNodeData>> {
        self.visible_rows
            .iter()
            .filter(|row| row.is_expanded && row.has_children && row.data.cached_children().is_none())
            .filter(|row| !self.loading.contains(&row.id))
            .map(|row| Arc::clone(&row.data))
            .collect()
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
        let Some(node_id) = node.cached_id() else {
            return false;
        };
        if node_id == target_id {
            return true;
        }

        // Only walk children that are already cached — never trigger I/O.
        let Some(children) = node.cached_children() else {
            return false;
        };

        for child in children {
            if self.find_and_expand(&child, target_id) {
                self.expanded.insert(node_id);
                return true;
            }
        }

        false
    }

    /// Rebuild the flattened visible row list from the current expansion state.
    fn rebuild(&mut self) {
        self.visible_rows.clear();
        Self::flatten(Arc::clone(&self.root), 0, &self.expanded, &self.loading, &mut self.visible_rows);
    }

    /// Recursively flatten the tree into visible rows.
    fn flatten(
        node: Arc<UiNodeData>,
        depth: usize,
        expanded: &HashSet<String>,
        loading: &HashSet<String>,
        out: &mut Vec<VisibleRow>,
    ) {
        let id = node.cached_id().unwrap_or_else(|| "<uncached-node>".to_string());
        let is_expanded = expanded.contains(&id);
        let label = node.cached_label().unwrap_or_else(|| id.clone());
        let is_valid = node.cached_is_valid().unwrap_or(true);
        let is_loading = loading.contains(&id);

        let loaded_children = if is_expanded { node.cached_children() } else { None };
        let has_children = match loaded_children.as_ref() {
            Some(children) => is_loading || !children.is_empty(),
            None => is_loading || node.cached_has_children().unwrap_or(true),
        };

        out.push(VisibleRow {
            id,
            label,
            depth,
            has_children,
            is_expanded,
            is_valid,
            is_loading,
            data: Arc::clone(&node),
        });

        if let Some(children) = loaded_children {
            Self::flatten_children(children, depth, expanded, loading, out);
        }
    }

    fn flatten_children(
        children: Vec<Arc<UiNodeData>>,
        parent_depth: usize,
        expanded: &HashSet<String>,
        loading: &HashSet<String>,
        out: &mut Vec<VisibleRow>,
    ) {
        for child in children {
            Self::flatten(child, parent_depth + 1, expanded, loading, out);
        }
    }
}
