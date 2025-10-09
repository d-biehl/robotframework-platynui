use crate::events;
use platynui_core::ui::attribute_names::focusable;
use platynui_core::ui::{Namespace, PatternError, RuntimeId, UiAttribute, UiNode, UiValue};
use std::sync::{Arc, LazyLock, RwLock};

static FOCUSED_NODE: LazyLock<RwLock<Option<RuntimeId>>> = LazyLock::new(|| RwLock::new(None));

pub(crate) fn reset() {
    *FOCUSED_NODE.write().expect("focus state lock poisoned") = None;
}

pub(crate) fn request_focus(runtime_id: RuntimeId) -> Result<(), PatternError> {
    let node = events::node_by_runtime_id(runtime_id.as_str());
    let window_runtime = node.as_ref().and_then(|node| resolve_window(node)).map(|window| window.runtime_id().clone());

    let mut guard = FOCUSED_NODE.write().expect("focus state lock poisoned");
    if guard.as_ref().is_some_and(|current| current == &runtime_id) {
        return Ok(());
    }

    let previous = guard.replace(runtime_id.clone());
    drop(guard);

    if let Some(prev) = previous {
        events::emit_node_updated(prev.as_str());
    }
    if node.is_some() {
        events::emit_node_updated(runtime_id.as_str());
    }
    if let Some(window_id) = window_runtime {
        events::emit_node_updated(window_id.as_str());
    }
    Ok(())
}

pub(crate) fn clear_if_matches(runtime_id: &RuntimeId) {
    let mut guard = FOCUSED_NODE.write().expect("focus state lock poisoned");
    let was_cleared = guard.as_ref().is_some_and(|current| current == runtime_id);
    if was_cleared {
        let previous = guard.take();
        drop(guard);
        if let Some(prev) = previous {
            events::emit_node_updated(prev.as_str());
        }
    } else {
        drop(guard);
    }

    if was_cleared
        && let Some(node) = events::node_by_runtime_id(runtime_id.as_str())
        && let Some(window) = resolve_window(&node)
    {
        events::emit_node_updated(window.runtime_id().as_str());
    }
}

pub(crate) fn focus_attribute(namespace: Namespace, runtime_id: RuntimeId) -> Arc<dyn UiAttribute> {
    Arc::new(FocusAttribute { namespace, runtime_id })
}

struct FocusAttribute {
    namespace: Namespace,
    runtime_id: RuntimeId,
}

impl UiAttribute for FocusAttribute {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        focusable::IS_FOCUSED
    }

    fn value(&self) -> UiValue {
        let focused = FOCUSED_NODE.read().expect("focus state lock poisoned").clone();
        let is_focused = focused
            .as_ref()
            .map(|current| {
                if current == &self.runtime_id {
                    true
                } else if let Some(node) = events::node_by_runtime_id(current.as_str()) {
                    is_ancestor(&node, &self.runtime_id)
                } else {
                    false
                }
            })
            .unwrap_or(false);
        UiValue::from(is_focused)
    }
}

fn resolve_window(node: &Arc<dyn UiNode>) -> Option<Arc<dyn UiNode>> {
    let mut current = Some(Arc::clone(node));
    while let Some(candidate) = current {
        if candidate.namespace() == Namespace::Control && candidate.role() == "Window" {
            return Some(candidate);
        }
        current = candidate.parent().and_then(|weak| weak.upgrade());
    }
    None
}

fn is_ancestor(node: &Arc<dyn UiNode>, candidate: &RuntimeId) -> bool {
    let mut current = Some(Arc::clone(node));
    while let Some(ref candidate_node) = current {
        if candidate_node.runtime_id() == candidate {
            return true;
        }
        current = candidate_node.parent().and_then(|weak| weak.upgrade());
    }
    false
}
