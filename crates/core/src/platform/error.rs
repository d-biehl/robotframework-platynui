use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformError {
    InitializationFailed { component: &'static str, details: Option<String> },

    CapabilityUnavailable { capability: &'static str, details: Option<String> },

    UnsupportedPlatform { platform: &'static str, details: Option<String> },

    OperationFailed { operation: &'static str, details: Option<String> },
}

impl Display for PlatformError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InitializationFailed { component, details } => {
                write!(f, "platform initialization failed for {component}")?;
                write_details(f, details)
            }
            Self::CapabilityUnavailable { capability, details } => {
                write!(f, "platform capability unavailable: {capability}")?;
                write_details(f, details)
            }
            Self::UnsupportedPlatform { platform, details } => {
                write!(f, "unsupported platform: {platform}")?;
                write_details(f, details)
            }
            Self::OperationFailed { operation, details } => {
                write!(f, "platform operation failed: {operation}")?;
                write_details(f, details)
            }
        }
    }
}

impl Error for PlatformError {}

fn write_details(f: &mut Formatter<'_>, details: &Option<String>) -> std::fmt::Result {
    if let Some(details) = details { write!(f, ": {details}") } else { Ok(()) }
}
