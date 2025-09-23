use crate::factory::{self, MockProviderFactory};
use crate::provider::{self, MockProvider};
use crate::tree::{NodeSpec, StaticMockTree, install_mock_tree, reset_mock_tree};
use platynui_core::provider::{UiTreeProvider, provider_factories};
use platynui_core::types::Point;
use platynui_core::ui::attribute_names::{activation_target, element, text_content};
use platynui_core::ui::contract::testkit::{
    AttributeExpectation, NodeExpectation, PatternExpectation, require_node, verify_node,
};
use platynui_core::ui::{Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiValue};
use rstest::rstest;
use serial_test::serial;
use std::sync::{Arc, LazyLock, Weak};

const ELEMENT_EXPECTATIONS: [AttributeExpectation; 3] = [
    AttributeExpectation::required(Namespace::Control, element::BOUNDS),
    AttributeExpectation::required(Namespace::Control, element::IS_VISIBLE),
    AttributeExpectation::required(Namespace::Control, element::IS_ENABLED),
];

const TEXT_CONTENT_EXPECTATIONS: [AttributeExpectation; 1] =
    [AttributeExpectation::required(Namespace::Control, text_content::TEXT)];

const ACTIVATION_TARGET_EXPECTATIONS: [AttributeExpectation; 1] =
    [AttributeExpectation::required(Namespace::Control, activation_target::ACTIVATION_POINT)];

fn mock_provider() -> Arc<dyn UiTreeProvider> {
    reset_mock_tree();
    provider::instantiate_registered_provider()
}

fn find_by_runtime_id(node: Arc<dyn UiNode>, target: &str) -> Option<Arc<dyn UiNode>> {
    if node.runtime_id().as_str() == target {
        return Some(node);
    }
    for child in node.children() {
        if let Some(found) = find_by_runtime_id(child, target) {
            return Some(found);
        }
    }
    None
}

struct DesktopNode;

impl UiNode for DesktopNode {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }

    fn role(&self) -> &str {
        "Desktop"
    }

    fn name(&self) -> &str {
        "Desktop"
    }

    fn runtime_id(&self) -> &RuntimeId {
        static ID: LazyLock<RuntimeId> = LazyLock::new(|| RuntimeId::from("mock://desktop"));
        &ID
    }

    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        None
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_> {
        Box::new(std::iter::empty())
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_> {
        Box::new(std::iter::empty())
    }

    fn supported_patterns(&self) -> &[PatternId] {
        &[]
    }

    fn invalidate(&self) {}
}

#[rstest]
fn provider_registration_present() {
    let ids: Vec<_> = provider_factories().map(|factory| factory.descriptor().id).collect();
    assert!(ids.contains(&factory::PROVIDER_ID));
}

#[rstest]
#[serial]
fn custom_tree_overrides_defaults() {
    let tree = StaticMockTree::new(vec![
        NodeSpec::new(Namespace::App, "Application", "Custom App", "mock://app/custom")
            .with_pattern("Application")
            .with_child(
                NodeSpec::new(
                    Namespace::Control,
                    "Window",
                    "Custom Window",
                    "mock://window/custom",
                )
                .with_pattern("Element")
                .with_child(
                    NodeSpec::new(Namespace::Control, "Button", "Launch", "mock://button/custom")
                        .with_pattern("Element"),
                ),
            ),
    ]);

    let guard = install_mock_tree(tree);
    let provider = MockProvider::new(MockProviderFactory::descriptor_static());
    drop(guard);
    let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
    let mut roots =
        provider.get_nodes(Arc::clone(&desktop)).expect("custom tree root").collect::<Vec<_>>();

    assert_eq!(roots.len(), 1);
    let app = roots.pop().unwrap();
    assert_eq!(app.runtime_id().as_str(), "mock://app/custom");
}

#[rstest]
#[serial]
fn root_application_is_returned_with_parent() {
    let provider = mock_provider();
    let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
    let mut roots = provider.get_nodes(Arc::clone(&desktop)).unwrap().collect::<Vec<_>>();
    assert!(!roots.is_empty());
    let app = roots.remove(0);
    assert_eq!(app.namespace(), Namespace::App);
    assert_eq!(app.runtime_id().as_str(), factory::APP_RUNTIME_ID);
    let parent = app.parent().and_then(|weak| weak.upgrade()).expect("desktop parent");
    assert_eq!(parent.runtime_id().as_str(), desktop.runtime_id().as_str());
}

#[rstest]
#[serial]
fn contract_expectations_for_button_hold() {
    let provider = mock_provider();
    let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
    let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
    let mut windows = provider.get_nodes(Arc::clone(&app)).unwrap();
    let window = windows
        .find(|node| node.runtime_id().as_str() == factory::WINDOW_RUNTIME_ID)
        .expect("main window present");

    let button = find_by_runtime_id(window, factory::BUTTON_RUNTIME_ID)
        .expect("button reachable in mock tree");

    let expectations = NodeExpectation::default()
        .with_pattern(PatternExpectation::new(PatternId::from("Element"), &ELEMENT_EXPECTATIONS))
        .with_pattern(PatternExpectation::new(
            PatternId::from("TextContent"),
            &TEXT_CONTENT_EXPECTATIONS,
        ))
        .with_pattern(PatternExpectation::new(
            PatternId::from("ActivationTarget"),
            &ACTIVATION_TARGET_EXPECTATIONS,
        ));

    require_node(button.as_ref(), &expectations).expect("button contract satisfied");
    let issues = verify_node(button.as_ref(), &expectations);
    assert!(issues.is_empty(), "contract issues: {issues:?}");
}

#[rstest]
#[serial]
fn rect_aliases_present() {
    let provider = mock_provider();
    let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
    let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
    let mut windows = provider.get_nodes(Arc::clone(&app)).unwrap();
    let window = windows
        .find(|node| node.runtime_id().as_str() == factory::WINDOW_RUNTIME_ID)
        .expect("main window present");

    let mut alias_names =
        window.attributes().map(|attr| attr.name().to_owned()).collect::<Vec<_>>();
    alias_names.sort();
    assert!(alias_names.contains(&"Bounds.X".to_owned()));
    assert!(alias_names.contains(&"Bounds.Y".to_owned()));
    assert!(alias_names.contains(&"Bounds.Width".to_owned()));
    assert!(alias_names.contains(&"Bounds.Height".to_owned()));
}

#[rstest]
#[serial]
fn activation_point_aliases_present() {
    let provider = mock_provider();
    let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
    let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
    let mut windows = provider.get_nodes(Arc::clone(&app)).unwrap();
    let window = windows
        .find(|node| node.runtime_id().as_str() == factory::WINDOW_RUNTIME_ID)
        .expect("main window present");
    let button =
        find_by_runtime_id(window, factory::BUTTON_RUNTIME_ID).expect("mock ok button present");

    let activation_point = button
        .attributes()
        .find(|attr| attr.name() == activation_target::ACTIVATION_POINT)
        .expect("activation point exists")
        .value();

    assert!(matches!(activation_point, UiValue::Point(Point { .. })));

    let aliases: Vec<_> = button
        .attributes()
        .filter(|attr| attr.name().starts_with("ActivationPoint."))
        .map(|attr| attr.name().to_owned())
        .collect();
    assert!(aliases.contains(&"ActivationPoint.X".to_owned()));
    assert!(aliases.contains(&"ActivationPoint.Y".to_owned()));
}
