use atspi_connection::AccessibilityConnection;
use zbus::Address;

use crate::error::AtspiError;
use crate::timeout::block_on_timeout_connect;

const A11Y_BUS_ENV: &str = "AT_SPI_BUS_ADDRESS";

/// Connect to the AT-SPI bus, preferring an explicit `bus_address` (from
/// `providers.atspi.bus_address`) over the environment/default discovery.
///
/// This is the config-aware entry point: `Some(addr)` binds to a specific
/// session's bus (needed when a runtime targets a display other than the
/// process default), while `None` reproduces the former behaviour
/// ([`connect_a11y_bus`], which honours `AT_SPI_BUS_ADDRESS` then discovers).
pub fn connect_a11y_bus_with(bus_address: Option<&str>) -> Result<AccessibilityConnection, AtspiError> {
    match bus_address {
        Some(address) => connect_address(address),
        None => connect_a11y_bus(),
    }
}

pub fn connect_a11y_bus() -> Result<AccessibilityConnection, AtspiError> {
    if let Ok(address) = std::env::var(A11Y_BUS_ENV) {
        tracing::debug!(address = %address, "connecting to AT-SPI bus via env address");
        return connect_address(&address);
    }

    tracing::debug!("connecting to AT-SPI bus via default session");
    let conn = block_on_timeout_connect(AccessibilityConnection::new())
        .ok_or_else(|| {
            tracing::error!("AT-SPI connection timed out");
            AtspiError::timeout("a11y connection")
        })?
        .map_err(|err| {
            tracing::error!(%err, "AT-SPI connection failed");
            AtspiError::ConnectionFailed(err.to_string())
        })?;
    tracing::info!("AT-SPI accessibility bus connected");
    Ok(conn)
}

fn connect_address(address: &str) -> Result<AccessibilityConnection, AtspiError> {
    let addr: Address =
        address.parse().map_err(|err| AtspiError::ConnectionFailed(format!("invalid bus address: {err}")))?;
    block_on_timeout_connect(AccessibilityConnection::from_address(addr))
        .ok_or_else(|| AtspiError::timeout("a11y bus connect"))?
        .map_err(|err| AtspiError::ConnectionFailed(err.to_string()))
}
