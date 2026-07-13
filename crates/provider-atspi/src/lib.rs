//! AT-SPI2 UiTree provider for Unix desktops.
//!
//! Provides a blocking D-Bus integration to query the accessibility tree on
//! Linux/X11 systems. Event streaming and full WindowSurface integration will
//! follow in later phases.

pub(crate) mod clearable_cell;
pub(crate) mod error;

mod connection;
mod node;
mod process;
mod timeout;

use crate::clearable_cell::ClearableCell;
use crate::connection::connect_a11y_bus_with;
use crate::error::AtspiError;
use crate::node::AtspiNode;
use atspi_common::{ObjectRefOwned, Role};
use atspi_connection::AccessibilityConnection;
use atspi_proxies::accessible::AccessibleProxy;
use platynui_core::config::RuntimeConfig;
use platynui_core::platform::{WindowId, WindowManager};
use platynui_core::provider::{ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory};
use platynui_core::types::{Point, Rect};
use platynui_core::ui::{TechnologyId, UiNode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use tracing::{debug, info, trace, warn};
use zbus::proxy::CacheProperties;

use crate::timeout::{block_on_timeout_call, block_on_timeout_init};

/// Cache current process ID once; stable for the entire process lifetime.
static SELF_PID: LazyLock<u32> = LazyLock::new(std::process::id);

pub const PROVIDER_ID: &str = "atspi";
pub const PROVIDER_NAME: &str = "AT-SPI2";
pub static TECHNOLOGY: LazyLock<TechnologyId> = LazyLock::new(|| TechnologyId::from("AT-SPI2"));

const REGISTRY_BUS: &str = "org.a11y.atspi.Registry";
const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";

static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
    ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from("AT-SPI2"), ProviderKind::Native)
});

pub struct AtspiFactory;

impl UiTreeProviderFactory for AtspiFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        &DESCRIPTOR
    }

    fn create(&self, config: &RuntimeConfig) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        Ok(Arc::new(self.build(config)))
    }
}

impl AtspiFactory {
    /// Build a concrete provider from `config` — split out from `create` so the
    /// config → `bus_address` wiring is unit-testable without a live bus.
    fn build(&self, config: &RuntimeConfig) -> AtspiProvider {
        let bus_address =
            config.provider(PROVIDER_ID).and_then(|atspi| atspi.get_str("bus_address")).map(str::to_owned);
        AtspiProvider::new(bus_address)
    }
}

pub struct AtspiProvider {
    descriptor: &'static ProviderDescriptor,
    conn: ClearableCell<Arc<AccessibilityConnection>>,
    is_shutdown: AtomicBool,
    /// Explicit AT-SPI bus address from `providers.atspi.bus_address`; `None`
    /// falls back to `AT_SPI_BUS_ADDRESS` / default discovery.
    bus_address: Option<String>,
    /// Per-runtime window manager injected via [`UiTreeProvider::set_window_manager`]
    /// after the runtime builds its platform bundle. Threaded into every
    /// top-level window node so window operations target this runtime's session
    /// instead of a process-global. Empty until injected (e.g. when the
    /// provider is used without a runtime).
    window_manager: ClearableCell<Arc<dyn WindowManager>>,
}

impl AtspiProvider {
    fn new(bus_address: Option<String>) -> Self {
        Self {
            descriptor: &DESCRIPTOR,
            conn: ClearableCell::new(),
            is_shutdown: AtomicBool::new(false),
            bus_address,
            window_manager: ClearableCell::new(),
        }
    }

    fn connection(&self) -> Result<Arc<AccessibilityConnection>, AtspiError> {
        if self.is_shutdown.load(Ordering::Acquire) {
            return Err(AtspiError::Shutdown);
        }
        let bus_address = self.bus_address.as_deref();
        self.conn.get_or_try_init(|| Ok(Arc::new(connect_a11y_bus_with(bus_address)?)))
    }
}

impl UiTreeProvider for AtspiProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
    }

    fn set_window_manager(&self, window_manager: Arc<dyn WindowManager>) {
        // Set-once (first-writer-wins via ClearableCell); the runtime calls
        // this exactly once after building its platform bundle.
        self.window_manager.set(window_manager);
    }

    fn shutdown(&self) {
        if self.is_shutdown.swap(true, Ordering::AcqRel) {
            return; // already shut down
        }
        info!("AT-SPI provider shutting down");
        self.conn.clear();
    }

    fn get_nodes(
        &self,
        parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
        let conn = self.connection()?;
        let proxy = block_on_timeout_init(
            AccessibleProxy::builder(conn.connection())
                .cache_properties(CacheProperties::No)
                .destination(REGISTRY_BUS)
                .map_err(|err| AtspiError::dbus("registry destination", err))?
                .path(ROOT_PATH)
                .map_err(|err| AtspiError::dbus("registry path", err))?
                .build(),
        )
        .ok_or_else(|| AtspiError::timeout("registry proxy build"))?
        .map_err(|err| AtspiError::dbus("registry proxy", err))?;

        let children = block_on_timeout_init(proxy.get_children())
            .ok_or_else(|| AtspiError::timeout("registry children"))?
            .map_err(|err| AtspiError::dbus("registry children", err))?;

        let parent = Arc::clone(&parent);
        let conn = conn.clone();
        let window_manager = self.window_manager.get();
        Ok(Box::new(children.into_iter().filter_map(move |child| {
            if AtspiNode::is_null_object(&child) {
                return None;
            }
            let app_bus = child.name_as_str().unwrap_or("<unknown>").to_string();
            let app_start = std::time::Instant::now();

            // Resolve the PID of this application's D-Bus connection
            // and skip it when it belongs to our own process.
            let app_pid: Option<u32> = {
                let bus_name = child.name_as_str()?;
                let conn_inner = conn.connection().clone();
                block_on_timeout_call(async {
                    let dbus = zbus::fdo::DBusProxy::new(&conn_inner).await.ok()?;
                    dbus.get_connection_unix_process_id(zbus::names::BusName::try_from(bus_name).ok()?).await.ok()
                })
                .flatten()
            };
            if app_pid == Some(*SELF_PID) {
                debug!(app = %app_bus, pid = *SELF_PID, "skipped own process");
                return None;
            }

            // Build a single proxy per registered application and
            // pre-resolve the essential properties (child_count,
            // interfaces, role, name) in one batch.  This avoids
            // duplicate proxy builds and D-Bus roundtrips later when
            // the tree view queries has_children / label / role.
            let name = child.name_as_str()?;
            let proxy = block_on_timeout_call(
                AccessibleProxy::builder(conn.connection())
                    .cache_properties(CacheProperties::No)
                    .destination(name)
                    .ok()?
                    .path(child.path_as_str())
                    .ok()?
                    .build(),
            )?
            .ok()?;

            // Filter zombie registrations / empty toolkits.
            let child_count = block_on_timeout_call(proxy.child_count())?.ok()?;
            if child_count == 0 {
                trace!(app = %app_bus, "skipped (0 children)");
                return None;
            }

            // Pre-resolve interfaces, role, and name using the same
            // proxy so that AtspiNode caches are warm on first access.
            let interfaces = block_on_timeout_call(proxy.get_interfaces()).and_then(|r| r.ok());
            let role = block_on_timeout_call(proxy.get_role()).and_then(|r| r.ok()).unwrap_or(Role::Invalid);
            let node_name = block_on_timeout_call(proxy.name()).and_then(|r| r.ok()).and_then(node::normalize_value);

            let node = AtspiNode::new(conn.clone(), child, Some(&parent), window_manager.clone());
            // Seed caches directly — no additional D-Bus calls inside.
            node.cached_child_count.set(Some(child_count));
            node.interfaces.set(interfaces);
            let (ns, role_name) = node::map_role_with_interfaces(role, interfaces);
            let _ = node.namespace.set(ns);
            let _ = node.role.set(role_name);
            node.cached_name.set(node_name.clone());

            let elapsed = app_start.elapsed();
            trace!(
                app = %app_bus,
                name = node_name.as_deref().unwrap_or(""),
                children = child_count,
                elapsed_ms = elapsed.as_millis() as u64,
                "get_nodes: resolved app",
            );
            if elapsed.as_millis() > 1000 {
                warn!(
                    app = %app_bus,
                    elapsed_ms = elapsed.as_millis() as u64,
                    "get_nodes: SLOW app resolution (>1000ms)",
                );
            }

            Some(node as Arc<dyn UiNode>)
        })))
    }

    fn element_at_point(&self, point: Point) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
        if self.is_shutdown.load(Ordering::Acquire) {
            return Err(ProviderError::CommunicationFailure {
                channel: "atspi",
                details: Some(AtspiError::Shutdown.to_string()),
            });
        }

        // Window-level z-order is authoritative from the platform window manager
        // (X11 EWMH stacking / the PlatynUI compositor). AT-SPI has no
        // cross-application window-stacking view, so without a window manager we
        // cannot pick the correct application and report the hit-test unsupported.
        let Some(window_manager) = self.window_manager.get() else {
            return Err(ProviderError::UnsupportedOperation {
                operation: "element_at_point",
                details: Some("no window manager for window-level hit-test".into()),
            });
        };
        let hit = window_manager.window_at_point(point).map_err(|err| ProviderError::CommunicationFailure {
            channel: "window manager",
            details: Some(err.to_string()),
        })?;
        let Some(hit) = hit else {
            return Ok(None);
        };
        // Correlating the native window to its AT-SPI application needs a PID.
        let Some(pid) = hit.pid else {
            debug!(?point, "window_at_point returned a window without a PID; cannot correlate to AT-SPI");
            return Ok(None);
        };
        // Never resolve the host process's own UI (consistent with get_nodes,
        // which skips SELF_PID). A picker over its own window picks nothing.
        if pid == *SELF_PID {
            return Ok(None);
        }

        let conn = self.connection()?;
        let Some(app_obj) = application_for_pid(&conn, pid) else {
            debug!(pid, ?point, "no AT-SPI application matched the window PID");
            return Ok(None);
        };

        let window_manager = Some(window_manager);
        let app_node: Arc<dyn UiNode> = AtspiNode::new(conn.clone(), app_obj.clone(), None, window_manager.clone());
        // Geometric subtree search within the WM-selected application, scoped to
        // the hit window's frame when one matches (see `descend_to_point`).
        Ok(Some(descend_to_point(&conn, &window_manager, app_node, app_obj, point, hit.id)))
    }
}

/// Resolve the AT-SPI application accessible whose D-Bus connection belongs to
/// `target_pid`, by enumerating the registry's application children.
fn application_for_pid(conn: &Arc<AccessibilityConnection>, target_pid: u32) -> Option<ObjectRefOwned> {
    let proxy = block_on_timeout_init(
        AccessibleProxy::builder(conn.connection())
            .cache_properties(CacheProperties::No)
            .destination(REGISTRY_BUS)
            .ok()?
            .path(ROOT_PATH)
            .ok()?
            .build(),
    )?
    .ok()?;
    let children = block_on_timeout_init(proxy.get_children())?.ok()?;
    // A single process can register more than one AT-SPI application root
    // (Qt notably registers an empty/transient root alongside the real one).
    // Prefer the populated registration — the same reason `get_nodes` skips
    // 0-child apps — and only fall back to an empty match if none has children.
    let mut fallback: Option<ObjectRefOwned> = None;
    for child in children {
        if AtspiNode::is_null_object(&child) {
            continue;
        }
        let Some(bus_name) = child.name_as_str().map(str::to_owned) else {
            continue;
        };
        let conn_inner = conn.connection().clone();
        let app_pid = block_on_timeout_call(async move {
            let dbus = zbus::fdo::DBusProxy::new(&conn_inner).await.ok()?;
            dbus.get_connection_unix_process_id(zbus::names::BusName::try_from(bus_name.as_str()).ok()?).await.ok()
        })
        .flatten();
        if app_pid != Some(target_pid) {
            continue;
        }
        if node::accessible_children(conn.as_ref(), &child).is_empty() {
            fallback.get_or_insert(child);
        } else {
            return Some(child);
        }
    }
    fallback
}

/// Resolve the accessible under `point` within the application by searching its
/// AT-SPI tree for the smallest-area node whose screen bounds contain the point.
///
/// Scope: when a top-level frame maps to the window `window_at_point` selected
/// (`target_window`), the search is confined to that frame — this keeps
/// multi-window apps correct (hovering a child dialog resolves the dialog, not
/// the main window). Otherwise — notably when the hit window is an
/// override-redirect popup (a menu or context menu) that has no managed frame —
/// the whole application tree is searched, because such popups are nested inside
/// the owning frame's subtree (or exposed as a separate popup frame) and are
/// drawn larger than, and outside, that frame's bounds.
///
/// The search is a geometric bounds descent, not the native
/// `Component.GetAccessibleAtPoint`: the native hit-test proved unreliable
/// across toolkits (Qt reports bad screen extents; AccessKit returns the widget
/// *beneath* an overlay), whereas resolving each node's toolkit-aware
/// `Control:Bounds` and picking the smallest containing box reaches menu items
/// regardless of how the toolkit exposes the popup. It is deliberately *not*
/// pruned by parent bounds, so a menu drawn outside its owning frame is still
/// reached; a node budget guards against pathological trees.
fn descend_to_point(
    conn: &Arc<AccessibilityConnection>,
    window_manager: &Option<Arc<dyn WindowManager>>,
    app_node: Arc<dyn UiNode>,
    app_obj: ObjectRefOwned,
    point: Point,
    target_window: WindowId,
) -> Arc<dyn UiNode> {
    let (root_node, root_obj) = match frame_for_window(conn, window_manager, &app_node, &app_obj, target_window) {
        Some(frame) => frame,
        None => (Arc::clone(&app_node), app_obj),
    };

    let mut budget = SUBTREE_SEARCH_BUDGET;
    let mut best: Option<SubtreeHit> = None;
    search_subtree(conn, window_manager, &root_node, &root_obj, point, 0, &mut budget, &mut best);
    best.map_or(root_node, |hit| hit.node)
}

/// Cap on nodes visited by the geometric subtree search, so a pathological or
/// very large accessibility tree cannot make an interactive hit-test hang.
const SUBTREE_SEARCH_BUDGET: u32 = 5000;

/// Best match tracked during the geometric subtree search: the built node, the
/// area of its bounds, and its depth below the search root (for tie-breaking).
struct SubtreeHit {
    node: Arc<dyn UiNode>,
    area: f64,
    depth: usize,
}

/// The application's top-level frame that maps to `target_window`, or `None`
/// when no managed frame matches (e.g. the hit window is an override-redirect
/// popup, which the window manager does not expose as a client).
fn frame_for_window(
    conn: &Arc<AccessibilityConnection>,
    window_manager: &Option<Arc<dyn WindowManager>>,
    app_node: &Arc<dyn UiNode>,
    app_obj: &ObjectRefOwned,
    target_window: WindowId,
) -> Option<(Arc<dyn UiNode>, ObjectRefOwned)> {
    let wm = window_manager.as_ref()?;
    for frame_obj in node::accessible_children(conn.as_ref(), app_obj) {
        if AtspiNode::is_null_object(&frame_obj) {
            continue;
        }
        let frame_node: Arc<dyn UiNode> = {
            let node = AtspiNode::new(conn.clone(), frame_obj.clone(), Some(app_node), window_manager.clone());
            node.hold_parent(Arc::clone(app_node));
            node
        };
        if wm.resolve_window(frame_node.as_ref()).is_ok_and(|wid| wid == target_window) {
            return Some((frame_node, frame_obj));
        }
    }
    None
}

/// Recursively search `node`'s subtree for the smallest-area accessible whose
/// screen bounds contain `point`, updating `best`. Every visited child is built
/// with its parent wired and held alive (`hold_parent`), so returning the winner
/// keeps its whole ancestor chain walkable for tree reveal. `budget` is
/// decremented per visited node and stops the search at zero.
fn search_subtree(
    conn: &Arc<AccessibilityConnection>,
    window_manager: &Option<Arc<dyn WindowManager>>,
    node: &Arc<dyn UiNode>,
    obj: &ObjectRefOwned,
    point: Point,
    depth: usize,
    budget: &mut u32,
    best: &mut Option<SubtreeHit>,
) {
    for child_obj in node::accessible_children(conn.as_ref(), obj) {
        if *budget == 0 {
            return;
        }
        if AtspiNode::is_null_object(&child_obj) {
            continue;
        }
        *budget -= 1;
        let child_node: Arc<dyn UiNode> = {
            let n = AtspiNode::new(conn.clone(), child_obj.clone(), Some(node), window_manager.clone());
            n.hold_parent(Arc::clone(node));
            n
        };
        let child_depth = depth + 1;
        if let Some(bounds) = node_screen_bounds(&child_node)
            && bounds.contains(point)
            && node_is_pickable(&child_node)
        {
            let area = bounds.width() * bounds.height();
            // Smallest area wins (most specific box); on a tie the deeper node
            // wins (closest to the leaf under the cursor).
            let better =
                best.as_ref().is_none_or(|cur| area < cur.area || (area == cur.area && child_depth > cur.depth));
            if better {
                *best = Some(SubtreeHit { node: Arc::clone(&child_node), area, depth: child_depth });
            }
        }
        search_subtree(conn, window_manager, &child_node, &child_obj, point, child_depth, budget, best);
    }
}

/// Screen bounds of a node via its `Control:Bounds` attribute (toolkit-aware
/// extents resolution).
fn node_screen_bounds(node: &Arc<dyn UiNode>) -> Option<Rect> {
    use platynui_core::ui::{Namespace, UiValue, attribute_names::element};
    match node.attribute(Namespace::Control, element::BOUNDS)?.value() {
        UiValue::Rect(rect) => Some(rect),
        _ => None,
    }
}

/// Whether a node may be *selected* as the hit result. A node whose bounds
/// geometrically contain the point but which is not actually shown on screen
/// (hidden widget, closed-menu item still laid out in the tree) SHALL NOT be
/// picked. Only the candidate check is gated — the search still recurses into
/// such a node, since a shown child can live under a parent the toolkit reports
/// as not visible.
///
/// Gated on `Control:IsVisible` and, when the provider surfaces it,
/// `Control:IsInView`. Both are excluded only when *explicitly* false: a
/// missing or non-boolean value is treated as visible, so a provider that does
/// not report the attribute does not make everything unpickable. (`IsInView`'s
/// precise meaning is still being defined; excluding on an explicit `false`
/// keeps this correct once it diverges from `IsVisible`.)
fn node_is_pickable(node: &Arc<dyn UiNode>) -> bool {
    use platynui_core::ui::{Namespace, UiValue, attribute_names::element};
    let is_false =
        |name| matches!(node.attribute(Namespace::Control, name).map(|a| a.value()), Some(UiValue::Bool(false)));
    !is_false(element::IS_VISIBLE) && !is_false(element::IS_IN_VIEW)
}

pub static ATSPI_FACTORY: AtspiFactory = AtspiFactory;

// Auto-register the AT-SPI provider when linked.
platynui_core::register_provider!(&ATSPI_FACTORY);

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::config::{ConfigMap, RuntimeConfig};

    #[test]
    fn factory_reads_configured_bus_address() {
        let providers = ConfigMap::new()
            .with("atspi", ConfigMap::new().with("bus_address", "unix:path=/run/user/1000/at-spi/bus_1"));
        let config = RuntimeConfig::new(ConfigMap::new(), providers);
        assert_eq!(AtspiFactory.build(&config).bus_address.as_deref(), Some("unix:path=/run/user/1000/at-spi/bus_1"));
    }

    #[test]
    fn factory_defaults_to_env_discovery_without_config() {
        // No providers.atspi.bus_address → None → connect_a11y_bus_with falls back to env/default.
        assert_eq!(AtspiFactory.build(&RuntimeConfig::default()).bus_address, None);
    }
}
