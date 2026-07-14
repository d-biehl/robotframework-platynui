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

/// Progress event emitted while tree children are loaded incrementally.
#[derive(Clone, Debug)]
pub enum ChildLoadProgressEvent {
    Started { request_id: u64, node_id: String },
    ChildLoaded,
    NodeUpdated,
}

/// Shared mutable progress for in-flight tree child loading.
#[derive(Default)]
pub struct ChildLoadProgress {
    pending_events: VecDeque<ChildLoadProgressEvent>,
}

impl ChildLoadProgress {
    fn push_event(&mut self, event: ChildLoadProgressEvent) {
        self.pending_events.push_back(event);
    }
}

/// A bounded batch drained from shared child-load progress.
pub struct ChildLoadProgressDrain {
    pub events: Vec<ChildLoadProgressEvent>,
    pub pending_count: usize,
}

/// Shared child-load progress container.
pub type SharedChildLoadProgress = Arc<Mutex<ChildLoadProgress>>;

/// Create a new shared progress container for tree child loading.
pub fn new_child_load_progress() -> SharedChildLoadProgress {
    Arc::new(Mutex::new(ChildLoadProgress::default()))
}

/// Drain up to `max_events` child-load progress events.
pub fn drain_child_load_progress(
    progress: &SharedChildLoadProgress,
    max_events: usize,
) -> Result<ChildLoadProgressDrain, String> {
    let mut state = progress.lock().map_err(|_| "child-load progress lock poisoned".to_string())?;
    let count = state.pending_events.len().min(max_events);
    let events = state.pending_events.drain(..count).collect();
    Ok(ChildLoadProgressDrain { events, pending_count: state.pending_events.len() })
}

/// Result of a background reveal operation (ancestor preload).
#[derive(Clone, Debug)]
pub enum RevealResult {
    /// Ancestor path pre-loaded successfully; target is ready to reveal.
    Ready { epoch: u64, target_id: String },
    /// Reveal was cancelled.
    Cancelled,
}

/// Result of a background highlight operation.
#[derive(Clone, Copy, Debug)]
pub struct HighlightResult;

/// Result payload for selected-node details.
#[derive(Clone, Debug)]
pub struct SelectionResult {
    pub request_id: u64,
    pub selected_label: String,
    pub attributes: Vec<crate::model::tree_data::DisplayAttribute>,
    pub is_root: bool,
    pub bounds: Option<platynui_core::types::Rect>,
}

/// Child-load behavior for tree expansion and refresh requests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildLoadMode {
    /// Load children if the cache is empty.
    Load,
    /// Invalidate this node before loading its children.
    Refresh,
    /// Invalidate this node and cached descendants before loading children.
    RefreshSubtree,
}

/// Result payload for asynchronous tree child loading.
#[derive(Clone, Debug)]
pub struct ChildLoadResult {
    pub request_id: u64,
    pub node_id: String,
}

/// Collects the resolved node's live parent chain from `target` up to (but not
/// including) the ancestor whose runtime id is `ancestor_id`, returned top-down
/// (the direct child of that ancestor first, down to `target`). Used to graft a
/// picker result into the tree when top-down enumeration can't reach it (dynamic
/// menus with unstable RuntimeIds). Returns `None` if the ancestor is not on the
/// chain. Bounded so a broken parent link can never loop forever.
fn resolved_chain_below(target: &Arc<dyn UiNode>, ancestor_id: &str) -> Option<Vec<Arc<dyn UiNode>>> {
    const MAX_DEPTH: usize = 256;
    let mut chain: Vec<Arc<dyn UiNode>> = vec![Arc::clone(target)];
    let mut current = Arc::clone(target);
    for _ in 0..MAX_DEPTH {
        let parent = current.parent()?.upgrade()?;
        if parent.runtime_id().as_str() == ancestor_id {
            chain.reverse();
            return Some(chain);
        }
        chain.push(Arc::clone(&parent));
        current = parent;
    }
    None
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
    refresh: bool,
) -> Result<RevealResult, String> {
    let target_id = target_node.runtime_id().as_str().to_string();

    if latest_epoch.load(Ordering::Relaxed) != epoch || !target_node.is_valid() {
        return Ok(RevealResult::Cancelled);
    }

    // Walk up the target node's parent chain to collect ancestor IDs. Keep the
    // resolved node itself for a possible graft (see the divergence branch below).
    let mut ancestors: Vec<String> = Vec::new();
    let mut current: Option<Arc<dyn UiNode>> = Some(Arc::clone(&target_node));
    while let Some(node) = current {
        if latest_epoch.load(Ordering::Relaxed) != epoch {
            return Ok(RevealResult::Cancelled);
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
            return Ok(RevealResult::Cancelled);
        }
        let aid = ancestor_id.clone();
        // Try the cached children first. Only when the ancestor is missing (a
        // just-opened window/menu the cache predates) do we reload THIS level.
        // A picker reveal fires on every element the cursor passes over, so
        // blindly clearing every level each time would re-enumerate the whole
        // desktop on each tick — making the top-level window/app list churn and
        // momentarily empty (worst while a modal context menu blocks UIA).
        let mut next = cursor.children().into_iter().find(|child| child.id() == aid);
        if next.is_none() && refresh {
            cursor.clear_children_cache();
            next = cursor.children().into_iter().find(|child| child.id() == aid);
        }
        match next {
            Some(next_cursor) => cursor = next_cursor,
            None => {
                // Top-down enumeration can't reach the target: dynamic XAML /
                // Chromium menus hand out unstable UIA RuntimeIds, so the ancestor
                // from the hit-test's parent walk never matches a re-enumerated
                // child. Graft the picker's already-resolved live chain from
                // `cursor` down so reveal can still select it.
                if let Some(chain) = resolved_chain_below(&target_node, &cursor.id()) {
                    cursor.graft_chain(&chain);
                    return Ok(RevealResult::Ready { epoch, target_id });
                }
                return Ok(RevealResult::Cancelled);
            }
        }
    }

    // Ensure the target itself is cached under its parent so the UI thread's
    // reveal_node_cached finds it. Reload only when it is not already there.
    let target_cached = cursor.children().into_iter().any(|child| child.id() == target_id);
    if refresh && !target_cached {
        cursor.clear_children_cache();
        let _ = cursor.children();
    }

    if latest_epoch.load(Ordering::Relaxed) != epoch {
        return Ok(RevealResult::Cancelled);
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

    Ok(SelectionResult { request_id, selected_label, attributes, is_root, bounds })
}

/// Task for loading tree children without blocking the UI thread.
pub async fn child_load_task(
    request_id: u64,
    node_data: Arc<UiNodeData>,
    mode: ChildLoadMode,
    progress: SharedChildLoadProgress,
) -> Result<ChildLoadResult, String> {
    match mode {
        ChildLoadMode::Load => {}
        ChildLoadMode::Refresh => node_data.refresh(),
        ChildLoadMode::RefreshSubtree => node_data.refresh_recursive(),
    }

    node_data.preload_row_caches();
    let node_id = node_data.id();
    progress
        .lock()
        .map_err(|_| "child-load progress lock poisoned".to_string())?
        .push_event(ChildLoadProgressEvent::Started { request_id, node_id: node_id.clone() });

    node_data.load_children_incremental(
        |_, _| {
            if let Ok(mut state) = progress.lock() {
                state.push_event(ChildLoadProgressEvent::ChildLoaded);
            }
        },
        |_, _| {
            if let Ok(mut state) = progress.lock() {
                state.push_event(ChildLoadProgressEvent::NodeUpdated);
            }
        },
    );
    node_data.preload_label();
    progress
        .lock()
        .map_err(|_| "child-load progress lock poisoned".to_string())?
        .push_event(ChildLoadProgressEvent::NodeUpdated);

    Ok(ChildLoadResult { request_id, node_id })
}

/// Task for evaluating an XPath expression.
pub async fn search_task(
    runtime: Arc<Runtime>,
    xpath: String,
    cancel_flag: Arc<AtomicBool>,
    progress: SharedSearchProgress,
    result_limit: Option<usize>,
) -> Result<SearchResult, String> {
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
    bounds: platynui_core::types::Rect,
) -> Result<HighlightResult, String> {
    if is_stale_epoch(&latest_epoch, epoch) {
        return Ok(HighlightResult);
    }

    let req = HighlightRequest::new(bounds).with_duration(Duration::from_millis(1500));
    runtime.highlight(&req).map_err(|err| err.to_string())?;
    Ok(HighlightResult)
}

/// Task for clearing the active highlight.
pub async fn clear_highlight_task(
    runtime: Arc<Runtime>,
    epoch: u64,
    latest_epoch: Arc<AtomicU64>,
) -> Result<HighlightResult, String> {
    if is_stale_epoch(&latest_epoch, epoch) {
        return Ok(HighlightResult);
    }

    runtime.clear_highlight().map_err(|err| err.to_string())?;
    Ok(HighlightResult)
}

/// Task for highlighting a node if it is not the desktop root.
pub async fn highlight_node_task(
    runtime: Arc<Runtime>,
    epoch: u64,
    latest_epoch: Arc<AtomicU64>,
    node_data: Arc<UiNodeData>,
) -> Result<HighlightResult, String> {
    if is_stale_epoch(&latest_epoch, epoch) || !node_data.is_valid() {
        return Ok(HighlightResult);
    }

    if node_data.has_parent_direct() {
        if let Some(bounds) = node_data.bounds_rect_direct() {
            highlight_bounds_task(runtime, epoch, latest_epoch, bounds).await
        } else {
            clear_highlight_task(runtime, epoch, latest_epoch).await
        }
    } else {
        clear_highlight_task(runtime, epoch, latest_epoch).await
    }
}

#[cfg(test)]
mod tests {
    use super::resolved_chain_below;
    use crate::model::tree_data::test_mock::MockNode;
    use platynui_core::ui::UiNode;
    use std::sync::Arc;

    #[test]
    fn resolved_chain_below_returns_the_slice_beneath_the_ancestor() {
        // Parent links: leaf -> mid -> top -> root.
        let root = MockNode::new("root");
        let top = MockNode::new("top");
        let mid = MockNode::new("mid");
        let leaf = MockNode::new("leaf");
        let root_dyn: Arc<dyn UiNode> = root.clone();
        let top_dyn: Arc<dyn UiNode> = top.clone();
        let mid_dyn: Arc<dyn UiNode> = mid.clone();
        top.set_parent(&root_dyn);
        mid.set_parent(&top_dyn);
        leaf.set_parent(&mid_dyn);
        let leaf_dyn: Arc<dyn UiNode> = leaf.clone();

        // Chain below "top" is top-down: its direct child first, down to the leaf.
        let chain = resolved_chain_below(&leaf_dyn, "top").expect("top is on the parent chain");
        let ids: Vec<String> = chain.iter().map(|n| n.runtime_id().as_str().to_string()).collect();
        assert_eq!(ids, vec!["mid".to_string(), "leaf".to_string()]);

        // An ancestor that is not on the chain yields nothing.
        assert!(resolved_chain_below(&leaf_dyn, "not-an-ancestor").is_none());
    }
}
