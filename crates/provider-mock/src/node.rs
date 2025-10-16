use platynui_core::ui::attribute_names::common;
use platynui_core::ui::{
    Namespace, PatternId, PatternRegistry, RuntimeId, UiAttribute, UiNode, UiPattern, UiValue, supported_patterns_value,
};
use std::sync::{Arc, Mutex, Weak};

pub(crate) struct MockNode {
    namespace: Namespace,
    role: String,
    name: String,
    runtime_id: RuntimeId,
    attributes: Vec<Arc<dyn UiAttribute>>,
    runtime_patterns: PatternRegistry,
    declared_patterns: Vec<PatternId>,
    order_key: Option<u64>,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    children: Mutex<Vec<Arc<MockNode>>>,
}

pub(crate) struct NodePatternContext {
    pub runtime_patterns: PatternRegistry,
    pub declared_patterns: Vec<PatternId>,
    pub order_key: Option<u64>,
}

impl MockNode {
    pub(crate) fn new(
        namespace: Namespace,
        role: impl Into<String>,
        name: impl Into<String>,
        runtime_id: &str,
        technology: &str,
        mut additional_attributes: Vec<Arc<dyn UiAttribute>>,
        pattern_context: NodePatternContext,
    ) -> Arc<Self> {
        let runtime_id = RuntimeId::from(runtime_id);
        let role_string = role.into();
        let name_string = name.into();
        let mut attributes = vec![
            attr(namespace, common::ROLE, UiValue::from(role_string.clone())),
            attr(namespace, common::NAME, UiValue::from(name_string.clone())),
            attr(namespace, common::RUNTIME_ID, UiValue::from(runtime_id.as_str().to_owned())),
            attr(namespace, common::TECHNOLOGY, UiValue::from(technology.to_owned())),
        ];

        let NodePatternContext { runtime_patterns, declared_patterns, order_key } = pattern_context;
        attributes.append(&mut additional_attributes);

        Arc::new(Self {
            namespace,
            role: role_string,
            name: name_string,
            runtime_id,
            attributes,
            runtime_patterns,
            declared_patterns,
            order_key,
            parent: Mutex::new(None),
            children: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn children_snapshot(&self) -> Vec<Arc<MockNode>> {
        self.children.lock().unwrap().clone()
    }

    pub(crate) fn set_parent(&self, parent: &Arc<dyn UiNode>) {
        *self.parent.lock().unwrap() = Some(Arc::downgrade(parent));
    }

    pub(crate) fn add_child(parent: &Arc<Self>, child: Arc<Self>) {
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
        &self.role
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn runtime_id(&self) -> &RuntimeId {
        &self.runtime_id
    }

    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent.lock().unwrap().clone()
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        let snapshot = self.children.lock().unwrap().clone();
        Box::new(snapshot.into_iter().map(|child| -> Arc<dyn UiNode> { child }))
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        let mut attributes = self.attributes.clone();
        let patterns = self.supported_patterns();
        attributes.push(attr(self.namespace, common::SUPPORTED_PATTERNS, supported_patterns_value(&patterns)));
        Box::new(attributes.into_iter())
    }

    fn supported_patterns(&self) -> Vec<PatternId> {
        let mut patterns = self.declared_patterns.clone();
        for id in self.runtime_patterns.supported() {
            if !patterns.contains(&id) {
                patterns.push(id);
            }
        }
        patterns
    }

    fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
        self.runtime_patterns.get(pattern)
    }

    fn doc_order_key(&self) -> Option<u64> {
        self.order_key
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

pub(crate) fn attr(namespace: Namespace, name: impl Into<String>, value: UiValue) -> Arc<dyn UiAttribute> {
    Arc::new(StaticAttribute { namespace, name: name.into(), value })
}
