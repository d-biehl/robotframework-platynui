use atspi_common::{
    Action as AtspiAction, CoordType, Interface, InterfaceSet, ObjectRefOwned, RelationType, Role, State, StateSet,
};
use atspi_connection::AccessibilityConnection;
use atspi_proxies::accessible::AccessibleProxy;
use atspi_proxies::action::ActionProxy;
use atspi_proxies::application::ApplicationProxy;
use atspi_proxies::collection::CollectionProxy;
use atspi_proxies::component::ComponentProxy;
use atspi_proxies::document::DocumentProxy;
use atspi_proxies::hyperlink::HyperlinkProxy;
use atspi_proxies::hypertext::HypertextProxy;
use atspi_proxies::image::ImageProxy;
use atspi_proxies::selection::SelectionProxy;
use atspi_proxies::table::TableProxy;
use atspi_proxies::table_cell::TableCellProxy;
use atspi_proxies::text::TextProxy;
use atspi_proxies::value::ValueProxy;
use futures_lite::future::block_on;
use once_cell::sync::OnceCell;
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::attribute_names::{activation_target, common, element, focusable};
use platynui_core::ui::{
    FocusableAction, Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiPattern, UiValue, supported_patterns_value,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use zbus::proxy::CacheProperties;

const NULL_PATH: &str = "/org/a11y/atspi/accessible/null";
const TECHNOLOGY: &str = "AT-SPI2";

pub struct AtspiNode {
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    self_weak: OnceCell<Weak<dyn UiNode>>,
    runtime_id: OnceCell<RuntimeId>,
    role: OnceCell<String>,
    namespace: OnceCell<Namespace>,
    state: OnceCell<Option<StateSet>>,
    interfaces: OnceCell<Option<InterfaceSet>>,
}

impl AtspiNode {
    pub fn new(conn: Arc<AccessibilityConnection>, obj: ObjectRefOwned, parent: Option<&Arc<dyn UiNode>>) -> Arc<Self> {
        let node = Arc::new(Self {
            conn,
            obj,
            parent: Mutex::new(parent.map(Arc::downgrade)),
            self_weak: OnceCell::new(),
            runtime_id: OnceCell::new(),
            role: OnceCell::new(),
            namespace: OnceCell::new(),
            state: OnceCell::new(),
            interfaces: OnceCell::new(),
        });
        let arc: Arc<dyn UiNode> = node.clone();
        let _ = node.self_weak.set(Arc::downgrade(&arc));
        node
    }

    pub fn is_null_object(obj: &ObjectRefOwned) -> bool {
        obj.path_as_str() == NULL_PATH
    }

    fn accessible(&self) -> Option<AccessibleProxy<'_>> {
        accessible_proxy(self.conn.as_ref(), &self.obj)
    }

    fn resolve_role(&self) {
        if self.role.get().is_some() {
            return;
        }
        let interfaces = self.resolve_interfaces();
        let role = self.accessible().and_then(|proxy| block_on(proxy.get_role()).ok()).unwrap_or(Role::Invalid);
        let (namespace, role_name) = map_role_with_interfaces(role, interfaces);
        let _ = self.namespace.set(namespace);
        let _ = self.role.set(role_name);
    }

    fn resolve_state(&self) -> Option<StateSet> {
        self.state
            .get_or_init(|| self.accessible().and_then(|proxy| block_on(proxy.get_state()).ok()))
            .as_ref()
            .copied()
    }

    fn resolve_interfaces(&self) -> Option<InterfaceSet> {
        self.interfaces
            .get_or_init(|| self.accessible().and_then(|proxy| block_on(proxy.get_interfaces()).ok()))
            .as_ref()
            .copied()
    }

    fn focusable(&self) -> bool {
        let interfaces = self.resolve_interfaces();
        let state = self.resolve_state();
        let supports_component = interfaces.map(|ifaces| ifaces.contains(Interface::Component)).unwrap_or(false);
        let focusable = state.map(|s| s.contains(State::Focusable) || s.contains(State::Focused)).unwrap_or(false);
        supports_component && focusable
    }
}

impl UiNode for AtspiNode {
    fn namespace(&self) -> Namespace {
        self.resolve_role();
        *self.namespace.get().unwrap_or(&Namespace::Control)
    }

    fn role(&self) -> &str {
        self.resolve_role();
        self.role.get().map(String::as_str).unwrap_or("Unknown")
    }

    fn name(&self) -> String {
        resolve_name(self.conn.as_ref(), &self.obj).unwrap_or_default()
    }

    fn runtime_id(&self) -> &RuntimeId {
        self.runtime_id.get_or_init(|| RuntimeId::from(object_runtime_id(&self.obj)))
    }

    fn id(&self) -> Option<String> {
        resolve_id(self.conn.as_ref(), &self.obj)
    }

    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent.lock().unwrap().clone()
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        let Some(children) = self.accessible().and_then(|proxy| block_on(proxy.get_children()).ok()) else {
            return Box::new(std::iter::empty());
        };
        let parent = self.self_weak.get().and_then(|weak| weak.upgrade());
        let conn = self.conn.clone();
        Box::new(children.into_iter().filter_map(move |child| {
            if AtspiNode::is_null_object(&child) {
                return None;
            }
            let node = AtspiNode::new(conn.clone(), child, parent.as_ref());
            Some(node as Arc<dyn UiNode>)
        }))
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        let rid_str = self.runtime_id().as_str().to_string();
        let owner = self.self_weak.get().and_then(|weak| weak.upgrade());
        Box::new(AttrsIter::new(self, owner, rid_str))
    }

    fn supported_patterns(&self) -> Vec<PatternId> {
        let mut patterns = Vec::new();
        if self.focusable() {
            patterns.push(PatternId::from("Focusable"));
        }
        patterns
    }

    fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
        if pattern.as_str() != "Focusable" {
            return None;
        }
        if !self.focusable() {
            return None;
        }
        let conn = self.conn.clone();
        let obj = self.obj.clone();
        let action =
            FocusableAction::new(move || grab_focus(conn.as_ref(), &obj).map_err(platynui_core::ui::PatternError::new));
        Some(Arc::new(action) as Arc<dyn UiPattern>)
    }

    fn invalidate(&self) {}
}

fn accessible_proxy<'a>(conn: &'a AccessibilityConnection, obj: &'a ObjectRefOwned) -> Option<AccessibleProxy<'a>> {
    let name = obj.name_as_str()?;
    let builder = AccessibleProxy::builder(conn.connection())
        .cache_properties(CacheProperties::No)
        .destination(name)
        .ok()?
        .path(obj.path_as_str())
        .ok()?;
    block_on(builder.build()).ok()
}

fn component_proxy<'a>(conn: &'a AccessibilityConnection, obj: &'a ObjectRefOwned) -> Option<ComponentProxy<'a>> {
    let name = obj.name_as_str()?;
    let builder = ComponentProxy::builder(conn.connection())
        .cache_properties(CacheProperties::No)
        .destination(name)
        .ok()?
        .path(obj.path_as_str())
        .ok()?;
    block_on(builder.build()).ok()
}

macro_rules! make_proxy {
    ($fn_name:ident, $proxy:ident) => {
        fn $fn_name<'a>(conn: &'a AccessibilityConnection, obj: &'a ObjectRefOwned) -> Option<$proxy<'a>> {
            let name = obj.name_as_str()?;
            let builder = $proxy::builder(conn.connection())
                .cache_properties(CacheProperties::No)
                .destination(name)
                .ok()?
                .path(obj.path_as_str())
                .ok()?;
            block_on(builder.build()).ok()
        }
    };
}

make_proxy!(action_proxy, ActionProxy);
make_proxy!(application_proxy, ApplicationProxy);
make_proxy!(collection_proxy, CollectionProxy);
make_proxy!(document_proxy, DocumentProxy);
make_proxy!(hyperlink_proxy, HyperlinkProxy);
make_proxy!(hypertext_proxy, HypertextProxy);
make_proxy!(image_proxy, ImageProxy);
make_proxy!(selection_proxy, SelectionProxy);
make_proxy!(table_proxy, TableProxy);
make_proxy!(table_cell_proxy, TableCellProxy);
make_proxy!(text_proxy, TextProxy);
make_proxy!(value_proxy, ValueProxy);

fn grab_focus(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Result<(), String> {
    let ok = block_on(component_proxy(conn, obj).ok_or("component interface missing")?.grab_focus())
        .map_err(|e| e.to_string())?;
    if ok { Ok(()) } else { Err("grab_focus returned false".to_string()) }
}

fn object_runtime_id(obj: &ObjectRefOwned) -> String {
    let name = obj.name_as_str().unwrap_or_default();
    format!("atspi://{}{}", name, obj.path_as_str())
}

fn normalize_value(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

fn pick_attr_value(attrs: &[(String, String)], keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some((_name, value)) = attrs.iter().find(|(name, _)| name.eq_ignore_ascii_case(key)) {
            if let Some(value) = normalize_value(value.clone()) {
                return Some(value);
            }
        }
    }
    None
}

fn resolve_attributes(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Option<Vec<(String, String)>> {
    let mut pairs: Vec<(String, String)> =
        block_on(accessible_proxy(conn, obj)?.get_attributes()).ok()?.into_iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    Some(pairs)
}

fn resolve_name(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Option<String> {
    if let Ok(name) = block_on(accessible_proxy(conn, obj)?.name()) {
        if let Some(value) = normalize_value(name) {
            return Some(value);
        }
    }
    resolve_attributes(conn, obj)
        .and_then(|attrs| pick_attr_value(&attrs, &["accessible-name", "name", "label", "title"]))
}

fn resolve_id(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Option<String> {
    if let Ok(id) = block_on(accessible_proxy(conn, obj)?.accessible_id()) {
        if let Some(value) = normalize_value(id) {
            return Some(value);
        }
    }
    resolve_attributes(conn, obj).and_then(|attrs| pick_attr_value(&attrs, &["accessible-id", "accessible_id", "id"]))
}

fn attributes_object(attrs: &[(String, String)]) -> UiValue {
    let mut map = BTreeMap::new();
    for (name, value) in attrs {
        if name.trim().is_empty() {
            continue;
        }
        map.insert(name.clone(), UiValue::from(value.clone()));
    }
    UiValue::Object(map)
}

fn string_map_object(map: &std::collections::HashMap<String, String>) -> UiValue {
    let mut out = BTreeMap::new();
    for (name, value) in map {
        if name.trim().is_empty() {
            continue;
        }
        out.insert(name.clone(), UiValue::from(value.clone()));
    }
    UiValue::Object(out)
}

fn object_refs_value(objects: Vec<ObjectRefOwned>) -> UiValue {
    UiValue::from(objects.into_iter().map(|obj| object_runtime_id(&obj)).collect::<Vec<_>>())
}

fn relation_set_value(relations: Vec<(RelationType, Vec<ObjectRefOwned>)>) -> UiValue {
    let mut map = BTreeMap::new();
    for (relation, targets) in relations {
        map.insert(format!("{relation:?}"), object_refs_value(targets));
    }
    UiValue::Object(map)
}

fn interface_set_value(interfaces: InterfaceSet) -> UiValue {
    UiValue::from(interfaces.iter().map(|iface| format!("{iface:?}")).collect::<Vec<_>>())
}

fn state_set_value(state: StateSet) -> UiValue {
    UiValue::from(state.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>())
}

fn actions_value(actions: Vec<AtspiAction>) -> UiValue {
    let values = actions
        .into_iter()
        .map(|action| {
            let mut map = BTreeMap::new();
            map.insert("Name".to_string(), UiValue::from(action.name));
            map.insert("Description".to_string(), UiValue::from(action.description));
            map.insert("KeyBinding".to_string(), UiValue::from(action.keybinding));
            UiValue::Object(map)
        })
        .collect::<Vec<_>>();
    UiValue::Array(values)
}

fn row_column_value(row: i32, column: i32) -> UiValue {
    let mut map = BTreeMap::new();
    map.insert("Row".to_string(), UiValue::from(row as i64));
    map.insert("Column".to_string(), UiValue::from(column as i64));
    UiValue::Object(map)
}

fn push_native(out: &mut Vec<(String, UiValue)>, interface: &str, name: &str, value: UiValue) {
    out.push((format!("{interface}.{name}"), value));
}

fn collect_accessible_native_properties(
    conn: &AccessibilityConnection,
    obj: &ObjectRefOwned,
    attrs: Option<&[(String, String)]>,
    interfaces: Option<InterfaceSet>,
) -> Vec<(String, UiValue)> {
    let Some(accessible) = accessible_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();

    if let Ok(name) = block_on(accessible.name()) {
        if let Some(value) = normalize_value(name) {
            push_native(&mut out, "Accessible", "Name", UiValue::from(value));
        }
    }
    if let Ok(description) = block_on(accessible.description()) {
        if let Some(value) = normalize_value(description) {
            push_native(&mut out, "Accessible", "Description", UiValue::from(value));
        }
    }
    if let Ok(help_text) = block_on(accessible.help_text()) {
        if let Some(value) = normalize_value(help_text) {
            push_native(&mut out, "Accessible", "HelpText", UiValue::from(value));
        }
    }
    if let Ok(locale) = block_on(accessible.locale()) {
        if let Some(value) = normalize_value(locale) {
            push_native(&mut out, "Accessible", "Locale", UiValue::from(value));
        }
    }
    if let Ok(role) = block_on(accessible.get_role()) {
        push_native(&mut out, "Accessible", "Role", UiValue::from(role.name().to_string()));
    }
    if let Ok(role_name) = block_on(accessible.get_role_name()) {
        if let Some(value) = normalize_value(role_name) {
            push_native(&mut out, "Accessible", "RoleName", UiValue::from(value));
        }
    }
    if let Ok(localized_role) = block_on(accessible.get_localized_role_name()) {
        if let Some(value) = normalize_value(localized_role) {
            push_native(&mut out, "Accessible", "LocalizedRoleName", UiValue::from(value));
        }
    }
    if let Ok(accessible_id) = block_on(accessible.accessible_id()) {
        if let Some(value) = normalize_value(accessible_id) {
            push_native(&mut out, "Accessible", "AccessibleId", UiValue::from(value));
        }
    }
    if let Ok(parent) = block_on(accessible.parent()) {
        push_native(&mut out, "Accessible", "Parent", UiValue::from(object_runtime_id(&parent)));
    }
    if let Ok(child_count) = block_on(accessible.child_count()) {
        push_native(&mut out, "Accessible", "ChildCount", UiValue::from(child_count as i64));
    }
    if let Ok(index) = block_on(accessible.get_index_in_parent()) {
        push_native(&mut out, "Accessible", "IndexInParent", UiValue::from(index as i64));
    }
    let interfaces = interfaces.or_else(|| block_on(accessible.get_interfaces()).ok());
    if let Some(interfaces) = interfaces {
        push_native(&mut out, "Accessible", "Interfaces", interface_set_value(interfaces));
    }
    if let Ok(state) = block_on(accessible.get_state()) {
        push_native(&mut out, "Accessible", "State", state_set_value(state));
    }
    if let Ok(relations) = block_on(accessible.get_relation_set()) {
        push_native(&mut out, "Accessible", "RelationSet", relation_set_value(relations));
    }
    if let Ok(application) = block_on(accessible.get_application()) {
        push_native(&mut out, "Accessible", "Application", UiValue::from(object_runtime_id(&application)));
    }
    if let Some(attrs) = attrs {
        if !attrs.is_empty() {
            push_native(&mut out, "Accessible", "Attributes", attributes_object(attrs));
            for (name, value) in attrs {
                if name.trim().is_empty() {
                    continue;
                }
                push_native(&mut out, "Accessible", &format!("Attribute.{name}"), UiValue::from(value.clone()));
            }
        }
    }

    out
}

fn collect_action_native_properties(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Vec<(String, UiValue)> {
    let Some(action) = action_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(count) = block_on(action.nactions()) {
        push_native(&mut out, "Action", "NActions", UiValue::from(count as i64));
    }
    if let Ok(actions) = block_on(action.get_actions()) {
        push_native(&mut out, "Action", "Actions", actions_value(actions));
    }
    out
}

fn collect_application_native_properties(
    conn: &AccessibilityConnection,
    obj: &ObjectRefOwned,
) -> Vec<(String, UiValue)> {
    let Some(app) = application_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(id) = block_on(app.id()) {
        push_native(&mut out, "Application", "Id", UiValue::from(id as i64));
    }
    if let Ok(version) = block_on(app.version()) {
        if let Some(value) = normalize_value(version) {
            push_native(&mut out, "Application", "Version", UiValue::from(value));
        }
    }
    if let Ok(toolkit) = block_on(app.toolkit_name()) {
        if let Some(value) = normalize_value(toolkit) {
            push_native(&mut out, "Application", "ToolkitName", UiValue::from(value));
        }
    }
    if let Ok(atspi_version) = block_on(app.atspi_version()) {
        if let Some(value) = normalize_value(atspi_version) {
            push_native(&mut out, "Application", "AtspiVersion", UiValue::from(value));
        }
    }
    if let Ok(address) = block_on(app.get_application_bus_address()) {
        if let Some(value) = normalize_value(address) {
            push_native(&mut out, "Application", "BusAddress", UiValue::from(value));
        }
    }
    out
}

fn collect_collection_native_properties(
    conn: &AccessibilityConnection,
    obj: &ObjectRefOwned,
) -> Vec<(String, UiValue)> {
    let Some(collection) = collection_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(descendant) = block_on(collection.get_active_descendant()) {
        push_native(&mut out, "Collection", "ActiveDescendant", UiValue::from(object_runtime_id(&descendant)));
    }
    out
}

fn collect_component_native_properties(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Vec<(String, UiValue)> {
    let Some(component) = component_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(alpha) = block_on(component.get_alpha()) {
        push_native(&mut out, "Component", "Alpha", UiValue::from(alpha));
    }
    if let Ok((x, y, w, h)) = block_on(component.get_extents(CoordType::Screen)) {
        push_native(&mut out, "Component", "Extents", UiValue::from(Rect::new(x as f64, y as f64, w as f64, h as f64)));
    }
    if let Ok((x, y)) = block_on(component.get_position(CoordType::Screen)) {
        push_native(&mut out, "Component", "Position", UiValue::from(Point::new(x as f64, y as f64)));
    }
    if let Ok((w, h)) = block_on(component.get_size()) {
        push_native(&mut out, "Component", "Size", UiValue::from(Size::new(w as f64, h as f64)));
    }
    if let Ok(layer) = block_on(component.get_layer()) {
        push_native(&mut out, "Component", "Layer", UiValue::from(format!("{layer:?}")));
    }
    if let Ok(order) = block_on(component.get_mdiz_order()) {
        push_native(&mut out, "Component", "MDIZOrder", UiValue::from(order as i64));
    }
    out
}

fn collect_document_native_properties(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Vec<(String, UiValue)> {
    let Some(document) = document_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(page) = block_on(document.page_count()) {
        push_native(&mut out, "Document", "PageCount", UiValue::from(page as i64));
    }
    if let Ok(page) = block_on(document.current_page_number()) {
        push_native(&mut out, "Document", "CurrentPageNumber", UiValue::from(page as i64));
    }
    if let Ok(locale) = block_on(document.get_locale()) {
        if let Some(value) = normalize_value(locale) {
            push_native(&mut out, "Document", "Locale", UiValue::from(value));
        }
    }
    if let Ok(attrs) = block_on(document.get_attributes()) {
        if !attrs.is_empty() {
            push_native(&mut out, "Document", "Attributes", string_map_object(&attrs));
        }
    }
    out
}

fn collect_hyperlink_native_properties(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Vec<(String, UiValue)> {
    let Some(link) = hyperlink_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(valid) = block_on(link.is_valid()) {
        push_native(&mut out, "Hyperlink", "IsValid", UiValue::from(valid));
    }
    if let Ok(end) = block_on(link.end_index()) {
        push_native(&mut out, "Hyperlink", "EndIndex", UiValue::from(end as i64));
    }
    if let Ok(start) = block_on(link.start_index()) {
        push_native(&mut out, "Hyperlink", "StartIndex", UiValue::from(start as i64));
    }
    if let Ok(count) = block_on(link.n_anchors()) {
        push_native(&mut out, "Hyperlink", "NAnchors", UiValue::from(count as i64));
    }
    out
}

fn collect_hypertext_native_properties(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Vec<(String, UiValue)> {
    let Some(text) = hypertext_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(count) = block_on(text.get_nlinks()) {
        push_native(&mut out, "Hypertext", "NLinks", UiValue::from(count as i64));
    }
    out
}

fn collect_image_native_properties(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Vec<(String, UiValue)> {
    let Some(image) = image_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(description) = block_on(image.image_description()) {
        if let Some(value) = normalize_value(description) {
            push_native(&mut out, "Image", "Description", UiValue::from(value));
        }
    }
    if let Ok(locale) = block_on(image.image_locale()) {
        if let Some(value) = normalize_value(locale) {
            push_native(&mut out, "Image", "Locale", UiValue::from(value));
        }
    }
    if let Ok((x, y, w, h)) = block_on(image.get_image_extents(CoordType::Screen)) {
        push_native(&mut out, "Image", "Extents", UiValue::from(Rect::new(x as f64, y as f64, w as f64, h as f64)));
    }
    if let Ok((x, y)) = block_on(image.get_image_position(CoordType::Screen)) {
        push_native(&mut out, "Image", "Position", UiValue::from(Point::new(x as f64, y as f64)));
    }
    if let Ok((w, h)) = block_on(image.get_image_size()) {
        push_native(&mut out, "Image", "Size", UiValue::from(Size::new(w as f64, h as f64)));
    }
    out
}

fn collect_selection_native_properties(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Vec<(String, UiValue)> {
    let Some(selection) = selection_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(count) = block_on(selection.nselected_children()) {
        push_native(&mut out, "Selection", "NSelectedChildren", UiValue::from(count as i64));
    }
    out
}

fn collect_table_native_properties(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Vec<(String, UiValue)> {
    let Some(table) = table_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(caption) = block_on(table.caption()) {
        push_native(&mut out, "Table", "Caption", UiValue::from(object_runtime_id(&caption)));
    }
    if let Ok(summary) = block_on(table.summary()) {
        push_native(&mut out, "Table", "Summary", UiValue::from(object_runtime_id(&summary)));
    }
    if let Ok(cols) = block_on(table.ncolumns()) {
        push_native(&mut out, "Table", "NColumns", UiValue::from(cols as i64));
    }
    if let Ok(rows) = block_on(table.nrows()) {
        push_native(&mut out, "Table", "NRows", UiValue::from(rows as i64));
    }
    if let Ok(cols) = block_on(table.nselected_columns()) {
        push_native(&mut out, "Table", "NSelectedColumns", UiValue::from(cols as i64));
    }
    if let Ok(rows) = block_on(table.nselected_rows()) {
        push_native(&mut out, "Table", "NSelectedRows", UiValue::from(rows as i64));
    }
    if let Ok(rows) = block_on(table.get_selected_rows()) {
        push_native(
            &mut out,
            "Table",
            "SelectedRows",
            UiValue::from(rows.into_iter().map(|v| v as i64).collect::<Vec<_>>()),
        );
    }
    if let Ok(cols) = block_on(table.get_selected_columns()) {
        push_native(
            &mut out,
            "Table",
            "SelectedColumns",
            UiValue::from(cols.into_iter().map(|v| v as i64).collect::<Vec<_>>()),
        );
    }
    out
}

fn collect_table_cell_native_properties(
    conn: &AccessibilityConnection,
    obj: &ObjectRefOwned,
) -> Vec<(String, UiValue)> {
    let Some(cell) = table_cell_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(span) = block_on(cell.column_span()) {
        push_native(&mut out, "TableCell", "ColumnSpan", UiValue::from(span as i64));
    }
    if let Ok(span) = block_on(cell.row_span()) {
        push_native(&mut out, "TableCell", "RowSpan", UiValue::from(span as i64));
    }
    if let Ok((row, column)) = block_on(cell.position()) {
        push_native(&mut out, "TableCell", "Position", row_column_value(row, column));
    }
    if let Ok(table) = block_on(cell.table()) {
        push_native(&mut out, "TableCell", "Table", UiValue::from(object_runtime_id(&table)));
    }
    out
}

fn collect_text_native_properties(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Vec<(String, UiValue)> {
    let Some(text) = text_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(count) = block_on(text.character_count()) {
        push_native(&mut out, "Text", "CharacterCount", UiValue::from(count as i64));
    }
    if let Ok(offset) = block_on(text.caret_offset()) {
        push_native(&mut out, "Text", "CaretOffset", UiValue::from(offset as i64));
    }
    if let Ok(count) = block_on(text.get_nselections()) {
        push_native(&mut out, "Text", "NSelections", UiValue::from(count as i64));
    }
    if let Ok(attrs) = block_on(text.get_default_attributes()) {
        if !attrs.is_empty() {
            push_native(&mut out, "Text", "DefaultAttributes", string_map_object(&attrs));
        }
    }
    if let Ok(attrs) = block_on(text.get_default_attribute_set()) {
        if !attrs.is_empty() {
            push_native(&mut out, "Text", "DefaultAttributeSet", string_map_object(&attrs));
        }
    }
    out
}

fn collect_value_native_properties(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Vec<(String, UiValue)> {
    let Some(value) = value_proxy(conn, obj) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(current) = block_on(value.current_value()) {
        push_native(&mut out, "Value", "CurrentValue", UiValue::from(current));
    }
    if let Ok(maximum) = block_on(value.maximum_value()) {
        push_native(&mut out, "Value", "MaximumValue", UiValue::from(maximum));
    }
    if let Ok(minimum) = block_on(value.minimum_value()) {
        push_native(&mut out, "Value", "MinimumValue", UiValue::from(minimum));
    }
    if let Ok(increment) = block_on(value.minimum_increment()) {
        push_native(&mut out, "Value", "MinimumIncrement", UiValue::from(increment));
    }
    if let Ok(text) = block_on(value.text()) {
        if let Some(value) = normalize_value(text) {
            push_native(&mut out, "Value", "Text", UiValue::from(value));
        }
    }
    out
}

fn collect_interface_native_properties(
    conn: &AccessibilityConnection,
    obj: &ObjectRefOwned,
    interfaces: Option<InterfaceSet>,
    attrs: Option<&[(String, String)]>,
) -> Vec<(String, UiValue)> {
    let interfaces =
        interfaces.or_else(|| accessible_proxy(conn, obj).and_then(|proxy| block_on(proxy.get_interfaces()).ok()));
    let mut out = collect_accessible_native_properties(conn, obj, attrs, interfaces);
    let Some(interfaces) = interfaces else {
        return out;
    };
    if interfaces.contains(Interface::Action) {
        out.extend(collect_action_native_properties(conn, obj));
    }
    if interfaces.contains(Interface::Application) {
        out.extend(collect_application_native_properties(conn, obj));
    }
    if interfaces.contains(Interface::Collection) {
        out.extend(collect_collection_native_properties(conn, obj));
    }
    if interfaces.contains(Interface::Component) {
        out.extend(collect_component_native_properties(conn, obj));
    }
    if interfaces.contains(Interface::Document) {
        out.extend(collect_document_native_properties(conn, obj));
    }
    if interfaces.contains(Interface::Hyperlink) {
        out.extend(collect_hyperlink_native_properties(conn, obj));
    }
    if interfaces.contains(Interface::Hypertext) {
        out.extend(collect_hypertext_native_properties(conn, obj));
    }
    if interfaces.contains(Interface::Image) {
        out.extend(collect_image_native_properties(conn, obj));
    }
    if interfaces.contains(Interface::Selection) {
        out.extend(collect_selection_native_properties(conn, obj));
    }
    if interfaces.contains(Interface::Table) {
        out.extend(collect_table_native_properties(conn, obj));
    }
    if interfaces.contains(Interface::TableCell) {
        out.extend(collect_table_cell_native_properties(conn, obj));
    }
    if interfaces.contains(Interface::Text) {
        out.extend(collect_text_native_properties(conn, obj));
    }
    if interfaces.contains(Interface::Value) {
        out.extend(collect_value_native_properties(conn, obj));
    }
    out
}

fn map_role(role: Role) -> (Namespace, String) {
    use Role::*;
    let (namespace, name): (Namespace, &str) = match role {
        Invalid => (Namespace::Control, "Unknown"),
        AcceleratorLabel => (Namespace::Control, "AcceleratorLabel"),
        Alert => (Namespace::Control, "Alert"),
        Animation => (Namespace::Control, "Animation"),
        Arrow => (Namespace::Control, "Arrow"),
        Calendar => (Namespace::Control, "Calendar"),
        Canvas => (Namespace::Control, "Canvas"),
        CheckBox => (Namespace::Control, "CheckBox"),
        CheckMenuItem => (Namespace::Item, "MenuItem"),
        ColorChooser => (Namespace::Control, "ColorChooser"),
        ColumnHeader => (Namespace::Item, "ColumnHeader"),
        ComboBox => (Namespace::Control, "ComboBox"),
        DateEditor => (Namespace::Control, "DateEditor"),
        DesktopIcon => (Namespace::Control, "DesktopIcon"),
        DesktopFrame => (Namespace::Control, "DesktopFrame"),
        Dial => (Namespace::Control, "Dial"),
        Dialog => (Namespace::Control, "Dialog"),
        DirectoryPane => (Namespace::Control, "DirectoryPane"),
        DrawingArea => (Namespace::Control, "DrawingArea"),
        FileChooser => (Namespace::Control, "FileChooser"),
        Filler => (Namespace::Control, "Filler"),
        FocusTraversable => (Namespace::Control, "FocusTraversable"),
        FontChooser => (Namespace::Control, "FontChooser"),
        Frame => (Namespace::Control, "Frame"),
        GlassPane => (Namespace::Control, "GlassPane"),
        HTMLContainer => (Namespace::Control, "HtmlContainer"),
        Icon => (Namespace::Control, "Icon"),
        Image => (Namespace::Control, "Image"),
        InternalFrame => (Namespace::Control, "InternalFrame"),
        Label => (Namespace::Control, "Label"),
        LayeredPane => (Namespace::Control, "LayeredPane"),
        List => (Namespace::Control, "List"),
        ListItem => (Namespace::Item, "ListItem"),
        Menu => (Namespace::Control, "Menu"),
        MenuBar => (Namespace::Control, "MenuBar"),
        MenuItem => (Namespace::Item, "MenuItem"),
        OptionPane => (Namespace::Control, "OptionPane"),
        PageTab => (Namespace::Item, "TabItem"),
        PageTabList => (Namespace::Control, "Tab"),
        Panel => (Namespace::Control, "Panel"),
        PasswordText => (Namespace::Control, "PasswordText"),
        PopupMenu => (Namespace::Control, "PopupMenu"),
        ProgressBar => (Namespace::Control, "ProgressBar"),
        Button => (Namespace::Control, "Button"),
        RadioButton => (Namespace::Control, "RadioButton"),
        RadioMenuItem => (Namespace::Item, "MenuItem"),
        RootPane => (Namespace::Control, "RootPane"),
        RowHeader => (Namespace::Item, "RowHeader"),
        ScrollBar => (Namespace::Control, "ScrollBar"),
        ScrollPane => (Namespace::Control, "ScrollPane"),
        Separator => (Namespace::Control, "Separator"),
        Slider => (Namespace::Control, "Slider"),
        SpinButton => (Namespace::Control, "SpinButton"),
        SplitPane => (Namespace::Control, "SplitPane"),
        StatusBar => (Namespace::Control, "StatusBar"),
        Table => (Namespace::Control, "Table"),
        TableCell => (Namespace::Item, "TableCell"),
        TableColumnHeader => (Namespace::Item, "TableColumnHeader"),
        TableRowHeader => (Namespace::Item, "TableRowHeader"),
        TearoffMenuItem => (Namespace::Item, "TearoffMenuItem"),
        Terminal => (Namespace::Control, "Terminal"),
        Text => (Namespace::Control, "Text"),
        ToggleButton => (Namespace::Control, "ToggleButton"),
        ToolBar => (Namespace::Control, "ToolBar"),
        ToolTip => (Namespace::Control, "ToolTip"),
        Tree => (Namespace::Control, "Tree"),
        TreeTable => (Namespace::Control, "TreeTable"),
        Unknown => (Namespace::Control, "Unknown"),
        Viewport => (Namespace::Control, "Viewport"),
        Window => (Namespace::Control, "Window"),
        Extended => (Namespace::Control, "Extended"),
        Header => (Namespace::Control, "Header"),
        Footer => (Namespace::Control, "Footer"),
        Paragraph => (Namespace::Control, "Paragraph"),
        Ruler => (Namespace::Control, "Ruler"),
        Application => (Namespace::App, "Application"),
        Autocomplete => (Namespace::Control, "Autocomplete"),
        Editbar => (Namespace::Control, "Editbar"),
        Embedded => (Namespace::Control, "Embedded"),
        Entry => (Namespace::Control, "Entry"),
        CHART => (Namespace::Control, "Chart"),
        Caption => (Namespace::Control, "Caption"),
        DocumentFrame => (Namespace::Control, "DocumentFrame"),
        Heading => (Namespace::Control, "Heading"),
        Page => (Namespace::Control, "Page"),
        Section => (Namespace::Control, "Section"),
        RedundantObject => (Namespace::Control, "RedundantObject"),
        Form => (Namespace::Control, "Form"),
        Link => (Namespace::Control, "Link"),
        InputMethodWindow => (Namespace::Control, "InputMethodWindow"),
        TableRow => (Namespace::Item, "TableRow"),
        TreeItem => (Namespace::Item, "TreeItem"),
        DocumentSpreadsheet => (Namespace::Control, "DocumentSpreadsheet"),
        DocumentPresentation => (Namespace::Control, "DocumentPresentation"),
        DocumentText => (Namespace::Control, "DocumentText"),
        DocumentWeb => (Namespace::Control, "DocumentWeb"),
        DocumentEmail => (Namespace::Control, "DocumentEmail"),
        Comment => (Namespace::Control, "Comment"),
        ListBox => (Namespace::Control, "ListBox"),
        Grouping => (Namespace::Control, "Grouping"),
        ImageMap => (Namespace::Control, "ImageMap"),
        Notification => (Namespace::Control, "Notification"),
        InfoBar => (Namespace::Control, "InfoBar"),
        LevelBar => (Namespace::Control, "LevelBar"),
        TitleBar => (Namespace::Control, "TitleBar"),
        BlockQuote => (Namespace::Control, "BlockQuote"),
        Audio => (Namespace::Control, "Audio"),
        Video => (Namespace::Control, "Video"),
        Definition => (Namespace::Control, "Definition"),
        Article => (Namespace::Control, "Article"),
        Landmark => (Namespace::Control, "Landmark"),
        Log => (Namespace::Control, "Log"),
        Marquee => (Namespace::Control, "Marquee"),
        Math => (Namespace::Control, "Math"),
        Rating => (Namespace::Control, "Rating"),
        Timer => (Namespace::Control, "Timer"),
        Static => (Namespace::Control, "Static"),
        MathFraction => (Namespace::Control, "MathFraction"),
        MathRoot => (Namespace::Control, "MathRoot"),
        Subscript => (Namespace::Control, "Subscript"),
        Superscript => (Namespace::Control, "Superscript"),
        DescriptionList => (Namespace::Control, "DescriptionList"),
        DescriptionTerm => (Namespace::Item, "DescriptionTerm"),
        DescriptionValue => (Namespace::Item, "DescriptionValue"),
        Footnote => (Namespace::Control, "Footnote"),
        ContentDeletion => (Namespace::Control, "ContentDeletion"),
        ContentInsertion => (Namespace::Control, "ContentInsertion"),
        Mark => (Namespace::Control, "Mark"),
        Suggestion => (Namespace::Control, "Suggestion"),
        PushButtonMenu => (Namespace::Control, "PushButtonMenu"),
    };
    (namespace, name.to_string())
}

fn map_role_with_interfaces(role: Role, interfaces: Option<InterfaceSet>) -> (Namespace, String) {
    if interfaces.map(|ifaces| ifaces.contains(Interface::Application)).unwrap_or(false) {
        return (Namespace::App, "Application".to_string());
    }
    map_role(role)
}

struct AttrsIter {
    idx: u8,
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
    namespace: Namespace,
    rid_str: String,
    owner: Option<Weak<dyn UiNode>>,
    interfaces: Option<InterfaceSet>,
    supports_component: bool,
    native_cache: Option<Vec<Arc<dyn UiAttribute>>>,
    native_pos: usize,
}

impl AttrsIter {
    fn new(node: &AtspiNode, owner: Option<Arc<dyn UiNode>>, rid_str: String) -> Self {
        let owner_weak = owner.as_ref().map(Arc::downgrade);
        let interfaces = node.resolve_interfaces();
        let supports_component = interfaces.map(|ifaces| ifaces.contains(Interface::Component)).unwrap_or(false);
        Self {
            idx: 0,
            conn: node.conn.clone(),
            obj: node.obj.clone(),
            namespace: node.namespace(),
            rid_str,
            owner: owner_weak,
            interfaces,
            supports_component,
            native_cache: None,
            native_pos: 0,
        }
    }
}

impl Iterator for AttrsIter {
    type Item = Arc<dyn UiAttribute>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = match self.idx {
                0 => Some(Arc::new(RoleAttr { namespace: self.namespace, role: self.role() }) as Arc<dyn UiAttribute>),
                1 => Some(Arc::new(NameAttr {
                    namespace: self.namespace,
                    conn: self.conn.clone(),
                    obj: self.obj.clone(),
                }) as Arc<dyn UiAttribute>),
                2 => {
                    let present = self.owner.as_ref().and_then(|w| w.upgrade()).and_then(|n| n.id()).is_some();
                    if present {
                        Some(Arc::new(IdAttr {
                            namespace: self.namespace,
                            conn: self.conn.clone(),
                            obj: self.obj.clone(),
                        }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                3 => Some(Arc::new(RuntimeIdAttr { namespace: self.namespace, rid: self.rid_str.clone() })
                    as Arc<dyn UiAttribute>),
                4 => Some(Arc::new(TechnologyAttr { namespace: self.namespace }) as Arc<dyn UiAttribute>),
                5 => {
                    if self.supports_component {
                        Some(Arc::new(BoundsAttr {
                            namespace: self.namespace,
                            conn: self.conn.clone(),
                            obj: self.obj.clone(),
                            supports_component: self.supports_component,
                        }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                6 => {
                    if self.supports_component {
                        Some(Arc::new(ActivationPointAttr {
                            namespace: self.namespace,
                            conn: self.conn.clone(),
                            obj: self.obj.clone(),
                            supports_component: self.supports_component,
                        }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                7 => {
                    if self.supports_component {
                        Some(Arc::new(IsEnabledAttr {
                            namespace: self.namespace,
                            conn: self.conn.clone(),
                            obj: self.obj.clone(),
                        }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                8 => {
                    if self.supports_component {
                        Some(Arc::new(IsVisibleAttr {
                            namespace: self.namespace,
                            conn: self.conn.clone(),
                            obj: self.obj.clone(),
                        }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                9 => {
                    if self.supports_component {
                        Some(Arc::new(IsOffscreenAttr {
                            namespace: self.namespace,
                            conn: self.conn.clone(),
                            obj: self.obj.clone(),
                        }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                10 => {
                    if self.supports_component {
                        Some(Arc::new(IsFocusedAttr {
                            namespace: self.namespace,
                            conn: self.conn.clone(),
                            obj: self.obj.clone(),
                        }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                11 => {
                    if self.supports_component {
                        Some(Arc::new(SupportedPatternsAttr {
                            namespace: self.namespace,
                            conn: self.conn.clone(),
                            obj: self.obj.clone(),
                        }) as Arc<dyn UiAttribute>)
                    } else {
                        None
                    }
                }
                12 => {
                    if self.native_cache.is_none() {
                        let attrs_pairs = resolve_attributes(self.conn.as_ref(), &self.obj);
                        let attrs: Vec<Arc<dyn UiAttribute>> = collect_interface_native_properties(
                            self.conn.as_ref(),
                            &self.obj,
                            self.interfaces,
                            attrs_pairs.as_deref(),
                        )
                        .into_iter()
                        .map(|(name, value)| Arc::new(NativePropAttr { name, value }) as Arc<dyn UiAttribute>)
                        .collect();
                        self.native_cache = Some(attrs);
                        self.native_pos = 0;
                    }
                    match self.native_cache.as_ref().and_then(|v| v.get(self.native_pos)).cloned() {
                        Some(attr) => {
                            self.native_pos += 1;
                            Some(attr)
                        }
                        None => None,
                    }
                }
                _ => None,
            };

            self.idx = self.idx.saturating_add(1);
            match item {
                Some(attr) => return Some(attr),
                None => {
                    if self.idx > 12 {
                        if let Some(list) = self.native_cache.as_ref() {
                            if self.native_pos < list.len() {
                                self.idx -= 1;
                                let attr = list[self.native_pos].clone();
                                self.native_pos += 1;
                                return Some(attr);
                            }
                        }
                        return None;
                    }
                    continue;
                }
            }
        }
    }
}

impl AttrsIter {
    fn role(&self) -> String {
        let role = accessible_proxy(self.conn.as_ref(), &self.obj)
            .and_then(|proxy| block_on(proxy.get_role()).ok())
            .unwrap_or(Role::Invalid);
        let (_ns, role_name) = map_role_with_interfaces(role, self.interfaces);
        role_name
    }
}

struct RoleAttr {
    namespace: Namespace,
    role: String,
}

impl UiAttribute for RoleAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        common::ROLE
    }

    fn value(&self) -> UiValue {
        UiValue::from(self.role.clone())
    }
}

struct NameAttr {
    namespace: Namespace,
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
}

impl UiAttribute for NameAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        common::NAME
    }

    fn value(&self) -> UiValue {
        UiValue::from(resolve_name(self.conn.as_ref(), &self.obj).unwrap_or_default())
    }
}

struct IdAttr {
    namespace: Namespace,
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
}

impl UiAttribute for IdAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        common::ID
    }

    fn value(&self) -> UiValue {
        UiValue::from(resolve_id(self.conn.as_ref(), &self.obj).unwrap_or_default())
    }
}

struct RuntimeIdAttr {
    namespace: Namespace,
    rid: String,
}

impl UiAttribute for RuntimeIdAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        common::RUNTIME_ID
    }

    fn value(&self) -> UiValue {
        UiValue::from(self.rid.clone())
    }
}

struct TechnologyAttr {
    namespace: Namespace,
}

impl UiAttribute for TechnologyAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        common::TECHNOLOGY
    }

    fn value(&self) -> UiValue {
        UiValue::from(TECHNOLOGY)
    }
}

struct BoundsAttr {
    namespace: Namespace,
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
    supports_component: bool,
}

impl UiAttribute for BoundsAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        element::BOUNDS
    }

    fn value(&self) -> UiValue {
        if !self.supports_component {
            return UiValue::from(Rect::new(0.0, 0.0, 0.0, 0.0));
        }
        let rect = component_proxy(self.conn.as_ref(), &self.obj)
            .and_then(|proxy| block_on(proxy.get_extents(CoordType::Screen)).ok())
            .map(|(x, y, w, h)| Rect::new(x as f64, y as f64, w as f64, h as f64))
            .unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0));
        UiValue::from(rect)
    }
}

struct ActivationPointAttr {
    namespace: Namespace,
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
    supports_component: bool,
}

impl UiAttribute for ActivationPointAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        activation_target::ACTIVATION_POINT
    }

    fn value(&self) -> UiValue {
        if !self.supports_component {
            return UiValue::from(Rect::new(0.0, 0.0, 0.0, 0.0).center());
        }
        let rect = component_proxy(self.conn.as_ref(), &self.obj)
            .and_then(|proxy| block_on(proxy.get_extents(CoordType::Screen)).ok())
            .map(|(x, y, w, h)| Rect::new(x as f64, y as f64, w as f64, h as f64))
            .unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0));
        UiValue::from(rect.center())
    }
}

struct IsEnabledAttr {
    namespace: Namespace,
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
}

impl UiAttribute for IsEnabledAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        element::IS_ENABLED
    }

    fn value(&self) -> UiValue {
        let enabled = accessible_proxy(self.conn.as_ref(), &self.obj)
            .and_then(|proxy| block_on(proxy.get_state()).ok())
            .map(|state| state.contains(State::Enabled) || state.contains(State::Sensitive))
            .unwrap_or(false);
        UiValue::from(enabled)
    }
}

struct IsVisibleAttr {
    namespace: Namespace,
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
}

impl UiAttribute for IsVisibleAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        element::IS_VISIBLE
    }

    fn value(&self) -> UiValue {
        let visible = accessible_proxy(self.conn.as_ref(), &self.obj)
            .and_then(|proxy| block_on(proxy.get_state()).ok())
            .map(|state| state.contains(State::Visible) || state.contains(State::Showing))
            .unwrap_or(false);
        UiValue::from(visible)
    }
}

struct IsOffscreenAttr {
    namespace: Namespace,
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
}

impl UiAttribute for IsOffscreenAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        element::IS_OFFSCREEN
    }

    fn value(&self) -> UiValue {
        let visible = accessible_proxy(self.conn.as_ref(), &self.obj)
            .and_then(|proxy| block_on(proxy.get_state()).ok())
            .map(|state| state.contains(State::Visible) || state.contains(State::Showing))
            .unwrap_or(false);
        UiValue::from(!visible)
    }
}

struct IsFocusedAttr {
    namespace: Namespace,
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
}

impl UiAttribute for IsFocusedAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        focusable::IS_FOCUSED
    }

    fn value(&self) -> UiValue {
        let focused = accessible_proxy(self.conn.as_ref(), &self.obj)
            .and_then(|proxy| block_on(proxy.get_state()).ok())
            .map(|state| state.contains(State::Focused))
            .unwrap_or(false);
        UiValue::from(focused)
    }
}

struct SupportedPatternsAttr {
    namespace: Namespace,
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
}

impl UiAttribute for SupportedPatternsAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        common::SUPPORTED_PATTERNS
    }

    fn value(&self) -> UiValue {
        let state = accessible_proxy(self.conn.as_ref(), &self.obj).and_then(|proxy| block_on(proxy.get_state()).ok());
        let focusable = state.map(|s| s.contains(State::Focusable) || s.contains(State::Focused)).unwrap_or(false);
        let mut patterns = Vec::new();
        if focusable {
            patterns.push(PatternId::from("Focusable"));
        }
        supported_patterns_value(&patterns)
    }
}

struct NativePropAttr {
    name: String,
    value: UiValue,
}

impl UiAttribute for NativePropAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Native
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn value(&self) -> UiValue {
        self.value.clone()
    }
}
