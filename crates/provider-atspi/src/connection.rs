use atspi_connection::AccessibilityConnection;
use futures_lite::future::block_on;
use platynui_core::provider::{ProviderError, ProviderErrorKind};
use zbus::Address;

const A11Y_BUS_ENV: &str = "AT_SPI_BUS_ADDRESS";

pub fn connect_a11y_bus() -> Result<AccessibilityConnection, ProviderError> {
    if let Ok(address) = std::env::var(A11Y_BUS_ENV) {
        return connect_address(&address);
    }

    block_on(AccessibilityConnection::new()).map_err(|err| {
        ProviderError::new(ProviderErrorKind::InitializationFailed, format!("a11y connection failed: {err}"))
    })
}

fn connect_address(address: &str) -> Result<AccessibilityConnection, ProviderError> {
    let addr: Address = address.parse().map_err(|err| {
        ProviderError::new(ProviderErrorKind::InitializationFailed, format!("a11y bus address invalid: {err}"))
    })?;
    block_on(AccessibilityConnection::from_address(addr)).map_err(|err| {
        ProviderError::new(ProviderErrorKind::InitializationFailed, format!("a11y bus connect failed: {err}"))
    })
}
