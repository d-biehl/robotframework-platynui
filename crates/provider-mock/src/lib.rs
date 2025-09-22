//! Deterministic mock UiTree provider for testing the runtime and CLI wiring.
//!
//! The provider surfaces a tiny, static tree consisting of a single application,
//! one window, and a button. It focuses on delivering predictable metadata and
//! stable `RuntimeId`s so higher layers can exercise enumeration, formatting,
//! and contract checks without native platform dependencies.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex, Weak};

use platynui_core::provider::{
    ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory,
};
use platynui_core::register_provider;
use platynui_core::types::{Point, Rect};
use platynui_core::ui::attribute_names::{activation_target, common, element, text_content};
use platynui_core::ui::{
    Namespace, PatternId, PatternRegistry, RuntimeId, TechnologyId, UiAttribute, UiNode, UiPattern,
    UiValue, supported_patterns_value,
};

#[cfg(test)]
use platynui_core::provider::provider_factories;

static MOCK_PROVIDER_FACTORY: MockProviderFactory = MockProviderFactory;

register_provider!(&MOCK_PROVIDER_FACTORY);

const PROVIDER_ID: &str = "mock";
const PROVIDER_NAME: &str = "PlatynUI Mock Provider";
const TECHNOLOGY: &str = "Mock";
const APP_RUNTIME_ID: &str = "mock://app/main";
const WINDOW_RUNTIME_ID: &str = "mock://window/main";
const BUTTON_RUNTIME_ID: &str = "mock://button/ok";

struct MockProviderFactory;

impl MockProviderFactory {
    fn descriptor_static() -> &'static ProviderDescriptor {
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
            ProviderDescriptor::new(
                PROVIDER_ID,
                PROVIDER_NAME,
                TechnologyId::from(TECHNOLOGY),
                ProviderKind::Native,
            )
        });
        &DESCRIPTOR
    }
}

impl UiTreeProviderFactory for MockProviderFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        Self::descriptor_static()
    }

    fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        let provider: Arc<MockProvider> = Arc::new(MockProvider::new(Self::descriptor_static()));
        Ok(provider)
    }
}

struct MockProvider {
    descriptor: &'static ProviderDescriptor,
    roots: Vec<Arc<MockNode>>,
    nodes: HashMap<String, Arc<MockNode>>,
}

impl MockProvider {
    fn new(descriptor: &'static ProviderDescriptor) -> Self {
        let app = MockNode::application(APP_RUNTIME_ID, "Mock Application", descriptor);
        let window = MockNode::window(WINDOW_RUNTIME_ID, "Main Window", descriptor);
        let button = MockNode::button(BUTTON_RUNTIME_ID, "OK", descriptor);

        MockNode::add_child(&window, Arc::clone(&button));
        MockNode::add_child(&app, Arc::clone(&window));

        let roots = vec![Arc::clone(&app)];
        let nodes = flatten_nodes(&roots)
            .into_iter()
            .map(|node| (node.runtime_id().as_str().to_owned(), node))
            .collect();

        Self { descriptor, roots, nodes }
    }

    fn children_for_parent(&self, parent: &Arc<dyn UiNode>) -> Vec<Arc<MockNode>> {
        if let Some(node) = self.nodes.get(parent.runtime_id().as_str()) {
            node.children_snapshot()
        } else if parent.namespace() == Namespace::Control && parent.role() == "Desktop" {
            self.roots.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }
}

impl UiTreeProvider for MockProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
    }

    fn get_nodes(
        &self,
        parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
        let children = self.children_for_parent(&parent);

        for child in &children {
            child.set_parent(&parent);
        }

        Ok(Box::new(children.into_iter().map(|child| -> Arc<dyn UiNode> { child })))
    }
}

struct MockNode {
    namespace: Namespace,
    role: &'static str,
    name: String,
    runtime_id: RuntimeId,
    attributes: Vec<Arc<dyn UiAttribute>>,
    runtime_patterns: PatternRegistry,
    supported_patterns: Vec<PatternId>,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    children: Mutex<Vec<Arc<MockNode>>>,
}

impl MockNode {
    fn application(id: &str, name: &str, descriptor: &ProviderDescriptor) -> Arc<Self> {
        Self::new(
            Namespace::App,
            "Application",
            name,
            id,
            descriptor,
            Vec::new(),
            PatternRegistry::new(),
            vec![PatternId::from("Application")],
        )
    }

    fn window(id: &str, name: &str, descriptor: &ProviderDescriptor) -> Arc<Self> {
        let bounds = Rect::new(100.0, 100.0, 800.0, 600.0);
        let mut attributes = Vec::new();
        attributes.push(attr(Namespace::Control, element::BOUNDS, UiValue::from(bounds)));
        attributes.extend(rect_aliases(Namespace::Control, "Bounds", &bounds));
        attributes.push(attr(Namespace::Control, element::IS_VISIBLE, UiValue::from(true)));
        attributes.push(attr(Namespace::Control, element::IS_ENABLED, UiValue::from(true)));

        Self::new(
            Namespace::Control,
            "Window",
            name,
            id,
            descriptor,
            attributes,
            PatternRegistry::new(),
            vec![PatternId::from("Element")],
        )
    }

    fn button(id: &str, name: &str, descriptor: &ProviderDescriptor) -> Arc<Self> {
        let bounds = Rect::new(140.0, 620.0, 120.0, 32.0);
        let activation = bounds.center();
        let mut attributes = Vec::new();
        attributes.push(attr(Namespace::Control, element::BOUNDS, UiValue::from(bounds)));
        attributes.extend(rect_aliases(Namespace::Control, "Bounds", &bounds));
        attributes.push(attr(Namespace::Control, element::IS_VISIBLE, UiValue::from(true)));
        attributes.push(attr(Namespace::Control, element::IS_ENABLED, UiValue::from(true)));
        attributes.push(attr(
            Namespace::Control,
            activation_target::ACTIVATION_POINT,
            UiValue::from(activation),
        ));
        attributes.extend(point_aliases(Namespace::Control, "ActivationPoint", &activation));
        attributes.push(attr(
            Namespace::Control,
            text_content::TEXT,
            UiValue::from(name.to_owned()),
        ));

        Self::new(
            Namespace::Control,
            "Button",
            name,
            id,
            descriptor,
            attributes,
            PatternRegistry::new(),
            vec![
                PatternId::from("Element"),
                PatternId::from("TextContent"),
                PatternId::from("ActivationTarget"),
            ],
        )
    }

    fn new(
        namespace: Namespace,
        role: &'static str,
        name: &str,
        runtime_id: &str,
        descriptor: &ProviderDescriptor,
        mut additional_attributes: Vec<Arc<dyn UiAttribute>>,
        runtime_patterns: PatternRegistry,
        supported_patterns: Vec<PatternId>,
    ) -> Arc<Self> {
        let technology = descriptor.technology.as_str().to_owned();
        let runtime_id = RuntimeId::from(runtime_id);
        let mut attributes = Vec::new();
        attributes.push(attr(namespace, common::ROLE, UiValue::from(role)));
        attributes.push(attr(namespace, common::NAME, UiValue::from(name.to_owned())));
        attributes.push(attr(
            namespace,
            common::RUNTIME_ID,
            UiValue::from(runtime_id.as_str().to_owned()),
        ));
        attributes.push(attr(namespace, common::TECHNOLOGY, UiValue::from(technology)));
        attributes.push(attr(
            namespace,
            common::SUPPORTED_PATTERNS,
            supported_patterns_value(&supported_patterns),
        ));
        attributes.append(&mut additional_attributes);

        Arc::new(Self {
            namespace,
            role,
            name: name.to_owned(),
            runtime_id,
            attributes,
            runtime_patterns,
            supported_patterns,
            parent: Mutex::new(None),
            children: Mutex::new(Vec::new()),
        })
    }

    fn children_snapshot(&self) -> Vec<Arc<MockNode>> {
        self.children.lock().unwrap().clone()
    }

    fn set_parent(&self, parent: &Arc<dyn UiNode>) {
        *self.parent.lock().unwrap() = Some(Arc::downgrade(parent));
    }

    fn add_child(parent: &Arc<Self>, child: Arc<Self>) {
        let parent_clone = Arc::clone(parent);
        let parent_trait: Arc<dyn UiNode> = parent_clone;
        child.set_parent(&parent_trait);
        parent.children.lock().unwrap().push(child);
    }
}

impl UiNode for MockNode {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn role(&self) -> &str {
        self.role
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn runtime_id(&self) -> &RuntimeId {
        &self.runtime_id
    }

    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent.lock().unwrap().clone()
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + '_> {
        let snapshot = self.children.lock().unwrap().clone();
        Box::new(snapshot.into_iter().map(|child| -> Arc<dyn UiNode> { child }))
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + '_> {
        Box::new(self.attributes.clone().into_iter())
    }

    fn supported_patterns(&self) -> &[PatternId] {
        &self.supported_patterns
    }

    fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
        self.runtime_patterns.get(pattern)
    }

    fn invalidate(&self) {}
}

#[derive(Clone)]
struct StaticAttribute {
    namespace: Namespace,
    name: String,
    value: UiValue,
}

impl UiAttribute for StaticAttribute {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn value(&self) -> UiValue {
        self.value.clone()
    }
}

fn attr(namespace: Namespace, name: impl Into<String>, value: UiValue) -> Arc<dyn UiAttribute> {
    Arc::new(StaticAttribute { namespace, name: name.into(), value })
}

fn rect_aliases(namespace: Namespace, base: &str, rect: &Rect) -> Vec<Arc<dyn UiAttribute>> {
    vec![
        attr(namespace, format!("{base}.X"), UiValue::from(rect.x())),
        attr(namespace, format!("{base}.Y"), UiValue::from(rect.y())),
        attr(namespace, format!("{base}.Width"), UiValue::from(rect.width())),
        attr(namespace, format!("{base}.Height"), UiValue::from(rect.height())),
    ]
}

fn point_aliases(namespace: Namespace, base: &str, point: &Point) -> Vec<Arc<dyn UiAttribute>> {
    vec![
        attr(namespace, format!("{base}.X"), UiValue::from(point.x())),
        attr(namespace, format!("{base}.Y"), UiValue::from(point.y())),
    ]
}

fn flatten_nodes(roots: &[Arc<MockNode>]) -> Vec<Arc<MockNode>> {
    let mut result = Vec::new();
    let mut queue: VecDeque<Arc<MockNode>> = roots.iter().cloned().collect();
    while let Some(node) = queue.pop_front() {
        for child in node.children_snapshot() {
            queue.push_back(child.clone());
        }
        result.push(node);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::types::Point;
    use platynui_core::ui::attribute_names::{activation_target, element, text_content};
    use platynui_core::ui::contract::testkit::{
        AttributeExpectation, NodeExpectation, PatternExpectation, require_node, verify_node,
    };
    use rstest::rstest;

    const ELEMENT_EXPECTATIONS: [AttributeExpectation; 3] = [
        AttributeExpectation::required(Namespace::Control, element::BOUNDS),
        AttributeExpectation::required(Namespace::Control, element::IS_VISIBLE),
        AttributeExpectation::required(Namespace::Control, element::IS_ENABLED),
    ];

    const TEXT_CONTENT_EXPECTATIONS: [AttributeExpectation; 1] =
        [AttributeExpectation::required(Namespace::Control, text_content::TEXT)];

    const ACTIVATION_TARGET_EXPECTATIONS: [AttributeExpectation; 1] =
        [AttributeExpectation::required(Namespace::Control, activation_target::ACTIVATION_POINT)];

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

    fn mock_provider() -> Arc<dyn UiTreeProvider> {
        provider_factories()
            .find(|factory| factory.descriptor().id == PROVIDER_ID)
            .expect("mock provider registered")
            .create()
            .expect("mock provider instantiation")
    }

    #[rstest]
    fn provider_registration_present() {
        let ids: Vec<_> = provider_factories().map(|factory| factory.descriptor().id).collect();
        assert!(ids.contains(&PROVIDER_ID));
    }

    #[rstest]
    fn root_application_is_returned_with_parent() {
        let provider = mock_provider();
        let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
        let mut roots =
            provider.get_nodes(Arc::clone(&desktop)).expect("root enumeration").collect::<Vec<_>>();
        assert_eq!(roots.len(), 1);
        let app = roots.pop().unwrap();
        assert_eq!(app.namespace(), Namespace::App);
        assert_eq!(app.runtime_id().as_str(), APP_RUNTIME_ID);
        let parent = app.parent().and_then(|weak| weak.upgrade()).expect("desktop parent");
        assert_eq!(parent.runtime_id().as_str(), desktop.runtime_id().as_str());
    }

    #[rstest]
    fn contract_expectations_for_button_hold() {
        let provider = mock_provider();
        let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
        let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
        let window = provider.get_nodes(Arc::clone(&app)).unwrap().next().unwrap();
        let button = provider.get_nodes(window).unwrap().next().unwrap();

        let expectations = NodeExpectation::default()
            .with_pattern(PatternExpectation::new(
                PatternId::from("Element"),
                &ELEMENT_EXPECTATIONS,
            ))
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
    fn rect_aliases_present() {
        let provider = mock_provider();
        let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
        let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
        let window = provider.get_nodes(Arc::clone(&app)).unwrap().next().unwrap();

        let mut alias_names =
            window.attributes().map(|attr| attr.name().to_owned()).collect::<Vec<_>>();
        alias_names.sort();
        assert!(alias_names.contains(&"Bounds.X".to_owned()));
        assert!(alias_names.contains(&"Bounds.Y".to_owned()));
        assert!(alias_names.contains(&"Bounds.Width".to_owned()));
        assert!(alias_names.contains(&"Bounds.Height".to_owned()));
    }

    #[rstest]
    fn activation_point_aliases_present() {
        let provider = mock_provider();
        let desktop: Arc<dyn UiNode> = Arc::new(DesktopNode);
        let app = provider.get_nodes(Arc::clone(&desktop)).unwrap().next().unwrap();
        let window = provider.get_nodes(Arc::clone(&app)).unwrap().next().unwrap();
        let button = provider.get_nodes(window).unwrap().next().unwrap();

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
}
