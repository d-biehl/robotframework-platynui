use crate::platform::PlatformError;
use std::any::Any;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

/// Opaque platform-specific key code owned by the keyboard provider.
#[derive(Clone)]
pub struct KeyCode(Arc<dyn Any + Send + Sync>);

impl KeyCode {
    pub fn new<T: Send + Sync + 'static>(value: T) -> Self {
        Self(Arc::new(value))
    }

    pub fn downcast_ref<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.0.as_ref().downcast_ref::<T>()
    }
}

impl fmt::Debug for KeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KeyCode(..)")
    }
}

impl PartialEq for KeyCode {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for KeyCode {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyState {
    Press,
    Release,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyboardEvent {
    pub code: KeyCode,
    pub state: KeyState,
}

impl KeyboardEvent {
    pub fn state(&self) -> KeyState {
        self.state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyCodeError {
    UnknownKey(String),
    DuplicateKey(String),
    UnsupportedDescriptor(String),
}

impl fmt::Display for KeyCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyCodeError::UnknownKey(name) => write!(f, "unknown key descriptor '{name}'"),
            KeyCodeError::DuplicateKey(name) => write!(f, "duplicate key descriptor '{name}'"),
            KeyCodeError::UnsupportedDescriptor(name) => {
                write!(f, "descriptor '{name}' is not supported on this platform")
            }
        }
    }
}

impl std::error::Error for KeyCodeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyboardError {
    Platform(PlatformError),
    UnsupportedKey(String),
    InputInProgress,
    NotReady,
}

impl fmt::Display for KeyboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyboardError::Platform(err) => write!(f, "platform error: {err}"),
            KeyboardError::UnsupportedKey(name) => {
                write!(f, "unsupported key '{name}' for this keyboard provider")
            }
            KeyboardError::InputInProgress => {
                write!(f, "a keyboard input sequence is already active")
            }
            KeyboardError::NotReady => write!(f, "keyboard provider is not ready"),
        }
    }
}

impl std::error::Error for KeyboardError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            KeyboardError::Platform(err) => Some(err),
            _ => None,
        }
    }
}

impl From<PlatformError> for KeyboardError {
    fn from(value: PlatformError) -> Self {
        KeyboardError::Platform(value)
    }
}

/// Global keyboard configuration controlled by the runtime.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyboardSettings {
    pub press_delay: Duration,
    pub release_delay: Duration,
    pub between_keys_delay: Duration,
    pub chord_press_delay: Duration,
    pub chord_release_delay: Duration,
    pub after_sequence_delay: Duration,
    pub after_text_delay: Duration,
}

impl Default for KeyboardSettings {
    fn default() -> Self {
        Self {
            press_delay: Duration::from_millis(35),
            release_delay: Duration::from_millis(25),
            between_keys_delay: Duration::from_millis(40),
            chord_press_delay: Duration::from_millis(45),
            chord_release_delay: Duration::from_millis(45),
            after_sequence_delay: Duration::from_millis(75),
            after_text_delay: Duration::from_millis(20),
        }
    }
}

/// Per-call overrides that tweak keyboard timings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyboardOverrides {
    pub press_delay: Option<Duration>,
    pub release_delay: Option<Duration>,
    pub between_keys_delay: Option<Duration>,
    pub chord_press_delay: Option<Duration>,
    pub chord_release_delay: Option<Duration>,
    pub after_sequence_delay: Option<Duration>,
    pub after_text_delay: Option<Duration>,
}

impl KeyboardOverrides {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn builder() -> Self {
        Self::default()
    }

    pub fn press_delay(mut self, delay: Duration) -> Self {
        self.press_delay = Some(delay);
        self
    }

    pub fn release_delay(mut self, delay: Duration) -> Self {
        self.release_delay = Some(delay);
        self
    }

    pub fn between_keys_delay(mut self, delay: Duration) -> Self {
        self.between_keys_delay = Some(delay);
        self
    }

    pub fn chord_press_delay(mut self, delay: Duration) -> Self {
        self.chord_press_delay = Some(delay);
        self
    }

    pub fn chord_release_delay(mut self, delay: Duration) -> Self {
        self.chord_release_delay = Some(delay);
        self
    }

    pub fn after_sequence_delay(mut self, delay: Duration) -> Self {
        self.after_sequence_delay = Some(delay);
        self
    }

    pub fn after_text_delay(mut self, delay: Duration) -> Self {
        self.after_text_delay = Some(delay);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.press_delay.is_none()
            && self.release_delay.is_none()
            && self.between_keys_delay.is_none()
            && self.chord_press_delay.is_none()
            && self.chord_release_delay.is_none()
            && self.after_sequence_delay.is_none()
            && self.after_text_delay.is_none()
    }
}

/// Trait implemented by platform keyboard providers.
pub trait KeyboardDevice: Send + Sync {
    fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError>;

    fn start_input(&self) -> Result<(), KeyboardError> {
        Ok(())
    }

    fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError>;

    fn end_input(&self) -> Result<(), KeyboardError> {
        Ok(())
    }
}

pub struct KeyboardRegistration {
    pub device: &'static dyn KeyboardDevice,
}

inventory::collect!(KeyboardRegistration);

pub fn keyboard_devices() -> impl Iterator<Item = &'static dyn KeyboardDevice> {
    inventory::iter::<KeyboardRegistration>.into_iter().map(|entry| entry.device)
}

#[macro_export]
macro_rules! register_keyboard_device {
    ($device:expr) => {
        inventory::submit! {
            $crate::platform::KeyboardRegistration { device: $device }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::PlatformErrorKind;
    use rstest::rstest;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct StubKeyboard {
        send_calls: AtomicUsize,
    }

    impl StubKeyboard {
        const fn new() -> Self {
            Self { send_calls: AtomicUsize::new(0) }
        }
    }

    impl KeyboardDevice for StubKeyboard {
        fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
            if name.eq_ignore_ascii_case("Control") {
                Ok(KeyCode::new(0x11u32))
            } else {
                Err(KeyboardError::UnsupportedKey(name.to_owned()))
            }
        }

        fn send_key_event(&self, _event: KeyboardEvent) -> Result<(), KeyboardError> {
            self.send_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    static STUB_KEYBOARD: StubKeyboard = StubKeyboard::new();

    register_keyboard_device!(&STUB_KEYBOARD);

    #[rstest]
    fn keyboard_overrides_builder_sets_values() {
        let overrides =
            KeyboardOverrides::new().press_delay(Duration::from_millis(10)).after_text_delay(Duration::from_millis(5));
        assert_eq!(overrides.press_delay, Some(Duration::from_millis(10)));
        assert_eq!(overrides.after_text_delay, Some(Duration::from_millis(5)));
        assert!(!overrides.is_empty());
    }

    #[rstest]
    fn key_to_code_maps_known_names() {
        let device: &dyn KeyboardDevice = &STUB_KEYBOARD;
        let code = device.key_to_code("Control").expect("known key resolved");
        assert_eq!(code.downcast_ref::<u32>(), Some(&0x11u32));
        let err = device.key_to_code("Unknown").expect_err("unknown key fails");
        match err {
            KeyboardError::UnsupportedKey(name) => assert_eq!(name, "Unknown"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[rstest]
    fn keyboard_error_converts_platform() {
        let err = KeyboardError::from(PlatformError::new(PlatformErrorKind::InitializationFailed, "test"));
        assert!(matches!(err, KeyboardError::Platform(_)));
    }
}
