use atspi_connection::AccessibilityConnection;
use platynui_core::provider::{ProviderError, ProviderErrorKind};
use zbus::Address;

use crate::timeout::block_on_timeout_connect;

const A11Y_BUS_ENV: &str = "AT_SPI_BUS_ADDRESS";

pub fn connect_a11y_bus() -> Result<AccessibilityConnection, ProviderError> {
    if let Ok(address) = std::env::var(A11Y_BUS_ENV) {
        tracing::debug!(address = %address, "connecting to AT-SPI bus via env address");
        return connect_address(&address);
    }

    tracing::debug!("connecting to AT-SPI bus via default session");
    let conn = block_on_timeout_connect(AccessibilityConnection::new())
        .ok_or_else(|| {
            tracing::error!("AT-SPI connection timed out");
            ProviderError::new(ProviderErrorKind::InitializationFailed, "a11y connection timed out".to_string())
        })?
        .map_err(|err| {
            tracing::error!(%err, "AT-SPI connection failed");
            ProviderError::new(ProviderErrorKind::InitializationFailed, format!("a11y connection failed: {err}"))
        })?;
    tracing::info!("AT-SPI accessibility bus connected");
    Ok(conn)
}

fn connect_address(address: &str) -> Result<AccessibilityConnection, ProviderError> {
    let addr: Address = address.parse().map_err(|err| {
        ProviderError::new(ProviderErrorKind::InitializationFailed, format!("a11y bus address invalid: {err}"))
    })?;
    block_on_timeout_connect(AccessibilityConnection::from_address(addr))
        .ok_or_else(|| {
            ProviderError::new(ProviderErrorKind::InitializationFailed, "a11y bus connect timed out".to_string())
        })?
        .map_err(|err| {
            ProviderError::new(ProviderErrorKind::InitializationFailed, format!("a11y bus connect failed: {err}"))
        })
}
