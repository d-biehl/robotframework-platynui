use std::fmt::{Display, Formatter};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context(pub Option<String>);

impl Display for Context {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(s) = &self.0 { write!(f, ": {s}") } else { Ok(()) }
    }
}

/// Fully-typed platform error used across platform integration crates.
///
/// Keep `PlatformErrorKind` and the legacy constructors `new/simple` for
/// compatibility with existing call sites in the workspace.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PlatformError {
    #[error("platform initialization failed{context}")]
    InitializationFailed { context: Context },

    #[error("platform capability unavailable{context}")]
    CapabilityUnavailable { context: Context },

    #[error("unsupported platform{context}")]
    UnsupportedPlatform { context: Context },
}

impl PlatformError {
    /// Back-compat constructor used by existing code paths.
    pub fn new(kind: PlatformErrorKind, message: impl Into<String>) -> Self {
        let context = Context(Some(message.into()));
        match kind {
            PlatformErrorKind::InitializationFailed => Self::InitializationFailed { context },
            PlatformErrorKind::CapabilityUnavailable => Self::CapabilityUnavailable { context },
            PlatformErrorKind::UnsupportedPlatform => Self::UnsupportedPlatform { context },
        }
    }

    /// Back-compat constructor without message.
    pub fn simple(kind: PlatformErrorKind) -> Self {
        match kind {
            PlatformErrorKind::InitializationFailed => Self::InitializationFailed { context: Context(None) },
            PlatformErrorKind::CapabilityUnavailable => Self::CapabilityUnavailable { context: Context(None) },
            PlatformErrorKind::UnsupportedPlatform => Self::UnsupportedPlatform { context: Context(None) },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformErrorKind {
    InitializationFailed,
    CapabilityUnavailable,
    UnsupportedPlatform,
}
