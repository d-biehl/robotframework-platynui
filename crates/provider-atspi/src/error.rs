//! Typed error definitions for the AT-SPI provider.
//!
//! [`AtspiError`] covers the common failure modes encountered when
//! communicating with the accessibility bus.  It converts losslessly into
//! [`ProviderError`] and [`PatternError`] so call-sites can use `?` without
//! ad-hoc string formatting.

use platynui_core::provider::{ProviderError, ProviderErrorKind};
use platynui_core::ui::PatternError;
use thiserror::Error;

/// Typed error for AT-SPI D-Bus operations.
#[derive(Debug, Error, Clone)]
pub enum AtspiError {
    /// The accessibility bus connection could not be established.
    #[error("AT-SPI connection failed: {0}")]
    ConnectionFailed(String),

    /// A D-Bus call did not complete within the configured timeout.
    #[error("AT-SPI call timed out: {context}")]
    Timeout {
        /// Human-readable description of the operation that timed out.
        context: &'static str,
    },

    /// A D-Bus method call or property read returned an error.
    #[error("AT-SPI D-Bus error in {context}: {message}")]
    DBus {
        /// Operation that produced the error.
        context: &'static str,
        /// Stringified D-Bus error message.
        message: String,
    },

    /// A required D-Bus proxy could not be constructed (e.g. missing bus
    /// name or invalid object path).
    #[error("AT-SPI proxy unavailable: {0}")]
    ProxyUnavailable(&'static str),

    /// The expected interface is not supported by the target accessible.
    #[error("AT-SPI interface missing: {0}")]
    InterfaceMissing(&'static str),

    /// No platform [`WindowManager`] is registered.
    #[error("no WindowManager registered")]
    NoWindowManager,

    /// The owning node has been dropped (weak reference expired).
    #[error("owning node has been dropped")]
    NodeDropped,

    /// A focus request returned `false`.
    #[error("grab_focus returned false")]
    FocusFailed,

    /// The provider has been shut down and can no longer service requests.
    #[error("AT-SPI provider has been shut down")]
    Shutdown,
}

impl AtspiError {
    /// Create a [`DBus`](AtspiError::DBus) variant from a context string and
    /// any error that implements [`ToString`].
    pub fn dbus(context: &'static str, err: impl ToString) -> Self {
        Self::DBus { context, message: err.to_string() }
    }

    /// Create a [`Timeout`](AtspiError::Timeout) variant.
    pub fn timeout(context: &'static str) -> Self {
        Self::Timeout { context }
    }
}

impl From<AtspiError> for ProviderError {
    fn from(err: AtspiError) -> Self {
        let kind = match &err {
            AtspiError::ConnectionFailed(_) => ProviderErrorKind::InitializationFailed,
            AtspiError::Timeout { .. } | AtspiError::DBus { .. } => ProviderErrorKind::CommunicationFailure,
            AtspiError::ProxyUnavailable(_) | AtspiError::InterfaceMissing(_) => {
                ProviderErrorKind::CommunicationFailure
            }
            AtspiError::NoWindowManager | AtspiError::NodeDropped | AtspiError::FocusFailed => {
                ProviderErrorKind::UnsupportedOperation
            }
            AtspiError::Shutdown => ProviderErrorKind::CommunicationFailure,
        };
        ProviderError::new(kind, err.to_string())
    }
}

impl From<AtspiError> for PatternError {
    fn from(err: AtspiError) -> Self {
        PatternError::new(err.to_string())
    }
}
