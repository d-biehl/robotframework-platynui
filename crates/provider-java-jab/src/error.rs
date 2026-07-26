//! Typed error definitions for the JAB backend.

use crate::ffi::VmId;
use platynui_core::provider::ProviderError;
use platynui_core::ui::PatternError;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub(crate) enum JabError {
    /// The client DLL could not be discovered or loaded.
    #[error("JAB client unavailable: {0}")]
    ClientUnavailable(String),

    /// A JAB call did not complete within `providers.java.jab.call_timeout_ms`.
    #[error("JAB call timed out: {op}")]
    Timeout { op: &'static str },

    /// The JVM behind `vmID` is marked degraded after repeated timeouts and is
    /// skipped until a health probe succeeds.
    #[error("JVM (vmID {vm}) is degraded after repeated timeouts")]
    VmDegraded { vm: VmId },

    /// The bridge returned FALSE for a call that carries no further error
    /// information (element gone, JVM shutting down, …).
    #[error("JAB call failed: {op}")]
    CallFailed { op: &'static str },

    /// The pump thread is gone (provider shut down or thread died).
    #[error("JAB pump thread is not available")]
    PumpUnavailable,

    /// No platform `WindowManager` was injected into the provider.
    #[error("no WindowManager registered")]
    NoWindowManager,

    /// The owning node has been dropped (weak reference expired).
    #[error("owning node has been dropped")]
    NodeDropped,

    /// The provider has been shut down and can no longer service requests.
    #[error("JAB backend has been shut down")]
    Shutdown,
}

impl From<JabError> for ProviderError {
    fn from(err: JabError) -> Self {
        let message = Some(err.to_string());
        match err {
            JabError::ClientUnavailable(_) => ProviderError::InitializationFailed { provider: "jab", details: message },
            JabError::Timeout { .. }
            | JabError::VmDegraded { .. }
            | JabError::CallFailed { .. }
            | JabError::PumpUnavailable
            | JabError::Shutdown => ProviderError::CommunicationFailure { channel: "jab", details: message },
            JabError::NoWindowManager | JabError::NodeDropped => {
                ProviderError::UnsupportedOperation { operation: "jab operation", details: message }
            }
        }
    }
}

impl From<JabError> for PatternError {
    fn from(err: JabError) -> Self {
        PatternError::new(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_unavailable_becomes_init_failed() {
        let err: ProviderError = JabError::ClientUnavailable("no dll".into()).into();
        assert!(matches!(err, ProviderError::InitializationFailed { .. }));
    }

    #[test]
    fn timeout_becomes_communication_failure() {
        let err: ProviderError = JabError::Timeout { op: "getAccessibleContextInfo" }.into();
        assert!(matches!(err, ProviderError::CommunicationFailure { .. }));
    }

    #[test]
    fn pattern_error_carries_message() {
        let err: PatternError = JabError::NoWindowManager.into();
        assert!(err.message().contains("WindowManager"));
    }
}
