use std::fmt;
use std::time::Duration;

use platynui_core::platform::KeyboardError;
use platynui_core::ui::PatternError;
use thiserror::Error;

use crate::keyboard_sequence::{KeyboardSequenceError, SyntaxHint};

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

/// A keyboard action that failed.
///
/// Its `Display` never repeats the sequence as a whole: a position counts the
/// characters of the sequence from 1, and only an error about one character
/// or key names it, with the device's reason. [`sensitive`](Self::sensitive)
/// gives a rendering without any part of the sequence and without the
/// device's text, for a sequence that must not appear anywhere (a `Secret`).
#[derive(Debug, Error)]
pub enum KeyboardActionError {
    /// The sequence does not parse.
    #[error(transparent)]
    Sequence(Box<KeyboardSequenceError>),
    /// A character (`shortcut == false`) or a key name of a shortcut
    /// (`shortcut == true`) at `position` cannot be converted into a key.
    #[error(
        "{} at position {position} cannot be typed: {source}{}",
        key_subject(.key, *.shortcut),
        key_hint(*.shortcut)
    )]
    Key { position: usize, key: String, shortcut: bool, source: KeyboardError },
    /// The keyboard device could not start the input.
    #[error("starting the keyboard input failed: {0}")]
    Start(#[source] KeyboardError),
    /// The keyboard device failed while the input was sent or ended.
    #[error("sending the keyboard input failed: {0}")]
    Send(#[source] KeyboardError),
    /// The runtime has no keyboard device ([`KeyboardError::NotReady`]).
    #[error(transparent)]
    Keyboard(#[from] KeyboardError),
}

impl KeyboardActionError {
    /// The position in the sequence the error refers to, counted in
    /// characters from 1; `None` for a failure that does not depend on a
    /// character or key.
    #[must_use]
    pub fn position(&self) -> Option<usize> {
        match self {
            KeyboardActionError::Sequence(err) => Some(err.position()),
            KeyboardActionError::Key { position, .. } => Some(*position),
            KeyboardActionError::Start(_) | KeyboardActionError::Send(_) | KeyboardActionError::Keyboard(_) => None,
        }
    }

    /// How to write a character of the sequence syntax literally, when the
    /// message suggests it: `\<` for an unknown key name of a shortcut, an
    /// unclosed key block or a missing key name; `\\` for a backslash that
    /// escapes nothing.
    #[must_use]
    pub fn syntax_hint(&self) -> Option<SyntaxHint> {
        match self {
            KeyboardActionError::Sequence(err) => err.syntax_hint(),
            KeyboardActionError::Key { shortcut: true, .. } => Some(SyntaxHint::LiteralLessThan),
            KeyboardActionError::Key { shortcut: false, .. }
            | KeyboardActionError::Start(_)
            | KeyboardActionError::Send(_)
            | KeyboardActionError::Keyboard(_) => None,
        }
    }

    /// The rendering for a sequence that must not appear anywhere: the
    /// position and the kind of failure, with the syntax hint where one
    /// applies, and no character, key name or text of the device.
    ///
    /// A failure without a position is given by its kind: no keyboard device
    /// is ready, a keyboard input is already active, starting the input
    /// failed, or sending it failed.
    #[must_use]
    pub fn sensitive(&self) -> impl fmt::Display + '_ {
        SensitiveKeyboardError(self)
    }
}

/// See [`KeyboardActionError::sensitive`].
struct SensitiveKeyboardError<'a>(&'a KeyboardActionError);

impl fmt::Display for SensitiveKeyboardError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            KeyboardActionError::Sequence(err) => f.write_str(&err.sensitive()),
            KeyboardActionError::Key { position, shortcut, .. } => {
                let subject = if *shortcut { "the key" } else { "the character" };
                write!(f, "{subject} at position {position} cannot be typed{}", key_hint(*shortcut))
            }
            KeyboardActionError::Start(err) => f.write_str(device_failure(err, "starting the keyboard input failed")),
            KeyboardActionError::Send(err) | KeyboardActionError::Keyboard(err) => {
                f.write_str(device_failure(err, "sending the keyboard input failed"))
            }
        }
    }
}

/// A device error by its kind, without the device's text.
fn device_failure(err: &KeyboardError, otherwise: &'static str) -> &'static str {
    match err {
        KeyboardError::NotReady => "no keyboard device is ready",
        KeyboardError::InputInProgress => "a keyboard input is already active",
        KeyboardError::Platform(_) | KeyboardError::UnsupportedKey(_) => otherwise,
    }
}

fn key_subject(key: &str, shortcut: bool) -> String {
    let subject = if shortcut { "the key" } else { "the character" };
    format!("{subject} '{}'", key.escape_debug())
}

fn key_hint(shortcut: bool) -> String {
    if shortcut { format!("; {}", SyntaxHint::LiteralLessThan.text()) } else { String::new() }
}

#[derive(Debug, Error)]
pub enum BringToFrontError {
    #[error("node `{runtime_id}` has no window-capable ancestor (Activatable pattern missing)")]
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
