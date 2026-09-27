//! Helper types for provider contract tests.
//!
//! This module provides structures for provider contract tests to declare
//! expected attributes per pattern. The actual verification logic lives in a
//! separate step; here we only define the data models those checks consume.

use crate::ui::attribute_names::common;
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
    #[must_use]
    pub const fn required(namespace: Namespace, name: &'static str) -> Self {
        Self { namespace, name, optional: false }
    }

    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    /// A pattern-independent common attribute is absent — see
    /// [`verify_common_attributes`].
    MissingCommonAttribute {
        namespace: Namespace,
        name: String,
    },
    /// A pattern-independent common attribute is present but carries no value.
    NullCommonAttribute {
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
    /// A listed child's parent is no longer reachable once everything but the
    /// child was dropped — see [`verify_children_keep_parent`].
    ChildParentUnreachable {
        child: String,
    },
    /// A listed child's parent is reachable, but is not the node the child was
    /// listed from.
    ChildParentMismatch {
        child: String,
        expected: String,
        actual: String,
    },
    /// A listed node is still alive after every reference the check took was
    /// dropped — see [`verify_subtree_released`].
    NodeNotReleased {
        node: String,
    },
    /// The check could not prove what it checks, and says so instead of
    /// passing.
    Unprovable {
        check: &'static str,
        reason: String,
    },
    /// A node's document-order key does not follow the key of a node before it
    /// in document order — see [`verify_doc_order_keys`].
    OrderKeyOutOfDocumentOrder {
        node: String,
        key: u64,
        preceding: String,
        preceding_key: u64,
    },
}

/// The attributes every `control:`/`item:` node carries regardless of the
/// patterns it advertises (`dev-docs/architecture.md` §6.3).
///
/// `Id` and `Description` are conditional by contract — present only when the
/// platform reports a value — so they are declared optional here; a provider
/// that omits them for an element without an automation id or description is
/// conforming.
pub const COMMON_ATTRIBUTES: &[AttributeExpectation] = &[
    AttributeExpectation::required(Namespace::Control, common::ROLE),
    AttributeExpectation::required(Namespace::Control, common::NAME),
    AttributeExpectation::required(Namespace::Control, common::RUNTIME_ID),
    AttributeExpectation::required(Namespace::Control, common::TECHNOLOGY),
    AttributeExpectation::required(Namespace::Control, common::SUPPORTED_PATTERNS),
    AttributeExpectation::optional(Namespace::Control, common::ID),
    AttributeExpectation::optional(Namespace::Control, common::DESCRIPTION),
];

/// The common-attribute expectations, for provider suites that want to inspect
/// or extend the set rather than call [`verify_common_attributes`] directly.
#[must_use]
pub fn common_attributes() -> &'static [AttributeExpectation] {
    COMMON_ATTRIBUTES
}

/// Verifies the pattern-independent common attributes of a `control:`/`item:`
/// node and returns all detected deviations.
///
/// Separate from [`verify_node`] because these attributes are not tied to any
/// pattern: they must be present even on a node that advertises nothing, so
/// gating them behind a [`PatternExpectation`] would report a missing pattern
/// instead of the missing attribute.
pub fn verify_common_attributes(node: &dyn UiNode) -> Vec<ContractIssue> {
    let attributes = collect_attributes(node);
    let mut issues = Vec::new();

    for expectation in COMMON_ATTRIBUTES {
        // Providers disagree on where standard attributes live: JAB and UIA put
        // them in `Control` unconditionally, AT-SPI and the mock use the node's
        // own namespace (`item:` nodes included). Both spellings satisfy the
        // contract until that inconsistency is settled, so accept either.
        let value = attributes
            .get(&(expectation.namespace, expectation.name.to_owned()))
            .or_else(|| attributes.get(&(node.namespace(), expectation.name.to_owned())));

        match value {
            None if !expectation.optional => issues.push(ContractIssue::MissingCommonAttribute {
                namespace: expectation.namespace,
                name: expectation.name.to_owned(),
            }),
            Some(value) if value.is_null() && !expectation.optional => {
                issues.push(ContractIssue::NullCommonAttribute {
                    namespace: expectation.namespace,
                    name: expectation.name.to_owned(),
                });
            }
            _ => {}
        }
    }

    issues
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
                _ => {
                    // Note: Derived geometry aliases (Bounds.X/Y/Width/Height, ActivationPoint.X/Y)
                    // are produced by the Runtime/XPath layer and are no longer part of the
                    // provider contract. Providers should expose only the base attributes such as
                    // Bounds (Rect) and ActivationPoint (Point).
                }
            }
        }
    }

    issues
}

/// Verifies a node and returns a detailed list on the first failure.
///
/// # Errors
///
/// Returns every [`ContractIssue`] that [`verify_node`] detects when the list
/// is not empty.
pub fn require_node(node: &dyn UiNode, expectations: &NodeExpectation) -> Result<(), Vec<ContractIssue>> {
    let issues = verify_node(node, expectations);
    if issues.is_empty() { Ok(()) } else { Err(issues) }
}

/// Verifies that the children a provider lists keep their parent reachable
/// (see [`UiNode::parent`]): lists up to `max_children` children of `parent`,
/// drops the listing and `parent`, and checks that each child's parent still
/// upgrades to the node it was listed from, and that each child keeps it on
/// its own rather than through a sibling.
///
/// Pass the only strong reference to `parent`. When `parent` outlives its
/// children — because the caller or the provider's own tree still holds it —
/// the check cannot tell whether the children keep it, and reports
/// [`ContractIssue::Unprovable`] instead of passing. A provider that owns its
/// whole tree, such as the mock provider, meets the rule through that
/// ownership and is reported this way. A node without children is reported
/// the same way.
#[must_use]
pub fn verify_children_keep_parent(parent: Arc<dyn UiNode>, max_children: usize) -> Vec<ContractIssue> {
    const CHECK: &str = "children keep their parent";
    let expected = parent.runtime_id().as_str().to_owned();
    let parent_alive = Arc::downgrade(&parent);
    let children: Vec<Arc<dyn UiNode>> = parent.children().take(max_children).collect();
    drop(parent);

    if children.is_empty() {
        return vec![ContractIssue::Unprovable { check: CHECK, reason: format!("{expected} lists no children") }];
    }

    let mut issues = Vec::new();
    for child in &children {
        let child_id = child.runtime_id().as_str().to_owned();
        match child.parent().and_then(|parent| parent.upgrade()) {
            None => issues.push(ContractIssue::ChildParentUnreachable { child: child_id }),
            Some(actual) if actual.runtime_id().as_str() != expected => {
                issues.push(ContractIssue::ChildParentMismatch {
                    child: child_id,
                    expected: expected.clone(),
                    actual: actual.runtime_id().as_str().to_owned(),
                });
            }
            Some(_) => {}
        }
    }

    // Siblings share their parent, so one child that keeps it makes every
    // sibling's parent reachable above. A child's own share shows when it is
    // dropped: dropping a child that keeps the parent lowers the parent's
    // count, and once the parent is gone, no child left keeps it.
    let mut not_keeping = Vec::new();
    for child in children {
        let child_id = child.runtime_id().as_str().to_owned();
        let held_elsewhere = Arc::strong_count(&child) > 1;
        let before = parent_alive.strong_count();
        drop(child);
        let lowered = parent_alive.strong_count() < before;
        if before == 0 || (!held_elsewhere && !lowered) {
            not_keeping.push(child_id);
        }
    }

    if parent_alive.strong_count() > 0 {
        if issues.is_empty() {
            return vec![ContractIssue::Unprovable {
                check: CHECK,
                reason: format!("{expected} outlives its children, so something besides them holds it"),
            }];
        }
        return issues;
    }
    for child in not_keeping {
        let reported = issues.iter().any(|issue| match issue {
            ContractIssue::ChildParentUnreachable { child: reported }
            | ContractIssue::ChildParentMismatch { child: reported, .. } => *reported == child,
            _ => false,
        });
        if !reported {
            issues.push(ContractIssue::ChildParentUnreachable { child });
        }
    }
    issues
}

/// Verifies that a provider's nodes do not hold their children: lists the
/// subtree below `root` breadth-first, up to `max_nodes` nodes, drops every
/// reference the check took, and checks that each listed node was released.
///
/// Together with [`verify_children_keep_parent`] this rules out cycles: a node
/// that kept its children alive while they keep it would never be released.
/// `root` itself may still be held elsewhere. A provider that owns its whole
/// tree, such as the mock provider, fails this check by design and is exempt
/// from it.
#[must_use]
pub fn verify_subtree_released(root: Arc<dyn UiNode>, max_nodes: usize) -> Vec<ContractIssue> {
    let mut listed: Vec<Arc<dyn UiNode>> = root.children().take(max_nodes).collect();
    let mut index = 0;
    while index < listed.len() && listed.len() < max_nodes {
        let remaining = max_nodes - listed.len();
        let children: Vec<Arc<dyn UiNode>> = listed[index].children().take(remaining).collect();
        listed.extend(children);
        index += 1;
    }

    if listed.is_empty() {
        return vec![ContractIssue::Unprovable {
            check: "nodes do not hold their children",
            reason: format!("{} lists no children", root.runtime_id().as_str()),
        }];
    }

    let watched: Vec<(String, std::sync::Weak<dyn UiNode>)> =
        listed.iter().map(|node| (node.runtime_id().as_str().to_owned(), Arc::downgrade(node))).collect();
    drop(listed);
    drop(root);

    watched
        .into_iter()
        .filter(|(_, node)| node.strong_count() > 0)
        .map(|(node, _)| ContractIssue::NodeNotReleased { node })
        .collect()
}

/// Lists `roots` and the subtrees below them in document order (pre-order),
/// up to `max_nodes` nodes, for checks that need nodes in that order.
#[must_use]
pub fn nodes_in_document_order(
    roots: impl IntoIterator<Item = Arc<dyn UiNode>>,
    max_nodes: usize,
) -> Vec<Arc<dyn UiNode>> {
    let mut pending: Vec<Arc<dyn UiNode>> = roots.into_iter().collect();
    pending.reverse();
    let mut ordered = Vec::new();
    while ordered.len() < max_nodes
        && let Some(node) = pending.pop()
    {
        let mut children: Vec<Arc<dyn UiNode>> = node.children().collect();
        children.reverse();
        pending.extend(children);
        ordered.push(node);
    }
    ordered
}

/// Verifies the document-order keys of `nodes`, which must be given in
/// document order: every key has to be greater than the key of each keyed node
/// before it (see [`UiNode::doc_order_key`]). Nodes without a key are skipped.
///
/// Give it a provider's top-level nodes as it lists them, or
/// [`nodes_in_document_order`] of them.
#[must_use]
pub fn verify_doc_order_keys(nodes: &[Arc<dyn UiNode>]) -> Vec<ContractIssue> {
    let mut issues = Vec::new();
    let mut greatest: Option<(u64, String)> = None;
    for node in nodes {
        let Some(key) = node.doc_order_key() else { continue };
        let id = node.runtime_id().as_str().to_owned();
        match &greatest {
            Some((preceding_key, preceding)) if key <= *preceding_key => {
                issues.push(ContractIssue::OrderKeyOutOfDocumentOrder {
                    node: id,
                    key,
                    preceding: preceding.clone(),
                    preceding_key: *preceding_key,
                });
            }
            _ => greatest = Some((key, id)),
        }
    }
    issues
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

        fn role(&self) -> &'static str {
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
    use crate::ui::attribute_names::{activatable, common, element, text_content};
    use crate::ui::pattern::{PatternRegistry, UiPattern};
    use crate::ui::{UiAttribute, UiNode, pattern_names};
    use rstest::rstest;
    use std::sync::{Arc, Mutex, Weak};

    const TEXT_CONTENT_ATTRS: &[AttributeExpectation] =
        &[AttributeExpectation::required(Namespace::Control, text_content::TEXT)];
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
            let arc: Arc<dyn UiPattern> = Arc::new(MockPattern(pattern));
            self.patterns.register_dyn(arc);
            self
        }
    }

    impl UiNode for MockNode {
        fn namespace(&self) -> Namespace {
            self.namespace
        }

        fn role(&self) -> &'static str {
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

    fn build_common_attribute_node() -> Arc<MockNode> {
        let node = MockNode::new(Namespace::Control);
        let attrs: Vec<Arc<dyn UiAttribute>> = vec![
            Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: common::ROLE,
                value: UiValue::from("Button"),
            }),
            Arc::new(StaticAttribute { namespace: Namespace::Control, name: common::NAME, value: UiValue::from("OK") }),
            Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: common::RUNTIME_ID,
                value: UiValue::from("node-1"),
            }),
            Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: common::TECHNOLOGY,
                value: UiValue::from("Mock"),
            }),
            Arc::new(StaticAttribute {
                namespace: Namespace::Control,
                name: common::SUPPORTED_PATTERNS,
                value: UiValue::Array(Vec::new()),
            }),
        ];
        {
            let mut lock = node.attributes.lock().unwrap();
            *lock = attrs;
        }
        Arc::new(node)
    }

    #[rstest]
    fn common_attributes_accepts_a_conforming_node() {
        let node = build_common_attribute_node();
        let issues = verify_common_attributes(node.as_ref());
        assert!(issues.is_empty(), "expected no issues, got {issues:?}");
    }

    #[rstest]
    fn common_attributes_reports_missing_technology() {
        let node = build_common_attribute_node();
        node.attributes.lock().unwrap().retain(|attr| attr.name() != common::TECHNOLOGY);

        let issues = verify_common_attributes(node.as_ref());
        assert!(
            issues.iter().any(|issue| matches!(issue,
                ContractIssue::MissingCommonAttribute { name, .. } if name == common::TECHNOLOGY
            )),
            "expected a missing-Technology issue, got {issues:?}"
        );
    }

    #[rstest]
    fn common_attributes_tolerate_the_nodes_own_namespace() {
        // AT-SPI and the mock attach standard attributes to the node's own
        // namespace; an `item:` node spelling them that way still conforms.
        let node = MockNode::new(Namespace::Item);
        for name in [common::ROLE, common::NAME, common::RUNTIME_ID, common::TECHNOLOGY] {
            node.attributes.lock().unwrap().push(Arc::new(StaticAttribute {
                namespace: Namespace::Item,
                name,
                value: UiValue::from("x"),
            }));
        }
        node.attributes.lock().unwrap().push(Arc::new(StaticAttribute {
            namespace: Namespace::Item,
            name: common::SUPPORTED_PATTERNS,
            value: UiValue::Array(Vec::new()),
        }));

        let issues = verify_common_attributes(&node);
        assert!(issues.is_empty(), "expected no issues, got {issues:?}");
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

#[cfg(test)]
mod ownership_tests {
    use super::*;
    use crate::ui::RuntimeId;
    use rstest::rstest;
    use std::sync::{Mutex, OnceLock, Weak};

    /// How a [`LazyNode`] treats its parent and its children.
    #[derive(Clone, Copy, Debug)]
    struct Policy {
        /// A listed child holds its parent strongly (the provider rule).
        keep_parent: bool,
        /// Only the first child of a listing holds its parent, as a provider
        /// that forgets the rule on one of its paths would.
        first_child_only: bool,
        /// A node keeps the children it listed, which together with the rule
        /// forms a cycle.
        cache_children: bool,
    }

    const WEAK_ONLY: Policy = Policy { keep_parent: false, first_child_only: false, cache_children: false };
    const KEEPS_PARENT: Policy = Policy { keep_parent: true, first_child_only: false, cache_children: false };
    const FIRST_CHILD_KEEPS_PARENT: Policy =
        Policy { keep_parent: true, first_child_only: true, cache_children: false };
    const KEEPS_PARENT_AND_CHILDREN: Policy =
        Policy { keep_parent: true, first_child_only: false, cache_children: true };

    /// A node that creates fresh children on every listing, as real providers
    /// do: a root, three children, and three grandchildren under each child.
    struct LazyNode {
        runtime_id: RuntimeId,
        depth: usize,
        policy: Policy,
        parent: Option<Weak<dyn UiNode>>,
        /// Held only to keep the parent alive (the provider rule).
        _kept_parent: Option<Arc<dyn UiNode>>,
        cached: Mutex<Option<Vec<Arc<dyn UiNode>>>>,
        self_weak: OnceLock<Weak<dyn UiNode>>,
    }

    impl LazyNode {
        fn root(policy: Policy) -> Arc<dyn UiNode> {
            Self::build("root".into(), 0, policy, None, false)
        }

        fn build(
            runtime_id: String,
            depth: usize,
            policy: Policy,
            parent: Option<&Arc<dyn UiNode>>,
            keep_parent: bool,
        ) -> Arc<dyn UiNode> {
            let node = Arc::new(Self {
                runtime_id: RuntimeId::from(runtime_id),
                depth,
                policy,
                parent: parent.map(Arc::downgrade),
                _kept_parent: parent.filter(|_| keep_parent).cloned(),
                cached: Mutex::new(None),
                self_weak: OnceLock::new(),
            });
            let erased: Arc<dyn UiNode> = node.clone();
            let _ = node.self_weak.set(Arc::downgrade(&erased));
            erased
        }
    }

    impl UiNode for LazyNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &'static str {
            "Pane"
        }
        fn name(&self) -> String {
            String::new()
        }
        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }
        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            self.parent.clone()
        }
        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
            if self.depth >= 2 {
                return Box::new(std::iter::empty());
            }
            let mut cached = self.cached.lock().unwrap();
            if let Some(children) = cached.as_ref() {
                return Box::new(children.clone().into_iter());
            }
            let me = self.self_weak.get().and_then(Weak::upgrade).expect("a node is alive while it lists");
            let children: Vec<Arc<dyn UiNode>> = (0..3)
                .map(|index| {
                    let id = format!("{}/{index}", self.runtime_id.as_str());
                    let keep = self.policy.keep_parent && (index == 0 || !self.policy.first_child_only);
                    Self::build(id, self.depth + 1, self.policy, Some(&me), keep)
                })
                .collect();
            if self.policy.cache_children {
                *cached = Some(children.clone());
            }
            Box::new(children.into_iter())
        }
        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
            Box::new(std::iter::empty())
        }
        fn supported_patterns(&self) -> Vec<PatternName> {
            Vec::new()
        }
        /// Forgets the listed children, which breaks the cycles of a node
        /// that kept them.
        fn invalidate(&self) {
            self.cached.lock().unwrap().take();
        }
    }

    /// A node of a tree that owns its children and links to its parent only
    /// weakly, as the mock provider does.
    struct OwnedNode {
        runtime_id: RuntimeId,
        parent: Mutex<Option<Weak<dyn UiNode>>>,
        children: Mutex<Vec<Arc<dyn UiNode>>>,
    }

    impl OwnedNode {
        /// A root with two children, each with two children of its own.
        fn tree() -> Arc<dyn UiNode> {
            let node = |id: &str| {
                Arc::new(Self {
                    runtime_id: RuntimeId::from(id),
                    parent: Mutex::new(None),
                    children: Mutex::new(Vec::new()),
                })
            };
            let root = node("owned");
            let root_dyn: Arc<dyn UiNode> = root.clone();
            for index in 0..2 {
                let child = node(&format!("owned/{index}"));
                let child_dyn: Arc<dyn UiNode> = child.clone();
                *child.parent.lock().unwrap() = Some(Arc::downgrade(&root_dyn));
                for inner in 0..2 {
                    let grandchild = node(&format!("owned/{index}/{inner}"));
                    *grandchild.parent.lock().unwrap() = Some(Arc::downgrade(&child_dyn));
                    child.children.lock().unwrap().push(grandchild);
                }
                root.children.lock().unwrap().push(child_dyn);
            }
            root_dyn
        }
    }

    impl UiNode for OwnedNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &'static str {
            "Pane"
        }
        fn name(&self) -> String {
            String::new()
        }
        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }
        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            self.parent.lock().unwrap().clone()
        }
        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
            Box::new(self.children.lock().unwrap().clone().into_iter())
        }
        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
            Box::new(std::iter::empty())
        }
        fn supported_patterns(&self) -> Vec<PatternName> {
            Vec::new()
        }
        fn invalidate(&self) {}
    }

    fn cannot_prove(issues: &[ContractIssue]) -> bool {
        matches!(issues, [ContractIssue::Unprovable { .. }])
    }

    #[rstest]
    fn children_that_keep_only_a_weak_parent_are_reported() {
        let issues = verify_children_keep_parent(LazyNode::root(WEAK_ONLY), 10);

        let expected: Vec<ContractIssue> =
            (0..3).map(|index| ContractIssue::ChildParentUnreachable { child: format!("root/{index}") }).collect();
        assert_eq!(issues, expected);
    }

    /// One child that keeps the parent makes every sibling's parent reachable
    /// while they are listed together; each child must keep it on its own.
    #[rstest]
    fn children_that_keep_their_parent_only_through_a_sibling_are_reported() {
        let issues = verify_children_keep_parent(LazyNode::root(FIRST_CHILD_KEEPS_PARENT), 10);

        let expected: Vec<ContractIssue> =
            (1..3).map(|index| ContractIssue::ChildParentUnreachable { child: format!("root/{index}") }).collect();
        assert_eq!(issues, expected);
    }

    #[rstest]
    fn children_that_keep_their_parent_pass() {
        let issues = verify_children_keep_parent(LazyNode::root(KEEPS_PARENT), 10);

        assert!(issues.is_empty(), "expected no issues, got {issues:?}");
    }

    #[rstest]
    #[case::weak_only(WEAK_ONLY)]
    #[case::keeps_parent(KEEPS_PARENT)]
    fn a_parent_the_caller_still_holds_cannot_prove_the_rule(#[case] policy: Policy) {
        let parent = LazyNode::root(policy);
        let held_elsewhere = Arc::clone(&parent);

        let issues = verify_children_keep_parent(parent, 10);

        assert!(cannot_prove(&issues), "expected the check to say it cannot prove the rule, got {issues:?}");
        drop(held_elsewhere);
    }

    #[rstest]
    fn a_node_without_children_cannot_prove_the_rule() {
        let root = LazyNode::root(KEEPS_PARENT);
        let leaf = root.children().next().expect("child").children().next().expect("grandchild");

        let issues = verify_children_keep_parent(leaf, 10);

        assert!(cannot_prove(&issues), "expected the check to say it cannot prove the rule, got {issues:?}");
    }

    /// A tree that owns its children keeps every parent reachable through that
    /// ownership, so no child is reported; but the check cannot attribute that
    /// to the children, and says so.
    #[rstest]
    fn an_owned_tree_keeps_its_parents_but_cannot_prove_the_rule() {
        let tree = OwnedNode::tree();
        let inner = tree.children().next().expect("inner node");

        let issues = verify_children_keep_parent(inner, 10);

        assert!(cannot_prove(&issues), "expected the check to say it cannot prove the rule, got {issues:?}");
    }

    #[rstest]
    #[case::weak_only(WEAK_ONLY)]
    #[case::keeps_parent(KEEPS_PARENT)]
    fn nodes_that_do_not_hold_their_children_are_released(#[case] policy: Policy) {
        let issues = verify_subtree_released(LazyNode::root(policy), 100);

        assert!(issues.is_empty(), "expected no issues, got {issues:?}");
    }

    #[rstest]
    fn a_root_held_elsewhere_still_releases_its_subtree() {
        let root = LazyNode::root(KEEPS_PARENT);

        let issues = verify_subtree_released(Arc::clone(&root), 100);

        assert!(issues.is_empty(), "expected no issues, got {issues:?}");
    }

    #[rstest]
    fn nodes_that_hold_their_children_are_reported() {
        let root = LazyNode::root(KEEPS_PARENT_AND_CHILDREN);
        let root_alive = Arc::downgrade(&root);

        let issues = verify_subtree_released(root, 100);

        assert_eq!(issues.len(), 12, "every listed node should be reported, got {issues:?}");
        assert!(issues.iter().all(|issue| matches!(issue, ContractIssue::NodeNotReleased { .. })));

        // Break the cycles the check found, so that the test leaks nothing.
        let mut all = vec![root_alive.upgrade().expect("the cycle keeps the root alive")];
        let mut index = 0;
        while index < all.len() {
            let children: Vec<Arc<dyn UiNode>> = all[index].children().collect();
            all.extend(children);
            index += 1;
        }
        for node in &all {
            node.invalidate();
        }
        drop(all);
        assert_eq!(root_alive.strong_count(), 0, "the cleanup must release the tree");
    }

    /// The mock provider owns its tree, so it fails this check by design and
    /// is exempt from it.
    #[rstest]
    fn an_owned_tree_is_not_released() {
        let tree = OwnedNode::tree();
        let inner = tree.children().next().expect("inner node");

        let issues = verify_subtree_released(inner, 100);

        assert_eq!(
            issues,
            vec![
                ContractIssue::NodeNotReleased { node: "owned/0/0".into() },
                ContractIssue::NodeNotReleased { node: "owned/0/1".into() },
            ]
        );
    }
}

#[cfg(test)]
mod order_key_tests {
    use super::*;
    use crate::ui::RuntimeId;
    use rstest::rstest;
    use std::sync::Weak;

    /// A node with a fixed key and children, owned by its parent.
    struct KeyedNode {
        runtime_id: RuntimeId,
        key: Option<u64>,
        children: Vec<Arc<dyn UiNode>>,
    }

    fn node(id: &str, key: Option<u64>, children: Vec<Arc<dyn UiNode>>) -> Arc<dyn UiNode> {
        Arc::new(KeyedNode { runtime_id: RuntimeId::from(id), key, children })
    }

    impl UiNode for KeyedNode {
        fn namespace(&self) -> Namespace {
            Namespace::Control
        }
        fn role(&self) -> &'static str {
            "Pane"
        }
        fn name(&self) -> String {
            String::new()
        }
        fn runtime_id(&self) -> &RuntimeId {
            &self.runtime_id
        }
        fn parent(&self) -> Option<Weak<dyn UiNode>> {
            None
        }
        fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
            Box::new(self.children.clone().into_iter())
        }
        fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
            Box::new(std::iter::empty())
        }
        fn supported_patterns(&self) -> Vec<PatternName> {
            Vec::new()
        }
        fn doc_order_key(&self) -> Option<u64> {
            self.key
        }
        fn invalidate(&self) {}
    }

    fn ids(nodes: &[Arc<dyn UiNode>]) -> Vec<&str> {
        nodes.iter().map(|node| node.runtime_id().as_str()).collect()
    }

    #[rstest]
    fn nodes_are_listed_in_document_order() {
        let roots = vec![
            node("a", None, vec![node("a/0", None, vec![node("a/0/0", None, vec![])]), node("a/1", None, vec![])]),
            node("b", None, vec![]),
        ];

        let ordered = nodes_in_document_order(roots.clone(), 100);
        assert_eq!(ids(&ordered), ["a", "a/0", "a/0/0", "a/1", "b"]);
        assert_eq!(ids(&nodes_in_document_order(roots, 2)), ["a", "a/0"]);
    }

    #[rstest]
    fn keys_in_document_order_pass_and_nodes_without_keys_are_skipped() {
        let roots = vec![
            node("a", Some(0), vec![node("a/0", None, vec![]), node("a/1", Some(5), vec![])]),
            node("b", Some(6), vec![]),
        ];

        let issues = verify_doc_order_keys(&nodes_in_document_order(roots, 100));

        assert!(issues.is_empty(), "expected no issues, got {issues:?}");
    }

    /// Process ids as keys: a node listed later can have the smaller one.
    #[rstest]
    fn keys_against_document_order_are_reported() {
        let listing = vec![node("app-1200", Some(1200), vec![]), node("app-800", Some(800), vec![])];

        let issues = verify_doc_order_keys(&listing);

        assert_eq!(
            issues,
            vec![ContractIssue::OrderKeyOutOfDocumentOrder {
                node: "app-800".into(),
                key: 800,
                preceding: "app-1200".into(),
                preceding_key: 1200,
            }]
        );
    }

    #[rstest]
    fn a_repeated_key_is_reported() {
        let listing = vec![node("a", Some(3), vec![]), node("b", Some(3), vec![])];

        let issues = verify_doc_order_keys(&listing);

        assert!(matches!(issues.as_slice(), [ContractIssue::OrderKeyOutOfDocumentOrder { node, .. }] if node == "b"));
    }
}
