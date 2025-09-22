use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformError {
    pub kind: PlatformErrorKind,
    pub message: Option<String>,
}

impl PlatformError {
    pub fn new(kind: PlatformErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: Some(message.into()) }
    }

    pub fn simple(kind: PlatformErrorKind) -> Self {
        Self { kind, message: None }
    }
}

impl Display for PlatformError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.message {
            Some(msg) => write!(f, "{msg}"),
            None => write!(f, "{:#?}", self.kind),
        }
    }
}

impl Error for PlatformError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformErrorKind {
    InitializationFailed,
    CapabilityUnavailable,
    UnsupportedPlatform,
}
