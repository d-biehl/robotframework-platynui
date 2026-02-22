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
use platynui_core::platform::{WindowId, window_managers};
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::attribute_names::{activation_target, application, common, element, focusable, window_surface};
use platynui_core::ui::{
    FocusableAction, Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiPattern, UiValue, WindowSurfaceActions,
    supported_patterns_value,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tracing::{trace, warn};
use zbus::proxy::CacheProperties;

use crate::clearable_cell::ClearableCell;
use crate::error::AtspiError;
use crate::timeout::block_on_timeout_call;

const NULL_PATH: &str = "/org/a11y/atspi/accessible/null";
const TECHNOLOGY: &str = "AT-SPI2";

pub struct AtspiNode {
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    self_weak: OnceLock<Weak<dyn UiNode>>,
    runtime_id: OnceLock<RuntimeId>,
    pub(crate) role: OnceLock<String>,
    pub(crate) namespace: OnceLock<Namespace>,
    state: ClearableCell<Option<StateSet>>,
    pub(crate) interfaces: ClearableCell<Option<InterfaceSet>>,
    /// Cached name resolved from the accessibility bus.
    pub(crate) cached_name: ClearableCell<Option<String>>,
    /// Cached child count (from AT-SPI `ChildCount` property).
    pub(crate) cached_child_count: ClearableCell<Option<i32>>,
    /// Cached process ID resolved from D-Bus connection credentials.
    cached_process_id: ClearableCell<Option<u32>>,
}

impl AtspiNode {
    pub fn new(conn: Arc<AccessibilityConnection>, obj: ObjectRefOwned, parent: Option<&Arc<dyn UiNode>>) -> Arc<Self> {
        let node = Arc::new(Self {
            conn,
            obj,
            parent: Mutex::new(parent.map(Arc::downgrade)),
            self_weak: OnceLock::new(),
            runtime_id: OnceLock::new(),
            role: OnceLock::new(),
            namespace: OnceLock::new(),
            state: ClearableCell::new(),
            interfaces: ClearableCell::new(),
            cached_name: ClearableCell::new(),
            cached_child_count: ClearableCell::new(),
            cached_process_id: ClearableCell::new(),
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
        let Some(proxy) = self.accessible() else {
            let _ = self.role.set("Unknown".to_string());
            let _ = self.namespace.set(Namespace::Control);
            return;
        };
        // Resolve interfaces via the same proxy when not yet cached.
        if !self.interfaces.is_set() {
            let ifaces = block_on_timeout_call(proxy.get_interfaces()).and_then(|r| r.ok());
            self.interfaces.set(ifaces);
        }
        let interfaces = self.interfaces.get().flatten();
        let role = block_on_timeout_call(proxy.get_role()).and_then(|r| r.ok()).unwrap_or(Role::Invalid);
        let (namespace, role_name) = map_role_with_interfaces(role, interfaces);
        let _ = self.namespace.set(namespace);
        let _ = self.role.set(role_name);
    }

    fn resolve_state(&self) -> Option<StateSet> {
        self.state.get_or_init(|| {
            self.accessible().and_then(|proxy| block_on_timeout_call(proxy.get_state()).and_then(|r| r.ok()))
        })
    }

    fn resolve_interfaces(&self) -> Option<InterfaceSet> {
        self.interfaces.get_or_init(|| {
            self.accessible().and_then(|proxy| block_on_timeout_call(proxy.get_interfaces()).and_then(|r| r.ok()))
        })
    }

    fn resolve_name(&self) -> Option<String> {
        self.cached_name.get_or_init(|| resolve_name(self.conn.as_ref(), &self.obj))
    }

    fn supports_component(&self) -> bool {
        self.resolve_interfaces().map(|ifaces| ifaces.contains(Interface::Component)).unwrap_or(false)
    }

    fn is_application(&self) -> bool {
        self.resolve_interfaces().map(|ifaces| ifaces.contains(Interface::Application)).unwrap_or(false)
    }

    /// Returns `true` if this node represents a top-level window surface
    /// (Frame, Window, or Dialog).
    fn is_window_surface(&self) -> bool {
        let role = self.role();
        matches!(role, "Frame" | "Window" | "Dialog")
    }

    /// Resolve the Unix process ID of the application owning this node's
    /// D-Bus bus name.  The result is cached in `cached_process_id`.
    fn resolve_process_id(&self) -> Option<u32> {
        self.cached_process_id.get_or_init(|| {
            let bus_name = self.obj.name_as_str()?;
            let conn = self.conn.connection();
            block_on_timeout_call(async {
                let dbus = zbus::fdo::DBusProxy::new(conn).await.ok()?;
                dbus.get_connection_unix_process_id(zbus::names::BusName::try_from(bus_name).ok()?).await.ok()
            })
            .flatten()
        })
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
        self.resolve_name().unwrap_or_default()
    }

    fn runtime_id(&self) -> &RuntimeId {
        self.runtime_id.get_or_init(|| RuntimeId::from(object_runtime_id(&self.obj)))
    }

    fn id(&self) -> Option<String> {
        // For Application nodes, prefer the process ID as a stable
        // identifier since accessible-id is typically empty.
        if self.is_application()
            && let Some(pid) = self.resolve_process_id()
        {
            return Some(pid.to_string());
        }
        resolve_id(self.conn.as_ref(), &self.obj)
    }

    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent.lock().ok()?.clone()
    }

    fn has_children(&self) -> bool {
        let count = self.cached_child_count.get_or_init(|| {
            self.accessible().and_then(|proxy| block_on_timeout_call(proxy.child_count()).and_then(|r| r.ok()))
        });
        count.map(|c| c > 0).unwrap_or(false)
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        let parent_path = self.obj.path_as_str().to_string();
        let parent_bus = self.obj.name_as_str().unwrap_or("<unknown>").to_string();
        let children_start = std::time::Instant::now();

        let Some(children) =
            self.accessible().and_then(|proxy| block_on_timeout_call(proxy.get_children()).and_then(|r| r.ok()))
        else {
            warn!(bus = %parent_bus, path = %parent_path, "children: get_children failed or timed out");
            return Box::new(std::iter::empty());
        };

        let child_count = children.len();
        let get_children_elapsed = children_start.elapsed();
        trace!(
            bus = %parent_bus,
            path = %parent_path,
            count = child_count,
            elapsed_ms = get_children_elapsed.as_millis() as u64,
            "children: fetched child list",
        );
        if get_children_elapsed.as_millis() > 1000 {
            warn!(
                bus = %parent_bus,
                path = %parent_path,
                elapsed_ms = get_children_elapsed.as_millis() as u64,
                "children: SLOW get_children (>1000ms)",
            );
        }

        let parent = self.self_weak.get().and_then(|weak| weak.upgrade());
        let conn = self.conn.clone();
        Box::new(children.into_iter().filter_map(move |child| {
            if AtspiNode::is_null_object(&child) {
                return None;
            }
            Some(AtspiNode::new(conn.clone(), child, parent.as_ref()) as Arc<dyn UiNode>)
        }))
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        let rid_str = self.runtime_id().as_str().to_string();
        Box::new(AttrsIter::new(self, rid_str))
    }

    fn supported_patterns(&self) -> Vec<PatternId> {
        let mut patterns = Vec::new();
        if self.focusable() {
            patterns.push(PatternId::from("Focusable"));
        }
        if self.is_window_surface() {
            patterns.push(PatternId::from("WindowSurface"));
        }
        patterns
    }

    fn pattern_by_id(&self, pattern: &PatternId) -> Option<Arc<dyn UiPattern>> {
        match pattern.as_str() {
            "Focusable" => {
                if !self.focusable() {
                    return None;
                }
                let conn = self.conn.clone();
                let obj = self.obj.clone();
                let action = FocusableAction::new(move || grab_focus(conn.as_ref(), &obj).map_err(Into::into));
                Some(Arc::new(action) as Arc<dyn UiPattern>)
            }
            "WindowSurface" => {
                if !self.is_window_surface() {
                    return None;
                }
                let weak1 = self.self_weak.get().cloned();
                let weak2 = weak1.clone();
                let weak3 = weak1.clone();
                let pattern = WindowSurfaceActions::new()
                    .with_activate(move || {
                        let node = weak1.as_ref().and_then(Weak::upgrade).ok_or(AtspiError::NodeDropped)?;
                        activate_window(node.as_ref()).map_err(Into::into)
                    })
                    .with_close(move || {
                        let node = weak2.as_ref().and_then(Weak::upgrade).ok_or(AtspiError::NodeDropped)?;
                        close_window(node.as_ref()).map_err(Into::into)
                    })
                    .with_accepts_user_input(move || {
                        let node = weak3.as_ref().and_then(Weak::upgrade);
                        Ok(node.and_then(|n| is_active_window(n.as_ref())))
                    });
                Some(Arc::new(pattern) as Arc<dyn UiPattern>)
            }
            _ => None,
        }
    }

    fn is_valid(&self) -> bool {
        // Cheap liveness probe: if we can still read the role, the D-Bus peer
        // is alive.  Returns `false` for zombie nodes (e.g. crashed apps).
        self.accessible().and_then(|proxy| block_on_timeout_call(proxy.get_role())).and_then(|r| r.ok()).is_some()
    }

    fn invalidate(&self) {
        self.state.clear();
        self.interfaces.clear();
        self.cached_name.clear();
        self.cached_child_count.clear();
        self.cached_process_id.clear();
    }
}

fn accessible_proxy<'a>(conn: &'a AccessibilityConnection, obj: &'a ObjectRefOwned) -> Option<AccessibleProxy<'a>> {
    let name = obj.name_as_str()?;
    let builder = AccessibleProxy::builder(conn.connection())
        .cache_properties(CacheProperties::No)
        .destination(name)
        .ok()?
        .path(obj.path_as_str())
        .ok()?;
    block_on_timeout_call(builder.build()).and_then(|r| r.ok())
}

fn component_proxy<'a>(conn: &'a AccessibilityConnection, obj: &'a ObjectRefOwned) -> Option<ComponentProxy<'a>> {
    let name = obj.name_as_str()?;
    let builder = ComponentProxy::builder(conn.connection())
        .cache_properties(CacheProperties::No)
        .destination(name)
        .ok()?
        .path(obj.path_as_str())
        .ok()?;
    block_on_timeout_call(builder.build()).and_then(|r| r.ok())
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
            block_on_timeout_call(builder.build()).and_then(|r| r.ok())
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

fn grab_focus(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Result<(), AtspiError> {
    let proxy = component_proxy(conn, obj).ok_or(AtspiError::InterfaceMissing("Component"))?;
    let ok = block_on_timeout_call(proxy.grab_focus())
        .ok_or(AtspiError::timeout("grab_focus"))?
        .map_err(|e| AtspiError::dbus("grab_focus", e))?;
    if ok { Ok(()) } else { Err(AtspiError::FocusFailed) }
}

/// Resolve the native window ID for this AT-SPI window node using the
/// registered [`WindowManager`].
///
/// The node itself carries PID (via `native:ProcessId`) and window name
/// (via `UiNode::name()`) so the platform-specific window manager can
/// match the correct top-level window.
fn resolve_window_id(node: &dyn UiNode) -> Result<WindowId, AtspiError> {
    let wm = window_managers().next().ok_or(AtspiError::NoWindowManager)?;
    wm.resolve_window(node).map_err(|e| AtspiError::dbus("resolve_window", e))
}

/// Bring a window to the foreground via the registered [`WindowManager`].
fn activate_window(node: &dyn UiNode) -> Result<(), AtspiError> {
    let wid = resolve_window_id(node)?;
    let wm = window_managers().next().ok_or(AtspiError::NoWindowManager)?;
    wm.activate(wid).map_err(|e| AtspiError::dbus("activate_window", e))
}

/// Close a window via the registered [`WindowManager`].
fn close_window(node: &dyn UiNode) -> Result<(), AtspiError> {
    let wid = resolve_window_id(node)?;
    let wm = window_managers().next().ok_or(AtspiError::NoWindowManager)?;
    wm.close(wid).map_err(|e| AtspiError::dbus("close_window", e))
}

/// Check whether a window is the currently active (foreground) window.
fn is_active_window(node: &dyn UiNode) -> Option<bool> {
    let wid = resolve_window_id(node).ok()?;
    let wm = window_managers().next()?;
    wm.is_active(wid).ok()
}

fn object_runtime_id(obj: &ObjectRefOwned) -> String {
    let name = obj.name_as_str().unwrap_or_default();
    format!("atspi://{}{}", name, obj.path_as_str())
}

pub(crate) fn normalize_value(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

fn pick_attr_value(attrs: &[(String, String)], keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some((_name, value)) = attrs.iter().find(|(name, _)| name.eq_ignore_ascii_case(key))
            && let Some(value) = normalize_value(value.clone())
        {
            return Some(value);
        }
    }
    None
}

fn resolve_attributes(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Option<Vec<(String, String)>> {
    let proxy = accessible_proxy(conn, obj)?;
    let mut pairs: Vec<(String, String)> =
        block_on_timeout_call(proxy.get_attributes()).and_then(|r| r.ok())?.into_iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    Some(pairs)
}

fn resolve_name(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Option<String> {
    if let Some(Ok(name)) = accessible_proxy(conn, obj).and_then(|p| block_on_timeout_call(p.name()))
        && let Some(value) = normalize_value(name)
    {
        return Some(value);
    }
    resolve_attributes(conn, obj)
        .and_then(|attrs| pick_attr_value(&attrs, &["accessible-name", "name", "label", "title"]))
}

fn resolve_id(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Option<String> {
    if let Some(Ok(id)) = accessible_proxy(conn, obj).and_then(|p| block_on_timeout_call(p.accessible_id()))
        && let Some(value) = normalize_value(id)
    {
        return Some(value);
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

fn actions_value(actions: Vec<AtspiAction>, names: &[Option<String>]) -> UiValue {
    let values = actions
        .into_iter()
        .enumerate()
        .map(|(i, action)| {
            let mut map = BTreeMap::new();
            // Machine-readable (non-localized) name via `GetName`.
            if let Some(Some(name)) = names.get(i) {
                map.insert("Name".to_string(), UiValue::from(name.clone()));
            }
            map.insert("LocalizedName".to_string(), UiValue::from(action.name));
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

pub(crate) fn map_role_with_interfaces(role: Role, interfaces: Option<InterfaceSet>) -> (Namespace, String) {
    if interfaces.map(|ifaces| ifaces.contains(Interface::Application)).unwrap_or(false) {
        return (Namespace::App, "Application".to_string());
    }
    map_role(role)
}

struct AttrsIter {
    idx: u8,
    namespace: Namespace,
    rid_str: String,
    supports_component: bool,
    /// Pre-resolved role string (avoids re-querying D-Bus).
    role: String,
    /// Shared lazy-resolution context for standard attributes.
    /// D-Bus calls are deferred until `.value()` and cached via `OnceLock`.
    ctx: Arc<LazyNodeData>,
    /// Cached process ID (only set for Application nodes).
    process_id: Option<u32>,
    /// Whether this node is a top-level window (Frame/Window/Dialog).
    is_window_surface: bool,
    /// Pre-filtered list of native property names applicable to this node.
    native_props: Vec<&'static str>,
    /// Current index into `native_props`.
    native_idx: usize,
}

impl AttrsIter {
    fn new(node: &AtspiNode, rid_str: String) -> Self {
        let supports_component = node.supports_component();
        let role = node.role().to_string();
        let ctx = Arc::new(LazyNodeData::new(
            node.conn.clone(),
            node.obj.clone(),
            role.clone(),
            node.self_weak.get().cloned(),
        ));
        // Standard attributes always live in the Control namespace,
        // regardless of the node's own namespace (e.g. App for
        // Application nodes).
        let namespace = Namespace::Control;

        // Build the list of applicable native property names based on
        // supported interfaces.  No D-Bus calls here — the interface set
        // is already cached on AtspiNode.
        let interfaces = node.resolve_interfaces();
        let mut native_props: Vec<&'static str> = vec![
            "Accessible.Name",
            "Accessible.Description",
            "Accessible.HelpText",
            "Accessible.Locale",
            "Accessible.Role",
            "Accessible.RoleName",
            "Accessible.LocalizedRoleName",
            "Accessible.AccessibleId",
            "Accessible.Parent",
            "Accessible.ChildCount",
            "Accessible.IndexInParent",
            "Accessible.Interfaces",
            "Accessible.State",
            "Accessible.RelationSet",
            "Accessible.Application",
            "Accessible.Attributes",
        ];
        if let Some(ifaces) = interfaces {
            if ifaces.contains(Interface::Action) {
                native_props.extend_from_slice(&["Action.NActions", "Action.Actions"]);
            }
            if ifaces.contains(Interface::Application) {
                native_props.extend_from_slice(&[
                    "Application.Id",
                    "Application.Version",
                    "Application.ToolkitName",
                    "Application.AtspiVersion",
                    "Application.BusAddress",
                ]);
            }
            if ifaces.contains(Interface::Collection) {
                native_props.push("Collection.ActiveDescendant");
            }
            if ifaces.contains(Interface::Component) {
                native_props.extend_from_slice(&[
                    "Component.Alpha",
                    "Component.Extents.Screen",
                    "Component.Extents.Window",
                    "Component.Extents.Parent",
                    "Component.Position.Screen",
                    "Component.Position.Window",
                    "Component.Position.Parent",
                    "Component.Size",
                    "Component.Layer",
                    "Component.MDIZOrder",
                ]);
            }
            if ifaces.contains(Interface::Document) {
                native_props.extend_from_slice(&[
                    "Document.PageCount",
                    "Document.CurrentPageNumber",
                    "Document.Locale",
                    "Document.Attributes",
                ]);
            }
            if ifaces.contains(Interface::Hyperlink) {
                native_props.extend_from_slice(&[
                    "Hyperlink.IsValid",
                    "Hyperlink.EndIndex",
                    "Hyperlink.StartIndex",
                    "Hyperlink.NAnchors",
                ]);
            }
            if ifaces.contains(Interface::Hypertext) {
                native_props.push("Hypertext.NLinks");
            }
            if ifaces.contains(Interface::Image) {
                native_props.extend_from_slice(&[
                    "Image.Description",
                    "Image.Locale",
                    "Image.Extents.Screen",
                    "Image.Extents.Window",
                    "Image.Extents.Parent",
                    "Image.Position.Screen",
                    "Image.Position.Window",
                    "Image.Position.Parent",
                    "Image.Size",
                ]);
            }
            if ifaces.contains(Interface::Selection) {
                native_props.push("Selection.NSelectedChildren");
            }
            if ifaces.contains(Interface::Table) {
                native_props.extend_from_slice(&[
                    "Table.Caption",
                    "Table.Summary",
                    "Table.NColumns",
                    "Table.NRows",
                    "Table.NSelectedColumns",
                    "Table.NSelectedRows",
                    "Table.SelectedRows",
                    "Table.SelectedColumns",
                ]);
            }
            if ifaces.contains(Interface::TableCell) {
                native_props.extend_from_slice(&[
                    "TableCell.ColumnSpan",
                    "TableCell.RowSpan",
                    "TableCell.Position",
                    "TableCell.Table",
                ]);
            }
            if ifaces.contains(Interface::Text) {
                native_props.extend_from_slice(&[
                    "Text.CharacterCount",
                    "Text.CaretOffset",
                    "Text.NSelections",
                    "Text.DefaultAttributes",
                    "Text.DefaultAttributeSet",
                ]);
            }
            if ifaces.contains(Interface::Value) {
                native_props.extend_from_slice(&[
                    "Value.CurrentValue",
                    "Value.MaximumValue",
                    "Value.MinimumValue",
                    "Value.MinimumIncrement",
                    "Value.Text",
                ]);
            }
        }

        let process_id = if node.is_application() { node.resolve_process_id() } else { None };

        let is_window_surface = node.is_window_surface();

        Self {
            idx: 0,
            namespace,
            rid_str,
            supports_component,
            role,
            ctx,
            process_id,
            is_window_surface,
            native_props,
            native_idx: 0,
        }
    }
}

impl Iterator for AttrsIter {
    type Item = Arc<dyn UiAttribute>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item: Option<Arc<dyn UiAttribute>> = match self.idx {
                0 => Some(Arc::new(RoleAttr { namespace: self.namespace, role: self.role.clone() })),
                1 => Some(Arc::new(LazyStdAttr {
                    namespace: self.namespace,
                    kind: StdAttrKind::Name,
                    ctx: self.ctx.clone(),
                })),
                2 => Some(Arc::new(LazyStdAttr {
                    namespace: self.namespace,
                    kind: StdAttrKind::Id,
                    ctx: self.ctx.clone(),
                })),
                3 => Some(Arc::new(RuntimeIdAttr { namespace: self.namespace, rid: self.rid_str.clone() })),
                4 => Some(Arc::new(TechnologyAttr { namespace: self.namespace })),
                5 => {
                    if self.supports_component {
                        Some(Arc::new(LazyStdAttr {
                            namespace: self.namespace,
                            kind: StdAttrKind::Bounds,
                            ctx: self.ctx.clone(),
                        }))
                    } else {
                        None
                    }
                }
                6 => {
                    if self.supports_component {
                        Some(Arc::new(LazyStdAttr {
                            namespace: self.namespace,
                            kind: StdAttrKind::ActivationPoint,
                            ctx: self.ctx.clone(),
                        }))
                    } else {
                        None
                    }
                }
                7 => {
                    if self.supports_component {
                        Some(Arc::new(LazyStdAttr {
                            namespace: self.namespace,
                            kind: StdAttrKind::IsEnabled,
                            ctx: self.ctx.clone(),
                        }))
                    } else {
                        None
                    }
                }
                8 => {
                    if self.supports_component {
                        Some(Arc::new(LazyStdAttr {
                            namespace: self.namespace,
                            kind: StdAttrKind::IsVisible,
                            ctx: self.ctx.clone(),
                        }))
                    } else {
                        None
                    }
                }
                9 => {
                    if self.supports_component {
                        Some(Arc::new(LazyStdAttr {
                            namespace: self.namespace,
                            kind: StdAttrKind::IsOffscreen,
                            ctx: self.ctx.clone(),
                        }))
                    } else {
                        None
                    }
                }
                10 => {
                    if self.supports_component {
                        Some(Arc::new(LazyStdAttr {
                            namespace: self.namespace,
                            kind: StdAttrKind::IsFocused,
                            ctx: self.ctx.clone(),
                        }))
                    } else {
                        None
                    }
                }
                11 => {
                    if self.supports_component {
                        Some(Arc::new(LazyStdAttr {
                            namespace: self.namespace,
                            kind: StdAttrKind::SupportedPatterns,
                            ctx: self.ctx.clone(),
                        }))
                    } else {
                        None
                    }
                }
                12 => self.process_id.map(|pid| Arc::new(ProcessIdAttr { pid }) as Arc<dyn UiAttribute>),
                13 => {
                    if self.is_window_surface {
                        Some(Arc::new(LazyStdAttr {
                            namespace: self.namespace,
                            kind: StdAttrKind::IsTopmost,
                            ctx: self.ctx.clone(),
                        }))
                    } else {
                        None
                    }
                }
                14 => {
                    if self.is_window_surface {
                        Some(Arc::new(LazyStdAttr {
                            namespace: self.namespace,
                            kind: StdAttrKind::AcceptsUserInput,
                            ctx: self.ctx.clone(),
                        }))
                    } else {
                        None
                    }
                }
                // Yield lazy native properties — D-Bus is only called
                // when the consumer invokes `.value()` on the attribute.
                _ => {
                    if self.native_idx < self.native_props.len() {
                        let name = self.native_props[self.native_idx];
                        self.native_idx += 1;
                        return Some(Arc::new(LazyNativeAttr {
                            conn: self.ctx.conn.clone(),
                            obj: self.ctx.obj.clone(),
                            name,
                        }));
                    }
                    return None;
                }
            };

            self.idx = self.idx.saturating_add(1);
            match item {
                Some(attr) => return Some(attr),
                None => {
                    if self.idx > 15 {
                        return None;
                    }
                    continue;
                }
            }
        }
    }
}

/// Shared lazy-resolution context for standard attributes.
///
/// D-Bus calls are deferred until first access and cached via `OnceLock`,
/// so multiple attributes that need the same underlying data (e.g. state)
/// share a single D-Bus roundtrip.
struct LazyNodeData {
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
    role: String,
    /// Weak reference to the owning UiNode, used for window manager queries.
    owner: Option<Weak<dyn UiNode>>,
    state: OnceLock<Option<StateSet>>,
    extents: OnceLock<Option<Rect>>,
    name: OnceLock<String>,
    id: OnceLock<Option<String>>,
}

impl LazyNodeData {
    fn new(
        conn: Arc<AccessibilityConnection>,
        obj: ObjectRefOwned,
        role: String,
        owner: Option<Weak<dyn UiNode>>,
    ) -> Self {
        Self {
            conn,
            obj,
            role,
            owner,
            state: OnceLock::new(),
            extents: OnceLock::new(),
            name: OnceLock::new(),
            id: OnceLock::new(),
        }
    }

    fn resolve_state(&self) -> Option<StateSet> {
        *self.state.get_or_init(|| {
            accessible_proxy(&self.conn, &self.obj)
                .and_then(|proxy| block_on_timeout_call(proxy.get_state()).and_then(|r| r.ok()))
        })
    }

    fn resolve_extents(&self) -> Option<Rect> {
        *self.extents.get_or_init(|| {
            component_proxy(&self.conn, &self.obj).and_then(|proxy| {
                block_on_timeout_call(proxy.get_extents(CoordType::Screen))
                    .and_then(|r| r.ok())
                    .map(|(x, y, w, h)| Rect::new(x as f64, y as f64, w as f64, h as f64))
            })
        })
    }

    fn resolve_name(&self) -> &str {
        self.name.get_or_init(|| resolve_name(&self.conn, &self.obj).unwrap_or_default())
    }

    fn resolve_id(&self) -> Option<&str> {
        self.id.get_or_init(|| resolve_id(&self.conn, &self.obj)).as_deref()
    }

    /// Check if this window is the currently active (foreground) window via
    /// the registered [`WindowManager`].  Returns `None` when the
    /// window ID cannot be resolved (e.g. no provider registered, or the node
    /// is not a top-level window).
    fn resolve_is_active_window(&self) -> Option<bool> {
        let node = self.owner.as_ref()?.upgrade()?;
        is_active_window(node.as_ref())
    }
}

/// Discriminant for lazily-evaluated standard attributes.
#[derive(Clone, Copy)]
enum StdAttrKind {
    Name,
    Id,
    Bounds,
    ActivationPoint,
    IsEnabled,
    IsVisible,
    IsOffscreen,
    IsFocused,
    SupportedPatterns,
    IsTopmost,
    AcceptsUserInput,
}

/// A lazily-evaluated standard attribute.
///
/// The attribute's name and namespace are available immediately; the actual
/// D-Bus roundtrip to resolve the value is deferred until
/// [`UiAttribute::value()`] is called.  Multiple attributes sharing the same
/// underlying data (e.g. state-dependent flags) reuse a single
/// [`LazyNodeData`] context so the D-Bus call happens at most once.
struct LazyStdAttr {
    namespace: Namespace,
    kind: StdAttrKind,
    ctx: Arc<LazyNodeData>,
}

impl UiAttribute for LazyStdAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        match self.kind {
            StdAttrKind::Name => common::NAME,
            StdAttrKind::Id => common::ID,
            StdAttrKind::Bounds => element::BOUNDS,
            StdAttrKind::ActivationPoint => activation_target::ACTIVATION_POINT,
            StdAttrKind::IsEnabled => element::IS_ENABLED,
            StdAttrKind::IsVisible => element::IS_VISIBLE,
            StdAttrKind::IsOffscreen => element::IS_OFFSCREEN,
            StdAttrKind::IsFocused => focusable::IS_FOCUSED,
            StdAttrKind::SupportedPatterns => common::SUPPORTED_PATTERNS,
            StdAttrKind::IsTopmost => window_surface::IS_TOPMOST,
            StdAttrKind::AcceptsUserInput => window_surface::ACCEPTS_USER_INPUT,
        }
    }

    fn value(&self) -> UiValue {
        match self.kind {
            StdAttrKind::Name => UiValue::from(self.ctx.resolve_name().to_string()),
            StdAttrKind::Id => UiValue::from(self.ctx.resolve_id().unwrap_or_default().to_string()),
            StdAttrKind::Bounds => {
                let rect = self.ctx.resolve_extents().unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0));
                UiValue::from(rect)
            }
            StdAttrKind::ActivationPoint => {
                let rect = self.ctx.resolve_extents().unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0));
                UiValue::from(rect.center())
            }
            StdAttrKind::IsEnabled => {
                let enabled = self
                    .ctx
                    .resolve_state()
                    .map(|s| s.contains(State::Enabled) || s.contains(State::Sensitive))
                    .unwrap_or(false);
                UiValue::from(enabled)
            }
            StdAttrKind::IsVisible => {
                let visible = self
                    .ctx
                    .resolve_state()
                    .map(|s| s.contains(State::Visible) || s.contains(State::Showing))
                    .unwrap_or(false);
                UiValue::from(visible)
            }
            StdAttrKind::IsOffscreen => {
                let visible = self
                    .ctx
                    .resolve_state()
                    .map(|s| s.contains(State::Visible) || s.contains(State::Showing))
                    .unwrap_or(false);
                UiValue::from(!visible)
            }
            StdAttrKind::IsFocused => {
                let focused = self.ctx.resolve_state().map(|s| s.contains(State::Focused)).unwrap_or(false);
                UiValue::from(focused)
            }
            StdAttrKind::SupportedPatterns => {
                let focusable = self
                    .ctx
                    .resolve_state()
                    .map(|s| s.contains(State::Focusable) || s.contains(State::Focused))
                    .unwrap_or(false);
                let window_surface = matches!(self.ctx.role.as_str(), "Frame" | "Window" | "Dialog");
                let mut patterns = Vec::new();
                if focusable {
                    patterns.push(PatternId::from("Focusable"));
                }
                if window_surface {
                    patterns.push(PatternId::from("WindowSurface"));
                }
                supported_patterns_value(&patterns)
            }
            StdAttrKind::IsTopmost => {
                let active = self.ctx.resolve_is_active_window().unwrap_or(false);
                UiValue::from(active)
            }
            StdAttrKind::AcceptsUserInput => {
                let accepts = self
                    .ctx
                    .resolve_state()
                    .map(|s| s.contains(State::Enabled) || s.contains(State::Sensitive))
                    .unwrap_or(false);
                UiValue::from(accepts)
            }
        }
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

struct ProcessIdAttr {
    pid: u32,
}

impl UiAttribute for ProcessIdAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }

    fn name(&self) -> &str {
        application::PROCESS_ID
    }

    fn value(&self) -> UiValue {
        UiValue::from(self.pid as i64)
    }
}

/// A lazily-evaluated native AT-SPI property attribute.
///
/// Iterating over attributes yields these without any D-Bus calls.
/// The actual D-Bus roundtrip is deferred until [`UiAttribute::value()`] is
/// called, so the XPath engine only pays for properties it actually reads.
struct LazyNativeAttr {
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
    /// Property name in `"Interface.Property"` format.
    name: &'static str,
}

impl UiAttribute for LazyNativeAttr {
    fn namespace(&self) -> Namespace {
        Namespace::Native
    }

    fn name(&self) -> &str {
        self.name
    }

    fn value(&self) -> UiValue {
        match self.name.split_once('.') {
            Some(("Accessible", prop)) => self.fetch_accessible(prop),
            Some(("Action", prop)) => self.fetch_action(prop),
            Some(("Application", prop)) => self.fetch_application(prop),
            Some(("Collection", prop)) => self.fetch_collection(prop),
            Some(("Component", prop)) => self.fetch_component(prop),
            Some(("Document", prop)) => self.fetch_document(prop),
            Some(("Hyperlink", prop)) => self.fetch_hyperlink(prop),
            Some(("Hypertext", prop)) => self.fetch_hypertext(prop),
            Some(("Image", prop)) => self.fetch_image(prop),
            Some(("Selection", prop)) => self.fetch_selection(prop),
            Some(("Table", prop)) => self.fetch_table(prop),
            Some(("TableCell", prop)) => self.fetch_table_cell(prop),
            Some(("Text", prop)) => self.fetch_text(prop),
            Some(("Value", prop)) => self.fetch_value_iface(prop),
            _ => UiValue::Null,
        }
    }
}

/// Fetch a D-Bus property and convert the result directly to [`UiValue`].
///
/// Returns [`UiValue::Null`] on timeout or D-Bus error.
fn fetch<T: Into<UiValue>, E>(future: impl std::future::Future<Output = Result<T, E>>) -> UiValue {
    block_on_timeout_call(future).and_then(|r| r.ok()).map(Into::into).unwrap_or(UiValue::Null)
}

/// Fetch a D-Bus string property, normalise it (trim, reject empty), and
/// convert to [`UiValue`].
fn fetch_str<E>(future: impl std::future::Future<Output = Result<String, E>>) -> UiValue {
    block_on_timeout_call(future)
        .and_then(|r| r.ok())
        .and_then(normalize_value)
        .map(UiValue::from)
        .unwrap_or(UiValue::Null)
}

/// Fetch a D-Bus property and apply a custom mapping to [`UiValue`].
fn fetch_map<T, E>(future: impl std::future::Future<Output = Result<T, E>>, f: impl FnOnce(T) -> UiValue) -> UiValue {
    block_on_timeout_call(future).and_then(|r| r.ok()).map(f).unwrap_or(UiValue::Null)
}

/// Shorthand for converting a D-Bus integer property to `UiValue::Integer`.
fn fetch_int<T: Into<i64>, E>(future: impl std::future::Future<Output = Result<T, E>>) -> UiValue {
    fetch_map(future, |v| UiValue::from(v.into()))
}

/// Convert D-Bus extents `(x, y, w, h)` to a [`Rect`] value.
fn extents_value((x, y, w, h): (i32, i32, i32, i32)) -> UiValue {
    UiValue::from(Rect::new(x as f64, y as f64, w as f64, h as f64))
}

/// Convert D-Bus position `(x, y)` to a [`Point`] value.
fn position_value((x, y): (i32, i32)) -> UiValue {
    UiValue::from(Point::new(x as f64, y as f64))
}

/// Convert D-Bus size `(w, h)` to a [`Size`] value.
fn size_value((w, h): (i32, i32)) -> UiValue {
    UiValue::from(Size::new(w as f64, h as f64))
}

/// Convert an [`ObjectRefOwned`] to its runtime-id string value.
fn object_ref_value(obj: ObjectRefOwned) -> UiValue {
    UiValue::from(object_runtime_id(&obj))
}

/// Fetch a D-Bus property that returns a `HashMap<String, String>` and
/// convert it to a [`UiValue::Object`].  Returns [`UiValue::Null`] when
/// the call fails, times out, or the map is empty.
fn fetch_string_map<E>(
    future: impl std::future::Future<Output = Result<std::collections::HashMap<String, String>, E>>,
) -> UiValue {
    block_on_timeout_call(future)
        .and_then(|r| r.ok())
        .filter(|attrs| !attrs.is_empty())
        .map(|attrs| string_map_object(&attrs))
        .unwrap_or(UiValue::Null)
}

impl LazyNativeAttr {
    fn fetch_accessible(&self, prop: &str) -> UiValue {
        let Some(proxy) = accessible_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "Name" => fetch_str(proxy.name()),
            "Description" => fetch_str(proxy.description()),
            "HelpText" => fetch_str(proxy.help_text()),
            "Locale" => fetch_str(proxy.locale()),
            "Role" => fetch_map(proxy.get_role(), |role| UiValue::from(role.name().to_string())),
            "RoleName" => fetch_str(proxy.get_role_name()),
            "LocalizedRoleName" => fetch_str(proxy.get_localized_role_name()),
            "AccessibleId" => fetch_str(proxy.accessible_id()),
            "Parent" => fetch_map(proxy.parent(), object_ref_value),
            "ChildCount" => fetch_int(proxy.child_count()),
            "IndexInParent" => fetch_int(proxy.get_index_in_parent()),
            "Interfaces" => fetch_map(proxy.get_interfaces(), interface_set_value),
            "State" => fetch_map(proxy.get_state(), state_set_value),
            "RelationSet" => fetch_map(proxy.get_relation_set(), relation_set_value),
            "Application" => fetch_map(proxy.get_application(), object_ref_value),
            "Attributes" => fetch_map(proxy.get_attributes(), |attrs| {
                let pairs: Vec<(String, String)> = attrs.into_iter().collect();
                attributes_object(&pairs)
            }),
            _ => UiValue::Null,
        }
    }

    fn fetch_action(&self, prop: &str) -> UiValue {
        let Some(proxy) = action_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "NActions" => fetch_int(proxy.nactions()),
            "Actions" => {
                let Some(actions) = block_on_timeout_call(proxy.get_actions()).and_then(|r| r.ok()) else {
                    return UiValue::Null;
                };
                // Enrich each action with its non-localized machine-readable
                // name via the per-index `GetName` method.
                let names: Vec<Option<String>> = (0..actions.len() as i32)
                    .map(|i| block_on_timeout_call(proxy.get_name(i)).and_then(|r| r.ok()).and_then(normalize_value))
                    .collect();
                actions_value(actions, &names)
            }
            _ => UiValue::Null,
        }
    }

    fn fetch_application(&self, prop: &str) -> UiValue {
        let Some(proxy) = application_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "Id" => fetch_map(proxy.id(), |id| UiValue::from(id as i64)),
            "Version" => fetch_str(proxy.version()),
            "ToolkitName" => fetch_str(proxy.toolkit_name()),
            "AtspiVersion" => fetch_str(proxy.atspi_version()),
            "BusAddress" => fetch_str(proxy.get_application_bus_address()),
            _ => UiValue::Null,
        }
    }

    fn fetch_collection(&self, prop: &str) -> UiValue {
        let Some(proxy) = collection_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "ActiveDescendant" => fetch_map(proxy.get_active_descendant(), object_ref_value),
            _ => UiValue::Null,
        }
    }

    fn fetch_component(&self, prop: &str) -> UiValue {
        let Some(proxy) = component_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "Alpha" => fetch(proxy.get_alpha()),
            "Extents.Screen" => fetch_map(proxy.get_extents(CoordType::Screen), extents_value),
            "Extents.Window" => fetch_map(proxy.get_extents(CoordType::Window), extents_value),
            "Extents.Parent" => fetch_map(proxy.get_extents(CoordType::Parent), extents_value),
            "Position.Screen" => fetch_map(proxy.get_position(CoordType::Screen), position_value),
            "Position.Window" => fetch_map(proxy.get_position(CoordType::Window), position_value),
            "Position.Parent" => fetch_map(proxy.get_position(CoordType::Parent), position_value),
            "Size" => fetch_map(proxy.get_size(), size_value),
            "Layer" => fetch_map(proxy.get_layer(), |layer| UiValue::from(format!("{layer:?}"))),
            "MDIZOrder" => fetch_map(proxy.get_mdiz_order(), |order| UiValue::from(order as i64)),
            _ => UiValue::Null,
        }
    }

    fn fetch_document(&self, prop: &str) -> UiValue {
        let Some(proxy) = document_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "PageCount" => fetch_int(proxy.page_count()),
            "CurrentPageNumber" => fetch_int(proxy.current_page_number()),
            "Locale" => fetch_str(proxy.get_locale()),
            "Attributes" => fetch_string_map(proxy.get_attributes()),
            _ => UiValue::Null,
        }
    }

    fn fetch_hyperlink(&self, prop: &str) -> UiValue {
        let Some(proxy) = hyperlink_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "IsValid" => fetch(proxy.is_valid()),
            "EndIndex" => fetch_int(proxy.end_index()),
            "StartIndex" => fetch_int(proxy.start_index()),
            "NAnchors" => fetch_int(proxy.n_anchors()),
            _ => UiValue::Null,
        }
    }

    fn fetch_hypertext(&self, prop: &str) -> UiValue {
        let Some(proxy) = hypertext_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "NLinks" => fetch_int(proxy.get_nlinks()),
            _ => UiValue::Null,
        }
    }

    fn fetch_image(&self, prop: &str) -> UiValue {
        let Some(proxy) = image_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "Description" => fetch_str(proxy.image_description()),
            "Locale" => fetch_str(proxy.image_locale()),
            "Extents.Screen" => fetch_map(proxy.get_image_extents(CoordType::Screen), extents_value),
            "Extents.Window" => fetch_map(proxy.get_image_extents(CoordType::Window), extents_value),
            "Extents.Parent" => fetch_map(proxy.get_image_extents(CoordType::Parent), extents_value),
            "Position.Screen" => fetch_map(proxy.get_image_position(CoordType::Screen), position_value),
            "Position.Window" => fetch_map(proxy.get_image_position(CoordType::Window), position_value),
            "Position.Parent" => fetch_map(proxy.get_image_position(CoordType::Parent), position_value),
            "Size" => fetch_map(proxy.get_image_size(), size_value),
            _ => UiValue::Null,
        }
    }

    fn fetch_selection(&self, prop: &str) -> UiValue {
        let Some(proxy) = selection_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "NSelectedChildren" => fetch_int(proxy.nselected_children()),
            _ => UiValue::Null,
        }
    }

    fn fetch_table(&self, prop: &str) -> UiValue {
        let Some(proxy) = table_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "Caption" => fetch_map(proxy.caption(), object_ref_value),
            "Summary" => fetch_map(proxy.summary(), object_ref_value),
            "NColumns" => fetch_int(proxy.ncolumns()),
            "NRows" => fetch_int(proxy.nrows()),
            "NSelectedColumns" => fetch_int(proxy.nselected_columns()),
            "NSelectedRows" => fetch_int(proxy.nselected_rows()),
            "SelectedRows" => fetch_map(proxy.get_selected_rows(), |rows| {
                UiValue::from(rows.into_iter().map(|v| v as i64).collect::<Vec<_>>())
            }),
            "SelectedColumns" => fetch_map(proxy.get_selected_columns(), |cols| {
                UiValue::from(cols.into_iter().map(|v| v as i64).collect::<Vec<_>>())
            }),
            _ => UiValue::Null,
        }
    }

    fn fetch_table_cell(&self, prop: &str) -> UiValue {
        let Some(proxy) = table_cell_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "ColumnSpan" => fetch_int(proxy.column_span()),
            "RowSpan" => fetch_int(proxy.row_span()),
            "Position" => fetch_map(proxy.position(), |(row, col)| row_column_value(row, col)),
            "Table" => fetch_map(proxy.table(), object_ref_value),
            _ => UiValue::Null,
        }
    }

    fn fetch_text(&self, prop: &str) -> UiValue {
        let Some(proxy) = text_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "CharacterCount" => fetch_int(proxy.character_count()),
            "CaretOffset" => fetch_int(proxy.caret_offset()),
            "NSelections" => fetch_int(proxy.get_nselections()),
            "DefaultAttributes" => fetch_string_map(proxy.get_default_attributes()),
            "DefaultAttributeSet" => fetch_string_map(proxy.get_default_attribute_set()),
            _ => UiValue::Null,
        }
    }

    fn fetch_value_iface(&self, prop: &str) -> UiValue {
        let Some(proxy) = value_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "CurrentValue" => fetch(proxy.current_value()),
            "MaximumValue" => fetch(proxy.maximum_value()),
            "MinimumValue" => fetch(proxy.minimum_value()),
            "MinimumIncrement" => fetch(proxy.minimum_increment()),
            "Text" => fetch_str(proxy.text()),
            _ => UiValue::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- normalize_value ----

    #[test]
    fn normalize_value_trims_whitespace() {
        assert_eq!(normalize_value("  hello  ".to_string()), Some("hello".to_string()));
    }

    #[test]
    fn normalize_value_empty_returns_none() {
        assert_eq!(normalize_value("".to_string()), None);
        assert_eq!(normalize_value("   ".to_string()), None);
    }

    #[test]
    fn normalize_value_preserves_inner_spaces() {
        assert_eq!(normalize_value("hello world".to_string()), Some("hello world".to_string()));
    }

    // ---- pick_attr_value ----

    #[test]
    fn pick_attr_value_finds_first_matching_key() {
        let attrs =
            vec![("name".to_string(), "Name Value".to_string()), ("label".to_string(), "Label Value".to_string())];
        assert_eq!(pick_attr_value(&attrs, &["name", "label"]), Some("Name Value".to_string()));
    }

    #[test]
    fn pick_attr_value_case_insensitive() {
        let attrs = vec![("NAME".to_string(), "upper".to_string())];
        assert_eq!(pick_attr_value(&attrs, &["name"]), Some("upper".to_string()));
    }

    #[test]
    fn pick_attr_value_skips_empty_values() {
        let attrs = vec![("name".to_string(), "   ".to_string()), ("label".to_string(), "fallback".to_string())];
        assert_eq!(pick_attr_value(&attrs, &["name", "label"]), Some("fallback".to_string()));
    }

    #[test]
    fn pick_attr_value_returns_none_when_no_match() {
        let attrs = vec![("other".to_string(), "value".to_string())];
        assert_eq!(pick_attr_value(&attrs, &["name", "label"]), None);
    }

    #[test]
    fn pick_attr_value_empty_attrs() {
        let attrs: Vec<(String, String)> = vec![];
        assert_eq!(pick_attr_value(&attrs, &["name"]), None);
    }

    // ---- map_role ----

    #[test]
    fn map_role_button() {
        let (ns, name) = map_role(Role::Button);
        assert_eq!(ns, Namespace::Control);
        assert_eq!(name, "Button");
    }

    #[test]
    fn map_role_application() {
        let (ns, name) = map_role(Role::Application);
        assert_eq!(ns, Namespace::App);
        assert_eq!(name, "Application");
    }

    #[test]
    fn map_role_invalid_maps_to_unknown() {
        let (ns, name) = map_role(Role::Invalid);
        assert_eq!(ns, Namespace::Control);
        assert_eq!(name, "Unknown");
    }

    #[test]
    fn map_role_list_item_is_item_namespace() {
        let (ns, name) = map_role(Role::ListItem);
        assert_eq!(ns, Namespace::Item);
        assert_eq!(name, "ListItem");
    }

    #[test]
    fn map_role_menu_item_is_item_namespace() {
        let (ns, name) = map_role(Role::MenuItem);
        assert_eq!(ns, Namespace::Item);
        assert_eq!(name, "MenuItem");
    }

    #[test]
    fn map_role_check_menu_item_maps_to_menu_item() {
        let (ns, name) = map_role(Role::CheckMenuItem);
        assert_eq!(ns, Namespace::Item);
        assert_eq!(name, "MenuItem");
    }

    #[test]
    fn map_role_page_tab_maps_to_tab_item() {
        let (ns, name) = map_role(Role::PageTab);
        assert_eq!(ns, Namespace::Item);
        assert_eq!(name, "TabItem");
    }

    #[test]
    fn map_role_page_tab_list_maps_to_tab() {
        let (ns, name) = map_role(Role::PageTabList);
        assert_eq!(ns, Namespace::Control);
        assert_eq!(name, "Tab");
    }

    #[test]
    fn map_role_tree_item_is_item_namespace() {
        let (ns, name) = map_role(Role::TreeItem);
        assert_eq!(ns, Namespace::Item);
        assert_eq!(name, "TreeItem");
    }

    // ---- map_role_with_interfaces ----

    #[test]
    fn map_role_with_interfaces_application_interface_overrides() {
        // Even if the role is not Application, the Application interface
        // should force the App namespace.
        let ifaces = InterfaceSet::new(Interface::Application);
        let (ns, name) = map_role_with_interfaces(Role::Frame, Some(ifaces));
        assert_eq!(ns, Namespace::App);
        assert_eq!(name, "Application");
    }

    #[test]
    fn map_role_with_interfaces_no_override_without_app() {
        let ifaces = InterfaceSet::new(Interface::Component);
        let (ns, name) = map_role_with_interfaces(Role::Button, Some(ifaces));
        assert_eq!(ns, Namespace::Control);
        assert_eq!(name, "Button");
    }

    #[test]
    fn map_role_with_interfaces_none_falls_through() {
        let (ns, name) = map_role_with_interfaces(Role::Dialog, None);
        assert_eq!(ns, Namespace::Control);
        assert_eq!(name, "Dialog");
    }

    // ---- helper value conversions ----

    #[test]
    fn attributes_object_skips_empty_keys() {
        let attrs = vec![
            ("key1".to_string(), "val1".to_string()),
            ("  ".to_string(), "ignored".to_string()),
            ("key2".to_string(), "val2".to_string()),
        ];
        let value = attributes_object(&attrs);
        match value {
            UiValue::Object(map) => {
                assert_eq!(map.len(), 2);
                assert!(map.contains_key("key1"));
                assert!(map.contains_key("key2"));
            }
            other => panic!("expected UiValue::Object, got {other:?}"),
        }
    }

    #[test]
    fn interface_set_value_format() {
        let ifaces = InterfaceSet::new(Interface::Accessible);
        let value = interface_set_value(ifaces);
        match value {
            UiValue::Array(arr) => {
                assert!(!arr.is_empty());
            }
            other => panic!("expected UiValue::Array, got {other:?}"),
        }
    }

    #[test]
    fn state_set_value_format() {
        let mut state = StateSet::empty();
        state.insert(State::Focused);
        let value = state_set_value(state);
        match value {
            UiValue::Array(arr) => {
                assert!(!arr.is_empty());
            }
            other => panic!("expected UiValue::Array, got {other:?}"),
        }
    }

    // ---- AtspiError conversions ----

    #[test]
    fn atspi_error_to_provider_error() {
        use platynui_core::provider::ProviderError;
        let err = AtspiError::timeout("test");
        let pe: ProviderError = err.into();
        assert!(matches!(pe, ProviderError::CommunicationFailure { .. }));
    }

    #[test]
    fn atspi_error_to_pattern_error() {
        use platynui_core::ui::PatternError;
        let err = AtspiError::InterfaceMissing("Component");
        let pe: PatternError = err.into();
        assert!(pe.message().contains("Component"));
    }

    #[test]
    fn atspi_error_connection_becomes_init_failed() {
        use platynui_core::provider::ProviderError;
        let err = AtspiError::ConnectionFailed("refused".to_string());
        let pe: ProviderError = err.into();
        assert!(matches!(pe, ProviderError::InitializationFailed { .. }));
    }

    #[test]
    fn atspi_error_dbus_helper() {
        let err = AtspiError::dbus("proxy.name", "some D-Bus error");
        assert!(err.to_string().contains("proxy.name"));
        assert!(err.to_string().contains("some D-Bus error"));
    }
}
