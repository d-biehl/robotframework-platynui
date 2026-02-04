//! AT-SPI2 UiTree provider for Unix desktops.
//!
//! Provides a blocking D-Bus integration to query the accessibility tree on
//! Linux/X11 systems. Event streaming and full WindowSurface integration will
//! follow in later phases.

mod connection;
mod node;

use crate::connection::connect_a11y_bus;
use crate::node::AtspiNode;
use atspi_connection::AccessibilityConnection;
use atspi_proxies::accessible::AccessibleProxy;
use futures_lite::future::block_on;
use once_cell::sync::{Lazy, OnceCell};
use platynui_core::provider::{
    ProviderDescriptor, ProviderError, ProviderErrorKind, ProviderKind, UiTreeProvider, UiTreeProviderFactory,
};
use platynui_core::ui::{TechnologyId, UiNode};
use std::sync::Arc;
use zbus::proxy::CacheProperties;

pub const PROVIDER_ID: &str = "atspi";
pub const PROVIDER_NAME: &str = "AT-SPI2";
pub static TECHNOLOGY: Lazy<TechnologyId> = Lazy::new(|| TechnologyId::from("AT-SPI2"));

const REGISTRY_BUS: &str = "org.a11y.atspi.Registry";
const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";

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
        let proxy = block_on(
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
        .map_err(|err| ProviderError::new(ProviderErrorKind::CommunicationFailure, format!("registry proxy: {err}")))?;

        let children = block_on(proxy.get_children()).map_err(|err| {
            ProviderError::new(ProviderErrorKind::CommunicationFailure, format!("registry children: {err}"))
        })?;

        let parent = Arc::clone(&parent);
        let conn = conn.clone();
        Ok(Box::new(children.into_iter().filter_map(move |child| {
            if AtspiNode::is_null_object(&child) {
                return None;
            }
            let node = AtspiNode::new(conn.clone(), child, Some(&parent));
            Some(node as Arc<dyn UiNode>)
        })))
    }
}

pub static ATSPI_FACTORY: AtspiFactory = AtspiFactory;

// Auto-register the AT-SPI provider when linked.
platynui_core::register_provider!(&ATSPI_FACTORY);
