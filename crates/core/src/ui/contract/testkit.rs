//! Helper types for provider contract tests.
//!
//! This module provides structures for provider contract tests to declare
//! expected attributes per pattern. The actual verification logic lives in a
//! separate step; here we only define the data models those checks consume.

use crate::ui::{Namespace, PatternName, UiAttribute, UiNode, UiValue};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Describes an expected attribute for contract tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttributeExpectation {
    pub namespace: Namespace,
    pub name: &'static str,
    pub optional: bool,
}

impl AttributeExpectation {
    pub const fn required(namespace: Namespace, name: &'static str) -> Self {
        Self { namespace, name, optional: false }
    }

    pub const fn optional(namespace: Namespace, name: &'static str) -> Self {
        Self { namespace, name, optional: true }
    }
}

/// Groups attribute expectations for a pattern.
#[derive(Debug)]
pub struct PatternExpectation {
    pub id: PatternName,
    pub attributes: &'static [AttributeExpectation],
}

impl PatternExpectation {
    pub const fn new(id: PatternName, attributes: &'static [AttributeExpectation]) -> Self {
        Self { id, attributes }
    }
}

/// Collection of pattern expectations for a node.
#[derive(Debug, Default)]
pub struct NodeExpectation {
    pub patterns: Vec<PatternExpectation>,
}

impl NodeExpectation {
    pub fn with_pattern(mut self, pattern: PatternExpectation) -> Self {
        self.patterns.push(pattern);
        self
    }
}

/// Result of a contract check.
#[derive(Clone, Debug, PartialEq)]
pub enum ContractIssue {
    MissingPattern {
        pattern: PatternName,
    },
    MissingAttribute {
        pattern: PatternName,
        namespace: Namespace,
        name: String,
    },
    NullAttribute {
        pattern: PatternName,
        namespace: Namespace,
        name: String,
    },
    MissingGeometryAlias {
        pattern: PatternName,
        namespace: Namespace,
        alias: String,
    },
    GeometryAliasMismatch {
        pattern: PatternName,
        namespace: Namespace,
        alias: String,
        expected: UiValue,
        actual: UiValue,
    },
}

/// Verifies a node against expectations and returns all detected deviations.
pub fn verify_node(node: &dyn UiNode, expectations: &NodeExpectation) -> Vec<ContractIssue> {
    let mut issues = Vec::new();

    let supported: HashSet<PatternName> = node.supported_patterns().into_iter().collect();
    let attributes = collect_attributes(node);

    for pattern in &expectations.patterns {
        if !supported.contains(&pattern.id) {
            issues.push(ContractIssue::MissingPattern { pattern: pattern.id.clone() });
            continue;
        }

        for attr in pattern.attributes {
            let key = (attr.namespace, attr.name.to_owned());
            match attributes.get(&key) {
                None if !attr.optional => {
                    issues.push(ContractIssue::MissingAttribute {
                        pattern: pattern.id.clone(),
                        namespace: attr.namespace,
                        name: attr.name.to_owned(),
                    });
                }
                Some(value) if value.is_null() && !attr.optional => {
                    issues.push(ContractIssue::NullAttribute {
                        pattern: pattern.id.clone(),
                        namespace: attr.namespace,
                        name: attr.name.to_owned(),
                    });
                }
                Some(_) => {
                    // Note: Derived geometry aliases (Bounds.X/Y/Width/Height, ActivationPoint.X/Y)
                    // are produced by the Runtime/XPath layer and are no longer part of the
                    // provider contract. Providers should expose only the base attributes such as
                    // Bounds (Rect) and ActivationPoint (Point).
                }
                _ => {}
            }
        }
    }

    issues
}

/// Verifies a node and returns a detailed list on the first failure.
pub fn require_node(node: &dyn UiNode, expectations: &NodeExpectation) -> Result<(), Vec<ContractIssue>> {
    let issues = verify_node(node, expectations);
    if issues.is_empty() { Ok(()) } else { Err(issues) }
}

fn collect_attributes(node: &dyn UiNode) -> HashMap<(Namespace, String), UiValue> {
    let mut map = HashMap::new();
    let attributes: Vec<Arc<dyn UiAttribute>> = node.attributes().collect();
    for attr in attributes {
        map.insert((attr.namespace(), attr.name().to_owned()), attr.value());
    }
    map
}

// Removed: geometry alias checks. Aliases are resolved by the Runtime/XPath layer.

#[cfg(test)]
mod geometry_tests {
    use super::*;
    use crate::types::{Point, Rect};
    use crate::ui::{Namespace, UiAttribute, pattern_names};
    use std::sync::{Arc, LazyLock};

    const ELEMENT_EXPECTATIONS: [AttributeExpectation; 1] =
        [AttributeExpectation::required(Namespace::Control, crate::ui::attribute_names::element::BOUNDS)];

    const ACTIVATION_EXPECTATIONS: [AttributeExpectation; 1] = [AttributeExpectation::required(
        Namespace::Control,
        crate::ui::attribute_names::activation_target::ACTIVATION_POINT,
    )];

    struct StaticAttribute {
        namespace: Namespace,
        name: &'static str,
        value: UiValue,
    }

    impl StaticAttribute {
        fn new(namespace: Namespace, name: &'static str, value: UiValue) -> Arc<Self> {
            Arc::new(Self { namespace, name, value })
        }
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

    fn sample_expectation() -> NodeExpectation {
        NodeExpectation::default()
            .with_pattern(PatternExpectation::new(PatternName::from(pattern_names::ELEMENT), &ELEMENT_EXPECTATIONS))
    }

    struct AttrNode {
        attributes: Vec<Arc<dyn UiAttribute>>,
    }

    impl AttrNode {
        fn new(attributes: Vec<Arc<dyn UiAttribute>>) -> Self {
            Self { attributes }
        }
    }

    impl UiNode for AttrNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }

        fn role(&self) -> &str {
            "Node"
        }

        fn name(&self) -> String {
            "Node".to_string()
        }

        fn runtime_id(&self) -> &crate::ui::identifiers::RuntimeId {
            static RID: LazyLock<crate::ui::identifiers::RuntimeId> =
                LazyLock::new(|| crate::ui::identifiers::RuntimeId::from("node"));
            &RID
        }

        fn parent(&self) -> Option<std::sync::Weak<dyn UiNode>> {
            None
        }

        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
            Box::new(std::iter::empty())
        }

        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
            Box::new(self.attributes.clone().into_iter())
        }

        fn supported_patterns(&self) -> Vec<PatternName> {
            vec![PatternName::from(pattern_names::ELEMENT), PatternName::from(pattern_names::ACTIVATION_TARGET)]
        }

        fn invalidate(&self) {}
    }

    #[test]
    fn does_not_require_geometry_aliases_anymore() {
        let node = AttrNode::new(vec![StaticAttribute::new(
            Namespace::Control,
            crate::ui::attribute_names::element::BOUNDS,
            UiValue::Rect(Rect::new(0.0, 0.0, 100.0, 50.0)),
        ) as Arc<dyn UiAttribute>]);
        let issues = verify_node(&node, &sample_expectation());
        assert!(issues.is_empty(), "no alias issues expected: {issues:?}");
    }

    #[test]
    fn does_not_compare_alias_values_anymore() {
        let node = AttrNode::new(vec![
            StaticAttribute::new(
                Namespace::Control,
                crate::ui::attribute_names::element::BOUNDS,
                UiValue::Rect(Rect::new(0.0, 0.0, 100.0, 50.0)),
            ),
            StaticAttribute::new(Namespace::Control, "Bounds.X", UiValue::from(1.0)),
        ]);
        let issues = verify_node(&node, &sample_expectation());
        assert!(issues.is_empty(), "no alias mismatch expected: {issues:?}");
    }

    #[test]
    fn activation_point_aliases_not_required() {
        let expectation = NodeExpectation::default().with_pattern(PatternExpectation::new(
            PatternName::from(pattern_names::ACTIVATION_TARGET),
            &ACTIVATION_EXPECTATIONS,
        ));
        let node = AttrNode::new(vec![StaticAttribute::new(
            Namespace::Control,
            crate::ui::attribute_names::activation_target::ACTIVATION_POINT,
            UiValue::Point(Point::new(10.0, 10.0)),
        )]);
        let issues = verify_node(&node, &expectation);
        assert!(issues.is_empty(), "no alias issues expected: {issues:?}");
    }
}

#[cfg(test)]
mod expectation_tests {
    use super::*;
    use crate::types::Rect;
    use crate::ui::attribute_names::{activatable, common, element};
    use crate::ui::pattern::{PatternRegistry, UiPattern};
    use crate::ui::{UiAttribute, UiNode, pattern_names};
    use rstest::rstest;
    use std::sync::{Arc, Mutex, Weak};

    const TEXT_CONTENT_ATTRS: &[AttributeExpectation] = &[AttributeExpectation::required(Namespace::Control, "Text")];
    const ELEMENT_ATTRS: &[AttributeExpectation] = &[
        AttributeExpectation::required(Namespace::Control, element::BOUNDS),
        AttributeExpectation::required(Namespace::Control, element::IS_VISIBLE),
        AttributeExpectation::optional(Namespace::Control, element::IS_IN_VIEW),
    ];
    const ACTIVATABLE_ATTRS: &[AttributeExpectation] =
        &[AttributeExpectation::required(Namespace::Control, activatable::IS_ACTIVATION_ENABLED)];

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

    struct MockPattern(PatternName);

    impl UiPattern for MockPattern {
        fn pattern_name(&self) -> PatternName {
            self.0.clone()
        }

        fn static_pattern_name() -> PatternName
        where
            Self: Sized,
        {
            PatternName::from(pattern_names::MOCK)
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    struct MockNode {
        namespace: Namespace,
        runtime_id: crate::ui::RuntimeId,
        attributes: Mutex<Vec<Arc<dyn UiAttribute>>>,
        patterns: PatternRegistry,
    }

    impl MockNode {
        fn new(namespace: Namespace) -> Self {
            Self {
                namespace,
                runtime_id: crate::ui::RuntimeId::from("node-1"),
                attributes: Mutex::new(Vec::new()),
                patterns: PatternRegistry::new(),
            }
        }

        fn with_attribute(self, attribute: Arc<dyn UiAttribute>) -> Self {
            self.attributes.lock().unwrap().push(attribute);
            self
        }

        fn with_pattern(self, pattern: PatternName) -> Self {
            let arc: Arc<dyn UiPattern> = Arc::new(MockPattern(pattern.clone()));
            self.patterns.register_dyn(arc);
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

        fn runtime_id(&self) -> &crate::ui::RuntimeId {
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

        fn supported_patterns(&self) -> Vec<PatternName> {
            self.patterns.supported()
        }

        fn pattern_by_name(&self, pattern: &PatternName) -> Option<Arc<dyn UiPattern>> {
            self.patterns.get(pattern)
        }

        fn invalidate(&self) {}
    }

    fn build_expectation() -> NodeExpectation {
        let text_pattern = PatternExpectation::new(PatternName::from(pattern_names::TEXT_CONTENT), TEXT_CONTENT_ATTRS);
        let element_pattern = PatternExpectation::new(PatternName::from(pattern_names::ELEMENT), ELEMENT_ATTRS);
        let activatable_pattern =
            PatternExpectation::new(PatternName::from(pattern_names::ACTIVATABLE), ACTIVATABLE_ATTRS);

        NodeExpectation::default()
            .with_pattern(text_pattern)
            .with_pattern(element_pattern)
            .with_pattern(activatable_pattern)
    }

    fn build_node() -> Arc<MockNode> {
        let node = MockNode::new(Namespace::Control)
            .with_pattern(PatternName::from(pattern_names::TEXT_CONTENT))
            .with_pattern(PatternName::from(pattern_names::ELEMENT))
            .with_pattern(PatternName::from(pattern_names::ACTIVATABLE));

        let attrs: Vec<Arc<dyn UiAttribute>> = vec![
            Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: common::ROLE,
                value: UiValue::from("Button"),
            }),
            Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: common::RUNTIME_ID,
                value: UiValue::from("node-1"),
            }),
            Arc::new(StaticAttribute { namespace: Namespace::Control, name: "Text", value: UiValue::from("OK") }),
            Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: element::BOUNDS,
                value: UiValue::Rect(Rect::new(0.0, 0.0, 10.0, 5.0)),
            }),
            Arc::new(StaticAttribute { namespace: Namespace::Control, name: "Bounds.X", value: UiValue::from(0.0) }),
            Arc::new(StaticAttribute { namespace: Namespace::Control, name: "Bounds.Y", value: UiValue::from(0.0) }),
            Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: "Bounds.Width",
                value: UiValue::from(10.0),
            }),
            Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: "Bounds.Height",
                value: UiValue::from(5.0),
            }),
            Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: element::IS_VISIBLE,
                value: UiValue::from(true),
            }),
            Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: activatable::IS_ACTIVATION_ENABLED,
                value: UiValue::from(true),
            }),
        ];

        {
            let mut lock = node.attributes.lock().unwrap();
            *lock = attrs;
        }

        Arc::new(node)
    }

    #[rstest]
    fn verify_node_detects_success() {
        let node = build_node();
        let expectations = build_expectation();

        let result = verify_node(node.as_ref(), &expectations);
        assert!(result.is_empty(), "expected no issues, got {result:?}");
    }

    #[rstest]
    fn verify_node_reports_missing_pattern() {
        let node = MockNode::new(Namespace::Control).with_pattern(PatternName::from(pattern_names::ELEMENT));
        let expectations = build_expectation();

        let result = verify_node(&node, &expectations);
        assert!(result.iter().any(
            |issue| matches!(issue, ContractIssue::MissingPattern { pattern } if pattern.as_str() == pattern_names::TEXT_CONTENT)
        ));
    }

    #[rstest]
    fn verify_node_reports_missing_attribute() {
        let node = build_node();
        node.attributes.lock().unwrap().retain(|attr| attr.name() != "Text");
        let expectations = build_expectation();

        let result = verify_node(node.as_ref(), &expectations);
        assert!(result.iter().any(|issue| matches!(issue,
            ContractIssue::MissingAttribute { pattern, name, .. }
                if pattern.as_str() == pattern_names::TEXT_CONTENT && name == "Text"
        )));
    }

    #[rstest]
    fn verify_node_reports_null_attribute() {
        let node = MockNode::new(Namespace::Control)
            .with_pattern(PatternName::from(pattern_names::ELEMENT))
            .with_pattern(PatternName::from(pattern_names::TEXT_CONTENT))
            .with_pattern(PatternName::from(pattern_names::ACTIVATABLE))
            .with_attribute(Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: "Text",
                value: UiValue::Null,
            }))
            .with_attribute(Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: element::BOUNDS,
                value: UiValue::Rect(Rect::new(0.0, 0.0, 10.0, 5.0)),
            }))
            .with_attribute(Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: "Bounds.X",
                value: UiValue::from(0.0),
            }))
            .with_attribute(Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: "Bounds.Y",
                value: UiValue::from(0.0),
            }))
            .with_attribute(Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: "Bounds.Width",
                value: UiValue::from(10.0),
            }))
            .with_attribute(Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: "Bounds.Height",
                value: UiValue::from(5.0),
            }))
            .with_attribute(Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: element::IS_VISIBLE,
                value: UiValue::from(true),
            }))
            .with_attribute(Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: activatable::IS_ACTIVATION_ENABLED,
                value: UiValue::from(true),
            }));
        let expectations = build_expectation();

        let result = verify_node(&node, &expectations);
        assert!(result.iter().any(|issue| matches!(issue,
            ContractIssue::NullAttribute { pattern, name, .. }
                if pattern.as_str() == pattern_names::TEXT_CONTENT && name == "Text"
        )));
    }

    #[rstest]
    fn require_node_returns_result() {
        let node = build_node();
        let expectations = build_expectation();
        assert!(require_node(node.as_ref(), &expectations).is_ok());

        let node = build_node();
        node.attributes.lock().unwrap().retain(|attr| attr.name() != element::IS_VISIBLE);
        assert!(require_node(node.as_ref(), &expectations).is_err());
    }
}
