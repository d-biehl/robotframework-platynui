use crate::factory::{self, MockProviderFactory};
use crate::provider::{self, MockProvider};
use crate::tree::{NodeSpec, StaticMockTree, install_mock_tree, reset_mock_tree};
use platynui_core::provider::{UiTreeProvider, provider_factories};
use platynui_core::types::{Point, Size};
use platynui_core::ui::attribute_names::{activation_target, element, focusable, text_content, window_surface};
use platynui_core::ui::contract::testkit::{
    AttributeExpectation, NodeExpectation, PatternExpectation, require_node, verify_node,
};
use platynui_core::ui::{
    FocusableAction, FocusablePattern, Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiValue,
    WindowSurfaceActions, WindowSurfacePattern,
};
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

const FOCUSABLE_EXPECTATIONS: [AttributeExpectation; 1] =
    [AttributeExpectation::required(Namespace::Control, focusable::IS_FOCUSED)];

const CANCEL_RUNTIME_ID: &str = "mock://button/cancel";

fn mock_provider() -> Arc<dyn UiTreeProvider> {
    reset_mock_tree();
    provider::instantiate_test_provider()
}

fn attr_bool(node: &Arc<dyn UiNode>, namespace: Namespace, name: &str) -> bool {
    node.attribute(namespace, name)
        .map(|attr| attr.value())
        .and_then(|value| match value {
            UiValue::Bool(v) => Some(v),
            UiValue::Integer(v) => Some(v != 0),
            UiValue::Number(v) => Some(v != 0.0),
            _ => None,
        })
        .unwrap_or(false)
}

fn attr_rect(node: &Arc<dyn UiNode>, namespace: Namespace, name: &str) -> Option<platynui_core::types::Rect> {
    node.attribute(namespace, name).map(|attr| attr.value()).and_then(|value| match value {
        UiValue::Rect(r) => Some(r),
        _ => None,
    })
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

    fn name(&self) -> String {
        "Desktop".to_string()
    }

    fn runtime_id(&self) -> &RuntimeId {
        static ID: LazyLock<RuntimeId> = LazyLock::new(|| RuntimeId::from("mock://desktop"));
        &ID
    }

    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        None
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        Box::new(std::iter::empty())
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        Box::new(std::iter::empty())
    }

    fn supported_patterns(&self) -> Vec<PatternId> {
        Vec::new()
    }

    fn invalidate(&self) {}
}

#[rstest]
fn provider_not_auto_registered() {
    // Mock providers should NOT be auto-registered; they're only available via explicit handles
    let ids: Vec<_> = provider_factories().map(|factory| factory.descriptor().id).collect();
    assert!(!ids.contains(&factory::PROVIDER_ID), "Mock provider should not be auto-registered, found in: {:?}", ids);
}

#[rstest]
#[serial]
fn custom_tree_overrides_defaults() {
    let tree = StaticMockTree::new(vec![
        NodeSpec::new(Namespace::App, "Application", "Custom App", "mock://app/custom")
            .with_pattern("Application")
            .with_child(
                NodeSpec::new(Namespace::Control, "Window", "Custom Window", "mock://window/custom")
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
    let mut roots = provider.get_nodes(Arc::clone(&desktop)).expect("custom tree root").collect::<Vec<_>>();

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
    let window =
        windows.find(|node| node.runtime_id().as_str() == factory::WINDOW_RUNTIME_ID).expect("main window present");

    let button = find_by_runtime_id(window, factory::BUTTON_RUNTIME_ID).expect("button reachable in mock tree");

    let expectations = NodeExpectation::default()
        .with_pattern(PatternExpectation::new(PatternId::from("Element"), &ELEMENT_EXPECTATIONS))
        .with_pattern(PatternExpectation::new(PatternId::from("TextContent"), &TEXT_CONTENT_EXPECTATIONS))
        .with_pattern(PatternExpectation::new(PatternId::from("ActivationTarget"), &ACTIVATION_TARGET_EXPECTATIONS))
        .with_pattern(PatternExpectation::new(PatternId::from("Focusable"), &FOCUSABLE_EXPECTATIONS));

    require_node(button.as_ref(), &expectations).expect("button contract satisfied");
    let issues = verify_node(button.as_ref(), &expectations);
    assert!(issues.is_empty(), "contract issues: {issues:?}");
}

#[rstest]
#[serial]
fn rect_aliases_absent_and_base_present() {
    let provider = mock_provider();
    let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
    let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
    let mut windows = provider.get_nodes(Arc::clone(&app)).unwrap();
    let window =
        windows.find(|node| node.runtime_id().as_str() == factory::WINDOW_RUNTIME_ID).expect("main window present");

    let mut names = window.attributes().map(|attr| attr.name().to_owned()).collect::<Vec<_>>();
    names.sort();
    // Derived aliases must NOT be provided by the provider anymore.
    assert!(!names.contains(&"Bounds.X".to_owned()));
    assert!(!names.contains(&"Bounds.Y".to_owned()));
    assert!(!names.contains(&"Bounds.Width".to_owned()));
    assert!(!names.contains(&"Bounds.Height".to_owned()));
    // The base attribute remains present.
    assert!(names.contains(&element::BOUNDS.to_owned()));
}

#[rstest]
#[serial]
fn window_surface_pattern_is_exposed() {
    let provider = mock_provider();
    let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
    let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
    let mut windows = provider.get_nodes(Arc::clone(&app)).unwrap();
    let window =
        windows.find(|node| node.runtime_id().as_str() == factory::WINDOW_RUNTIME_ID).expect("main window present");

    let patterns = window.supported_patterns();
    assert!(patterns.contains(&PatternId::from("WindowSurface")));
    assert!(patterns.contains(&PatternId::from("Focusable")));

    let window_surface = window.pattern::<WindowSurfaceActions>().expect("window surface pattern registered");
    assert!(window_surface.accepts_user_input().unwrap().is_some());

    let focus_pattern = window.pattern::<FocusableAction>().expect("window focusable pattern available");
    focus_pattern.focus().expect("focusing window succeeds");
    let focused_attr = window.attribute(Namespace::Control, focusable::IS_FOCUSED).expect("focus attribute present");
    assert_eq!(focused_attr.value(), UiValue::from(true));
}

#[rstest]
#[serial]
fn window_surface_actions_update_state() {
    let provider = mock_provider();
    let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
    let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
    let mut windows = provider.get_nodes(Arc::clone(&app)).unwrap();
    let window =
        windows.find(|node| node.runtime_id().as_str() == factory::WINDOW_RUNTIME_ID).expect("main window present");

    let pattern = window.pattern::<WindowSurfaceActions>().expect("window surface pattern registered");

    pattern.activate().expect("activate succeeds");
    assert!(attr_bool(&window, Namespace::Control, focusable::IS_FOCUSED));

    assert!(!attr_bool(&window, Namespace::Control, window_surface::IS_MINIMIZED));
    assert!(attr_bool(&window, Namespace::Control, window_surface::SUPPORTS_MOVE));
    assert!(attr_bool(&window, Namespace::Control, window_surface::SUPPORTS_RESIZE));
    assert_eq!(pattern.accepts_user_input().unwrap(), Some(true));

    pattern.minimize().expect("minimize succeeds");
    assert!(attr_bool(&window, Namespace::Control, window_surface::IS_MINIMIZED));
    assert_eq!(pattern.accepts_user_input().unwrap(), Some(false));
    assert!(!attr_bool(&window, Namespace::Control, focusable::IS_FOCUSED));

    pattern.restore().expect("restore succeeds");
    assert!(!attr_bool(&window, Namespace::Control, window_surface::IS_MINIMIZED));
    assert_eq!(pattern.accepts_user_input().unwrap(), Some(true));
    assert!(attr_bool(&window, Namespace::Control, focusable::IS_FOCUSED));

    pattern.maximize().expect("maximize succeeds");
    assert!(attr_bool(&window, Namespace::Control, window_surface::IS_MAXIMIZED));

    pattern.move_to(Point::new(240.0, 260.0)).expect("move succeeds");
    let r = attr_rect(&window, Namespace::Control, element::BOUNDS).expect("bounds present");
    assert_eq!(r.x(), 240.0);
    assert_eq!(r.y(), 260.0);

    pattern.resize(Size::new(820.0, 610.0)).expect("resize succeeds");
    let r = attr_rect(&window, Namespace::Control, element::BOUNDS).expect("bounds present");
    assert_eq!(r.width(), 820.0);
    assert_eq!(r.height(), 610.0);

    pattern.close().expect("close succeeds");
    assert!(!attr_bool(&window, Namespace::Control, focusable::IS_FOCUSED));
}

#[rstest]
#[serial]
fn activation_point_aliases_absent_and_value_ok() {
    let provider = mock_provider();
    let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
    let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
    let mut windows = provider.get_nodes(Arc::clone(&app)).unwrap();
    let window =
        windows.find(|node| node.runtime_id().as_str() == factory::WINDOW_RUNTIME_ID).expect("main window present");
    let button = find_by_runtime_id(window, factory::BUTTON_RUNTIME_ID).expect("mock ok button present");

    let activation_point = button
        .attributes()
        .find(|attr| attr.name() == activation_target::ACTIVATION_POINT)
        .expect("activation point exists")
        .value();

    assert!(matches!(activation_point, UiValue::Point(Point { .. })));

    // Provider no longer returns aliases, only the base value
    let has_alias = button.attributes().any(|attr| attr.name().starts_with("ActivationPoint."));
    assert!(!has_alias);
    // Base value matches the coordinates set in the XML (OK button: 200,636)
    match activation_point {
        UiValue::Point(p) => {
            assert_eq!(p.x(), 200.0);
            assert_eq!(p.y(), 636.0);
        }
        other => panic!("expected ActivationPoint as Point, got {:?}", other),
    }
}

#[rstest]
#[serial]
fn focusable_pattern_switches_focus() {
    let provider = mock_provider();
    let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
    let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
    let mut windows = provider.get_nodes(Arc::clone(&app)).unwrap();
    let window =
        windows.find(|node| node.runtime_id().as_str() == factory::WINDOW_RUNTIME_ID).expect("main window present");

    let button = find_by_runtime_id(window.clone(), factory::BUTTON_RUNTIME_ID).expect("ok button present");
    let cancel = find_by_runtime_id(window, CANCEL_RUNTIME_ID).expect("cancel button present");

    let focus_attr = button.attribute(Namespace::Control, focusable::IS_FOCUSED).expect("focus attribute present");
    assert_eq!(focus_attr.value(), UiValue::from(true));

    let focusable_action = button.pattern::<FocusableAction>().expect("focusable pattern available on ok button");
    focusable_action.focus().expect("focus action succeeds");

    let focused_value =
        button.attribute(Namespace::Control, focusable::IS_FOCUSED).expect("focus attribute after focus").value();
    assert_eq!(focused_value, UiValue::from(true));

    let cancel_action = cancel.pattern::<FocusableAction>().expect("focusable pattern available on cancel button");
    cancel_action.focus().expect("focus action on cancel succeeds");

    let button_focus = button
        .attribute(Namespace::Control, focusable::IS_FOCUSED)
        .expect("ok button attribute after cancel focus")
        .value();
    let cancel_focus =
        cancel.attribute(Namespace::Control, focusable::IS_FOCUSED).expect("cancel attribute after focus").value();

    assert_eq!(button_focus, UiValue::from(false));
    assert_eq!(cancel_focus, UiValue::from(true));
}
