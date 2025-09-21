use super::attributes;
use super::identifiers::{PatternId, RuntimeId, TechnologyId};
use super::namespace::Namespace;
use super::value::UiValue;
use crate::types::Rect;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttributeKey {
    namespace: Namespace,
    name: Arc<str>,
}

impl AttributeKey {
    pub fn new(namespace: Namespace, name: impl Into<Arc<str>>) -> Self {
        Self { namespace, name: name.into() }
    }

    pub fn namespace(&self) -> Namespace {
        self.namespace
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonitorInfo {
    pub name: Option<String>,
    pub bounds: Rect,
    pub scale_factor: Option<f64>,
    pub is_primary: bool,
}

impl MonitorInfo {
    fn to_value(&self) -> UiValue {
        let mut map = BTreeMap::new();
        map.insert(attributes::names::BOUNDS.to_owned(), UiValue::from(self.bounds));
        map.insert("IsPrimary".to_owned(), UiValue::Bool(self.is_primary));
        if let Some(name) = &self.name {
            map.insert("Name".to_owned(), UiValue::String(name.clone()));
        }
        if let Some(scale) = self.scale_factor {
            map.insert("ScaleFactor".to_owned(), UiValue::Number(scale));
        }
        UiValue::Object(map)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DesktopInfo {
    pub name: Arc<str>,
    pub bounds: Rect,
    pub runtime_id: RuntimeId,
    pub technology: TechnologyId,
    pub os_name: String,
    pub os_version: Option<String>,
    pub monitors: Vec<MonitorInfo>,
}

impl DesktopInfo {
    pub fn new(
        name: impl Into<Arc<str>>,
        bounds: Rect,
        runtime_id: impl Into<RuntimeId>,
        technology: impl Into<TechnologyId>,
        os_name: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            bounds,
            runtime_id: runtime_id.into(),
            technology: technology.into(),
            os_name: os_name.into(),
            os_version: None,
            monitors: Vec::new(),
        }
    }

    pub fn with_os_version(mut self, version: impl Into<String>) -> Self {
        self.os_version = Some(version.into());
        self
    }

    pub fn with_monitors(mut self, monitors: Vec<MonitorInfo>) -> Self {
        self.monitors = monitors;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiNode {
    namespace: Namespace,
    role: Arc<str>,
    name: Arc<str>,
    bounds: Rect,
    is_visible: bool,
    is_offscreen: Option<bool>,
    runtime_id: RuntimeId,
    technology: TechnologyId,
    supported_patterns: BTreeSet<PatternId>,
    attributes: BTreeMap<AttributeKey, UiValue>,
    children: Vec<UiNode>,
}

impl UiNode {
    pub fn builder(
        namespace: Namespace,
        role: impl Into<Arc<str>>,
        name: impl Into<Arc<str>>,
        bounds: Rect,
        runtime_id: impl Into<RuntimeId>,
        technology: impl Into<TechnologyId>,
    ) -> UiNodeBuilder {
        UiNodeBuilder::new(
            namespace,
            role.into(),
            name.into(),
            bounds,
            runtime_id.into(),
            technology.into(),
        )
    }

    pub fn desktop(info: DesktopInfo) -> Self {
        let DesktopInfo { name, bounds, runtime_id, technology, os_name, os_version, monitors } =
            info;

        let display_count = monitors.len();
        let mut builder = UiNode::builder(
            Namespace::Control,
            "Desktop",
            name.clone(),
            bounds,
            runtime_id,
            technology,
        )
        .visible(true)
        .with_attribute(Namespace::Control, attributes::names::OS_NAME, UiValue::from(os_name))
        .with_attribute(
            Namespace::Control,
            attributes::names::DISPLAY_COUNT,
            UiValue::Number(display_count as f64),
        );

        if let Some(version) = os_version {
            builder = builder.with_attribute(
                Namespace::Control,
                attributes::names::OS_VERSION,
                UiValue::from(version),
            );
        }

        if !monitors.is_empty() {
            let monitors: Vec<UiValue> = monitors.iter().map(MonitorInfo::to_value).collect();
            builder = builder.with_attribute(
                Namespace::Control,
                attributes::names::MONITORS,
                UiValue::Array(monitors),
            );
        }

        builder.build()
    }

    pub fn namespace(&self) -> Namespace {
        self.namespace
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    pub fn is_offscreen(&self) -> Option<bool> {
        self.is_offscreen
    }

    pub fn runtime_id(&self) -> &RuntimeId {
        &self.runtime_id
    }

    pub fn technology(&self) -> &TechnologyId {
        &self.technology
    }

    pub fn supported_patterns(&self) -> impl Iterator<Item = &PatternId> {
        self.supported_patterns.iter()
    }

    pub fn attributes(&self) -> &BTreeMap<AttributeKey, UiValue> {
        &self.attributes
    }

    pub fn attribute(&self, namespace: Namespace, name: &str) -> Option<&UiValue> {
        let key = AttributeKey::new(namespace, Arc::<str>::from(name));
        self.attributes.get(&key)
    }

    pub fn children(&self) -> &[UiNode] {
        &self.children
    }

    fn required_attributes(&self) -> BTreeMap<AttributeKey, UiValue> {
        let mut map = BTreeMap::new();
        map.insert(
            AttributeKey::new(self.namespace, attributes::names::BOUNDS),
            UiValue::from(self.bounds),
        );
        map.insert(
            AttributeKey::new(self.namespace, attributes::names::ROLE),
            UiValue::from(self.role.as_ref()),
        );
        map.insert(
            AttributeKey::new(self.namespace, attributes::names::NAME),
            UiValue::from(self.name.as_ref()),
        );
        map.insert(
            AttributeKey::new(self.namespace, attributes::names::IS_VISIBLE),
            UiValue::Bool(self.is_visible),
        );
        if let Some(is_offscreen) = self.is_offscreen {
            map.insert(
                AttributeKey::new(self.namespace, attributes::names::IS_OFFSCREEN),
                UiValue::Bool(is_offscreen),
            );
        }
        map.insert(
            AttributeKey::new(self.namespace, attributes::names::RUNTIME_ID),
            UiValue::from(self.runtime_id.as_str()),
        );
        map.insert(
            AttributeKey::new(self.namespace, attributes::names::TECHNOLOGY),
            UiValue::from(self.technology.as_str()),
        );
        let patterns: Vec<UiValue> = self
            .supported_patterns
            .iter()
            .map(|pattern| UiValue::from(pattern.as_str().to_owned()))
            .collect();
        map.insert(
            AttributeKey::new(self.namespace, attributes::names::SUPPORTED_PATTERNS),
            UiValue::Array(patterns),
        );
        map
    }
}

pub struct UiNodeBuilder {
    namespace: Namespace,
    role: Arc<str>,
    name: Arc<str>,
    bounds: Rect,
    is_visible: bool,
    is_offscreen: Option<bool>,
    runtime_id: RuntimeId,
    technology: TechnologyId,
    supported_patterns: BTreeSet<PatternId>,
    attributes: BTreeMap<AttributeKey, UiValue>,
    children: Vec<UiNode>,
}

impl UiNodeBuilder {
    fn new(
        namespace: Namespace,
        role: Arc<str>,
        name: Arc<str>,
        bounds: Rect,
        runtime_id: RuntimeId,
        technology: TechnologyId,
    ) -> Self {
        Self {
            namespace,
            role,
            name,
            bounds,
            is_visible: true,
            is_offscreen: None,
            runtime_id,
            technology,
            supported_patterns: BTreeSet::new(),
            attributes: BTreeMap::new(),
            children: Vec::new(),
        }
    }

    pub fn visible(mut self, is_visible: bool) -> Self {
        self.is_visible = is_visible;
        self
    }

    pub fn offscreen(mut self, is_offscreen: Option<bool>) -> Self {
        self.is_offscreen = is_offscreen;
        self
    }

    pub fn add_pattern(mut self, pattern: impl Into<PatternId>) -> Self {
        self.supported_patterns.insert(pattern.into());
        self
    }

    pub fn with_patterns<I>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = PatternId>,
    {
        for pattern in patterns {
            self.supported_patterns.insert(pattern);
        }
        self
    }

    pub fn with_attribute(
        mut self,
        namespace: Namespace,
        name: impl Into<Arc<str>>,
        value: UiValue,
    ) -> Self {
        self.attributes.insert(AttributeKey::new(namespace, name), value);
        self
    }

    pub fn push_child(mut self, child: UiNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn with_children<I>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = UiNode>,
    {
        self.children.extend(children);
        self
    }

    pub fn build(self) -> UiNode {
        let UiNodeBuilder {
            namespace,
            role,
            name,
            bounds,
            is_visible,
            is_offscreen,
            runtime_id,
            technology,
            supported_patterns,
            attributes,
            children,
        } = self;

        let mut node = UiNode {
            namespace,
            role,
            name,
            bounds,
            is_visible,
            is_offscreen,
            runtime_id,
            technology,
            supported_patterns,
            attributes,
            children,
        };

        let mut attributes = node.attributes.clone();
        attributes.extend(node.required_attributes());
        node.attributes = Self::flatten_structured_attributes(&attributes);
        node
    }
    fn flatten_structured_attributes(
        attributes: &BTreeMap<AttributeKey, UiValue>,
    ) -> BTreeMap<AttributeKey, UiValue> {
        let mut flattened = BTreeMap::new();
        for (key, value) in attributes.iter() {
            flattened.insert(key.clone(), value.clone());
            for (extra_key, extra_value) in Self::expand_structured_attribute(key, value) {
                flattened.insert(extra_key, extra_value);
            }
        }
        flattened
    }

    fn expand_structured_attribute(
        key: &AttributeKey,
        value: &UiValue,
    ) -> Vec<(AttributeKey, UiValue)> {
        match value {
            UiValue::Rect(rect) => vec![
                (Self::component_key(key, "X"), UiValue::Number(rect.x())),
                (Self::component_key(key, "Y"), UiValue::Number(rect.y())),
                (Self::component_key(key, "Width"), UiValue::Number(rect.width())),
                (Self::component_key(key, "Height"), UiValue::Number(rect.height())),
            ],
            UiValue::Point(point) => vec![
                (Self::component_key(key, "X"), UiValue::Number(point.x())),
                (Self::component_key(key, "Y"), UiValue::Number(point.y())),
            ],
            UiValue::Size(size) => vec![
                (Self::component_key(key, "Width"), UiValue::Number(size.width())),
                (Self::component_key(key, "Height"), UiValue::Number(size.height())),
            ],
            _ => Vec::new(),
        }
    }

    fn component_key(key: &AttributeKey, suffix: &str) -> AttributeKey {
        let mut name = String::with_capacity(key.name().len() + suffix.len() + 1);
        name.push_str(key.name());
        name.push('.');
        name.push_str(suffix);
        AttributeKey::new(key.namespace(), name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Rect;
    use rstest::{fixture, rstest};

    #[fixture]
    fn sample_node() -> UiNode {
        UiNode::builder(
            Namespace::Control,
            "Button",
            "OK",
            Rect::new(0.0, 0.0, 100.0, 30.0),
            RuntimeId::from("button-1"),
            TechnologyId::from("UIAutomation"),
        )
        .add_pattern(PatternId::from("Activatable"))
        .with_attribute(Namespace::Native, "ControlType", UiValue::from("Button"))
        .build()
    }

    #[rstest]
    fn builder_composes_required_attributes(sample_node: UiNode) {
        let node = sample_node;
        let role = node.attribute(Namespace::Control, attributes::names::ROLE).cloned().unwrap();
        assert_eq!(role, UiValue::from("Button"));
        assert!(
            node.attribute(Namespace::Control, attributes::names::SUPPORTED_PATTERNS).is_some()
        );
        let bounds =
            node.attribute(Namespace::Control, attributes::names::BOUNDS).cloned().unwrap();
        assert!(matches!(bounds, UiValue::Rect(_)));
        let bounds_x = node.attribute(Namespace::Control, "Bounds.X").cloned().unwrap();
        assert_eq!(bounds_x, UiValue::Number(0.0));
    }

    #[rstest]
    fn desktop_builder_adds_desktop_specific_attributes() {
        let desktop = UiNode::desktop(
            DesktopInfo::new(
                "Desktop",
                Rect::new(0.0, 0.0, 1920.0, 1080.0),
                RuntimeId::from("desktop"),
                TechnologyId::from("Runtime"),
                "Linux",
            )
            .with_os_version("6.9.0")
            .with_monitors(vec![MonitorInfo {
                name: Some("Primary".into()),
                bounds: Rect::new(0.0, 0.0, 1920.0, 1080.0),
                scale_factor: Some(1.0),
                is_primary: true,
            }]),
        );

        let display_count = desktop
            .attribute(Namespace::Control, attributes::names::DISPLAY_COUNT)
            .cloned()
            .unwrap();
        assert_eq!(display_count, UiValue::Number(1.0));

        let monitors =
            desktop.attribute(Namespace::Control, attributes::names::MONITORS).cloned().unwrap();
        match monitors {
            UiValue::Array(entries) => assert_eq!(entries.len(), 1),
            _ => panic!("expected array"),
        }
    }
}
