use crate::events;
use platynui_core::ui::attribute_names::focusable;
use platynui_core::ui::{Namespace, PatternError, RuntimeId, UiAttribute, UiValue};
use std::sync::{Arc, LazyLock, RwLock};

static FOCUSED_NODE: LazyLock<RwLock<Option<RuntimeId>>> = LazyLock::new(|| RwLock::new(None));

pub(crate) fn reset() {
    *FOCUSED_NODE.write().expect("focus state lock poisoned") = None;
}

pub(crate) fn is_focused(runtime_id: &RuntimeId) -> bool {
    FOCUSED_NODE
        .read()
        .expect("focus state lock poisoned")
        .as_ref()
        .map(|current| current == runtime_id)
        .unwrap_or(false)
}

pub(crate) fn request_focus(runtime_id: RuntimeId) -> Result<(), PatternError> {
    if events::node_by_runtime_id(runtime_id.as_str()).is_none() {
        return Err(PatternError::new("node is no longer available"));
    }

    let mut guard = FOCUSED_NODE.write().expect("focus state lock poisoned");
    if guard.as_ref().is_some_and(|current| current == &runtime_id) {
        return Ok(());
    }

    let previous = guard.replace(runtime_id.clone());
    drop(guard);

    if let Some(prev) = previous {
        events::emit_node_updated(prev.as_str());
    }
    events::emit_node_updated(runtime_id.as_str());
    Ok(())
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
        UiValue::from(is_focused(&self.runtime_id))
    }
}
