use std::fmt::{Display, Formatter};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context(pub Option<String>);

impl Display for Context {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(s) = &self.0 { write!(f, ": {s}") } else { Ok(()) }
    }
}

/// Fully-typed provider error with variants for common failure classes.
///
/// For back-compat with existing code, the helper constructors `new/simple`
/// remain and map a `ProviderErrorKind` + optional message onto variants.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProviderError {
    #[error("provider initialization failed{context}")]
    InitializationFailed { context: Context },

    #[error("unsupported provider operation{context}")]
    UnsupportedOperation { context: Context },

    #[error("provider communication failure{context}")]
    CommunicationFailure { context: Context },

    #[error("invalid argument for provider{context}")]
    InvalidArgument { context: Context },

    #[error("provider tree unavailable{context}")]
    TreeUnavailable { context: Context },
}

impl ProviderError {
    pub fn new(kind: ProviderErrorKind, message: impl Into<String>) -> Self {
        let context = Context(Some(message.into()));
        match kind {
            ProviderErrorKind::InitializationFailed => Self::InitializationFailed { context },
            ProviderErrorKind::UnsupportedOperation => Self::UnsupportedOperation { context },
            ProviderErrorKind::CommunicationFailure => Self::CommunicationFailure { context },
            ProviderErrorKind::InvalidArgument => Self::InvalidArgument { context },
            ProviderErrorKind::TreeUnavailable => Self::TreeUnavailable { context },
        }
    }

    pub fn simple(kind: ProviderErrorKind) -> Self {
        match kind {
            ProviderErrorKind::InitializationFailed => Self::InitializationFailed { context: Context(None) },
            ProviderErrorKind::UnsupportedOperation => Self::UnsupportedOperation { context: Context(None) },
            ProviderErrorKind::CommunicationFailure => Self::CommunicationFailure { context: Context(None) },
            ProviderErrorKind::InvalidArgument => Self::InvalidArgument { context: Context(None) },
            ProviderErrorKind::TreeUnavailable => Self::TreeUnavailable { context: Context(None) },
        }
    }
}

/// Categorises provider failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderErrorKind {
    InitializationFailed,
    UnsupportedOperation,
    CommunicationFailure,
    InvalidArgument,
    TreeUnavailable,
}
