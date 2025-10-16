use super::UiTreeProviderFactory;
use inventory::collect;

pub struct ProviderRegistration {
    pub factory: &'static dyn UiTreeProviderFactory,
}

collect!(ProviderRegistration);

pub fn provider_factories() -> impl Iterator<Item = &'static dyn UiTreeProviderFactory> {
    inventory::iter::<ProviderRegistration>.into_iter().map(|entry| entry.factory)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, register_provider};
    use crate::ui::{Namespace, RuntimeId, UiAttribute, UiNode, UiValue};
    use rstest::rstest;
    use std::sync::{Arc, Mutex, Weak};

    struct DummyAttribute;

    impl UiAttribute for DummyAttribute {
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

    struct DummyNode {
        runtime_id: RuntimeId,
        attributes: Mutex<Vec<Arc<dyn UiAttribute>>>,
    }

    impl DummyNode {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                runtime_id: RuntimeId::from("dummy"),
                attributes: Mutex::new(vec![Arc::new(DummyAttribute) as Arc<dyn UiAttribute>]),
            })
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
            Box::new(self.attributes.lock().unwrap().clone().into_iter())
        }

        fn supported_patterns(&self) -> Vec<crate::ui::PatternId> {
            Vec::new()
        }

        fn invalidate(&self) {}
    }

    struct DummyProvider {
        node: Arc<dyn UiNode>,
        descriptor: ProviderDescriptor,
    }

    impl DummyProvider {
        fn new() -> Self {
            Self {
                node: DummyNode::new(),
                descriptor: ProviderDescriptor::new(
                    "dummy",
                    "Dummy Provider",
                    crate::ui::TechnologyId::from("Dummy"),
                    ProviderKind::Native,
                ),
            }
        }
    }

    impl UiTreeProvider for DummyProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }

        fn get_nodes(
            &self,
            _parent: Arc<dyn UiNode>,
        ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
            Ok(Box::new(std::iter::once(Arc::clone(&self.node))))
        }
    }

    struct DummyFactory;

    impl UiTreeProviderFactory for DummyFactory {
        fn descriptor(&self) -> &ProviderDescriptor {
            static DESCRIPTOR: once_cell::sync::Lazy<ProviderDescriptor> = once_cell::sync::Lazy::new(|| {
                ProviderDescriptor::new(
                    "dummy-factory",
                    "Dummy Factory",
                    crate::ui::TechnologyId::from("Dummy"),
                    ProviderKind::Native,
                )
            });
            &DESCRIPTOR
        }

        fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
            Ok(Arc::new(DummyProvider::new()))
        }
    }

    static FACTORY: DummyFactory = DummyFactory;

    register_provider!(&FACTORY);

    #[rstest]
    fn registered_factory_is_exposed() {
        let factories: Vec<_> = provider_factories().collect();
        assert!(!factories.is_empty());
        assert!(factories.iter().any(|factory| factory.descriptor().id == "dummy-factory"));
    }
}
