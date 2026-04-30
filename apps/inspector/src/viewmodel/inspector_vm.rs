//! ViewModel: Overall application state for the Inspector.

use crate::model::tree_data::{DisplayAttribute, SearchResultItem, UiNodeData};
use crate::viewmodel::{async_tasks, tree_vm::TreeViewModel};
use eframe::egui;
use egui_async::Bind;
use platynui_runtime::Runtime;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

const SEARCH_RESULTS_PER_FRAME: usize = 512;
const INITIAL_TREE_LOAD_DEFER_FRAMES: u8 = 2;
const TREE_CHILD_PROGRESS_EVENTS_PER_FRAME: usize = 256;

#[derive(Clone)]
struct ChildLoadRequest {
    node: Arc<UiNodeData>,
    mode: async_tasks::ChildLoadMode,
}

struct ActiveChildLoad {
    request_id: u64,
    node: Arc<UiNodeData>,
    node_id: Option<String>,
}

#[derive(Clone, Debug)]
enum ResultStatus {
    Searching { count: usize, elapsed_secs: f64 },
    Draining { visible_count: usize, total_count: usize, pending_count: usize },
    Completed { count: usize, elapsed_ms: f64 },
    Limited { count: usize, limit: usize, elapsed_ms: f64 },
    Cancelled { count: usize, elapsed_ms: f64 },
    Error(String),
}

impl ResultStatus {
    fn text(&self) -> String {
        match self {
            Self::Searching { count, elapsed_secs } => {
                format!("Searching\u{2026} {count} result{} ({elapsed_secs:.1}s)", if *count == 1 { "" } else { "s" })
            }
            Self::Draining { visible_count, total_count, pending_count } => {
                format!("Loading results\u{2026} {visible_count}/{total_count} shown, {pending_count} queued")
            }
            Self::Completed { count, elapsed_ms } => {
                format!("{count} result{} ({elapsed_ms:.1}ms)", if *count == 1 { "" } else { "s" })
            }
            Self::Limited { count, limit, elapsed_ms } => {
                format!("Showing first {count} results; limit {limit} reached ({elapsed_ms:.1}ms)")
            }
            Self::Cancelled { count, elapsed_ms } => {
                format!("Cancelled \u{2014} {count} result{} ({elapsed_ms:.1}ms)", if *count == 1 { "" } else { "s" })
            }
            Self::Error(summary) => format!("Error: {summary}"),
        }
    }
}

fn next_epoch(epoch: &AtomicU64) -> u64 {
    epoch.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
}

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
    result_status: Option<ResultStatus>,
    /// Full search error message shown below the search field.
    search_error_hint: Option<String>,
    /// Focused row index in the results panel (keyboard cursor).
    pub result_focused_index: usize,
    /// When true, the tree view should scroll to the focused row on the next frame.
    /// Consumed (set to false) after rendering.
    pub scroll_to_focused: bool,
    /// PlatynUI runtime (kept alive for the entire application).
    runtime: Arc<Runtime>,
    /// Root node data kept for delayed initial tree loading after first paint.
    init_root: Option<Arc<UiNodeData>>,
    /// Number of frames to wait before starting initial tree iteration.
    init_defer_frames: u8,
    /// On-demand tree child loading handled by egui-async.
    child_load_task: Bind<async_tasks::ChildLoadResult, String>,
    /// Shared in-flight tree child progress for incremental UI updates.
    child_load_progress: async_tasks::SharedChildLoadProgress,
    /// Queued expand/refresh child loads while one child task is active.
    pending_child_loads: VecDeque<ChildLoadRequest>,
    /// Currently active child load metadata.
    active_child_load: Option<ActiveChildLoad>,
    /// Monotonic request id to ignore stale child-load results.
    child_load_request_id: u64,
    /// XPath search state handled by egui-async.
    search_task: Bind<async_tasks::SearchResult, String>,
    /// Shared in-flight search progress for incremental UI updates.
    search_progress: Option<async_tasks::SharedSearchProgress>,
    /// Completed search metadata waiting for the visible progress queue to drain.
    search_completion: Option<async_tasks::SearchResult>,
    /// Cancellation flag for in-flight search task.
    search_cancel_flag: Option<Arc<AtomicBool>>,
    /// Start time of in-flight search for live status.
    search_started_at: Option<Instant>,
    /// Maximum XPath search results to collect for the Inspector UI.
    search_result_limit: Option<usize>,
    /// Background reveal (tree sync) state handled by egui-async.
    reveal_task: Bind<async_tasks::RevealResult, String>,
    /// Latest reveal request epoch.
    reveal_epoch: Arc<AtomicU64>,
    /// Background selected-node details state handled by egui-async.
    selection_task: Bind<async_tasks::SelectionResult, String>,
    /// Highlight clear/show task handled by egui-async.
    highlight_task: Bind<async_tasks::HighlightResult, String>,
    /// Latest highlight request epoch.
    highlight_epoch: Arc<AtomicU64>,
    /// Monotonic request id to ignore stale selection results.
    selection_request_id: u64,
    /// Frame counter for spinner animation.
    spinner_frame: usize,
}

impl InspectorViewModel {
    /// Create a new inspector ViewModel backed by the given runtime.
    pub fn new(runtime: Arc<Runtime>, root_data: Arc<UiNodeData>, search_result_limit: Option<usize>) -> Self {
        let mut tree = TreeViewModel::new(Arc::clone(&root_data));
        let init_root = if root_data.cached_children().is_some() {
            tree.expand_root();
            None
        } else {
            Some(root_data)
        };

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
            search_error_hint: None,
            result_focused_index: 0,
            scroll_to_focused: false,
            runtime,
            init_root,
            // Let the first frames paint before touching potentially slow
            // UIA root-child iteration.
            init_defer_frames: INITIAL_TREE_LOAD_DEFER_FRAMES,
            child_load_task: Bind::new(false),
            child_load_progress: async_tasks::new_child_load_progress(),
            pending_child_loads: VecDeque::new(),
            active_child_load: None,
            child_load_request_id: 0,
            search_task: Bind::new(false),
            search_progress: None,
            search_completion: None,
            search_cancel_flag: None,
            search_started_at: None,
            search_result_limit,
            reveal_task: Bind::new(false),
            reveal_epoch: Arc::new(AtomicU64::new(0)),
            selection_task: Bind::new(false),
            highlight_task: Bind::new(false),
            highlight_epoch: Arc::new(AtomicU64::new(0)),
            selection_request_id: 0,
            spinner_frame: 0,
        }
    }

    /// Poll the initial tree load state. Call this every frame.
    pub fn poll_initial_load(&mut self, ctx: &egui::Context) {
        if self.init_defer_frames > 0 {
            self.init_defer_frames -= 1;
            ctx.request_repaint();
            return;
        }

        if let Some(root) = self.init_root.take() {
            self.enqueue_child_load(root, async_tasks::ChildLoadMode::Load);
            ctx.request_repaint();
        }
    }

    /// Poll on-demand tree child loading. Call this every frame.
    pub fn poll_child_load(&mut self, ctx: &egui::Context) {
        let mut progress_changed_tree = false;
        match async_tasks::drain_child_load_progress(&self.child_load_progress, TREE_CHILD_PROGRESS_EVENTS_PER_FRAME) {
            Ok(drain) => {
                for event in drain.events {
                    self.apply_child_load_progress_event(event);
                    progress_changed_tree = true;
                }
                if drain.pending_count > 0 {
                    ctx.request_repaint();
                }
            }
            Err(err) => {
                tracing::warn!(%err, "failed to drain tree child-load progress");
            }
        }

        if progress_changed_tree {
            self.tree.rebuild_rows();
            ctx.request_repaint();
        }

        if let Some(result) = self.child_load_task.take() {
            let active = self.active_child_load.take();
            match result {
                Ok(result) => {
                    if let Some(active) = active.as_ref()
                        && active.request_id != result.request_id
                    {
                        if let Some(node_id) = active.node_id.as_deref() {
                            self.tree.set_loading(node_id, false);
                        }
                        self.start_next_child_load();
                        ctx.request_repaint();
                        return;
                    }

                    if let Some(active) = active.as_ref()
                        && let Some(node_id) = active.node_id.as_deref()
                    {
                        self.tree.set_loading(node_id, false);
                    }
                    self.tree.set_loading(&result.node_id, false);
                    self.tree.rebuild_rows();
                    self.enqueue_visible_expanded_child_loads();
                    self.start_next_child_load();
                    ctx.request_repaint();
                }
                Err(err) => {
                    if let Some(active) = active.as_ref()
                        && let Some(node_id) = active.node_id.as_deref()
                    {
                        self.tree.set_loading(node_id, false);
                    }
                    tracing::warn!(%err, "tree child load task failed");
                    self.start_next_child_load();
                    ctx.request_repaint();
                }
            }
        } else if self.child_load_task.is_pending() || !self.pending_child_loads.is_empty() {
            self.start_next_child_load();
            ctx.request_repaint();
        }
    }

    fn apply_child_load_progress_event(&mut self, event: async_tasks::ChildLoadProgressEvent) {
        match event {
            async_tasks::ChildLoadProgressEvent::Started { request_id, node_id } => {
                if let Some(active) = self.active_child_load.as_mut()
                    && active.request_id == request_id
                {
                    active.node_id = Some(node_id.clone());
                    self.tree.set_loading(&node_id, true);
                    if Arc::ptr_eq(&active.node, self.tree.root()) {
                        self.tree.expand_root();
                    }
                }
            }
            async_tasks::ChildLoadProgressEvent::ChildLoaded | async_tasks::ChildLoadProgressEvent::NodeUpdated => {}
        }
    }

    fn enqueue_child_load(&mut self, node: Arc<UiNodeData>, mode: async_tasks::ChildLoadMode) {
        let node_id = node.cached_id();
        let active_for_node = node_id.as_ref().is_some_and(|node_id| {
            self.active_child_load.as_ref().is_some_and(|active| active.node_id.as_deref() == Some(node_id.as_str()))
        });
        let pending_for_node = self.pending_child_loads.iter().any(|request| {
            node_id.as_ref().is_some_and(|node_id| request.node.cached_id().as_deref() == Some(node_id.as_str()))
        });
        if mode == async_tasks::ChildLoadMode::Load && (active_for_node || pending_for_node) {
            return;
        }

        if mode != async_tasks::ChildLoadMode::Load {
            self.pending_child_loads.retain(|request| {
                node_id.as_ref().is_none_or(|node_id| request.node.cached_id().as_deref() != Some(node_id.as_str()))
            });
        }

        if let Some(node_id) = node_id.as_deref() {
            self.tree.set_loading(node_id, true);
        }
        self.pending_child_loads.push_back(ChildLoadRequest { node, mode });
        self.start_next_child_load();
    }

    fn enqueue_visible_expanded_child_loads(&mut self) {
        let missing_children = self.tree.expanded_nodes_missing_children();
        for node in missing_children {
            self.enqueue_child_load(node, async_tasks::ChildLoadMode::Load);
        }
    }

    fn start_next_child_load(&mut self) {
        if self.active_child_load.is_some() || self.child_load_task.is_pending() {
            return;
        }

        if let Some(request) = self.pending_child_loads.pop_front() {
            let node_id = request.node.cached_id();

            self.child_load_request_id = self.child_load_request_id.wrapping_add(1);
            let request_id = self.child_load_request_id;
            self.active_child_load = Some(ActiveChildLoad { request_id, node: Arc::clone(&request.node), node_id });
            self.child_load_task.refresh(async_tasks::child_load_task(
                request_id,
                request.node,
                request.mode,
                Arc::clone(&self.child_load_progress),
            ));
        }
    }

    /// Select a tree node by index, updating the attributes panel and highlighting.
    pub fn select_node(&mut self, index: usize) {
        // Ignore redundant re-selection of the same row to avoid repeatedly
        // clearing attributes and replacing in-flight background loads.
        if self.selected_index == Some(index) {
            self.focused_index = index;
            self.scroll_to_focused = true;
            return;
        }

        self.selected_index = Some(index);
        self.focused_index = index;
        self.scroll_to_focused = true;

        if let Some(row) = self.tree.rows().get(index) {
            self.selected_label = row.label.clone();

            self.selection_request_id = self.selection_request_id.wrapping_add(1);
            let request_id = self.selection_request_id;
            let selected_label = row.label.clone();
            let node_data = Arc::clone(&row.data);

            self.selection_task.refresh(async_tasks::selection_task(request_id, selected_label, node_data));
        }
    }

    /// Poll selected-node details loading. Call this every frame.
    pub fn poll_selection(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.selection_task.take() {
            match result {
                Ok(async_tasks::SelectionResult { request_id, selected_label, attributes, is_root, bounds }) => {
                    // Ignore stale result from an older selection.
                    if request_id != self.selection_request_id {
                        return;
                    }

                    self.selected_label = selected_label;
                    self.selected_attributes = attributes;

                    if is_root {
                        self.clear_highlight();
                    } else {
                        self.highlight_bounds(bounds);
                    }

                    ctx.request_repaint();
                }
                Err(err) => {
                    tracing::warn!(%err, "selection task failed");
                }
            }
        } else if self.selection_task.is_pending() {
            ctx.request_repaint();
        }
    }

    /// Poll highlight task state. Call this every frame.
    pub fn poll_highlight(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.highlight_task.take() {
            match result {
                Ok(_) => {}
                Err(err) => {
                    tracing::warn!(%err, "highlight task failed");
                }
            }
        } else if self.highlight_task.is_pending() {
            ctx.request_repaint();
        }
    }

    /// Return true when any egui-async background task is currently running.
    pub fn has_pending_background_work(&mut self) -> bool {
        self.child_load_task.is_pending()
            || !self.pending_child_loads.is_empty()
            || self.search_task.is_pending()
            || self.search_completion.is_some()
            || self.reveal_task.is_pending()
            || self.selection_task.is_pending()
            || self.highlight_task.is_pending()
    }

    /// Current status text for the global status bar.
    pub fn status_bar_text(&self) -> Option<String> {
        self.result_status.as_ref().map(ResultStatus::text)
    }

    /// Full search error hint shown under the search field.
    pub fn search_error_hint(&self) -> Option<&str> {
        self.search_error_hint.as_deref()
    }

    /// Clear search error hint when the query text changes.
    pub fn on_search_text_changed(&mut self) {
        self.search_error_hint = None;
        if matches!(self.result_status, Some(ResultStatus::Error(_))) {
            self.result_status = None;
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
                self.expand_row(idx);
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

    /// Navigate up by a page.
    pub fn navigate_page_up(&mut self, page_size: usize) {
        self.focused_index = self.focused_index.saturating_sub(page_size);
        self.select_node(self.focused_index);
    }

    /// Navigate down by a page.
    pub fn navigate_page_down(&mut self, page_size: usize) {
        let count = self.tree.row_count();
        if count > 0 {
            self.focused_index = (self.focused_index + page_size).min(count - 1);
            self.select_node(self.focused_index);
        }
    }

    /// Refresh a specific tree row.
    pub fn refresh_row(&mut self, index: usize) {
        if let Some(node) = self.tree.refresh_row(index) {
            self.enqueue_child_load(node, async_tasks::ChildLoadMode::Refresh);
        }
    }

    /// Refresh a tree row and its entire subtree.
    pub fn refresh_subtree(&mut self, index: usize) {
        if let Some(node) = self.tree.refresh_subtree(index) {
            self.enqueue_child_load(node, async_tasks::ChildLoadMode::RefreshSubtree);
        }
    }

    /// Toggle a tree row and queue child loading if expansion needs it.
    pub fn toggle_row(&mut self, index: usize) {
        if let Some(node) = self.tree.toggle(index) {
            self.enqueue_child_load(node, async_tasks::ChildLoadMode::Load);
        }
    }

    /// Expand a tree row and queue child loading if needed.
    pub fn expand_row(&mut self, index: usize) {
        if let Some(node) = self.tree.expand(index) {
            self.enqueue_child_load(node, async_tasks::ChildLoadMode::Load);
        }
    }

    fn highlight_bounds(&mut self, bounds: Option<platynui_core::types::Rect>) {
        if let Some(bounds) = bounds {
            let rt = Arc::clone(&self.runtime);
            let epoch = next_epoch(&self.highlight_epoch);
            self.highlight_task.refresh(async_tasks::highlight_bounds_task(
                rt,
                epoch,
                Arc::clone(&self.highlight_epoch),
                bounds,
            ));
            return;
        }

        self.clear_highlight();
    }

    fn clear_highlight(&mut self) {
        let rt = Arc::clone(&self.runtime);
        let epoch = next_epoch(&self.highlight_epoch);
        self.highlight_task.refresh(async_tasks::clear_highlight_task(rt, epoch, Arc::clone(&self.highlight_epoch)));
    }

    /// Clear current search results and status.
    pub fn clear_results(&mut self) {
        self.results.clear();
        self.result_status = None;
        self.search_error_hint = None;
        self.search_progress = None;
        self.search_completion = None;
        self.result_focused_index = 0;
    }

    /// Refresh the currently selected tree row, if any.
    pub fn refresh_selected_row(&mut self) {
        if let Some(index) = self.selected_index {
            self.refresh_row(index);
        }
    }

    /// Refresh the currently selected tree row subtree, if any.
    pub fn refresh_selected_subtree(&mut self) {
        if let Some(index) = self.selected_index {
            self.refresh_subtree(index);
        }
    }

    /// Highlight the currently selected tree row, if any.
    pub fn highlight_selected_row(&mut self) {
        if let Some(index) = self.selected_index {
            self.highlight_row(index);
        }
    }

    /// Highlight a specific tree row, if possible.
    pub fn highlight_row(&mut self, index: usize) {
        if let Some(row) = self.tree.rows().get(index) {
            let rt = Arc::clone(&self.runtime);
            let node_data = Arc::clone(&row.data);
            let epoch = next_epoch(&self.highlight_epoch);
            self.highlight_task.refresh(async_tasks::highlight_node_task(
                rt,
                epoch,
                Arc::clone(&self.highlight_epoch),
                node_data,
            ));
        }
    }

    /// Highlight a specific search result, if it is backed by a UI node.
    pub fn highlight_result(&mut self, index: usize) {
        if let Some(result) = self.results.get(index)
            && let Some(node) = result.ui_node().cloned()
        {
            let rt = Arc::clone(&self.runtime);
            let node_data = Arc::new(UiNodeData::new(node));
            let epoch = next_epoch(&self.highlight_epoch);
            self.highlight_task.refresh(async_tasks::highlight_node_task(
                rt,
                epoch,
                Arc::clone(&self.highlight_epoch),
                node_data,
            ));
        }
    }

    /// Expand the currently selected tree row, if any.
    pub fn expand_selected_row(&mut self) {
        if let Some(index) = self.selected_index {
            self.expand_row(index);
        }
    }

    /// Collapse the currently selected tree row, if any.
    pub fn collapse_selected_row(&mut self) {
        if let Some(index) = self.selected_index {
            self.tree.collapse(index);
        }
    }

    /// Evaluate the current `search_text` as an XPath expression (non-blocking).
    ///
    /// Cancels any in-progress search, then starts an egui-async background task.
    pub fn evaluate_xpath(&mut self) {
        // Cancel any running search first.
        self.cancel_search();

        let xpath = self.search_text.trim().to_string();
        if xpath.is_empty() {
            self.results.clear();
            self.result_status = None;
            self.search_error_hint = None;
            self.search_progress = None;
            self.search_completion = None;
            self.result_focused_index = 0;
            return;
        }

        // Clear previous results.
        self.results.clear();
        self.result_status = Some(ResultStatus::Searching { count: 0, elapsed_secs: 0.0 });
        self.search_error_hint = None;
        self.search_completion = None;
        self.result_focused_index = 0;

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let progress = async_tasks::new_search_progress();
        self.search_progress = Some(Arc::clone(&progress));
        self.search_cancel_flag = Some(Arc::clone(&cancel_flag));
        self.search_started_at = Some(Instant::now());
        self.search_task.refresh(async_tasks::search_task(
            Arc::clone(&self.runtime),
            xpath,
            cancel_flag,
            progress,
            self.search_result_limit,
        ));
    }

    /// Poll the background search for new results. Call this every frame.
    ///
    /// While a search is active, requests a repaint so the next frame polls again.
    pub fn poll_search(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.search_task.take() {
            self.search_cancel_flag = None;
            self.search_started_at = None;

            match result {
                Ok(summary) => {
                    self.search_completion = Some(summary);
                }
                Err(err) => {
                    self.search_progress = None;
                    self.search_completion = None;
                    self.search_error_hint = Some(err.clone());
                    self.result_status = Some(ResultStatus::Error(short_error_summary(&err)));
                }
            }
        }

        let mut drained_pending_count = 0;
        let mut drained_total_count = self.results.len();
        if let Some(progress) = &self.search_progress {
            match async_tasks::drain_search_progress(progress, SEARCH_RESULTS_PER_FRAME) {
                Ok(drain) => {
                    drained_total_count = drain.total_count;
                    drained_pending_count = drain.pending_count;
                    if !drain.results.is_empty() {
                        self.results.extend(drain.results);
                    }
                }
                Err(err) => {
                    tracing::warn!(%err, "failed to drain search progress");
                    self.search_progress = None;
                    self.search_completion = None;
                    self.result_status = Some(ResultStatus::Error(err));
                    return;
                }
            }
        }

        if let Some(summary) = &self.search_completion {
            if drained_pending_count == 0 {
                let summary = self.search_completion.take().expect("search completion checked above");
                self.search_progress = None;
                let count = self.results.len().max(summary.result_count);
                let elapsed_ms = summary.elapsed.as_secs_f64() * 1000.0;
                self.result_status = Some(if summary.cancelled {
                    ResultStatus::Cancelled { count, elapsed_ms }
                } else if let Some(limit) = summary.limit_reached {
                    ResultStatus::Limited { count, limit, elapsed_ms }
                } else {
                    ResultStatus::Completed { count, elapsed_ms }
                });
            } else {
                self.result_status = Some(ResultStatus::Draining {
                    visible_count: self.results.len(),
                    total_count: summary.result_count,
                    pending_count: drained_pending_count,
                });
                ctx.request_repaint();
            }
        } else if self.search_task.is_pending() {
            if drained_pending_count > 0 {
                ctx.request_repaint();
            }

            // Update live status while search is in progress.
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
            let elapsed = self.search_started_at.map_or(0.0, |start| start.elapsed().as_secs_f64());
            let count = self.results.len().max(drained_total_count);
            self.result_status = Some(ResultStatus::Searching { count, elapsed_secs: elapsed });
            // Request repaint so next frame continues polling.
            ctx.request_repaint();
        }
    }

    /// Cancel the current background search, if any.
    pub fn cancel_search(&mut self) {
        if self.search_task.is_pending() || self.search_completion.is_some() || self.search_progress.is_some() {
            if let Some(cancel_flag) = &self.search_cancel_flag {
                cancel_flag.store(true, Ordering::Relaxed);
            }
            if self.search_task.is_pending() {
                self.search_task.abort();
            }

            let count = self.results.len();
            let elapsed_ms = self.search_started_at.map_or(0.0, |start| start.elapsed().as_secs_f64() * 1000.0);
            self.result_status = Some(ResultStatus::Cancelled { count, elapsed_ms });

            self.search_progress = None;
            self.search_completion = None;
            self.search_cancel_flag = None;
            self.search_started_at = None;
        }
    }

    /// Returns `true` if a background search is currently running.
    pub fn is_searching(&mut self) -> bool {
        self.search_task.is_pending() || self.search_completion.is_some()
    }

    /// When a result is selected, reveal its node in the tree (non-blocking).
    ///
    /// Spawns a background task that pre-loads the ancestor path into the
    /// `UiNodeData` cache (expensive AT-SPI / UIA calls). Once ready, the
    /// UI thread performs the cheap expand + rebuild + select.
    ///
    /// If a previous reveal is still in progress it is cancelled.
    pub fn reveal_and_select_result(&mut self, result_index: usize) {
        // Cancel any in-flight reveal and start a fresh request.
        let epoch = next_epoch(&self.reveal_epoch);
        self.reveal_task.abort();

        let item = match self.results.get(result_index) {
            Some(item) => item.clone(),
            None => return,
        };

        let Some(target_node) = item.ui_node().cloned() else {
            return;
        };

        if !target_node.is_valid() {
            tracing::warn!(epoch, "reveal_result: target node is no longer valid");
            return;
        }

        let target_id = target_node.runtime_id().as_str().to_string();

        // Quick path: node is already visible in the cached tree.
        if self.tree.reveal_node_cached(&target_id) {
            self.select_node_if_visible(&target_id);
            return;
        }

        // Slow path: spawn egui-async task to pre-load ancestor caches.
        let root = Arc::clone(self.tree.root());
        self.reveal_task.refresh(async_tasks::reveal_task(epoch, Arc::clone(&self.reveal_epoch), root, target_node));
    }

    /// Helper: select a node if it's visible in the tree.
    fn select_node_if_visible(&mut self, target_id: &str) {
        if let Some(tree_index) = self.tree.rows().iter().position(|row| row.id == target_id) {
            self.select_node(tree_index);
        } else {
            tracing::warn!(target_id, "reveal_node: not found in visible rows after expand");
        }
    }

    /// Poll the background reveal operation via egui-async. Call this every frame.
    pub fn poll_reveal(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.reveal_task.take() {
            match result {
                Ok(async_tasks::RevealResult::Ready { epoch, target_id }) => {
                    let latest_epoch = self.reveal_epoch.load(Ordering::Relaxed);
                    if epoch != latest_epoch {
                        return;
                    }
                    // Now find_and_expand is cheap (children are cached).
                    if self.tree.reveal_node_cached(&target_id) {
                        self.select_node_if_visible(&target_id);
                    } else {
                        tracing::warn!(target_id, "reveal_node: not found after background preload");
                    }
                }
                Ok(async_tasks::RevealResult::Cancelled) => {}
                Err(err) => {
                    tracing::error!(%err, "reveal task failed");
                }
            }
        } else if self.reveal_task.is_pending() {
            // Still working — request repaint for next poll.
            ctx.request_repaint();
        }
    }
}

fn short_error_summary(error: &str) -> String {
    let first_line = error.lines().find(|line| !line.trim().is_empty()).unwrap_or("XPath evaluation failed").trim();
    let summary_with_location = if let Some((line, column)) = extract_line_column(error) {
        format!("L{line}:C{column} {first_line}")
    } else {
        first_line.to_string()
    };
    const MAX_CHARS: usize = 96;
    if summary_with_location.chars().count() <= MAX_CHARS {
        summary_with_location
    } else {
        let shortened: String = summary_with_location.chars().take(MAX_CHARS - 1).collect();
        format!("{shortened}\u{2026}")
    }
}

fn extract_line_column(error: &str) -> Option<(usize, usize)> {
    let line = number_after_keyword(error, "line")?;
    let column = number_after_keyword(error, "column")
        .or_else(|| number_after_keyword(error, "col"))
        .or_else(|| number_after_keyword(error, "position"))?;
    Some((line, column))
}

fn number_after_keyword(input: &str, keyword: &str) -> Option<usize> {
    let lower = input.to_ascii_lowercase();
    let idx = lower.find(keyword)?;
    let rest = &lower[idx + keyword.len()..];

    let mut digits = String::new();
    let mut seen_digit = false;
    for ch in rest.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            seen_digit = true;
        } else if seen_digit {
            break;
        }
    }

    if digits.is_empty() { None } else { digits.parse::<usize>().ok() }
}
