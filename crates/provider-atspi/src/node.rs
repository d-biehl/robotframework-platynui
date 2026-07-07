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
use platynui_core::platform::{WindowId, WindowManager, window_manager};
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::attribute_names::{
    activation_target, application, common, element, focusable, text_content, window_state as window_state_attr,
};
use platynui_core::ui::{
    ActivatableAction, CloseableAction, FocusableAction, MaximizableAction, MinimizableAction, MovableAction,
    Namespace, PatternError, PatternName, ResizableAction, ResponsiveAction, RestorableAction, RuntimeId, UiAttribute,
    UiNode, UiNodeExt, UiPattern, UiValue, pattern_names, supported_patterns_value,
};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, OnceLock, Weak};
use tracing::{trace, warn};
use zbus::proxy::CacheProperties;

use crate::clearable_cell::ClearableCell;
use crate::error::AtspiError;
use crate::timeout::block_on_timeout_call;

const NULL_PATH: &str = "/org/a11y/atspi/accessible/null";
const ALT_NULL_PATH: &str = "/org/a11y/atspi/null";
const ATSPI_ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";
const TECHNOLOGY: &str = "AT-SPI2";

/// Cached toolkit names keyed by D-Bus bus name (one lookup per application).
static TOOLKIT_NAME_CACHE: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct AtspiNode {
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    /// Whether the parent is an `Application` accessible.  Resolved at
    /// construction time (when the parent `Arc` is guaranteed alive) and
    /// cached, so the check survives the parent `Weak` becoming dangling
    /// later — which happens routinely once the consumer drops the
    /// children iterator that held the parent strong-ref alive.
    parent_is_application: bool,
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
        let parent_is_application = parent.map(|p| p.namespace() == Namespace::App).unwrap_or(false);
        let node = Arc::new(Self {
            conn,
            obj,
            parent: Mutex::new(parent.map(Arc::downgrade)),
            parent_is_application,
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
        let path = obj.path_as_str();
        path == NULL_PATH || path == ALT_NULL_PATH
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

    /// Returns `true` if this node is a real platform top-level window — i.e.
    /// a direct child of an accessible exposing the AT-SPI `Application`
    /// interface.
    ///
    /// The role alone is **not** sufficient: Qt MDI subwindows (and similar
    /// embedded surfaces in other toolkits) expose the same `Frame`/`Window`/
    /// `Dialog` role as real top-levels, but live deeper in the AT-SPI tree.
    /// Treating them as top-levels would expose useless window patterns
    /// (`Activatable`, `Closeable`, …) and break coordinate resolution, since
    /// AT-SPI's `CoordType::Window` is relative to the toolkit's real
    /// top-level window — not to any embedded `Frame`/`Window`/`Dialog`.
    ///
    /// The result is cached at construction time (see [`Self::new`]); we do
    /// **not** re-resolve the parent at query time because the parent `Weak`
    /// may have already expired (it does not keep the parent alive).
    fn is_window_surface(&self) -> bool {
        self.parent_is_application
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

    fn supported_patterns(&self) -> Vec<PatternName> {
        let mut patterns = Vec::new();
        if self.focusable() {
            patterns.push(PatternName::from(pattern_names::FOCUSABLE));
        }
        if self.is_window_surface() {
            patterns.push(PatternName::from(pattern_names::ACTIVATABLE));
            patterns.push(PatternName::from(pattern_names::MINIMIZABLE));
            patterns.push(PatternName::from(pattern_names::MAXIMIZABLE));
            patterns.push(PatternName::from(pattern_names::RESTORABLE));
            patterns.push(PatternName::from(pattern_names::CLOSEABLE));
            patterns.push(PatternName::from(pattern_names::MOVABLE));
            patterns.push(PatternName::from(pattern_names::RESIZABLE));
            patterns.push(PatternName::from(pattern_names::RESPONSIVE));
        }
        patterns
    }

    fn pattern_by_name(&self, pattern: &PatternName) -> Option<Arc<dyn UiPattern>> {
        let id = pattern.as_str();
        if id == pattern_names::FOCUSABLE {
            if !self.focusable() {
                return None;
            }
            let conn = self.conn.clone();
            let obj = self.obj.clone();
            let action = FocusableAction::new(move || grab_focus(conn.as_ref(), &obj).map_err(Into::into));
            Some(Arc::new(action) as Arc<dyn UiPattern>)
        } else if matches!(
            id,
            x if x == pattern_names::ACTIVATABLE
                || x == pattern_names::MINIMIZABLE
                || x == pattern_names::MAXIMIZABLE
                || x == pattern_names::RESTORABLE
                || x == pattern_names::CLOSEABLE
                || x == pattern_names::MOVABLE
                || x == pattern_names::RESIZABLE
                || x == pattern_names::RESPONSIVE
        ) {
            if !self.is_window_surface() {
                return None;
            }
            let weak = self.self_weak.get().cloned()?;
            let core = Arc::new(AtspiWindowSurface { node: weak, conn: self.conn.clone(), obj: self.obj.clone() });
            Some(make_window_pattern(id, core))
        } else {
            None
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

make_proxy!(accessible_proxy, AccessibleProxy);
make_proxy!(component_proxy, ComponentProxy);
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

/// Resolve the toolkit identifier for the application owning the given
/// accessible object by querying `Application.ToolkitName` and
/// `Application.Version` on the AT-SPI root node of that bus name.
///
/// The returned string combines the toolkit name with its major version
/// (e.g. `"gtk4"`, `"qt6"`).  If the version is unavailable, only the
/// lowercase toolkit name is returned (e.g. `"gtk"`).
///
/// Successful results are cached per D-Bus bus name so the calls happen
/// at most once per application.  Failures are **not** cached so that
/// transient D-Bus timeouts can recover on the next attempt.
fn resolve_toolkit_name(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Option<String> {
    let bus_name = obj.name_as_str()?;

    // Fast path: return cached successful result without holding the lock
    // during D-Bus calls.
    {
        let cache = TOOLKIT_NAME_CACHE.lock().expect("toolkit name cache mutex poisoned");
        if let Some(cached) = cache.get(bus_name) {
            return cached.clone();
        }
    }

    // Slow path: D-Bus calls outside the lock.
    let result = (|| {
        let proxy = ApplicationProxy::builder(conn.connection())
            .cache_properties(CacheProperties::No)
            .destination(bus_name)
            .ok()?
            .path(ATSPI_ROOT_PATH)
            .ok()?;
        let proxy = block_on_timeout_call(proxy.build()).and_then(|r| r.ok())?;
        let name = block_on_timeout_call(proxy.toolkit_name()).and_then(|r| r.ok())?.to_lowercase();
        // Append the major version number if available (e.g. "gtk" + "4" → "gtk4").
        let version = block_on_timeout_call(proxy.version()).and_then(|r| r.ok());
        match version.as_deref().and_then(|v| v.split('.').next()) {
            Some(major) if !major.is_empty() => Some(format!("{name}{major}")),
            _ => Some(name),
        }
    })();

    // Only cache successful results so transient failures can be retried.
    if let Some(ref toolkit) = result {
        trace!(bus_name, toolkit, "resolved toolkit name");
        TOOLKIT_NAME_CACHE
            .lock()
            .expect("toolkit name cache mutex poisoned")
            .insert(bus_name.to_string(), Some(toolkit.clone()));
    } else {
        trace!(bus_name, "failed to resolve toolkit name");
    }

    result
}

fn grab_focus(conn: &AccessibilityConnection, obj: &ObjectRefOwned) -> Result<(), AtspiError> {
    let proxy = component_proxy(conn, obj).ok_or(AtspiError::InterfaceMissing("Component"))?;
    let ok = block_on_timeout_call(proxy.grab_focus())
        .ok_or(AtspiError::timeout("grab_focus"))?
        .map_err(|e| AtspiError::dbus("grab_focus", e))?;
    if ok { Ok(()) } else { Err(AtspiError::FocusFailed) }
}

/// Shared resolver for AT-SPI window-surface sub-patterns.
///
/// Holds a single [`Weak`] reference to the owning [`UiNode`] and delegates
/// all operations to the registered [`WindowManager`]. Wrapped by
/// [`make_window_pattern`] into the eight orthogonal sub-pattern actions.
struct AtspiWindowSurface {
    node: Weak<dyn UiNode>,
    conn: Arc<AccessibilityConnection>,
    obj: ObjectRefOwned,
}

impl AtspiWindowSurface {
    /// Upgrade the weak node reference and resolve the window manager + window ID.
    fn resolve(&self) -> Result<(&'static dyn WindowManager, WindowId), AtspiError> {
        let node = self.node.upgrade().ok_or(AtspiError::NodeDropped)?;
        let wm = window_manager().ok_or(AtspiError::NoWindowManager)?;
        let wid = wm.resolve_window(node.as_ref()).map_err(|e| AtspiError::dbus("resolve_window", e))?;
        Ok((wm, wid))
    }

    fn resolve_state(&self) -> Option<StateSet> {
        accessible_proxy(self.conn.as_ref(), &self.obj)
            .and_then(|proxy| block_on_timeout_call(proxy.get_state()).and_then(|r| r.ok()))
    }

    fn activate(&self) -> Result<(), PatternError> {
        let (wm, wid) = self.resolve()?;
        wm.activate(wid)?;
        Ok(())
    }

    fn minimize(&self) -> Result<(), PatternError> {
        let (wm, wid) = self.resolve()?;
        wm.minimize(wid)?;
        Ok(())
    }

    fn maximize(&self) -> Result<(), PatternError> {
        let (wm, wid) = self.resolve()?;
        wm.maximize(wid)?;
        Ok(())
    }

    fn restore(&self) -> Result<(), PatternError> {
        let (wm, wid) = self.resolve()?;
        wm.restore(wid)?;
        Ok(())
    }

    fn close(&self) -> Result<(), PatternError> {
        let (wm, wid) = self.resolve()?;
        wm.close(wid)?;
        Ok(())
    }

    fn move_to(&self, position: Point) -> Result<(), PatternError> {
        let (wm, wid) = self.resolve()?;
        wm.move_to(wid, position)?;
        Ok(())
    }

    fn resize(&self, size: Size) -> Result<(), PatternError> {
        let (wm, wid) = self.resolve()?;
        wm.resize(wid, size)?;
        Ok(())
    }

    fn accepts_user_input(&self) -> Result<Option<bool>, PatternError> {
        // If the AT-SPI peer responds to a state query, its event loop is running
        // and it can accept user input \u2014 analogous to WaitForInputIdle on Windows.
        Ok(Some(self.resolve_state().is_some()))
    }
}

fn make_window_pattern(id: &str, core: Arc<AtspiWindowSurface>) -> Arc<dyn UiPattern> {
    match id {
        x if x == pattern_names::ACTIVATABLE => {
            let core = Arc::clone(&core);
            Arc::new(ActivatableAction::new(move || core.activate()))
        }
        x if x == pattern_names::MINIMIZABLE => {
            let core = Arc::clone(&core);
            Arc::new(MinimizableAction::new(move || core.minimize()))
        }
        x if x == pattern_names::MAXIMIZABLE => {
            let core = Arc::clone(&core);
            Arc::new(MaximizableAction::new(move || core.maximize()))
        }
        x if x == pattern_names::RESTORABLE => {
            let core = Arc::clone(&core);
            Arc::new(RestorableAction::new(move || core.restore()))
        }
        x if x == pattern_names::CLOSEABLE => {
            let core = Arc::clone(&core);
            Arc::new(CloseableAction::new(move || core.close()))
        }
        x if x == pattern_names::MOVABLE => {
            let core = Arc::clone(&core);
            Arc::new(MovableAction::new(move |point| core.move_to(point)))
        }
        x if x == pattern_names::RESIZABLE => {
            let core = Arc::clone(&core);
            Arc::new(ResizableAction::new(move |size| core.resize(size)))
        }
        x if x == pattern_names::RESPONSIVE => {
            let core = Arc::clone(&core);
            Arc::new(ResponsiveAction::new(move || core.accepts_user_input()))
        }
        _ => unreachable!("make_window_pattern called with non-window pattern id"),
    }
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
    /// Whether this node exposes the AT-SPI `Text` interface, gating the
    /// canonical `control:Text` attribute (TextContent).
    supports_text: bool,
    /// Pre-resolved role string (avoids re-querying D-Bus).
    role: String,
    /// Shared lazy-resolution context for standard attributes.
    /// D-Bus calls are deferred until `.value()` and cached via `OnceLock`.
    ctx: Arc<LazyNodeData>,
    /// Cached process ID (only set for Application nodes).
    process_id: Option<u32>,
    /// Whether this node is a real platform top-level window
    /// (direct child of an `Application` accessible).
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
            node.self_weak.get().cloned(),
            node.is_window_surface(),
        ));
        // Standard attributes always live in the Control namespace,
        // regardless of the node's own namespace (e.g. App for
        // Application nodes).
        let namespace = Namespace::Control;

        // Build the list of applicable native property names based on
        // supported interfaces.  No D-Bus calls here — the interface set
        // is already cached on AtspiNode.
        let interfaces = node.resolve_interfaces();
        let supports_text = interfaces.as_ref().map(|ifaces| ifaces.contains(Interface::Text)).unwrap_or(false);
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
            supports_text,
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
                            kind: StdAttrKind::IsInView,
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
                13 => self.process_id.map(|pid| Arc::new(AppProcessNameAttr { pid }) as Arc<dyn UiAttribute>),
                14 => self.process_id.map(|pid| Arc::new(AppExecutablePathAttr { pid }) as Arc<dyn UiAttribute>),
                15 => self.process_id.map(|pid| Arc::new(AppCommandLineAttr { pid }) as Arc<dyn UiAttribute>),
                16 => self.process_id.map(|pid| Arc::new(AppUserNameAttr { pid }) as Arc<dyn UiAttribute>),
                17 => self.process_id.map(|pid| Arc::new(AppStartTimeAttr { pid }) as Arc<dyn UiAttribute>),
                18 => self.process_id.map(|pid| Arc::new(AppArchitectureAttr { pid }) as Arc<dyn UiAttribute>),
                19 => {
                    if self.is_window_surface {
                        Some(Arc::new(LazyStdAttr {
                            namespace: self.namespace,
                            kind: StdAttrKind::IsActive,
                            ctx: self.ctx.clone(),
                        }))
                    } else {
                        None
                    }
                }
                20 => {
                    if self.is_window_surface {
                        Some(Arc::new(LazyStdAttr {
                            namespace: self.namespace,
                            kind: StdAttrKind::IsModal,
                            ctx: self.ctx.clone(),
                        }))
                    } else {
                        None
                    }
                }
                21 => {
                    if self.supports_text {
                        Some(Arc::new(LazyStdAttr {
                            namespace: self.namespace,
                            kind: StdAttrKind::Text,
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
                    if self.idx > 22 {
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
    /// Weak reference to the owning UiNode, used for window manager queries.
    owner: Option<Weak<dyn UiNode>>,
    /// Whether this node is a real platform top-level window.  Cached at
    /// construction so the answer is robust against the parent `Weak` (on
    /// the owning `AtspiNode`) becoming dangling later.
    is_real_toplevel: bool,
    state: OnceLock<Option<StateSet>>,
    extents: OnceLock<Option<Rect>>,
    name: OnceLock<String>,
    id: OnceLock<Option<String>>,
    /// Cached full text content (`GetText(0,-1)`) for text-bearing nodes.
    /// `Some("")` for an empty field; `None` only when the D-Bus read fails.
    text: OnceLock<Option<String>>,
}

impl LazyNodeData {
    fn new(
        conn: Arc<AccessibilityConnection>,
        obj: ObjectRefOwned,
        owner: Option<Weak<dyn UiNode>>,
        is_real_toplevel: bool,
    ) -> Self {
        Self {
            conn,
            obj,
            owner,
            is_real_toplevel,
            state: OnceLock::new(),
            extents: OnceLock::new(),
            name: OnceLock::new(),
            id: OnceLock::new(),
            text: OnceLock::new(),
        }
    }

    fn resolve_state(&self) -> Option<StateSet> {
        *self.state.get_or_init(|| {
            accessible_proxy(&self.conn, &self.obj)
                .and_then(|proxy| block_on_timeout_call(proxy.get_state()).and_then(|r| r.ok()))
        })
    }

    /// Resolve the element's full text content via the AT-SPI `Text`
    /// interface (`GetText(0,-1)`), verbatim. Callers gate this on the
    /// interface being present. Returns `Some("")` for an empty field and
    /// `None` only when the D-Bus read fails.
    fn resolve_text(&self) -> Option<String> {
        self.text
            .get_or_init(|| {
                text_proxy(&self.conn, &self.obj)
                    .and_then(|proxy| block_on_timeout_call(proxy.get_text(0, -1)).and_then(|r| r.ok()))
            })
            .clone()
    }

    /// Returns `true` if this node is a real platform top-level window
    /// (direct child of an accessible exposing the AT-SPI `Application`
    /// interface).  See [`AtspiNode::is_window_surface`] for the rationale.
    fn is_real_toplevel(&self) -> bool {
        self.is_real_toplevel
    }

    fn resolve_extents(&self) -> Option<Rect> {
        *self.extents.get_or_init(|| {
            // Step 1: real platform top-level → WM bounds.
            if self.is_real_toplevel()
                && let Some(bounds) = self.resolve_window_manager_bounds()
            {
                return Some(bounds);
            }

            // Step 2: walk up via CoordType::Parent.  We deliberately do
            // **not** use CoordType::Window: at least Qt's AT-SPI bridge
            // treats embedded surfaces (QMdiSubWindow, popup widgets …) as
            // window boundaries, so window-relative extents for everything
            // underneath are reported relative to that embedded surface
            // rather than the real toolkit top-level window — which makes
            // window-relative coordinates unsafe to combine with the
            // top-level's WM bounds.  Parent-relative coordinates are
            // unambiguous, so we sum them up the parent chain instead.
            if let Some(rect) = self.resolve_extents_via_parent_chain() {
                return Some(rect);
            }

            // Fallback: Screen extents (works on X11 where AT-SPI reports
            // real screen coordinates; on Wayland clients return 0,0).
            component_proxy(&self.conn, &self.obj).and_then(|proxy| {
                block_on_timeout_call(proxy.get_extents(CoordType::Screen))
                    .and_then(|r| r.ok())
                    .map(|(x, y, w, h)| Rect::new(x as f64, y as f64, w as f64, h as f64))
            })
        })
    }

    /// Compute absolute screen extents by adding our parent-relative
    /// position to the parent's absolute bounds.  The parent's bounds are
    /// resolved through its own `Bounds` attribute, which recurses through
    /// the same logic — terminating at the real top-level (step 1 above).
    fn resolve_extents_via_parent_chain(&self) -> Option<Rect> {
        let owner = self.owner.as_ref()?.upgrade()?;
        let parent = owner.parent_arc()?;

        // The Application accessible has no meaningful on-screen geometry.
        // We should never reach here for a real top-level (those are handled
        // by step 1), but defend against unexpected tree shapes.
        if parent.namespace() == Namespace::App {
            return None;
        }

        let proxy = component_proxy(&self.conn, &self.obj)?;
        let (x, y, w, h) = block_on_timeout_call(proxy.get_extents(CoordType::Parent)).and_then(|r| r.ok())?;

        let parent_bounds_attr = parent.attribute(Namespace::Control, element::BOUNDS)?;
        let UiValue::Rect(parent_bounds) = parent_bounds_attr.value() else {
            return None;
        };

        Some(Rect::new(parent_bounds.x() + x as f64, parent_bounds.y() + y as f64, w as f64, h as f64))
    }

    /// Resolve the window manager and window ID for this node.
    fn resolve_window(&self) -> Option<(&'static dyn WindowManager, WindowId)> {
        let node = self.owner.as_ref()?.upgrade()?;
        let wm = window_manager()?;
        let wid = wm.resolve_window(node.as_ref()).ok()?;
        Some((wm, wid))
    }

    /// Resolve the toolkit identifier for the application owning this node.
    fn resolve_toolkit(&self) -> Option<String> {
        resolve_toolkit_name(&self.conn, &self.obj)
    }

    /// Ask the registered [`WindowManager`] for the bounds of this window.
    fn resolve_window_manager_bounds(&self) -> Option<Rect> {
        let (wm, wid) = self.resolve_window()?;
        wm.bounds(wid, self.resolve_toolkit().as_deref()).ok()
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
        let (wm, wid) = self.resolve_window()?;
        wm.is_active(wid).ok()
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
    IsInView,
    IsFocused,
    SupportedPatterns,
    IsActive,
    IsModal,
    Text,
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
            StdAttrKind::IsInView => element::IS_IN_VIEW,
            StdAttrKind::IsFocused => focusable::IS_FOCUSED,
            StdAttrKind::SupportedPatterns => common::SUPPORTED_PATTERNS,
            StdAttrKind::IsActive => window_state_attr::IS_ACTIVE,
            StdAttrKind::IsModal => window_state_attr::IS_MODAL,
            StdAttrKind::Text => text_content::TEXT,
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
            StdAttrKind::IsInView => {
                let visible = self
                    .ctx
                    .resolve_state()
                    .map(|s| s.contains(State::Visible) || s.contains(State::Showing))
                    .unwrap_or(false);
                UiValue::from(visible)
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
                let window_surface = self.ctx.is_real_toplevel();
                let mut patterns = Vec::new();
                if focusable {
                    patterns.push(PatternName::from(pattern_names::FOCUSABLE));
                }
                if window_surface {
                    patterns.push(PatternName::from(pattern_names::ACTIVATABLE));
                    patterns.push(PatternName::from(pattern_names::MINIMIZABLE));
                    patterns.push(PatternName::from(pattern_names::MAXIMIZABLE));
                    patterns.push(PatternName::from(pattern_names::RESTORABLE));
                    patterns.push(PatternName::from(pattern_names::CLOSEABLE));
                    patterns.push(PatternName::from(pattern_names::MOVABLE));
                    patterns.push(PatternName::from(pattern_names::RESIZABLE));
                    patterns.push(PatternName::from(pattern_names::RESPONSIVE));
                }
                supported_patterns_value(&patterns)
            }
            StdAttrKind::IsActive => {
                let active = self.ctx.resolve_is_active_window().unwrap_or(false);
                UiValue::from(active)
            }
            StdAttrKind::IsModal => {
                let modal = self.ctx.resolve_state().map(|s| s.contains(State::Modal)).unwrap_or(false);
                UiValue::from(modal)
            }
            StdAttrKind::Text => {
                // Verbatim GetText(0,-1); preserve empty strings (an empty
                // text field must stay present-and-empty, not collapse to
                // Null) — so this deliberately bypasses `fetch_str`'s
                // empty-to-Null normalization used for names.
                self.ctx.resolve_text().map(UiValue::from).unwrap_or(UiValue::Null)
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

// ---------------------------------------------------------------------------
// Application-specific attribute types (app:* namespace)
//
// These mirror the Windows UIA Application node attributes, reading
// process metadata from the Linux `/proc` filesystem.
// ---------------------------------------------------------------------------

struct AppProcessNameAttr {
    pid: u32,
}

impl UiAttribute for AppProcessNameAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }

    fn name(&self) -> &str {
        application::PROCESS_NAME
    }

    fn value(&self) -> UiValue {
        crate::process::query_process_name(self.pid).map(UiValue::from).unwrap_or(UiValue::from(""))
    }
}

struct AppExecutablePathAttr {
    pid: u32,
}

impl UiAttribute for AppExecutablePathAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }

    fn name(&self) -> &str {
        application::EXECUTABLE_PATH
    }

    fn value(&self) -> UiValue {
        crate::process::query_executable_path(self.pid).map(UiValue::from).unwrap_or(UiValue::from(""))
    }
}

struct AppCommandLineAttr {
    pid: u32,
}

impl UiAttribute for AppCommandLineAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }

    fn name(&self) -> &str {
        application::COMMAND_LINE
    }

    fn value(&self) -> UiValue {
        crate::process::query_command_line(self.pid).map(UiValue::from).unwrap_or(UiValue::Null)
    }
}

struct AppUserNameAttr {
    pid: u32,
}

impl UiAttribute for AppUserNameAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }

    fn name(&self) -> &str {
        application::USER_NAME
    }

    fn value(&self) -> UiValue {
        crate::process::query_user_name(self.pid).map(UiValue::from).unwrap_or(UiValue::from(""))
    }
}

struct AppStartTimeAttr {
    pid: u32,
}

impl UiAttribute for AppStartTimeAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }

    fn name(&self) -> &str {
        application::START_TIME
    }

    fn value(&self) -> UiValue {
        crate::process::query_start_time(self.pid).map(UiValue::from).unwrap_or(UiValue::from(""))
    }
}

struct AppArchitectureAttr {
    pid: u32,
}

impl UiAttribute for AppArchitectureAttr {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }

    fn name(&self) -> &str {
        application::ARCHITECTURE
    }

    fn value(&self) -> UiValue {
        crate::process::query_architecture(self.pid).map(UiValue::from).unwrap_or(UiValue::from("unknown"))
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
            "NActions" => fetch_int(proxy.n_actions()),
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
            "NLinks" => fetch_int(proxy.get_n_links()),
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
            "NSelectedChildren" => fetch_int(proxy.n_selected_children()),
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
            "NColumns" => fetch_int(proxy.n_columns()),
            "NRows" => fetch_int(proxy.n_rows()),
            "NSelectedColumns" => fetch_int(proxy.n_selected_columns()),
            "NSelectedRows" => fetch_int(proxy.n_selected_rows()),
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
            "NSelections" => fetch_int(proxy.get_n_selections()),
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
