use platynui_core::provider::{
    ProviderDescriptor, ProviderError, UiTreeProvider, UiTreeProviderFactory, provider_factories,
};
use platynui_core::ui::identifiers::TechnologyId;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProviderEntry {
    pub descriptor: &'static ProviderDescriptor,
    factory: &'static dyn UiTreeProviderFactory,
}

impl ProviderEntry {
    pub fn instantiate(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        self.factory.create()
    }
}

pub struct ProviderRegistry {
    entries: Vec<ProviderEntry>,
    by_technology: HashMap<String, Vec<usize>>,
}

impl ProviderRegistry {
    pub fn discover() -> Self {
        let mut entries: Vec<ProviderEntry> =
            provider_factories().map(|factory| ProviderEntry { descriptor: factory.descriptor(), factory }).collect();

        entries.sort_by(|a, b| {
            let tech_cmp = a.descriptor.technology.as_str().cmp(b.descriptor.technology.as_str());
            if tech_cmp != std::cmp::Ordering::Equal {
                return tech_cmp;
            }
            a.descriptor.id.cmp(b.descriptor.id)
        });

        let by_technology = Self::rebuild_by_technology(&entries);

        Self { entries, by_technology }
    }

    pub fn entries(&self) -> impl Iterator<Item = &ProviderEntry> {
        self.entries.iter()
    }

    pub fn providers_for(&self, technology: &TechnologyId) -> impl Iterator<Item = &ProviderEntry> {
        self.by_technology
            .get(technology.as_str())
            .into_iter()
            .flat_map(move |indices| indices.iter().map(|&idx| &self.entries[idx]))
    }

    pub fn instantiate_all(&self) -> Result<Vec<Arc<dyn UiTreeProvider>>, ProviderError> {
        self.entries.iter().map(|entry| entry.instantiate()).collect()
    }

    pub fn with_factories(factories: &[&'static dyn UiTreeProviderFactory]) -> Self {
        let mut entries: Vec<ProviderEntry> = factories
            .iter()
            .map(|factory| ProviderEntry { descriptor: factory.descriptor(), factory: *factory })
            .collect();
        entries.sort_by(|a, b| {
            let tech_cmp = a.descriptor.technology.as_str().cmp(b.descriptor.technology.as_str());
            if tech_cmp != std::cmp::Ordering::Equal {
                return tech_cmp;
            }
            a.descriptor.id.cmp(b.descriptor.id)
        });
        let by_technology = Self::rebuild_by_technology(&entries);
        Self { entries, by_technology }
    }

    /// Returns a new registry that only includes providers with an `id` contained in `ids`.
    /// Order within a technology is preserved from discovery.
    pub fn filter_by_ids(&self, ids: &[&str]) -> Self {
        if ids.is_empty() {
            // Keep current behavior if no filter specified
            return Self { entries: self.entries.clone(), by_technology: self.by_technology.clone() };
        }
        let wanted: HashSet<&str> = ids.iter().copied().collect();
        let entries: Vec<ProviderEntry> =
            self.entries.iter().filter(|e| wanted.contains(e.descriptor.id)).cloned().collect();
        let by_technology = Self::rebuild_by_technology(&entries);
        Self { entries, by_technology }
    }

    fn rebuild_by_technology(entries: &[ProviderEntry]) -> HashMap<String, Vec<usize>> {
        let mut by_technology: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, entry) in entries.iter().enumerate() {
            by_technology.entry(entry.descriptor.technology.as_str().to_owned()).or_default().push(idx);
        }
        by_technology
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::event::ProviderEventDispatcher;
    use platynui_core::provider::{
        ProviderDescriptor, ProviderEvent, ProviderEventKind, ProviderEventListener, ProviderKind,
        UiTreeProviderFactory, register_provider,
    };
    use platynui_core::ui::identifiers::TechnologyId;
    use platynui_core::ui::{Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiValue};
    use rstest::rstest;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, LazyLock, Weak};

    struct DummyAttribute;
    impl UiAttribute for DummyAttribute {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn name(&self) -> &str {
            "Role"
        }
        fn value(&self) -> UiValue {
            UiValue::from("Dummy")
        }
    }

    struct DummyNode {
        runtime_id: RuntimeId,
    }
    impl DummyNode {
        fn new(id: &str) -> Self {
            Self { runtime_id: RuntimeId::from(id) }
        }
    }

    impl UiNode for DummyNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &str {
            "Button"
        }
        fn name(&self) -> String {
            "Dummy".to_string()
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
            Box::new(vec![Arc::new(DummyAttribute) as Arc<dyn UiAttribute>].into_iter())
        }
        fn supported_patterns(&self) -> Vec<PatternId> {
            Vec::new()
        }
        fn invalidate(&self) {}
    }

    static SHUTDOWN_TRIGGERED: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));
    static SUBSCRIPTION_FLAG: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));

    struct StubProvider {
        descriptor: &'static ProviderDescriptor,
        node: Arc<dyn UiNode>,
    }
    impl StubProvider {
        fn new(descriptor: &'static ProviderDescriptor) -> Self {
            Self { descriptor, node: Arc::new(DummyNode::new(descriptor.id)) }
        }
    }

    impl UiTreeProvider for StubProvider {
        fn descriptor(&self) -> &'static ProviderDescriptor {
            self.descriptor
        }
        fn get_nodes(
            &self,
            _parent: Arc<dyn UiNode>,
        ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
            Ok(Box::new(std::iter::once(self.node.clone())))
        }
        fn subscribe_events(&self, listener: Arc<dyn ProviderEventListener>) -> Result<(), ProviderError> {
            listener.on_event(ProviderEvent { kind: ProviderEventKind::TreeInvalidated });
            SUBSCRIPTION_FLAG.store(true, Ordering::SeqCst);
            Ok(())
        }
        fn shutdown(&self) {
            SHUTDOWN_TRIGGERED.store(true, Ordering::SeqCst);
        }
    }

    struct DummyFactory;

    impl DummyFactory {
        fn descriptor_static() -> &'static ProviderDescriptor {
            static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
                ProviderDescriptor::new("dummy", "Dummy", TechnologyId::from("DummyTech"), ProviderKind::Native)
            });
            &DESCRIPTOR
        }
    }

    impl UiTreeProviderFactory for DummyFactory {
        fn descriptor(&self) -> &'static ProviderDescriptor {
            Self::descriptor_static()
        }

        fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
            Ok(Arc::new(StubProvider::new(Self::descriptor_static())))
        }
    }

    static DUMMY_FACTORY: DummyFactory = DummyFactory;

    register_provider!(&DUMMY_FACTORY);

    #[rstest]
    fn registry_builds_from_factories() {
        let registry = ProviderRegistry::with_factories(&[&DUMMY_FACTORY]);
        assert!(registry.entries().any(|entry| entry.descriptor.id == "dummy"));
    }

    #[rstest]
    fn instantiate_all_registers_events_and_shutdown() {
        SHUTDOWN_TRIGGERED.store(false, Ordering::SeqCst);
        SUBSCRIPTION_FLAG.store(false, Ordering::SeqCst);

        let dispatcher = Arc::new(ProviderEventDispatcher::new());
        let registry = ProviderRegistry::with_factories(&[&DUMMY_FACTORY]);
        let providers = registry.instantiate_all().expect("providers");
        assert!(!providers.is_empty());

        for provider in &providers {
            provider.subscribe_events(dispatcher.clone()).expect("subscribe ok");
        }
        assert!(SUBSCRIPTION_FLAG.load(Ordering::SeqCst));

        for provider in &providers {
            provider.shutdown();
        }
        assert!(SHUTDOWN_TRIGGERED.load(Ordering::SeqCst));
    }
}
