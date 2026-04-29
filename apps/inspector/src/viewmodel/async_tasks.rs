//! Background tasks for egui-async integration.
//!
//! Defines task types and wrappers for long-running operations.

use crate::model::tree_data::{SearchResultItem, UiNodeData};
use platynui_core::platform::HighlightRequest;
use platynui_core::ui::UiNode;
use platynui_runtime::Runtime;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;

/// Result payload for XPath search.
pub struct SearchResult {
    pub result_count: usize,
    pub elapsed: Duration,
    pub cancelled: bool,
    pub limit_reached: Option<usize>,
}

/// Shared mutable progress for in-flight search.
#[derive(Default)]
pub struct SearchProgress {
    pending_results: VecDeque<SearchResultItem>,
    result_count: usize,
}

impl SearchProgress {
    fn push_result(&mut self, result: SearchResultItem) {
        self.result_count += 1;
        self.pending_results.push_back(result);
    }
}

/// A bounded batch drained from shared search progress.
pub struct SearchProgressDrain {
    pub results: Vec<SearchResultItem>,
    pub total_count: usize,
    pub pending_count: usize,
}

/// Shared search progress container.
pub type SharedSearchProgress = Arc<Mutex<SearchProgress>>;

/// Create a new shared progress container for search.
pub fn new_search_progress() -> SharedSearchProgress {
    Arc::new(Mutex::new(SearchProgress::default()))
}

/// Drain up to `max_results` search results from shared progress.
pub fn drain_search_progress(
    progress: &SharedSearchProgress,
    max_results: usize,
) -> Result<SearchProgressDrain, String> {
    let mut state = progress.lock().map_err(|_| "search progress lock poisoned".to_string())?;
    let count = state.pending_results.len().min(max_results);
    let results = state.pending_results.drain(..count).collect();
    Ok(SearchProgressDrain { results, total_count: state.result_count, pending_count: state.pending_results.len() })
}

/// Result of a background reveal operation (ancestor preload).
#[derive(Clone, Debug)]
pub enum RevealResult {
    /// Ancestor path pre-loaded successfully; target is ready to reveal.
    Ready { epoch: u64, target_id: String },
    /// Reveal was cancelled.
    Cancelled { epoch: u64 },
}

/// Result of a background highlight operation.
#[derive(Clone, Copy, Debug)]
pub struct HighlightResult {
    pub epoch: u64,
    pub skipped: bool,
}

/// Result payload for selected-node details.
#[derive(Clone, Debug)]
pub struct SelectionResult {
    pub request_id: u64,
    pub selected_label: String,
    pub attributes: Vec<crate::model::tree_data::DisplayAttribute>,
    pub is_root: bool,
    pub bounds: Option<platynui_core::types::Rect>,
    pub node_id: String,
}

/// Task for pre-loading ancestor paths in the tree.
///
/// Walks the target node's parent chain and loads children caches along
/// the ancestor path to make the target node cheap to expand in the UI.
pub async fn reveal_task(
    epoch: u64,
    latest_epoch: Arc<AtomicU64>,
    root: Arc<UiNodeData>,
    target_node: Arc<dyn UiNode>,
) -> Result<RevealResult, String> {
    let target_id = target_node.runtime_id().as_str().to_string();

    tracing::debug!(
        epoch,
        target_id,
        thread_id = ?std::thread::current().id(),
        "starting ancestor preload for tree reveal on egui-async worker"
    );

    if latest_epoch.load(Ordering::Relaxed) != epoch || !target_node.is_valid() {
        tracing::debug!(epoch, target_id, "skipping stale or invalid reveal task");
        return Ok(RevealResult::Cancelled { epoch });
    }

    // Walk up the target node's parent chain to collect ancestor IDs.
    let mut ancestors: Vec<String> = Vec::new();
    let mut current: Option<Arc<dyn UiNode>> = Some(target_node);
    while let Some(node) = current {
        if latest_epoch.load(Ordering::Relaxed) != epoch {
            tracing::debug!(epoch, target_id, "cancelling stale reveal task while collecting ancestors");
            return Ok(RevealResult::Cancelled { epoch });
        }
        if let Some(parent_weak) = node.parent() {
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
    // level at a time along the ancestor path.
    let mut cursor = Arc::clone(&root);
    // If ancestors[0] matches root, skip it — we start there.
    let start = if !ancestors.is_empty() && cursor.id() == ancestors[0] { 1 } else { 0 };

    for ancestor_id in &ancestors[start..] {
        if latest_epoch.load(Ordering::Relaxed) != epoch {
            tracing::debug!(epoch, target_id, "cancelling stale reveal task while loading ancestors");
            return Ok(RevealResult::Cancelled { epoch });
        }
        let aid = ancestor_id.clone();
        let children = cursor.children();

        if let Some(next_cursor) = children.into_iter().find(|child| child.id() == aid) {
            cursor = next_cursor;
        } else {
            // Path diverged — tree structure may have changed.
            return Ok(RevealResult::Cancelled { epoch });
        }
    }

    // Load the target's parent's children so the target itself
    // is in the cache when the UI thread runs reveal_node_cached.
    let _ = cursor.children();

    if latest_epoch.load(Ordering::Relaxed) != epoch {
        tracing::debug!(epoch, target_id, "cancelling stale reveal task after preload");
        return Ok(RevealResult::Cancelled { epoch });
    }

    Ok(RevealResult::Ready { epoch, target_id })
}

/// Task for loading selected-node details.
pub async fn selection_task(
    request_id: u64,
    selected_label: String,
    node_data: Arc<UiNodeData>,
) -> Result<SelectionResult, String> {
    let attributes = node_data.display_attributes_direct();
    let is_root = !node_data.has_parent_direct();
    let bounds = if is_root { None } else { node_data.bounds_rect_direct() };
    let node_id = node_data.id();

    Ok(SelectionResult { request_id, selected_label, attributes, is_root, bounds, node_id })
}

/// Task for loading the initial root children.
pub async fn initial_load_task(root: Arc<dyn UiNode>) -> Result<Vec<Arc<UiNodeData>>, String> {
    let mut out = Vec::new();
    for node in root.children() {
        let node_data = Arc::new(UiNodeData::new(node));
        node_data.preload_caches();
        out.push(node_data);
    }

    Ok(out)
}

/// Task for evaluating an XPath expression.
pub async fn search_task(
    runtime: Arc<Runtime>,
    xpath: String,
    cancel_flag: Arc<AtomicBool>,
    progress: SharedSearchProgress,
    result_limit: Option<usize>,
) -> Result<SearchResult, String> {
    tracing::debug!(
        xpath,
        thread_id = ?std::thread::current().id(),
        "starting XPath evaluation on egui-async worker"
    );

    let start = Instant::now();
    let mut result_count = 0;

    match runtime.evaluate_iter_owned_cancellable(None, &xpath, Arc::clone(&cancel_flag)) {
        Ok(iter) => {
            for item_result in iter {
                if cancel_flag.load(Ordering::Relaxed) {
                    return Ok(SearchResult {
                        result_count,
                        elapsed: start.elapsed(),
                        cancelled: true,
                        limit_reached: None,
                    });
                }

                match item_result {
                    Ok(item) => {
                        let result_item = SearchResultItem::from_evaluation_item(&item);
                        result_count += 1;
                        progress
                            .lock()
                            .map_err(|_| "search progress lock poisoned".to_string())?
                            .push_result(result_item);

                        if result_limit.is_some_and(|limit| result_count >= limit) {
                            tracing::debug!(
                                xpath,
                                result_count,
                                result_limit,
                                "stopping XPath evaluation after inspector search result limit was reached"
                            );
                            return Ok(SearchResult {
                                result_count,
                                elapsed: start.elapsed(),
                                cancelled: false,
                                limit_reached: result_limit,
                            });
                        }
                    }
                    Err(err) => {
                        let msg = err.to_string();
                        if msg.contains("cancelled") && cancel_flag.load(Ordering::Relaxed) {
                            return Ok(SearchResult {
                                result_count,
                                elapsed: start.elapsed(),
                                cancelled: true,
                                limit_reached: None,
                            });
                        }
                        return Err(msg);
                    }
                }
            }

            Ok(SearchResult {
                result_count,
                elapsed: start.elapsed(),
                cancelled: cancel_flag.load(Ordering::Relaxed),
                limit_reached: None,
            })
        }
        Err(err) => Err(err.to_string()),
    }
}

fn is_stale_epoch(latest_epoch: &AtomicU64, epoch: u64) -> bool {
    latest_epoch.load(Ordering::Relaxed) != epoch
}

/// Task for showing a temporary highlight for given bounds.
pub async fn highlight_bounds_task(
    runtime: Arc<Runtime>,
    epoch: u64,
    latest_epoch: Arc<AtomicU64>,
    node_id: String,
    bounds: platynui_core::types::Rect,
) -> Result<HighlightResult, String> {
    tracing::debug!(
        epoch,
        node_id,
        thread_id = ?std::thread::current().id(),
        "highlighting node bounds on egui-async worker"
    );

    if is_stale_epoch(&latest_epoch, epoch) {
        tracing::debug!(epoch, node_id, "skipping stale highlight task");
        return Ok(HighlightResult { epoch, skipped: true });
    }

    let req = HighlightRequest::new(bounds).with_duration(Duration::from_millis(1500));
    runtime.highlight(&req).map_err(|err| err.to_string())?;
    Ok(HighlightResult { epoch, skipped: false })
}

/// Task for clearing the active highlight.
pub async fn clear_highlight_task(
    runtime: Arc<Runtime>,
    epoch: u64,
    latest_epoch: Arc<AtomicU64>,
) -> Result<HighlightResult, String> {
    tracing::debug!(
        epoch,
        thread_id = ?std::thread::current().id(),
        "clearing highlight on egui-async worker"
    );

    if is_stale_epoch(&latest_epoch, epoch) {
        tracing::debug!(epoch, "skipping stale clear-highlight task");
        return Ok(HighlightResult { epoch, skipped: true });
    }

    runtime.clear_highlight().map_err(|err| err.to_string())?;
    Ok(HighlightResult { epoch, skipped: false })
}

/// Task for highlighting a node if it is not the desktop root.
pub async fn highlight_node_task(
    runtime: Arc<Runtime>,
    epoch: u64,
    latest_epoch: Arc<AtomicU64>,
    node_data: Arc<UiNodeData>,
) -> Result<HighlightResult, String> {
    if is_stale_epoch(&latest_epoch, epoch) || !node_data.is_valid() {
        tracing::debug!(epoch, node_id = node_data.id(), "skipping stale or invalid highlight-node task");
        return Ok(HighlightResult { epoch, skipped: true });
    }

    if node_data.has_parent_direct() {
        if let Some(bounds) = node_data.bounds_rect_direct() {
            highlight_bounds_task(runtime, epoch, latest_epoch, node_data.id(), bounds).await
        } else {
            clear_highlight_task(runtime, epoch, latest_epoch).await
        }
    } else {
        clear_highlight_task(runtime, epoch, latest_epoch).await
    }
}
