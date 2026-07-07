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
use atspi_common::Role;
use atspi_connection::AccessibilityConnection;
use atspi_proxies::accessible::AccessibleProxy;
use platynui_core::config::RuntimeConfig;
use platynui_core::platform::WindowManager;
use platynui_core::provider::{ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory};
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
