//! ViewModel: Overall application state for the Inspector.

use crate::model::tree_data::{DisplayAttribute, SearchResultItem, UiNodeData};
use crate::viewmodel::tree_vm::TreeViewModel;
use platynui_core::platform::HighlightRequest;
use platynui_runtime::Runtime;
use std::sync::Arc;
use std::time::Duration;

/// Top-level ViewModel holding the complete inspector state.
pub struct InspectorViewModel {
    /// Tree view model (expand/collapse, flattened rows).
    pub tree: TreeViewModel,
    /// Currently selected row index (mouse click or keyboard).
    pub selected_index: Option<usize>,
    /// Currently focused row index (keyboard navigation).
    pub focused_index: usize,
    /// XPath search text.
    pub search_text: String,
    /// Whether the window should stay on top.
    pub always_on_top: bool,
    /// Cached attributes for the currently selected node.
    pub selected_attributes: Vec<DisplayAttribute>,
    /// Label for the currently selected node.
    pub selected_label: String,
    /// Results from XPath evaluation.
    pub results: Vec<SearchResultItem>,
    /// Status / error message for the results panel.
    pub result_status: Option<String>,
    /// When true, the tree view should scroll to the focused row on the next frame.
    /// Consumed (set to false) after rendering.
    pub scroll_to_focused: bool,
    /// PlatynUI runtime (kept alive for the entire application).
    runtime: Arc<Runtime>,
}

impl InspectorViewModel {
    /// Create a new inspector ViewModel backed by the given runtime.
    pub fn new(runtime: Arc<Runtime>) -> Self {
        let desktop_node = runtime.desktop_node();
        let root_data = Arc::new(UiNodeData::new(desktop_node));
        let tree = TreeViewModel::new(root_data);

        Self {
            tree,
            selected_index: None,
            focused_index: 0,
            search_text: String::new(),
            always_on_top: false,
            selected_attributes: Vec::new(),
            selected_label: String::new(),
            results: Vec::new(),
            result_status: None,
            scroll_to_focused: false,
            runtime,
        }
    }

    /// Select a tree node by index, updating the properties panel and highlighting.
    pub fn select_node(&mut self, index: usize) {
        self.selected_index = Some(index);
        self.focused_index = index;
        self.scroll_to_focused = true;

        if let Some(row) = self.tree.rows().get(index) {
            self.selected_label = row.label.clone();
            self.selected_attributes = row.data.display_attributes();

            // Highlight bounds on screen (skip root desktop node)
            let is_root = !row.data.has_parent();
            if !is_root {
                if let Some(bounds) = row.data.bounds_rect() {
                    let rt = Arc::clone(&self.runtime);
                    std::thread::spawn(move || {
                        let req = HighlightRequest::new(bounds).with_duration(Duration::from_millis(1500));
                        if let Err(err) = rt.highlight(&req) {
                            tracing::error!(%err, "highlight failed");
                        }
                    });
                } else {
                    let rt = Arc::clone(&self.runtime);
                    std::thread::spawn(move || {
                        let _ = rt.clear_highlight();
                    });
                }
            }
        }
    }

    /// Navigate up one row.
    pub fn navigate_up(&mut self) {
        if self.focused_index > 0 {
            self.focused_index -= 1;
            self.select_node(self.focused_index);
        }
    }

    /// Navigate down one row.
    pub fn navigate_down(&mut self) {
        if self.focused_index + 1 < self.tree.row_count() {
            self.focused_index += 1;
            self.select_node(self.focused_index);
        }
    }

    /// Navigate left: collapse or go to parent.
    pub fn navigate_left(&mut self) {
        let idx = self.focused_index;
        if let Some(row) = self.tree.rows().get(idx) {
            if row.has_children && row.is_expanded {
                self.tree.collapse(idx);
            } else if let Some(parent) = self.tree.parent_index(idx) {
                self.focused_index = parent;
                self.select_node(parent);
            }
        }
    }

    /// Navigate right: expand or go to first child.
    pub fn navigate_right(&mut self) {
        let idx = self.focused_index;
        let count = self.tree.row_count();
        if let Some(row) = self.tree.rows().get(idx).cloned() {
            if row.has_children && !row.is_expanded {
                self.tree.expand(idx);
            } else if row.has_children && row.is_expanded && idx + 1 < count {
                self.focused_index = idx + 1;
                self.select_node(idx + 1);
            }
        }
    }

    /// Navigate to the first row.
    pub fn navigate_home(&mut self) {
        if self.tree.row_count() > 0 {
            self.focused_index = 0;
            self.select_node(0);
        }
    }

    /// Navigate to the last row.
    pub fn navigate_end(&mut self) {
        let count = self.tree.row_count();
        if count > 0 {
            self.focused_index = count - 1;
            self.select_node(count - 1);
        }
    }

    /// Navigate up by a page (~15 rows).
    pub fn navigate_page_up(&mut self) {
        self.focused_index = self.focused_index.saturating_sub(15);
        self.select_node(self.focused_index);
    }

    /// Navigate down by a page (~15 rows).
    pub fn navigate_page_down(&mut self) {
        let count = self.tree.row_count();
        if count > 0 {
            self.focused_index = (self.focused_index + 15).min(count - 1);
            self.select_node(self.focused_index);
        }
    }

    /// Refresh a specific tree row.
    pub fn refresh_row(&mut self, index: usize) {
        self.tree.refresh_row(index);
    }

    /// Refresh a tree row and its entire subtree.
    pub fn refresh_subtree(&mut self, index: usize) {
        self.tree.refresh_subtree(index);
    }

    /// Evaluate the current `search_text` as an XPath expression.
    pub fn evaluate_xpath(&mut self) {
        let xpath = self.search_text.trim();
        if xpath.is_empty() {
            self.results.clear();
            self.result_status = None;
            return;
        }

        let start = std::time::Instant::now();
        match self.runtime.evaluate(None, xpath) {
            Ok(items) => {
                let count = items.len();
                self.results = items.iter().map(SearchResultItem::from_evaluation_item).collect();
                let elapsed = start.elapsed();
                self.result_status = Some(format!(
                    "{count} result{} ({:.1}ms)",
                    if count == 1 { "" } else { "s" },
                    elapsed.as_secs_f64() * 1000.0,
                ));
            }
            Err(err) => {
                self.results.clear();
                self.result_status = Some(format!("Error: {err}"));
            }
        }
    }

    /// When a result is clicked, reveal its node in the tree and select it.
    pub fn reveal_and_select_result(&mut self, result_index: usize) {
        let item = match self.results.get(result_index) {
            Some(item) => item.clone(),
            None => return,
        };

        if let Some(node) = item.ui_node() {
            if let Some(tree_index) = self.tree.reveal_node(node) {
                self.select_node(tree_index);
            } else {
                tracing::warn!("could not find node in tree after expanding ancestors");
            }
        }
    }
}
