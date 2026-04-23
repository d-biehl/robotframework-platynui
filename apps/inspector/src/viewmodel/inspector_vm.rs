//! ViewModel: Overall application state for the Inspector.

use crate::model::automation;
use crate::model::tree_data::{DisplayAttribute, SearchResultItem, UiNodeData};
use crate::viewmodel::tree_vm::TreeViewModel;
use eframe::egui;
use platynui_core::platform::HighlightRequest;
use platynui_core::ui::UiNode;
use platynui_runtime::Runtime;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

/// Messages from initial background tree loading.
enum InitLoadMsg {
    Node(Arc<crate::model::tree_data::UiNodeData>),
    Done,
}

/// State of initial background tree loading.
struct ActiveInitLoad {
    receiver: mpsc::Receiver<InitLoadMsg>,
}

/// Messages sent from the background reveal thread to the UI thread.
enum RevealMsg {
    /// The ancestor path has been pre-loaded; ready to expand + select.
    Ready {
        /// Runtime ID of the target node to reveal.
        target_id: String,
    },
    /// Reveal was cancelled (a new reveal was started).
    Cancelled,
}

/// Messages from background node-selection details loading.
enum SelectionMsg {
    /// Selected node details are ready for UI update.
    Ready {
        request_id: u64,
        selected_label: String,
        attributes: Vec<DisplayAttribute>,
        is_root: bool,
        bounds: Option<platynui_core::types::Rect>,
        node_id: String,
    },
}

/// State of in-progress selected-node details loading.
struct ActiveSelection {
    receiver: mpsc::Receiver<SelectionMsg>,
}

/// State of an in-progress background reveal operation.
struct ActiveReveal {
    /// Receiver for the reveal result.
    receiver: mpsc::Receiver<RevealMsg>,
    /// Cancel flag shared with the background thread.
    cancel_flag: Arc<AtomicBool>,
}

/// Messages sent from the background evaluation thread to the UI thread.
pub enum SearchMsg {
    /// A single evaluation result item.
    Result(SearchResultItem),
    /// Evaluation completed successfully.
    Done { elapsed: Duration },
    /// Evaluation failed with an error.
    Error(String),
    /// Evaluation was cancelled by the user.
    Cancelled,
}

/// State of an in-progress background XPath search.
pub struct ActiveSearch {
    /// Receiver for streaming results from the background thread.
    receiver: mpsc::Receiver<SearchMsg>,
    /// Cancel flag shared with the background thread and the XPath engine.
    cancel_flag: Arc<AtomicBool>,
    /// When the search was started (for elapsed time display).
    start: Instant,
    /// Number of results received so far.
    count: usize,
}

/// Spinner characters for the search status animation.
const SPINNER_CHARS: &[char] = &['◐', '◓', '◑', '◒'];

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
    /// Focused row index in the results panel (keyboard cursor).
    pub result_focused_index: usize,
    /// When true, the tree view should scroll to the focused row on the next frame.
    /// Consumed (set to false) after rendering.
    pub scroll_to_focused: bool,
    /// PlatynUI runtime (kept alive for the entire application).
    runtime: Arc<Runtime>,
    /// Root node kept for lazy start of initial tree loading on first frame.
    pending_init_root: Option<Arc<dyn UiNode>>,
    /// Number of frames to wait before starting initial tree iteration.
    defer_init_frames: u8,
    /// In-progress initial tree load background thread, if any.
    active_init_load: Option<ActiveInitLoad>,
    /// Currently active background search, if any.
    active_search: Option<ActiveSearch>,
    /// Currently active background reveal (tree sync), if any.
    active_reveal: Option<ActiveReveal>,
    /// In-progress selected-node details loading, if any.
    active_selection: Option<ActiveSelection>,
    /// Monotonic request id to ignore stale selection results.
    selection_request_id: u64,
    /// Frame counter for spinner animation.
    spinner_frame: usize,
}

impl InspectorViewModel {
    /// Create a new inspector ViewModel backed by the given runtime.
    pub fn new(runtime: Arc<Runtime>, preloaded_root_children: Vec<Arc<UiNodeData>>) -> Self {
        let rt_for_root = Arc::clone(&runtime);
        let desktop_node = automation::run(move || rt_for_root.desktop_node());
        let root_data = Arc::new(UiNodeData::new(Arc::clone(&desktop_node)));
        root_data.init_children_cache();

        for child in preloaded_root_children {
            root_data.push_cached_child(Arc::clone(&child));
        }

        let mut tree = TreeViewModel::new(Arc::clone(&root_data));
        if root_data.cached_children().is_some_and(|children| !children.is_empty()) {
            tree.expand_root();
        }

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
            result_focused_index: 0,
            scroll_to_focused: false,
            runtime,
            pending_init_root: if root_data.cached_children().is_some_and(|children| !children.is_empty()) {
                None
            } else {
                Some(desktop_node)
            },
            // Ensure one frame is presented before touching potentially slow
            // UIA root-child iteration.
            defer_init_frames: 1,
            active_init_load: None,
            active_search: None,
            active_reveal: None,
            active_selection: None,
            selection_request_id: 0,
            spinner_frame: 0,
        }
    }

    /// Poll the initial tree load state. Call this every frame.
    pub fn poll_initial_load(&mut self, ctx: &egui::Context) {
        if self.defer_init_frames > 0 {
            self.defer_init_frames -= 1;
            ctx.request_repaint();
            return;
        }

        if self.active_init_load.is_none()
            && let Some(root) = self.pending_init_root.take()
        {
            let (tx, rx) = mpsc::channel();
            automation::dispatch_tracked(automation::TaskKind::InitialLoad, move || {
                tracing::debug!(
                    thread_id = ?std::thread::current().id(),
                    "starting initial tree load on automation worker"
                );
                for node in root.children() {
                    let node_data = Arc::new(crate::model::tree_data::UiNodeData::new(node));
                    node_data.preload_caches();
                    if tx.send(InitLoadMsg::Node(node_data)).is_err() {
                        return;
                    }
                }
                tracing::debug!(
                    thread_id = ?std::thread::current().id(),
                    "completed initial tree load on automation worker"
                );
                let _ = tx.send(InitLoadMsg::Done);
            });

            self.active_init_load = Some(ActiveInitLoad { receiver: rx });
        }

        if self.active_init_load.is_none() {
            return;
        }

        // Drain a bounded batch per frame to keep UI updates smooth.
        const BATCH_SIZE: usize = 64;
        let mut loaded_batch: Vec<Arc<crate::model::tree_data::UiNodeData>> = Vec::new();
        let mut finished = false;

        if let Some(load) = self.active_init_load.as_ref() {
            for _ in 0..BATCH_SIZE {
                match load.receiver.try_recv() {
                    Ok(InitLoadMsg::Node(node_data)) => loaded_batch.push(node_data),
                    Ok(InitLoadMsg::Done) => {
                        finished = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        finished = true;
                        break;
                    }
                }
            }
        }

        let had_progress = !loaded_batch.is_empty();
        if had_progress {
            let root = Arc::clone(self.tree.root());
            for node_data in loaded_batch {
                root.push_cached_child(Arc::clone(&node_data));
            }
        }

        if finished {
            self.active_init_load = None;
        }

        if had_progress || finished {
            if self.tree.row_count() == 0 {
                // First children arrived — expand root so rows become visible.
                self.tree.expand_root();
            } else {
                // More children arrived — refresh visible rows from updated cache.
                self.tree.rebuild_rows();
            }
        }

        if self.active_init_load.is_some() || had_progress {
            ctx.request_repaint();
        }
    }

    /// Number of already-cached root children.
    pub fn loaded_root_children_count(&self) -> usize {
        self.tree.root().cached_children().map_or(0, |children| children.len())
    }

    /// Select a tree node by index, updating the properties panel and highlighting.
    pub fn select_node(&mut self, index: usize) {
        // Ignore redundant re-selection of the same row to avoid repeatedly
        // clearing properties and replacing in-flight background loads.
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
            let (tx, rx) = mpsc::channel();

            automation::dispatch_tracked(automation::TaskKind::Reveal, move || {
                let attributes = node_data.display_attributes();
                let is_root = !node_data.has_parent();
                let bounds = if is_root { None } else { node_data.bounds_rect() };
                let node_id = node_data.id();
                let _ =
                    tx.send(SelectionMsg::Ready { request_id, selected_label, attributes, is_root, bounds, node_id });
            });

            self.active_selection = Some(ActiveSelection { receiver: rx });
        }
    }

    /// Poll selected-node details loading. Call this every frame.
    pub fn poll_selection(&mut self, ctx: &egui::Context) {
        let Some(active) = self.active_selection.as_ref() else {
            return;
        };

        match active.receiver.try_recv() {
            Ok(SelectionMsg::Ready { request_id, selected_label, attributes, is_root, bounds, node_id }) => {
                self.active_selection = None;

                // Ignore stale result from an older selection.
                if request_id != self.selection_request_id {
                    return;
                }

                self.selected_label = selected_label;
                self.selected_attributes = attributes;

                if is_root {
                    self.clear_highlight();
                } else {
                    self.highlight_bounds(node_id, bounds);
                }

                ctx.request_repaint();
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint();
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.active_selection = None;
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
        self.tree.refresh_row(index);
    }

    /// Refresh a tree row and its entire subtree.
    pub fn refresh_subtree(&mut self, index: usize) {
        self.tree.refresh_subtree(index);
    }

    fn highlight_bounds(&self, node_id: String, bounds: Option<platynui_core::types::Rect>) {
        if let Some(bounds) = bounds {
            let rt = Arc::clone(&self.runtime);
            automation::dispatch_tracked(automation::TaskKind::Highlight, move || {
                tracing::debug!(
                    node_id,
                    thread_id = ?std::thread::current().id(),
                    "highlighting node bounds on automation worker"
                );
                let req = HighlightRequest::new(bounds).with_duration(Duration::from_millis(1500));
                if let Err(err) = rt.highlight(&req) {
                    tracing::error!(%err, "highlight failed");
                }
            });
        } else {
            self.clear_highlight();
        }
    }

    fn clear_highlight(&self) {
        let rt = Arc::clone(&self.runtime);
        automation::dispatch_tracked(automation::TaskKind::Highlight, move || {
            tracing::debug!(
                thread_id = ?std::thread::current().id(),
                "clearing highlight on automation worker"
            );
            let _ = rt.clear_highlight();
        });
    }

    /// Clear current search results and status.
    pub fn clear_results(&mut self) {
        self.results.clear();
        self.result_status = None;
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
    pub fn highlight_selected_row(&self) {
        if let Some(index) = self.selected_index {
            self.highlight_row(index);
        }
    }

    /// Highlight a specific tree row, if possible.
    pub fn highlight_row(&self, index: usize) {
        if let Some(row) = self.tree.rows().get(index) {
            if row.data.has_parent() {
                self.highlight_bounds(row.data.id(), row.data.bounds_rect());
            } else {
                self.clear_highlight();
            }
        }
    }

    /// Expand the currently selected tree row, if any.
    pub fn expand_selected_row(&mut self) {
        if let Some(index) = self.selected_index {
            self.tree.expand(index);
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
    /// Cancels any in-progress search, then spawns a background thread that
    /// streams results back via `mpsc::channel`. Call [`poll_search`] each frame
    /// to drain incoming results into `self.results`.
    pub fn evaluate_xpath(&mut self) {
        // Cancel any running search first.
        self.cancel_search();

        let xpath = self.search_text.trim().to_string();
        if xpath.is_empty() {
            self.results.clear();
            self.result_status = None;
            self.result_focused_index = 0;
            return;
        }

        // Clear previous results.
        self.results.clear();
        self.result_status = Some("Searching\u{2026}".to_string());
        self.result_focused_index = 0;

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();

        let rt = Arc::clone(&self.runtime);
        let flag = Arc::clone(&cancel_flag);

        automation::dispatch_tracked(automation::TaskKind::Search, move || {
            tracing::debug!(
                xpath,
                thread_id = ?std::thread::current().id(),
                "starting XPath evaluation on automation worker"
            );
            let start = Instant::now();
            let stream = rt.evaluate_iter_owned_cancellable(None, &xpath, Arc::clone(&flag));

            match stream {
                Ok(iter) => {
                    for item_result in iter {
                        // Check cancel flag before sending (fast exit).
                        if flag.load(Ordering::Relaxed) {
                            let _ = tx.send(SearchMsg::Cancelled);
                            return;
                        }
                        match item_result {
                            Ok(item) => {
                                let result_item = SearchResultItem::from_evaluation_item(&item);
                                if tx.send(SearchMsg::Result(result_item)).is_err() {
                                    // Receiver dropped (search cancelled from UI side).
                                    return;
                                }
                            }
                            Err(err) => {
                                // Check if this error is actually a cancellation.
                                let msg = err.to_string();
                                if msg.contains("cancelled") && flag.load(Ordering::Relaxed) {
                                    let _ = tx.send(SearchMsg::Cancelled);
                                } else {
                                    let _ = tx.send(SearchMsg::Error(msg));
                                }
                                return;
                            }
                        }
                    }
                    // Check if cancelled during the final iteration.
                    if flag.load(Ordering::Relaxed) {
                        let _ = tx.send(SearchMsg::Cancelled);
                    } else {
                        let _ = tx.send(SearchMsg::Done { elapsed: start.elapsed() });
                    }
                }
                Err(err) => {
                    let _ = tx.send(SearchMsg::Error(err.to_string()));
                }
            }
        });

        self.active_search = Some(ActiveSearch { receiver: rx, cancel_flag, start: Instant::now(), count: 0 });
    }

    /// Poll the background search for new results. Call this every frame.
    ///
    /// Drains up to `batch_size` items per call to keep the UI responsive.
    /// While a search is active, requests a repaint so the next frame polls again.
    pub fn poll_search(&mut self, ctx: &egui::Context) {
        let Some(search) = &mut self.active_search else {
            return;
        };

        // Drain up to 100 items per frame.
        let batch_size = 100;
        let mut finished = false;

        for _ in 0..batch_size {
            match search.receiver.try_recv() {
                Ok(SearchMsg::Result(item)) => {
                    self.results.push(item);
                    search.count += 1;
                }
                Ok(SearchMsg::Done { elapsed }) => {
                    let count = search.count;
                    self.result_status = Some(format!(
                        "{count} result{} ({:.1}ms)",
                        if count == 1 { "" } else { "s" },
                        elapsed.as_secs_f64() * 1000.0,
                    ));
                    finished = true;
                    break;
                }
                Ok(SearchMsg::Error(msg)) => {
                    self.result_status = Some(format!("Error: {msg}"));
                    finished = true;
                    break;
                }
                Ok(SearchMsg::Cancelled) => {
                    let count = search.count;
                    let elapsed = search.start.elapsed();
                    self.result_status = Some(format!(
                        "Cancelled \u{2014} {count} result{} ({:.1}ms)",
                        if count == 1 { "" } else { "s" },
                        elapsed.as_secs_f64() * 1000.0,
                    ));
                    finished = true;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Thread exited without sending Done/Error/Cancelled.
                    let count = search.count;
                    let elapsed = search.start.elapsed();
                    self.result_status = Some(format!(
                        "{count} result{} ({:.1}ms, stream ended)",
                        if count == 1 { "" } else { "s" },
                        elapsed.as_secs_f64() * 1000.0,
                    ));
                    finished = true;
                    break;
                }
            }
        }

        if finished {
            self.active_search = None;
        } else {
            // Update live status while search is in progress.
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
            let spinner = SPINNER_CHARS[self.spinner_frame / 3 % SPINNER_CHARS.len()];
            let count = search.count;
            let elapsed = search.start.elapsed();
            self.result_status = Some(format!(
                "{spinner} Searching\u{2026} {count} result{} ({:.1}s)",
                if count == 1 { "" } else { "s" },
                elapsed.as_secs_f64(),
            ));
            // Request repaint so next frame continues polling.
            ctx.request_repaint();
        }
    }

    /// Cancel the current background search, if any.
    pub fn cancel_search(&mut self) {
        if let Some(search) = self.active_search.take() {
            search.cancel_flag.store(true, Ordering::Relaxed);
            // The background thread will detect the flag and send Cancelled.
            // We drop the receiver so the thread's send will error out quickly.
        }
    }

    /// Returns `true` if a background search is currently running.
    pub fn is_searching(&self) -> bool {
        self.active_search.is_some()
    }

    /// When a result is selected, reveal its node in the tree (non-blocking).
    ///
    /// Spawns a background thread that pre-loads the ancestor path into the
    /// `UiNodeData` cache (expensive AT-SPI / UIA calls).  Once ready, the
    /// UI thread performs the cheap expand + rebuild + select.
    ///
    /// If a previous reveal is still in progress it is cancelled.
    pub fn reveal_and_select_result(&mut self, result_index: usize) {
        // Cancel any in-flight reveal.
        self.cancel_reveal();

        let item = match self.results.get(result_index) {
            Some(item) => item.clone(),
            None => return,
        };

        let Some(target_node) = item.ui_node().cloned() else {
            return;
        };

        let target_id = target_node.runtime_id().as_str().to_string();

        // Quick path: node is already visible in the cached tree.
        let root = Arc::clone(self.tree.root());
        if self.tree.reveal_node_cached(&target_id) {
            self.select_node_if_visible(&target_id);
            return;
        }

        // Slow path: spawn background thread to pre-load ancestor caches.
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel_flag);
        let (tx, rx) = mpsc::channel();

        automation::dispatch_tracked(automation::TaskKind::Reveal, move || {
            tracing::debug!(
                target_id,
                thread_id = ?std::thread::current().id(),
                "starting ancestor preload for tree reveal on automation worker"
            );
            // Walk up the target node's parent chain to collect ancestor IDs.
            let mut ancestors: Vec<String> = Vec::new();
            let mut current: Option<Arc<dyn platynui_core::ui::UiNode>> = Some(target_node);
            while let Some(n) = current {
                if flag.load(Ordering::Relaxed) {
                    let _ = tx.send(RevealMsg::Cancelled);
                    return;
                }
                if let Some(parent_weak) = n.parent() {
                    if let Some(parent) = parent_weak.upgrade() {
                        ancestors.push(parent.runtime_id().as_str().to_string());
                        current = Some(parent);
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            // Ancestors are root→…→parent (reverse of collection order).
            ancestors.reverse();

            // Walk DOWN from the root UiNodeData, loading children one
            // level at a time along the ancestor path.  This is
            // O(path_length × avg_children_per_level) instead of
            // O(path_length × tree_size) that `find_node_static` caused.
            let mut cursor = Arc::clone(&root);
            // If ancestors[0] matches root, skip it — we start there.
            let start = if !ancestors.is_empty() && cursor.id() == ancestors[0] { 1 } else { 0 };

            for ancestor_id in &ancestors[start..] {
                if flag.load(Ordering::Relaxed) {
                    let _ = tx.send(RevealMsg::Cancelled);
                    return;
                }
                // Calling .children() populates the Mutex cache (AT-SPI I/O).
                let children = cursor.children();
                if let Some(next) = children.into_iter().find(|c| c.id() == *ancestor_id) {
                    cursor = next;
                } else {
                    // Path diverged — tree structure may have changed.
                    let _ = tx.send(RevealMsg::Cancelled);
                    return;
                }
            }

            if flag.load(Ordering::Relaxed) {
                let _ = tx.send(RevealMsg::Cancelled);
                return;
            }

            // Load the target's parent's children so the target itself
            // is in the cache when the UI thread runs find_and_expand.
            let _ = cursor.children();

            if flag.load(Ordering::Relaxed) {
                let _ = tx.send(RevealMsg::Cancelled);
            } else {
                let _ = tx.send(RevealMsg::Ready { target_id });
            }
        });

        self.active_reveal = Some(ActiveReveal { receiver: rx, cancel_flag });
    }

    /// Helper: select a node if it's visible in the tree.
    fn select_node_if_visible(&mut self, target_id: &str) {
        if let Some(tree_index) = self.tree.rows().iter().position(|row| row.data.id() == target_id) {
            self.select_node(tree_index);
        } else {
            tracing::warn!(target_id, "reveal_node: not found in visible rows after expand");
        }
    }

    /// Poll the background reveal operation. Call this every frame.
    pub fn poll_reveal(&mut self, ctx: &egui::Context) {
        let Some(reveal) = &self.active_reveal else {
            return;
        };

        match reveal.receiver.try_recv() {
            Ok(RevealMsg::Ready { target_id }) => {
                self.active_reveal = None;
                // Now find_and_expand is cheap (children are cached).
                if self.tree.reveal_node_cached(&target_id) {
                    self.select_node_if_visible(&target_id);
                } else {
                    tracing::warn!(target_id, "reveal_node: not found after background preload");
                }
            }
            Ok(RevealMsg::Cancelled) => {
                self.active_reveal = None;
            }
            Err(mpsc::TryRecvError::Empty) => {
                // Still working — request repaint for next poll.
                ctx.request_repaint();
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                // Thread exited without sending a message.
                self.active_reveal = None;
            }
        }
    }

    /// Cancel the current background reveal, if any.
    fn cancel_reveal(&mut self) {
        if let Some(reveal) = self.active_reveal.take() {
            reveal.cancel_flag.store(true, Ordering::Relaxed);
        }
    }
}
