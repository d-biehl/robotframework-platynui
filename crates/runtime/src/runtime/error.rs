use std::time::Duration;

use platynui_core::platform::KeyboardError;
use platynui_core::ui::PatternError;
use thiserror::Error;

use crate::keyboard_sequence::KeyboardSequenceError;

#[derive(Debug, Error)]
pub enum FocusError {
    #[error("node `{runtime_id}` does not expose the Focusable pattern")]
    PatternMissing { runtime_id: String },
    #[error("focus action failed for node `{runtime_id}`: {source}")]
    ActionFailed {
        runtime_id: String,
        #[source]
        source: PatternError,
    },
}

#[derive(Debug, Error)]
pub enum KeyboardActionError {
    #[error("invalid keyboard sequence: {0}")]
    Sequence(Box<KeyboardSequenceError>),
    #[error(transparent)]
    Keyboard(#[from] KeyboardError),
}

#[derive(Debug, Error)]
pub enum BringToFrontError {
    #[error("node `{runtime_id}` has no window-capable ancestor (WindowSurface pattern missing)")]
    PatternMissing { runtime_id: String },
    #[error("bringing window `{runtime_id}` to front failed: {source}")]
    ActionFailed {
        runtime_id: String,
        #[source]
        source: PatternError,
    },
    #[error("window `{runtime_id}` did not become input-ready within {waited:?}")]
    Timeout { runtime_id: String, waited: Duration },
}

impl From<KeyboardSequenceError> for KeyboardActionError {
    fn from(err: KeyboardSequenceError) -> Self {
        KeyboardActionError::Sequence(Box::new(err))
    }
}
