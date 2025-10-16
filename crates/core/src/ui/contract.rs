use super::identifiers::PatternId;
use super::{Namespace, UiNode};

pub mod testkit;
use std::collections::HashSet;

/// Describes violations against the shared node contract.
#[derive(Debug, PartialEq, Eq)]
pub enum ContractViolation {
    /// The validator does not support the namespace of the given node.
    UnsupportedNamespace { namespace: Namespace },
    /// `SupportedPatterns` contains duplicate entries.
    DuplicatePattern { pattern: PatternId },
}

/// Ensures that `control:` and `item:` nodes expose the mandatory attributes and
/// that all advertised runtime patterns provide concrete implementations.
pub fn validate_control_or_item(node: &dyn UiNode) -> Result<(), ContractViolation> {
    match node.namespace() {
        Namespace::Control | Namespace::Item => {
            let mut seen = HashSet::new();
            for pattern in node.supported_patterns() {
                if !seen.insert(pattern.clone()) {
                    return Err(ContractViolation::DuplicatePattern { pattern: pattern.clone() });
                }
            }

            Ok(())
        }
        namespace => Err(ContractViolation::UnsupportedNamespace { namespace }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Rect;
    use crate::ui::attribute_names::{common, element};
    use crate::ui::pattern::{PatternError, PatternRegistry};
    use crate::ui::value::UiValue;
    use crate::ui::{UiAttribute, UiPattern};
    use rstest::rstest;
    use std::sync::{Arc, Weak};

    struct StaticAttribute {
        namespace: Namespace,
        name: &'static str,
        value: UiValue,
    }

    impl UiAttribute for StaticAttribute {
        fn namespace(&self) -> Namespace {
            self.namespace
        }

        fn name(&self) -> &str {
            self.name
        }

        fn value(&self) -> UiValue {
            self.value.clone()
        }
    }

    struct MockNode {
        namespace: Namespace,
        runtime_id: super::super::identifiers::RuntimeId,
        attributes: Vec<Arc<dyn UiAttribute>>,
        patterns: PatternRegistry,
        supported: Vec<PatternId>,
    }

    impl MockNode {
        fn new(namespace: Namespace) -> Self {
            Self {
                namespace,
                runtime_id: super::super::identifiers::RuntimeId::from("node-1"),
                attributes: Vec::new(),
                patterns: PatternRegistry::new(),
                supported: Vec::new(),
            }
        }

        fn with_required_attributes(mut self) -> Self {
            let rect = UiValue::Rect(Rect::new(0.0, 0.0, 10.0, 10.0));
            self.attributes.push(Arc::new(StaticAttribute {
                namespace: self.namespace,
                name: element::BOUNDS,
                value: rect.clone(),
            }));
            self.attributes.push(Arc::new(StaticAttribute {
                namespace: self.namespace,
                name: common::ROLE,
                value: UiValue::from("Button"),
            }));
            self.attributes.push(Arc::new(StaticAttribute {
                namespace: self.namespace,
                name: common::NAME,
                value: UiValue::from("OK"),
            }));
            self.attributes.push(Arc::new(StaticAttribute {
                namespace: self.namespace,
                name: element::IS_VISIBLE,
                value: UiValue::from(true),
            }));
            self.attributes.push(Arc::new(StaticAttribute {
                namespace: self.namespace,
                name: element::IS_ENABLED,
                value: UiValue::from(true),
            }));
            self.attributes.push(Arc::new(StaticAttribute {
                namespace: self.namespace,
                name: common::RUNTIME_ID,
                value: UiValue::from("node-1"),
            }));
            self.attributes.push(Arc::new(StaticAttribute {
                namespace: self.namespace,
                name: common::TECHNOLOGY,
                value: UiValue::from("Mock"),
            }));
            let focusable = PatternId::from("Focusable");
            self.attributes.push(Arc::new(StaticAttribute {
                namespace: self.namespace,
                name: common::SUPPORTED_PATTERNS,
                value: UiValue::Array(vec![UiValue::from(focusable.as_str().to_owned())]),
            }));
            self.supported = vec![focusable];
            self
        }

        fn with_focusable_pattern(mut self) -> Self {
            self.patterns.register_dyn(Arc::new(MockFocusablePattern) as Arc<dyn UiPattern>);
            let focusable = PatternId::from("Focusable");
            if !self.supported.iter().any(|id| id == &focusable) {
                self.supported.push(focusable);
            }
            self
        }
    }

    impl UiNode for MockNode {
        fn namespace(&self) -> Namespace {
            self.namespace
        }

        fn role(&self) -> &str {
            "Button"
        }

        fn name(&self) -> String {
            "OK".to_string()
        }

        fn runtime_id(&self) -> &super::super::identifiers::RuntimeId {
            &self.runtime_id
        }

        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            None
        }

        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
            Box::new(Vec::<Arc<dyn UiNode>>::new().into_iter())
        }

        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
            Box::new(self.attributes.clone().into_iter())
        }

        fn supported_patterns(&self) -> Vec<PatternId> {
            if self.supported.is_empty() { self.patterns.supported() } else { self.supported.clone() }
        }

        fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
            self.patterns.get(pattern)
        }

        fn invalidate(&self) {}
    }

    struct MockFocusablePattern;

    impl UiPattern for MockFocusablePattern {
        fn id(&self) -> PatternId {
            PatternId::from("Focusable")
        }

        fn static_id() -> PatternId
        where
            Self: Sized,
        {
            PatternId::from("Focusable")
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    impl crate::ui::pattern::FocusablePattern for MockFocusablePattern {
        fn focus(&self) -> Result<(), PatternError> {
            Ok(())
        }
    }

    #[rstest]
    fn validates_successfully_for_complete_node() {
        let node = MockNode::new(Namespace::Control).with_required_attributes().with_focusable_pattern();
        assert!(validate_control_or_item(&node).is_ok());
    }

    #[rstest]
    fn runtime_pattern_without_instance_is_allowed() {
        let node = MockNode::new(Namespace::Control).with_required_attributes();
        assert!(validate_control_or_item(&node).is_ok());
    }

    #[rstest]
    fn fails_when_supported_patterns_duplicate_values() {
        let mut node = MockNode::new(Namespace::Control).with_required_attributes();
        node.supported.push(PatternId::from("Focusable"));
        let result = validate_control_or_item(&node);
        assert!(matches!(
            result,
            Err(ContractViolation::DuplicatePattern { pattern }) if pattern.as_str() == "Focusable"
        ));
    }

    #[rstest]
    fn rejects_unsupported_namespace() {
        let node = MockNode::new(Namespace::App).with_required_attributes();
        let result = validate_control_or_item(&node);
        assert!(
            matches!(result, Err(ContractViolation::UnsupportedNamespace { namespace }) if namespace == Namespace::App)
        );
    }
}
