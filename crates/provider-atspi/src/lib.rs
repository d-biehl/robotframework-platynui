//! AT-SPI2 UiTree provider for Unix desktops.
//!
//! Provides a blocking D-Bus integration to query the accessibility tree on
//! Linux/X11 systems. Event streaming and full WindowSurface integration will
//! follow in later phases.

pub(crate) mod clearable_cell;
pub(crate) mod error;

mod connection;
mod node;
mod timeout;

use crate::clearable_cell::ClearableCell;
use crate::connection::connect_a11y_bus;
use crate::error::AtspiError;
use crate::node::AtspiNode;
use atspi_common::Role;
use atspi_connection::AccessibilityConnection;
use atspi_proxies::accessible::AccessibleProxy;
use platynui_core::provider::{ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory};
use platynui_core::ui::{TechnologyId, UiNode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use tracing::{info, trace, warn};
use zbus::proxy::CacheProperties;

use crate::timeout::{block_on_timeout_call, block_on_timeout_init};

pub const PROVIDER_ID: &str = "atspi";
pub const PROVIDER_NAME: &str = "AT-SPI2";
pub static TECHNOLOGY: LazyLock<TechnologyId> = LazyLock::new(|| TechnologyId::from("AT-SPI2"));

const REGISTRY_BUS: &str = "org.a11y.atspi.Registry";
const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";

pub struct AtspiFactory;

impl UiTreeProviderFactory for AtspiFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
            ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from("AT-SPI2"), ProviderKind::Native)
        });
        &DESCRIPTOR
    }

    fn create(&self) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        Ok(Arc::new(AtspiProvider::new()))
    }
}

pub struct AtspiProvider {
    descriptor: &'static ProviderDescriptor,
    conn: ClearableCell<Arc<AccessibilityConnection>>,
    is_shutdown: AtomicBool,
}

impl AtspiProvider {
    fn new() -> Self {
        static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
            ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from("AT-SPI2"), ProviderKind::Native)
        });
        Self { descriptor: &DESCRIPTOR, conn: ClearableCell::new(), is_shutdown: AtomicBool::new(false) }
    }

    fn connection(&self) -> Result<Arc<AccessibilityConnection>, AtspiError> {
        if self.is_shutdown.load(Ordering::Acquire) {
            return Err(AtspiError::Shutdown);
        }
        self.conn.get_or_try_init(|| Ok(Arc::new(connect_a11y_bus()?)))
    }
}

impl UiTreeProvider for AtspiProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
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
        Ok(Box::new(children.into_iter().filter_map(move |child| {
            if AtspiNode::is_null_object(&child) {
                return None;
            }
            let app_bus = child.name_as_str().unwrap_or("<unknown>").to_string();
            let app_start = std::time::Instant::now();

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

            let node = AtspiNode::new(conn.clone(), child, Some(&parent));
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
