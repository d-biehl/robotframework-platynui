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

/// Connect to the accessibility bus at an explicit address.
///
/// Every failure names the address it tried. In a sidecar deployment the bus
/// address is redirected into another container, and a socket that is not
/// shared, or a uid that does not match, otherwise surfaces as a bare I/O error —
/// and one level up, where the runtime logs a failing provider and carries on, as
/// an empty tree that looks like anything else.
fn connect_address(address: &str) -> Result<AccessibilityConnection, AtspiError> {
    let addr: Address = address
        .parse()
        .map_err(|err| AtspiError::ConnectionFailed(format!("invalid accessibility bus address `{address}`: {err}")))?;
    block_on_timeout_connect(AccessibilityConnection::from_address(addr))
        .ok_or_else(|| {
            AtspiError::ConnectionFailed(format!("timed out connecting to the accessibility bus at `{address}`"))
        })?
        .map_err(|err| {
            AtspiError::ConnectionFailed(format!("cannot connect to the accessibility bus at `{address}`: {err}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bus socket nobody listens on — the shape of a sidecar whose bus socket
    /// was never shared into the runtime's container.
    fn unreachable_bus() -> (std::path::PathBuf, String) {
        let path = std::env::temp_dir().join(format!("platynui-no-a11y-bus-{}", std::process::id()));
        let address = format!("unix:path={}", path.display());
        (path, address)
    }

    #[test]
    fn an_unreachable_bus_names_the_address_it_tried() {
        let (path, address) = unreachable_bus();
        let err = connect_address(&address).expect_err("nothing listens on that socket");
        assert!(err.to_string().contains(&path.display().to_string()), "the error must name the bus: {err}");
    }

    #[test]
    fn a_malformed_address_is_named_too() {
        let err = connect_address("not-a-bus-address").expect_err("that is no D-Bus address");
        assert!(err.to_string().contains("not-a-bus-address"), "the error must name the address: {err}");
    }
}
