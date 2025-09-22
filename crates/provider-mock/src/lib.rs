//! Deterministic mock UiTree provider for testing the runtime.
//!
//! Eventually this crate will expose scriptable trees and predictable RuntimeId
//! assignments to support XPath unit and integration tests.

/// Stub marker type for den Mock-Provider während der Scaffold-Phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MockProviderStub;

#[cfg(test)]
mod tests {
    use platynui_core::types::{Point, Rect};
    use platynui_core::ui::RuntimeId;
    use platynui_core::ui::attribute_names::{activation_target, common, element, text_content};
    use platynui_core::ui::contract::testkit::{
        AttributeExpectation, ContractIssue, NodeExpectation, PatternExpectation, require_node,
        verify_node,
    };
    use platynui_core::ui::{
        Namespace, PatternId, PatternRegistry, UiAttribute, UiNode, UiPattern, UiValue,
    };
    use rstest::rstest;
    use std::sync::{Arc, Mutex, Weak};

    const TEXT_CONTENT_ATTRS: &[AttributeExpectation] =
        &[AttributeExpectation::required(Namespace::Control, text_content::TEXT)];
    const ELEMENT_ATTRS: &[AttributeExpectation] = &[
        AttributeExpectation::required(Namespace::Control, element::BOUNDS),
        AttributeExpectation::required(Namespace::Control, element::IS_VISIBLE),
    ];
    const ACTIVATION_TARGET_ATTRS: &[AttributeExpectation] = &[
        AttributeExpectation::required(Namespace::Control, activation_target::ACTIVATION_POINT),
        AttributeExpectation::optional(Namespace::Control, activation_target::ACTIVATION_AREA),
    ];

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

    struct FocusablePattern;

    impl UiPattern for FocusablePattern {
        fn id(&self) -> PatternId {
            Self::static_id()
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

    struct MockElement {
        namespace: Namespace,
        runtime_id: RuntimeId,
        attributes: Mutex<Vec<Arc<dyn UiAttribute>>>,
        runtime_patterns: PatternRegistry,
        supported: Vec<PatternId>,
    }

    impl MockElement {
        fn focusable_button() -> Arc<Self> {
            let mut runtime_patterns = PatternRegistry::new();
            runtime_patterns.register(Arc::new(FocusablePattern));

            let element = Arc::new(Self {
                namespace: Namespace::Control,
                runtime_id: RuntimeId::from("mock-node"),
                attributes: Mutex::new(Vec::new()),
                runtime_patterns,
                supported: vec![
                    PatternId::from("Focusable"),
                    PatternId::from("TextContent"),
                    PatternId::from("Element"),
                    PatternId::from("ActivationTarget"),
                ],
            });

            let attrs = vec![
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: common::ROLE,
                    value: UiValue::from("Button"),
                }) as Arc<dyn UiAttribute>,
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: common::NAME,
                    value: UiValue::from("OK"),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: common::RUNTIME_ID,
                    value: UiValue::from("mock-node"),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: common::TECHNOLOGY,
                    value: UiValue::from("Mock"),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: common::SUPPORTED_PATTERNS,
                    value: UiValue::Array(vec![
                        UiValue::from("Focusable"),
                        UiValue::from("TextContent"),
                        UiValue::from("Element"),
                        UiValue::from("ActivationTarget"),
                    ]),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: text_content::TEXT,
                    value: UiValue::from("OK"),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: element::BOUNDS,
                    value: UiValue::Rect(Rect::new(0.0, 0.0, 32.0, 16.0)),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: "Bounds.X",
                    value: UiValue::from(0.0),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: "Bounds.Y",
                    value: UiValue::from(0.0),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: "Bounds.Width",
                    value: UiValue::from(32.0),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: "Bounds.Height",
                    value: UiValue::from(16.0),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: element::IS_VISIBLE,
                    value: UiValue::from(true),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: activation_target::ACTIVATION_POINT,
                    value: UiValue::Point(Point::new(16.0, 8.0)),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: "ActivationPoint.X",
                    value: UiValue::from(16.0),
                }),
                Arc::new(StaticAttribute {
                    namespace: Namespace::Control,
                    name: "ActivationPoint.Y",
                    value: UiValue::from(8.0),
                }),
            ];

            element.attributes.lock().unwrap().extend(attrs);
            element
        }
    }

    impl UiNode for MockElement {
        fn namespace(&self) -> Namespace {
            self.namespace
        }

        fn role(&self) -> &str {
            "Button"
        }

        fn name(&self) -> &str {
            "OK"
        }

        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }

        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            None
        }

        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_> {
            Box::new(Vec::<Arc<dyn UiNode>>::new().into_iter())
        }

        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_> {
            Box::new(self.attributes.lock().unwrap().clone().into_iter())
        }

        fn supported_patterns(&self) -> &[PatternId] {
            &self.supported
        }

        fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
            self.runtime_patterns.get(pattern)
        }

        fn invalidate(&self) {}
    }

    fn focusable_expectation() -> NodeExpectation {
        let text = PatternExpectation::new(PatternId::from("TextContent"), TEXT_CONTENT_ATTRS);
        let element = PatternExpectation::new(PatternId::from("Element"), ELEMENT_ATTRS);
        let activation_target =
            PatternExpectation::new(PatternId::from("ActivationTarget"), ACTIVATION_TARGET_ATTRS);

        NodeExpectation::default()
            .with_pattern(text)
            .with_pattern(element)
            .with_pattern(activation_target)
    }

    #[rstest]
    fn contract_helper_accepts_valid_node() {
        let node = MockElement::focusable_button();
        let expectations = focusable_expectation();
        assert!(require_node(node.as_ref(), &expectations).is_ok());
    }

    #[rstest]
    fn contract_helper_flags_missing_attribute() {
        let node = MockElement::focusable_button();
        // Entferne Bounds, damit der Helper schlägt.
        node.attributes.lock().unwrap().retain(|attr| attr.name() != element::BOUNDS);

        let expectations = focusable_expectation();
        let result =
            require_node(node.as_ref(), &expectations).expect_err("expected contract failure");

        assert!(result.iter().any(|issue| matches!(issue,
            ContractIssue::MissingAttribute { name, .. } if name == element::BOUNDS
        )));
    }

    #[rstest]
    fn contract_helper_flags_missing_activation_point() {
        let node = MockElement::focusable_button();
        node.attributes
            .lock()
            .unwrap()
            .retain(|attr| attr.name() != activation_target::ACTIVATION_POINT);

        let expectations = focusable_expectation();
        let result =
            require_node(node.as_ref(), &expectations).expect_err("expected contract failure");

        assert!(result.iter().any(|issue| matches!(issue,
            ContractIssue::MissingAttribute { name, .. } if name == activation_target::ACTIVATION_POINT
        )));
    }

    #[rstest]
    fn contract_helper_flags_missing_alias() {
        let node = MockElement::focusable_button();
        node.attributes
            .lock()
            .unwrap()
            .retain(|attr| attr.name() != "Bounds.X");

        let issues = verify_node(
            node.as_ref(),
            &NodeExpectation::default().with_pattern(PatternExpectation::new(
                PatternId::from("Element"),
                ELEMENT_ATTRS,
            )),
        );

        assert!(issues.iter().any(|issue| matches!(
            issue,
            ContractIssue::MissingGeometryAlias { alias, .. } if alias == "Bounds.X"
        )));
    }
}
