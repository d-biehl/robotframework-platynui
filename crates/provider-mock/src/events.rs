use crate::provider::MockProvider;
use platynui_core::provider::ProviderEventKind;
use platynui_core::ui::UiNode;
use std::sync::{Arc, LazyLock, RwLock, Weak};

static ACTIVE_PROVIDERS: LazyLock<RwLock<Vec<Weak<MockProvider>>>> = LazyLock::new(|| RwLock::new(Vec::new()));

pub(crate) fn register_active_instance(provider: &Arc<MockProvider>) {
    let weak = Arc::downgrade(provider);
    let mut list = ACTIVE_PROVIDERS.write().unwrap();
    list.retain(|entry| entry.upgrade().is_some());
    list.push(weak);
}

fn active_providers() -> Vec<Arc<MockProvider>> {
    let mut list = ACTIVE_PROVIDERS.write().unwrap();
    list.retain(|entry| entry.upgrade().is_some());
    list.iter().filter_map(|entry| entry.upgrade()).collect()
}

pub fn emit_event(event: ProviderEventKind) {
    for provider in active_providers() {
        provider.notify_listeners(event.clone());
    }
}

pub fn emit_node_updated(runtime_id: &str) {
    for provider in active_providers() {
        if let Some(node) = provider.clone_node(runtime_id) {
            provider.notify_listeners(ProviderEventKind::NodeUpdated { node });
        }
    }
}

pub fn node_by_runtime_id(runtime_id: &str) -> Option<Arc<dyn UiNode>> {
    for provider in active_providers() {
        if let Some(node) = provider.clone_node(runtime_id) {
            return Some(node);
        }
    }
    None
}
