//! AT-SPI2 UiTree provider for Unix desktops.
//!
//! Provides a blocking D-Bus integration to query the accessibility tree on
//! Linux/X11 systems. Event streaming and full WindowSurface integration will
//! follow in later phases.

mod connection;
mod ewmh;
mod node;

use crate::connection::connect_a11y_bus;
use crate::node::AtspiNode;
use atspi_common::Role;
use atspi_connection::AccessibilityConnection;
use atspi_proxies::accessible::AccessibleProxy;
use futures_lite::future::block_on;
use node::block_on_timeout;
use once_cell::sync::{Lazy, OnceCell};
use platynui_core::provider::{
    ProviderDescriptor, ProviderError, ProviderErrorKind, ProviderKind, UiTreeProvider, UiTreeProviderFactory,
};
use platynui_core::ui::{TechnologyId, UiNode};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};
use zbus::proxy::CacheProperties;

pub const PROVIDER_ID: &str = "atspi";
pub const PROVIDER_NAME: &str = "AT-SPI2";
pub static TECHNOLOGY: Lazy<TechnologyId> = Lazy::new(|| TechnologyId::from("AT-SPI2"));

const REGISTRY_BUS: &str = "org.a11y.atspi.Registry";
const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";

/// Timeout for D-Bus calls during provider initialisation.
const DBUS_TIMEOUT: Duration = Duration::from_secs(5);

/// Execute a future with a timeout. Returns `None` if the future does not
/// complete within [`DBUS_TIMEOUT`].
///
/// This variant uses the longer init timeout (5 s) for one-off calls during
/// provider startup such as building the registry proxy.
fn block_on_timeout_init<F: std::future::Future>(future: F) -> Option<F::Output> {
    block_on(async {
        futures_lite::future::or(async { Some(future.await) }, async {
            async_io::Timer::after(DBUS_TIMEOUT).await;
            None
        })
        .await
    })
}

pub struct AtspiFactory;

impl UiTreeProviderFactory for AtspiFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        static DESCRIPTOR: Lazy<ProviderDescriptor> = Lazy::new(|| {
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
    conn: OnceCell<Arc<AccessibilityConnection>>,
}

impl AtspiProvider {
    fn new() -> Self {
        static DESCRIPTOR: Lazy<ProviderDescriptor> = Lazy::new(|| {
            ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from("AT-SPI2"), ProviderKind::Native)
        });
        Self { descriptor: &DESCRIPTOR, conn: OnceCell::new() }
    }

    fn connection(&self) -> Result<Arc<AccessibilityConnection>, ProviderError> {
        self.conn
            .get_or_try_init(|| connect_a11y_bus().map(Arc::new))
            .map(Arc::clone)
            .map_err(|err| ProviderError::new(ProviderErrorKind::TreeUnavailable, err.to_string()))
    }
}

impl UiTreeProvider for AtspiProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
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
                .map_err(|err| {
                    ProviderError::new(ProviderErrorKind::CommunicationFailure, format!("registry destination: {err}"))
                })?
                .path(ROOT_PATH)
                .map_err(|err| {
                    ProviderError::new(ProviderErrorKind::CommunicationFailure, format!("registry path: {err}"))
                })?
                .build(),
        )
        .ok_or_else(|| ProviderError::new(ProviderErrorKind::CommunicationFailure, "registry proxy build timed out".to_string()))?
        .map_err(|err| ProviderError::new(ProviderErrorKind::CommunicationFailure, format!("registry proxy: {err}")))?;

        let children = block_on_timeout_init(proxy.get_children())
            .ok_or_else(|| ProviderError::new(ProviderErrorKind::CommunicationFailure, "registry children timed out".to_string()))?
            .map_err(|err| {
                ProviderError::new(ProviderErrorKind::CommunicationFailure, format!("registry children: {err}"))
            })?;

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
            let proxy = block_on_timeout(
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
            let child_count = block_on_timeout(proxy.child_count())?.ok()?;
            if child_count == 0 {
                debug!(app = %app_bus, "skipped (0 children)");
                return None;
            }

            // Pre-resolve interfaces, role, and name using the same
            // proxy so that AtspiNode caches are warm on first access.
            let interfaces = block_on_timeout(proxy.get_interfaces()).and_then(|r| r.ok());
            let role = block_on_timeout(proxy.get_role())
                .and_then(|r| r.ok())
                .unwrap_or(Role::Invalid);
            let node_name = block_on_timeout(proxy.name())
                .and_then(|r| r.ok())
                .and_then(node::normalize_value);

            let node = AtspiNode::new(conn.clone(), child, Some(&parent));
            // Seed caches directly — no additional D-Bus calls inside.
            let _ = node.cached_child_count.set(Some(child_count));
            let _ = node.interfaces.set(interfaces);
            let (ns, role_name) = node::map_role_with_interfaces(role, interfaces);
            let _ = node.namespace.set(ns);
            let _ = node.role.set(role_name);
            let _ = node.cached_name.set(node_name.clone());

            let elapsed = app_start.elapsed();
            debug!(
                app = %app_bus,
                name = node_name.as_deref().unwrap_or(""),
                children = child_count,
                elapsed_ms = elapsed.as_millis() as u64,
                "get_nodes: resolved app",
            );
            if elapsed.as_millis() > 200 {
                warn!(
                    app = %app_bus,
                    elapsed_ms = elapsed.as_millis() as u64,
                    "get_nodes: SLOW app resolution (>200ms)",
                );
            }

            Some(node as Arc<dyn UiNode>)
        })))
    }
}

pub static ATSPI_FACTORY: AtspiFactory = AtspiFactory;

// Auto-register the AT-SPI provider when linked.
platynui_core::register_provider!(&ATSPI_FACTORY);
