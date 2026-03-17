use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    InitializationFailed { provider: &'static str, details: Option<String> },

    UnsupportedOperation { operation: &'static str, details: Option<String> },

    CommunicationFailure { channel: &'static str, details: Option<String> },

    InvalidArgument { argument: &'static str, details: Option<String> },

    TreeUnavailable { provider: &'static str, details: Option<String> },
}

impl Display for ProviderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InitializationFailed { provider, details } => {
                write!(f, "provider initialization failed for {provider}")?;
                write_details(f, details)
            }
            Self::UnsupportedOperation { operation, details } => {
                write!(f, "unsupported provider operation: {operation}")?;
                write_details(f, details)
            }
            Self::CommunicationFailure { channel, details } => {
                write!(f, "provider communication failure on {channel}")?;
                write_details(f, details)
            }
            Self::InvalidArgument { argument, details } => {
                write!(f, "invalid provider argument: {argument}")?;
                write_details(f, details)
            }
            Self::TreeUnavailable { provider, details } => {
                write!(f, "provider tree unavailable for {provider}")?;
                write_details(f, details)
            }
        }
    }
}

impl Error for ProviderError {}

fn write_details(f: &mut Formatter<'_>, details: &Option<String>) -> std::fmt::Result {
    if let Some(details) = details { write!(f, ": {details}") } else { Ok(()) }
}
