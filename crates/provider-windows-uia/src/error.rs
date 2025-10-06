use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum UiaError {
    #[error("COM initialization failed: {0}")]
    ComInit(String),

    #[error("UIAutomation API error in {context}: {message}")]
    Api { context: &'static str, message: String },

    #[error("unexpected null from UIAutomation: {0}")]
    Null(&'static str),

    #[error("no clickable point available for element")]
    NoClickablePoint,
}

impl UiaError {
    pub fn api(context: &'static str, err: impl ToString) -> Self {
        Self::Api { context, message: err.to_string() }
    }
}

/// Helper to map `windows` API errors into `UiaError` with a static context string.
pub fn uia_api<T>(context: &'static str, result: windows::core::Result<T>) -> Result<T, UiaError> {
    result.map_err(|e| UiaError::api(context, e))
}
