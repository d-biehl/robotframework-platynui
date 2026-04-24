//! Background tasks for egui-async integration.
//!
//! Defines task types and wrappers for long-running operations.

use crate::model::tree_data::{SearchResultItem, UiNodeData};
use platynui_core::platform::HighlightRequest;
use platynui_core::ui::UiNode;
use platynui_runtime::Runtime;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;

/// Result payload for XPath search.
pub struct SearchResult {
    pub results: Vec<SearchResultItem>,
    pub elapsed: Duration,
    pub cancelled: bool,
}

/// Shared mutable progress for in-flight search.
#[derive(Default)]
pub struct SearchProgress {
    pub results: Vec<SearchResultItem>,
}

/// Shared search progress container.
pub type SharedSearchProgress = Arc<Mutex<SearchProgress>>;

/// Create a new shared progress container for search.
pub fn new_search_progress() -> SharedSearchProgress {
    Arc::new(Mutex::new(SearchProgress::default()))
}

/// Result of a background reveal operation (ancestor preload).
#[derive(Clone, Debug)]
pub enum RevealResult {
    /// Ancestor path pre-loaded successfully; target is ready to reveal.
    Ready { target_id: String },
    /// Reveal was cancelled.
    Cancelled,
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
pub async fn reveal_task(root: Arc<UiNodeData>, target_node: Arc<dyn UiNode>) -> Result<RevealResult, String> {
    let target_id = target_node.runtime_id().as_str().to_string();

    tracing::debug!(
        target_id,
        thread_id = ?std::thread::current().id(),
        "starting ancestor preload for tree reveal on egui-async worker"
    );

    // Walk up the target node's parent chain to collect ancestor IDs.
    let mut ancestors: Vec<String> = Vec::new();
    let mut current: Option<Arc<dyn UiNode>> = Some(target_node);
    while let Some(node) = current {
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
        let aid = ancestor_id.clone();
        let children = cursor.children();

        if let Some(next_cursor) = children.into_iter().find(|child| child.id() == aid) {
            cursor = next_cursor;
        } else {
            // Path diverged — tree structure may have changed.
            return Ok(RevealResult::Cancelled);
        }
    }

    // Load the target's parent's children so the target itself
    // is in the cache when the UI thread runs reveal_node_cached.
    let _ = cursor.children();

    Ok(RevealResult::Ready { target_id })
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
) -> Result<SearchResult, String> {
    tracing::debug!(
        xpath,
        thread_id = ?std::thread::current().id(),
        "starting XPath evaluation on egui-async worker"
    );

    let start = Instant::now();
    let mut results = Vec::new();

    match runtime.evaluate_iter_owned_cancellable(None, &xpath, Arc::clone(&cancel_flag)) {
        Ok(iter) => {
            for item_result in iter {
                if cancel_flag.load(Ordering::Relaxed) {
                    return Ok(SearchResult { results, elapsed: start.elapsed(), cancelled: true });
                }

                match item_result {
                    Ok(item) => {
                        let result_item = SearchResultItem::from_evaluation_item(&item);
                        results.push(result_item.clone());
                        if let Ok(mut state) = progress.lock() {
                            state.results.push(result_item);
                        }
                    }
                    Err(err) => {
                        let msg = err.to_string();
                        if msg.contains("cancelled") && cancel_flag.load(Ordering::Relaxed) {
                            return Ok(SearchResult { results, elapsed: start.elapsed(), cancelled: true });
                        }
                        return Err(msg);
                    }
                }
            }

            Ok(SearchResult { results, elapsed: start.elapsed(), cancelled: cancel_flag.load(Ordering::Relaxed) })
        }
        Err(err) => Err(err.to_string()),
    }
}

/// Task for showing a temporary highlight for given bounds.
pub async fn highlight_bounds_task(
    runtime: Arc<Runtime>,
    node_id: String,
    bounds: platynui_core::types::Rect,
) -> Result<(), String> {
    tracing::debug!(
        node_id,
        thread_id = ?std::thread::current().id(),
        "highlighting node bounds on egui-async worker"
    );

    let req = HighlightRequest::new(bounds).with_duration(Duration::from_millis(1500));
    runtime.highlight(&req).map_err(|err| err.to_string())
}

/// Task for clearing the active highlight.
pub async fn clear_highlight_task(runtime: Arc<Runtime>) -> Result<(), String> {
    tracing::debug!(
        thread_id = ?std::thread::current().id(),
        "clearing highlight on egui-async worker"
    );

    runtime.clear_highlight().map_err(|err| err.to_string())
}

/// Task for highlighting a node if it is not the desktop root.
pub async fn highlight_node_task(runtime: Arc<Runtime>, node_data: Arc<UiNodeData>) -> Result<(), String> {
    if node_data.has_parent_direct() {
        if let Some(bounds) = node_data.bounds_rect_direct() {
            highlight_bounds_task(runtime, node_data.id(), bounds).await
        } else {
            clear_highlight_task(runtime).await
        }
    } else {
        clear_highlight_task(runtime).await
    }
}
