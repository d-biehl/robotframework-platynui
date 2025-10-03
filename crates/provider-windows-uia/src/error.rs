use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum UiaError {
    #[error("COM initialization failed: {0}")]
    ComInit(String),

    #[error("UIAutomation API error in {context}: {message}")]
    Api { context: &'static str, message: String },

    #[error("unexpected null from UIAutomation: {0}")]
    Null(&'static str),
}

impl UiaError {
    pub fn api(context: &'static str, err: impl ToString) -> Self {
        Self::Api { context, message: err.to_string() }
    }
}

