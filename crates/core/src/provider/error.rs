use std::error::Error;
use std::fmt::{Display, Formatter};

/// General error reported by providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub message: Option<String>,
}

impl ProviderError {
    pub fn new(kind: ProviderErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: Some(message.into()) }
    }

    pub fn simple(kind: ProviderErrorKind) -> Self {
        Self { kind, message: None }
    }
}

impl Display for ProviderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.message {
            Some(msg) => write!(f, "{msg}"),
            None => write!(f, "{:#?}", self.kind),
        }
    }
}

impl Error for ProviderError {}

/// Categorises provider failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderErrorKind {
    InitializationFailed,
    UnsupportedOperation,
    CommunicationFailure,
    InvalidArgument,
    TreeUnavailable,
}
