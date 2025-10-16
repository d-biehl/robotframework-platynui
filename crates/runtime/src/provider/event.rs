use platynui_core::provider::{ProviderEvent, ProviderEventListener};
use std::sync::{Arc, RwLock};

/// Trait implemented by components willing to receive provider events.
pub trait ProviderEventSink: Send + Sync {
    fn dispatch(&self, event: ProviderEvent);
}

/// Simple fan-out dispatcher that forwards provider events to registered sinks.
pub struct ProviderEventDispatcher {
    sinks: RwLock<Vec<Arc<dyn ProviderEventSink>>>,
}

impl ProviderEventDispatcher {
    pub fn new() -> Self {
        Self { sinks: RwLock::new(Vec::new()) }
    }

    pub fn register(&self, sink: Arc<dyn ProviderEventSink>) {
        self.sinks.write().unwrap().push(sink);
    }

    pub fn dispatch(&self, event: ProviderEvent) {
        for sink in self.sinks.read().unwrap().iter() {
            sink.dispatch(event.clone());
        }
    }

    pub fn sink_count(&self) -> usize {
        self.sinks.read().unwrap().len()
    }

    pub fn shutdown(&self) {
        self.sinks.write().unwrap().clear();
    }
}

impl Default for ProviderEventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderEventListener for ProviderEventDispatcher {
    fn on_event(&self, event: ProviderEvent) {
        self.dispatch(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::provider::{ProviderEvent, ProviderEventKind};
    use platynui_core::ui::PatternId;
    use platynui_core::ui::{Namespace, RuntimeId, UiAttribute, UiNode, UiValue};
    use rstest::rstest;
    use std::sync::{Arc, Mutex, Weak};

    struct StubAttribute;
    impl UiAttribute for StubAttribute {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn name(&self) -> &str {
            "Role"
        }
        fn value(&self) -> UiValue {
            UiValue::from("Stub")
        }
    }

    struct StubNode {
        runtime_id: RuntimeId,
    }

    impl StubNode {
        fn new() -> Self {
            Self { runtime_id: RuntimeId::from("stub") }
        }
    }

    impl UiNode for StubNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &str {
            "Button"
        }
        fn name(&self) -> String {
            "Stub".to_string()
        }
        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }
        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            None
        }
        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
            Box::new(Vec::<Arc<dyn UiNode>>::new().into_iter())
        }
        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
            Box::new(vec![Arc::new(StubAttribute) as Arc<dyn UiAttribute>].into_iter())
        }
        fn supported_patterns(&self) -> Vec<PatternId> {
            Vec::new()
        }
        fn invalidate(&self) {}
    }

    struct RecordingSink {
        events: Mutex<Vec<ProviderEventKind>>,
    }

    impl RecordingSink {
        fn new() -> Self {
            Self { events: Mutex::new(Vec::new()) }
        }
    }

    impl ProviderEventSink for RecordingSink {
        fn dispatch(&self, event: ProviderEvent) {
            self.events.lock().unwrap().push(event.kind);
        }
    }

    #[rstest]
    fn dispatcher_forwards_events() {
        let dispatcher = ProviderEventDispatcher::new();
        let sink = Arc::new(RecordingSink::new());
        dispatcher.register(sink.clone());

        let node = Arc::new(StubNode::new());
        dispatcher.dispatch(ProviderEvent { kind: ProviderEventKind::NodeUpdated { node: node.clone() } });

        let events = sink.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            ProviderEventKind::NodeUpdated { node: recorded } => {
                assert_eq!(recorded.runtime_id().as_str(), node.runtime_id().as_str());
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[rstest]
    fn shutdown_clears_sinks() {
        let dispatcher = ProviderEventDispatcher::new();
        let sink = Arc::new(RecordingSink::new());
        dispatcher.register(sink);
        assert_eq!(dispatcher.sink_count(), 1);
        dispatcher.shutdown();
        assert_eq!(dispatcher.sink_count(), 0);
    }
}
