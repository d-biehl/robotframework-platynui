use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock, Weak};

use platynui_core::platform::{DesktopInfo, MonitorInfo};
use platynui_core::provider::UiTreeProvider;
use platynui_core::ui::attribute_names;
use platynui_core::ui::{
    DESKTOP_RUNTIME_ID, Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiValue, supported_patterns_value,
};

pub(super) struct DesktopNode {
    info: DesktopInfo,
    attributes: Vec<Arc<dyn UiAttribute>>,
    supported: Vec<PatternId>,
    providers: Vec<Arc<dyn UiTreeProvider>>,
    self_weak: OnceLock<Weak<dyn UiNode>>,
}

impl DesktopNode {
    pub(super) fn new(info: DesktopInfo, providers: Vec<Arc<dyn UiTreeProvider>>) -> Arc<Self> {
        let mut info = info;
        info.runtime_id = RuntimeId::from(DESKTOP_RUNTIME_ID);
        let namespace = Namespace::Control;
        let mut attributes: Vec<Arc<dyn UiAttribute>> = Vec::new();
        let supported = vec![PatternId::from("Desktop")];

        attributes.push(attr(namespace, attribute_names::common::ROLE, UiValue::from("Desktop")));
        attributes.push(attr(namespace, attribute_names::common::NAME, UiValue::from(info.name.clone())));
        attributes.push(attr(
            namespace,
            attribute_names::common::RUNTIME_ID,
            UiValue::from(info.runtime_id.as_str().to_owned()),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::common::TECHNOLOGY,
            UiValue::from(info.technology.as_str().to_owned()),
        ));
        attributes.push(attr(
            namespace,
            attribute_names::common::SUPPORTED_PATTERNS,
            supported_patterns_value(&supported),
        ));

        attributes.push(attr(namespace, attribute_names::element::BOUNDS, UiValue::from(info.bounds)));
        attributes.push(attr(namespace, attribute_names::element::IS_VISIBLE, UiValue::from(true)));
        attributes.push(attr(namespace, attribute_names::element::IS_ENABLED, UiValue::from(true)));
        attributes.push(attr(namespace, attribute_names::element::IS_OFFSCREEN, UiValue::from(false)));

        attributes.push(attr(
            namespace,
            attribute_names::desktop::DISPLAY_COUNT,
            UiValue::from(info.display_count() as i64),
        ));
        attributes.push(attr(namespace, attribute_names::desktop::OS_NAME, UiValue::from(info.os_name.clone())));
        attributes.push(attr(namespace, attribute_names::desktop::OS_VERSION, UiValue::from(info.os_version.clone())));
        attributes.push(attr(
            namespace,
            attribute_names::desktop::MONITORS,
            UiValue::Array(info.monitors.iter().map(monitor_to_value).collect()),
        ));

        Arc::new(Self { info, attributes, supported, providers, self_weak: OnceLock::new() })
    }

    pub(super) fn info(&self) -> &DesktopInfo {
        &self.info
    }

    pub(super) fn as_ui_node(self: &Arc<Self>) -> Arc<dyn UiNode> {
        Arc::clone(self) as Arc<dyn UiNode>
    }

    pub(super) fn init_self(this: &Arc<Self>) {
        let arc: Arc<dyn UiNode> = this.clone();
        let _ = this.self_weak.set(Arc::downgrade(&arc));
    }

    // children are provided on-demand from providers; no replacement snapshot
}

impl UiNode for DesktopNode {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }

    fn role(&self) -> &str {
        "Desktop"
    }

    fn name(&self) -> String {
        self.info.name.clone()
    }

    fn runtime_id(&self) -> &RuntimeId {
        &self.info.runtime_id
    }

    fn parent(&self) -> Option<std::sync::Weak<dyn UiNode>> {
        None
    }

    fn has_children(&self) -> bool {
        !self.providers.is_empty()
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        struct DesktopChildrenIter {
            providers: Vec<Arc<dyn UiTreeProvider>>,
            idx: usize,
            parent: Arc<dyn UiNode>,
            current: Option<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>>,
        }
        impl Iterator for DesktopChildrenIter {
            type Item = Arc<dyn UiNode>;
            fn next(&mut self) -> Option<Self::Item> {
                loop {
                    if let Some(it) = self.current.as_mut() {
                        if let Some(next) = it.next() {
                            return Some(next);
                        }
                        self.current = None;
                    }
                    if self.idx >= self.providers.len() {
                        return None;
                    }
                    let prov = &self.providers[self.idx];
                    self.idx += 1;
                    match prov.get_nodes(Arc::clone(&self.parent)) {
                        Ok(iter) => {
                            self.current = Some(iter);
                        }
                        Err(err) => {
                            tracing::error!(%err, "DesktopNode: provider get_nodes failed, skipping");
                        }
                    }
                }
            }
        }
        let parent = self.self_weak.get().and_then(|w| w.upgrade()).expect("desktop self weak set");
        let providers = self.providers.to_vec();
        Box::new(DesktopChildrenIter { providers, idx: 0, parent, current: None })
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        Box::new(self.attributes.clone().into_iter())
    }

    fn supported_patterns(&self) -> Vec<PatternId> {
        self.supported.clone()
    }

    fn invalidate(&self) {}
}

fn attr(namespace: Namespace, name: impl Into<String>, value: UiValue) -> Arc<dyn UiAttribute> {
    Arc::new(DesktopAttribute { namespace, name: name.into(), value })
}

fn monitor_to_value(info: &MonitorInfo) -> UiValue {
    let mut map = BTreeMap::new();
    map.insert("Id".to_string(), UiValue::from(info.id.clone()));
    if let Some(name) = &info.name {
        map.insert("Name".to_string(), UiValue::from(name.clone()));
    }
    map.insert("Bounds".to_string(), UiValue::from(info.bounds));
    map.insert("IsPrimary".to_string(), UiValue::from(info.is_primary));
    if let Some(scale) = info.scale_factor {
        map.insert("ScaleFactor".to_string(), UiValue::from(scale));
    }
    UiValue::Object(map)
}

struct DesktopAttribute {
    namespace: Namespace,
    name: String,
    value: UiValue,
}

impl UiAttribute for DesktopAttribute {
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
