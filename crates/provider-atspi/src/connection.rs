use atspi_connection::AccessibilityConnection;
use futures_lite::future::block_on;
use platynui_core::provider::{ProviderError, ProviderErrorKind};
use std::time::Duration;
use zbus::Address;

const A11Y_BUS_ENV: &str = "AT_SPI_BUS_ADDRESS";
/// Generous timeout for connection establishment (one-time cost).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

fn block_on_timeout<F: std::future::Future>(future: F) -> Option<F::Output> {
    block_on(async {
        futures_lite::future::or(async { Some(future.await) }, async {
            async_io::Timer::after(CONNECT_TIMEOUT).await;
            None
        })
        .await
    })
}

pub fn connect_a11y_bus() -> Result<AccessibilityConnection, ProviderError> {
    if let Ok(address) = std::env::var(A11Y_BUS_ENV) {
        return connect_address(&address);
    }

    block_on_timeout(AccessibilityConnection::new())
        .ok_or_else(|| {
            ProviderError::new(ProviderErrorKind::InitializationFailed, "a11y connection timed out".to_string())
        })?
        .map_err(|err| {
            ProviderError::new(ProviderErrorKind::InitializationFailed, format!("a11y connection failed: {err}"))
        })
}

fn connect_address(address: &str) -> Result<AccessibilityConnection, ProviderError> {
    let addr: Address = address.parse().map_err(|err| {
        ProviderError::new(ProviderErrorKind::InitializationFailed, format!("a11y bus address invalid: {err}"))
    })?;
    block_on_timeout(AccessibilityConnection::from_address(addr))
        .ok_or_else(|| {
            ProviderError::new(ProviderErrorKind::InitializationFailed, "a11y bus connect timed out".to_string())
        })?
        .map_err(|err| {
            ProviderError::new(ProviderErrorKind::InitializationFailed, format!("a11y bus connect failed: {err}"))
        })
}
