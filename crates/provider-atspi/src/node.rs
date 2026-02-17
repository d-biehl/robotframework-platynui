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
use platynui_core::ui::attribute_names::{activation_target, application, common, element, focusable, window_surface};
use platynui_core::ui::{
    FocusableAction, Namespace, PatternId, RuntimeId, UiAttribute, UiNode, UiPattern, UiValue, WindowSurfaceActions,
    supported_patterns_value,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tracing::{trace, warn};
use zbus::proxy::CacheProperties;

const NULL_PATH: &str = "/org/a11y/atspi/accessible/null";
const TECHNOLOGY: &str = "AT-SPI2";
/// Timeout for individual D-Bus calls to prevent hangs from unresponsive
/// applications.
const DBUS_TIMEOUT: Duration = Duration::from_secs(1);

/// Execute a future with a timeout. Returns `None` if the future does not
/// complete within [`DBUS_TIMEOUT`].
pub(crate) fn block_on_timeout<F: std::future::Future>(future: F) -> Option<F::Output> {
    let start = std::time::Instant::now();
    let result = block_on(async {
        futures_lite::future::or(async { Some(future.await) }, async {
            async_io::Timer::after(DBUS_TIMEOUT).await;
            None
        })
        .await
    });
    if result.is_none() {
        warn!(
            elapsed_ms = start.elapsed().as_millis() as u64,
            "D-Bus call TIMED OUT ({}ms limit)",
            DBUS_TIMEOUT.as_millis()
        );
    }
    result
}

pub struct AtspiNode {
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    self_weak: OnceCell<Weak<dyn UiNode>>,
    runtime_id: OnceCell<RuntimeId>,
    pub(crate) role: OnceCell<String>,
    pub(crate) namespace: OnceCell<Namespace>,
    state: OnceCell<Option<StateSet>>,
    pub(crate) interfaces: OnceCell<Option<InterfaceSet>>,
    /// Cached name resolved from the accessibility bus.
    pub(crate) cached_name: OnceCell<Option<String>>,
    /// Cached child count (from AT-SPI `ChildCount` property).
    pub(crate) cached_child_count: OnceCell<Option<i32>>,
    /// Cached process ID resolved from D-Bus connection credentials.
    cached_process_id: OnceCell<Option<u32>>,
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
            cached_name: OnceCell::new(),
            cached_child_count: OnceCell::new(),
            cached_process_id: OnceCell::new(),
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
        if self.interfaces.get().is_none() {
            let ifaces = block_on_timeout(proxy.get_interfaces()).and_then(|r| r.ok());
            let _ = self.interfaces.set(ifaces);
        }
        let interfaces = self.interfaces.get().copied().flatten();
        let role = block_on_timeout(proxy.get_role()).and_then(|r| r.ok()).unwrap_or(Role::Invalid);
        let (namespace, role_name) = map_role_with_interfaces(role, interfaces);
        let _ = self.namespace.set(namespace);
        let _ = self.role.set(role_name);
    }

    fn resolve_state(&self) -> Option<StateSet> {
        self.state
            .get_or_init(|| {
                self.accessible().and_then(|proxy| block_on_timeout(proxy.get_state()).and_then(|r| r.ok()))
            })
            .as_ref()
            .copied()
    }

    fn resolve_interfaces(&self) -> Option<InterfaceSet> {
        self.interfaces
            .get_or_init(|| {
                self.accessible().and_then(|proxy| block_on_timeout(proxy.get_interfaces()).and_then(|r| r.ok()))
            })
            .as_ref()
            .copied()
    }

    fn resolve_name(&self) -> Option<String> {
        self.cached_name.get_or_init(|| resolve_name(self.conn.as_ref(), &self.obj)).clone()
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
        *self.cached_process_id.get_or_init(|| {
            let bus_name = self.obj.name_as_str()?;
            let conn = self.conn.connection();
            block_on_timeout(async {
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

    /// Pre-resolve commonly needed properties using a single proxy.
    ///
    /// This avoids repeated proxy builds + D-Bus roundtrips when the
    /// inspector (or any consumer) queries `has_children`, `role`, `name`
    /// etc. in quick succession.
    fn resolve_basics(&self) {
        let start = std::time::Instant::now();
        let obj_path = self.obj.path_as_str().to_string();
        let obj_bus = self.obj.name_as_str().unwrap_or("<unknown>").to_string();

        let Some(proxy) = self.accessible() else {
            let _ = self.role.set("Unknown".to_string());
            let _ = self.namespace.set(Namespace::Control);
            let _ = self.cached_child_count.set(None);
            let _ = self.cached_name.set(None);
            warn!(bus = %obj_bus, path = %obj_path, "resolve_basics: no proxy");
            return;
        };
        // child_count
        if self.cached_child_count.get().is_none() {
            let count = block_on_timeout(proxy.child_count()).and_then(|r| r.ok());
            let _ = self.cached_child_count.set(count);
        }
        // interfaces + role
        if self.interfaces.get().is_none() {
            let ifaces = block_on_timeout(proxy.get_interfaces()).and_then(|r| r.ok());
            let _ = self.interfaces.set(ifaces);
        }
        if self.role.get().is_none() {
            let interfaces = self.interfaces.get().copied().flatten();
            let role = block_on_timeout(proxy.get_role()).and_then(|r| r.ok()).unwrap_or(Role::Invalid);
            let (namespace, role_name) = map_role_with_interfaces(role, interfaces);
            let _ = self.namespace.set(namespace);
            let _ = self.role.set(role_name);
        }
        // name
        if self.cached_name.get().is_none() {
            let name = block_on_timeout(proxy.name()).and_then(|r| r.ok()).and_then(normalize_value);
            let _ = self.cached_name.set(name);
        }

        let elapsed = start.elapsed();
        trace!(
            bus = %obj_bus,
            path = %obj_path,
            role = self.role.get().map(String::as_str).unwrap_or("?"),
            name = self.cached_name.get().and_then(|n| n.as_deref()).unwrap_or(""),
            elapsed_ms = elapsed.as_millis() as u64,
            "resolve_basics",
        );
        if elapsed.as_millis() > 200 {
            warn!(
                bus = %obj_bus,
                path = %obj_path,
                elapsed_ms = elapsed.as_millis() as u64,
                "resolve_basics: SLOW node (>200ms)",
            );
        }
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
        self.parent.lock().unwrap().clone()
    }

    fn has_children(&self) -> bool {
        let count = self.cached_child_count.get_or_init(|| {
            self.accessible().and_then(|proxy| block_on_timeout(proxy.child_count()).and_then(|r| r.ok()))
        });
        count.map(|c| c > 0).unwrap_or(false)
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        let parent_path = self.obj.path_as_str().to_string();
        let parent_bus = self.obj.name_as_str().unwrap_or("<unknown>").to_string();
        let children_start = std::time::Instant::now();

        let Some(children) =
            self.accessible().and_then(|proxy| block_on_timeout(proxy.get_children()).and_then(|r| r.ok()))
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
        if get_children_elapsed.as_millis() > 200 {
            warn!(
                bus = %parent_bus,
                path = %parent_path,
                elapsed_ms = get_children_elapsed.as_millis() as u64,
                "children: SLOW get_children (>200ms)",
            );
        }

        let parent = self.self_weak.get().and_then(|weak| weak.upgrade());
        let conn = self.conn.clone();
        Box::new(children.into_iter().filter_map(move |child| {
            if AtspiNode::is_null_object(&child) {
                return None;
            }
            let node = AtspiNode::new(conn.clone(), child, parent.as_ref());
            // Pre-resolve child_count, interfaces, role, and name using
            // a single proxy so that later has_children/label calls are
            // satisfied from cache without extra D-Bus roundtrips.
            node.resolve_basics();
            Some(node as Arc<dyn UiNode>)
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
                let action = FocusableAction::new(move || {
                    grab_focus(conn.as_ref(), &obj).map_err(platynui_core::ui::PatternError::new)
                });
                Some(Arc::new(action) as Arc<dyn UiPattern>)
            }
            "WindowSurface" => {
                if !self.is_window_surface() {
                    return None;
                }
                let conn = self.conn.clone();
                let obj = self.obj.clone();
                let conn2 = self.conn.clone();
                let obj2 = self.obj.clone();
                let conn3 = self.conn.clone();
                let obj3 = self.obj.clone();
                let pattern = WindowSurfaceActions::new()
                    .with_activate(move || {
                        activate_window(conn.as_ref(), &obj).map_err(platynui_core::ui::PatternError::new)
                    })
                    .with_close(move || {
                        close_window(conn2.as_ref(), &obj2).map_err(platynui_core::ui::PatternError::new)
                    })
                    .with_accepts_user_input(move || Ok(is_active_window(conn3.as_ref(), &obj3)));
                Some(Arc::new(pattern) as Arc<dyn UiPattern>)
            }
            _ => None,
        }
    }

    fn invalidate(&self) {
        // Clear cached data so the next access re-queries the bus.
        // OnceCell does not support clearing, but we can document that
        // invalidate is a best-effort operation. Full re-resolution happens
        // when a new AtspiNode is created for the same object.
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
    block_on_timeout(builder.build()).and_then(|r| r.ok())
}

fn component_proxy<'a>(conn: &'a AccessibilityConnection, obj: &'a ObjectRefOwned) -> Option<ComponentProxy<'a>> {
    let name = obj.name_as_str()?;
    let builder = ComponentProxy::builder(conn.connection())
        .cache_properties(CacheProperties::No)
        .destination(name)
        .ok()?
        .path(obj.path_as_str())
        .ok()?;
    block_on_timeout(builder.build()).and_then(|r| r.ok())
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
            block_on_timeout(builder.build()).and_then(|r| r.ok())
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
    let proxy = component_proxy(conn, obj).ok_or("component interface missing")?;
    let ok = block_on_timeout(proxy.grab_focus())
        .ok_or_else(|| "grab_focus timed out".to_string())?
        .map_err(|e| e.to_string())?;
    if ok { Ok(()) } else { Err("grab_focus returned false".to_string()) }
}

/// Resolve the X11 window ID for this AT-SPI window node.
///
/// Uses the process ID (from the D-Bus bus name) and the component's screen
/// extents to find the matching top-level X11 window via `_NET_CLIENT_LIST`
/// and `_NET_WM_PID`.
fn resolve_xid(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Result<u32, String> {
    // Resolve PID from D-Bus connection credentials.
    let bus_name = obj.name_as_str().ok_or("missing bus name")?;
    let pid: u32 = block_on_timeout(async {
        let dbus = zbus::fdo::DBusProxy::new(conn.connection()).await.ok()?;
        dbus.get_connection_unix_process_id(zbus::names::BusName::try_from(bus_name).ok()?).await.ok()
    })
    .flatten()
    .ok_or("could not resolve PID from D-Bus")?;

    // Try to get screen extents for precise matching.
    let extents = component_proxy(conn, obj)
        .and_then(|proxy| block_on_timeout(proxy.get_extents(CoordType::Screen)).and_then(|r| r.ok()));

    match extents {
        Some((x, y, w, h)) => crate::ewmh::find_xid_for_pid(pid, x, y, w, h),
        None => crate::ewmh::find_xid_for_pid_simple(pid),
    }
}

/// Bring a window to the foreground via EWMH `_NET_ACTIVE_WINDOW`.
fn activate_window(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Result<(), String> {
    let xid = resolve_xid(conn, obj)?;
    crate::ewmh::activate_window(xid)
}

/// Close a window via EWMH `_NET_CLOSE_WINDOW`.
fn close_window(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Result<(), String> {
    let xid = resolve_xid(conn, obj)?;
    crate::ewmh::close_window(xid)
}

/// Check whether a window is the currently active (foreground) window.
fn is_active_window(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Option<bool> {
    let xid = resolve_xid(conn, obj).ok()?;
    crate::ewmh::is_active_window(xid).ok()
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
        block_on_timeout(proxy.get_attributes()).and_then(|r| r.ok())?.into_iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    Some(pairs)
}

fn resolve_name(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Option<String> {
    if let Some(Ok(name)) = accessible_proxy(conn, obj).and_then(|p| block_on_timeout(p.name()))
        && let Some(value) = normalize_value(name)
    {
        return Some(value);
    }
    resolve_attributes(conn, obj)
        .and_then(|attrs| pick_attr_value(&attrs, &["accessible-name", "name", "label", "title"]))
}

fn resolve_id(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Option<String> {
    if let Some(Ok(id)) = accessible_proxy(conn, obj).and_then(|p| block_on_timeout(p.accessible_id()))
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
    /// D-Bus calls are deferred until `.value()` and cached via `OnceCell`.
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
        let ctx = Arc::new(LazyNodeData::new(node.conn.clone(), node.obj.clone(), role.clone()));
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
                    "Component.Extents",
                    "Component.Position",
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
                    "Image.Extents",
                    "Image.Position",
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
                12 => self
                    .process_id
                    .map(|pid| Arc::new(ProcessIdAttr { namespace: self.namespace, pid }) as Arc<dyn UiAttribute>),
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
/// D-Bus calls are deferred until first access and cached via `OnceCell`,
/// so multiple attributes that need the same underlying data (e.g. state)
/// share a single D-Bus roundtrip.
struct LazyNodeData {
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
    role: String,
    state: OnceCell<Option<StateSet>>,
    extents: OnceCell<Option<Rect>>,
    name: OnceCell<String>,
    id: OnceCell<Option<String>>,
}

impl LazyNodeData {
    fn new(conn: Arc<AccessibilityConnection>, obj: ObjectRefOwned, role: String) -> Self {
        Self {
            conn,
            obj,
            role,
            state: OnceCell::new(),
            extents: OnceCell::new(),
            name: OnceCell::new(),
            id: OnceCell::new(),
        }
    }

    fn resolve_state(&self) -> Option<StateSet> {
        *self.state.get_or_init(|| {
            accessible_proxy(&self.conn, &self.obj)
                .and_then(|proxy| block_on_timeout(proxy.get_state()).and_then(|r| r.ok()))
        })
    }

    fn resolve_extents(&self) -> Option<Rect> {
        *self.extents.get_or_init(|| {
            component_proxy(&self.conn, &self.obj).and_then(|proxy| {
                block_on_timeout(proxy.get_extents(CoordType::Screen))
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
    /// EWMH `_NET_ACTIVE_WINDOW`.  Returns `None` when the XID cannot be
    /// resolved (e.g. not a top-level window on X11).
    fn resolve_is_active_window(&self) -> Option<bool> {
        is_active_window(&self.conn, &self.obj)
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
    namespace: Namespace,
    pid: u32,
}

impl UiAttribute for ProcessIdAttr {
    fn namespace(&self) -> Namespace {
        self.namespace
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

impl LazyNativeAttr {
    fn fetch_accessible(&self, prop: &str) -> UiValue {
        let Some(proxy) = accessible_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "Name" => block_on_timeout(proxy.name())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "Description" => block_on_timeout(proxy.description())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "HelpText" => block_on_timeout(proxy.help_text())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "Locale" => block_on_timeout(proxy.locale())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "Role" => block_on_timeout(proxy.get_role())
                .and_then(|r| r.ok())
                .map(|role| UiValue::from(role.name().to_string()))
                .unwrap_or(UiValue::Null),
            "RoleName" => block_on_timeout(proxy.get_role_name())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "LocalizedRoleName" => block_on_timeout(proxy.get_localized_role_name())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "AccessibleId" => block_on_timeout(proxy.accessible_id())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "Parent" => block_on_timeout(proxy.parent())
                .and_then(|r| r.ok())
                .map(|p| UiValue::from(object_runtime_id(&p)))
                .unwrap_or(UiValue::Null),
            "ChildCount" => block_on_timeout(proxy.child_count())
                .and_then(|r| r.ok())
                .map(|c| UiValue::from(c as i64))
                .unwrap_or(UiValue::Null),
            "IndexInParent" => block_on_timeout(proxy.get_index_in_parent())
                .and_then(|r| r.ok())
                .map(|i| UiValue::from(i as i64))
                .unwrap_or(UiValue::Null),
            "Interfaces" => block_on_timeout(proxy.get_interfaces())
                .and_then(|r| r.ok())
                .map(interface_set_value)
                .unwrap_or(UiValue::Null),
            "State" => {
                block_on_timeout(proxy.get_state()).and_then(|r| r.ok()).map(state_set_value).unwrap_or(UiValue::Null)
            }
            "RelationSet" => block_on_timeout(proxy.get_relation_set())
                .and_then(|r| r.ok())
                .map(relation_set_value)
                .unwrap_or(UiValue::Null),
            "Application" => block_on_timeout(proxy.get_application())
                .and_then(|r| r.ok())
                .map(|a| UiValue::from(object_runtime_id(&a)))
                .unwrap_or(UiValue::Null),
            "Attributes" => block_on_timeout(proxy.get_attributes())
                .and_then(|r| r.ok())
                .map(|attrs| {
                    let pairs: Vec<(String, String)> = attrs.into_iter().collect();
                    attributes_object(&pairs)
                })
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }

    fn fetch_action(&self, prop: &str) -> UiValue {
        let Some(proxy) = action_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "NActions" => block_on_timeout(proxy.nactions())
                .and_then(|r| r.ok())
                .map(|c| UiValue::from(c as i64))
                .unwrap_or(UiValue::Null),
            "Actions" => {
                block_on_timeout(proxy.get_actions()).and_then(|r| r.ok()).map(actions_value).unwrap_or(UiValue::Null)
            }
            _ => UiValue::Null,
        }
    }

    fn fetch_application(&self, prop: &str) -> UiValue {
        let Some(proxy) = application_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "Id" => block_on_timeout(proxy.id())
                .and_then(|r| r.ok())
                .map(|id| UiValue::from(id as i64))
                .unwrap_or(UiValue::Null),
            "Version" => block_on_timeout(proxy.version())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "ToolkitName" => block_on_timeout(proxy.toolkit_name())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "AtspiVersion" => block_on_timeout(proxy.atspi_version())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "BusAddress" => block_on_timeout(proxy.get_application_bus_address())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }

    fn fetch_collection(&self, prop: &str) -> UiValue {
        let Some(proxy) = collection_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "ActiveDescendant" => block_on_timeout(proxy.get_active_descendant())
                .and_then(|r| r.ok())
                .map(|d| UiValue::from(object_runtime_id(&d)))
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }

    fn fetch_component(&self, prop: &str) -> UiValue {
        let Some(proxy) = component_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "Alpha" => {
                block_on_timeout(proxy.get_alpha()).and_then(|r| r.ok()).map(UiValue::from).unwrap_or(UiValue::Null)
            }
            "Extents" => block_on_timeout(proxy.get_extents(CoordType::Screen))
                .and_then(|r| r.ok())
                .map(|(x, y, w, h)| UiValue::from(Rect::new(x as f64, y as f64, w as f64, h as f64)))
                .unwrap_or(UiValue::Null),
            "Position" => block_on_timeout(proxy.get_position(CoordType::Screen))
                .and_then(|r| r.ok())
                .map(|(x, y)| UiValue::from(Point::new(x as f64, y as f64)))
                .unwrap_or(UiValue::Null),
            "Size" => block_on_timeout(proxy.get_size())
                .and_then(|r| r.ok())
                .map(|(w, h)| UiValue::from(Size::new(w as f64, h as f64)))
                .unwrap_or(UiValue::Null),
            "Layer" => block_on_timeout(proxy.get_layer())
                .and_then(|r| r.ok())
                .map(|layer| UiValue::from(format!("{layer:?}")))
                .unwrap_or(UiValue::Null),
            "MDIZOrder" => block_on_timeout(proxy.get_mdiz_order())
                .and_then(|r| r.ok())
                .map(|order| UiValue::from(order as i64))
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }

    fn fetch_document(&self, prop: &str) -> UiValue {
        let Some(proxy) = document_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "PageCount" => block_on_timeout(proxy.page_count())
                .and_then(|r| r.ok())
                .map(|c| UiValue::from(c as i64))
                .unwrap_or(UiValue::Null),
            "CurrentPageNumber" => block_on_timeout(proxy.current_page_number())
                .and_then(|r| r.ok())
                .map(|c| UiValue::from(c as i64))
                .unwrap_or(UiValue::Null),
            "Locale" => block_on_timeout(proxy.get_locale())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "Attributes" => block_on_timeout(proxy.get_attributes())
                .and_then(|r| r.ok())
                .filter(|attrs| !attrs.is_empty())
                .map(|attrs| string_map_object(&attrs))
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }

    fn fetch_hyperlink(&self, prop: &str) -> UiValue {
        let Some(proxy) = hyperlink_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "IsValid" => {
                block_on_timeout(proxy.is_valid()).and_then(|r| r.ok()).map(UiValue::from).unwrap_or(UiValue::Null)
            }
            "EndIndex" => block_on_timeout(proxy.end_index())
                .and_then(|r| r.ok())
                .map(|i| UiValue::from(i as i64))
                .unwrap_or(UiValue::Null),
            "StartIndex" => block_on_timeout(proxy.start_index())
                .and_then(|r| r.ok())
                .map(|i| UiValue::from(i as i64))
                .unwrap_or(UiValue::Null),
            "NAnchors" => block_on_timeout(proxy.n_anchors())
                .and_then(|r| r.ok())
                .map(|c| UiValue::from(c as i64))
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }

    fn fetch_hypertext(&self, prop: &str) -> UiValue {
        let Some(proxy) = hypertext_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "NLinks" => block_on_timeout(proxy.get_nlinks())
                .and_then(|r| r.ok())
                .map(|c| UiValue::from(c as i64))
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }

    fn fetch_image(&self, prop: &str) -> UiValue {
        let Some(proxy) = image_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "Description" => block_on_timeout(proxy.image_description())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "Locale" => block_on_timeout(proxy.image_locale())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "Extents" => block_on_timeout(proxy.get_image_extents(CoordType::Screen))
                .and_then(|r| r.ok())
                .map(|(x, y, w, h)| UiValue::from(Rect::new(x as f64, y as f64, w as f64, h as f64)))
                .unwrap_or(UiValue::Null),
            "Position" => block_on_timeout(proxy.get_image_position(CoordType::Screen))
                .and_then(|r| r.ok())
                .map(|(x, y)| UiValue::from(Point::new(x as f64, y as f64)))
                .unwrap_or(UiValue::Null),
            "Size" => block_on_timeout(proxy.get_image_size())
                .and_then(|r| r.ok())
                .map(|(w, h)| UiValue::from(Size::new(w as f64, h as f64)))
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }

    fn fetch_selection(&self, prop: &str) -> UiValue {
        let Some(proxy) = selection_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "NSelectedChildren" => block_on_timeout(proxy.nselected_children())
                .and_then(|r| r.ok())
                .map(|c| UiValue::from(c as i64))
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }

    fn fetch_table(&self, prop: &str) -> UiValue {
        let Some(proxy) = table_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "Caption" => block_on_timeout(proxy.caption())
                .and_then(|r| r.ok())
                .map(|c| UiValue::from(object_runtime_id(&c)))
                .unwrap_or(UiValue::Null),
            "Summary" => block_on_timeout(proxy.summary())
                .and_then(|r| r.ok())
                .map(|s| UiValue::from(object_runtime_id(&s)))
                .unwrap_or(UiValue::Null),
            "NColumns" => block_on_timeout(proxy.ncolumns())
                .and_then(|r| r.ok())
                .map(|c| UiValue::from(c as i64))
                .unwrap_or(UiValue::Null),
            "NRows" => block_on_timeout(proxy.nrows())
                .and_then(|r| r.ok())
                .map(|r| UiValue::from(r as i64))
                .unwrap_or(UiValue::Null),
            "NSelectedColumns" => block_on_timeout(proxy.nselected_columns())
                .and_then(|r| r.ok())
                .map(|c| UiValue::from(c as i64))
                .unwrap_or(UiValue::Null),
            "NSelectedRows" => block_on_timeout(proxy.nselected_rows())
                .and_then(|r| r.ok())
                .map(|r| UiValue::from(r as i64))
                .unwrap_or(UiValue::Null),
            "SelectedRows" => block_on_timeout(proxy.get_selected_rows())
                .and_then(|r| r.ok())
                .map(|rows| UiValue::from(rows.into_iter().map(|v| v as i64).collect::<Vec<_>>()))
                .unwrap_or(UiValue::Null),
            "SelectedColumns" => block_on_timeout(proxy.get_selected_columns())
                .and_then(|r| r.ok())
                .map(|cols| UiValue::from(cols.into_iter().map(|v| v as i64).collect::<Vec<_>>()))
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }

    fn fetch_table_cell(&self, prop: &str) -> UiValue {
        let Some(proxy) = table_cell_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "ColumnSpan" => block_on_timeout(proxy.column_span())
                .and_then(|r| r.ok())
                .map(|s| UiValue::from(s as i64))
                .unwrap_or(UiValue::Null),
            "RowSpan" => block_on_timeout(proxy.row_span())
                .and_then(|r| r.ok())
                .map(|s| UiValue::from(s as i64))
                .unwrap_or(UiValue::Null),
            "Position" => block_on_timeout(proxy.position())
                .and_then(|r| r.ok())
                .map(|(row, col)| row_column_value(row, col))
                .unwrap_or(UiValue::Null),
            "Table" => block_on_timeout(proxy.table())
                .and_then(|r| r.ok())
                .map(|t| UiValue::from(object_runtime_id(&t)))
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }

    fn fetch_text(&self, prop: &str) -> UiValue {
        let Some(proxy) = text_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "CharacterCount" => block_on_timeout(proxy.character_count())
                .and_then(|r| r.ok())
                .map(|c| UiValue::from(c as i64))
                .unwrap_or(UiValue::Null),
            "CaretOffset" => block_on_timeout(proxy.caret_offset())
                .and_then(|r| r.ok())
                .map(|o| UiValue::from(o as i64))
                .unwrap_or(UiValue::Null),
            "NSelections" => block_on_timeout(proxy.get_nselections())
                .and_then(|r| r.ok())
                .map(|c| UiValue::from(c as i64))
                .unwrap_or(UiValue::Null),
            "DefaultAttributes" => block_on_timeout(proxy.get_default_attributes())
                .and_then(|r| r.ok())
                .filter(|attrs| !attrs.is_empty())
                .map(|attrs| string_map_object(&attrs))
                .unwrap_or(UiValue::Null),
            "DefaultAttributeSet" => block_on_timeout(proxy.get_default_attribute_set())
                .and_then(|r| r.ok())
                .filter(|attrs| !attrs.is_empty())
                .map(|attrs| string_map_object(&attrs))
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }

    fn fetch_value_iface(&self, prop: &str) -> UiValue {
        let Some(proxy) = value_proxy(&self.conn, &self.obj) else {
            return UiValue::Null;
        };
        match prop {
            "CurrentValue" => {
                block_on_timeout(proxy.current_value()).and_then(|r| r.ok()).map(UiValue::from).unwrap_or(UiValue::Null)
            }
            "MaximumValue" => {
                block_on_timeout(proxy.maximum_value()).and_then(|r| r.ok()).map(UiValue::from).unwrap_or(UiValue::Null)
            }
            "MinimumValue" => {
                block_on_timeout(proxy.minimum_value()).and_then(|r| r.ok()).map(UiValue::from).unwrap_or(UiValue::Null)
            }
            "MinimumIncrement" => block_on_timeout(proxy.minimum_increment())
                .and_then(|r| r.ok())
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            "Text" => block_on_timeout(proxy.text())
                .and_then(|r| r.ok())
                .and_then(normalize_value)
                .map(UiValue::from)
                .unwrap_or(UiValue::Null),
            _ => UiValue::Null,
        }
    }
}
