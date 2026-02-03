use platynui_core::platform::{KeyCode, KeyboardDevice, KeyboardError, KeyboardEvent};
use platynui_core::register_keyboard_device;

// Phase 1 minimal keyboard: not ready yet, but exposes key name list.
pub struct LinuxKeyboardDevice;

impl KeyboardDevice for LinuxKeyboardDevice {
    fn known_key_names(&self) -> Vec<String> {
        // Minimal set; will be replaced by xkbcommon-rs based resolver in Phase 2
        vec![
            "ESCAPE",
            "RETURN",
            "ENTER",
            "TAB",
            "SPACE",
            "BACKSPACE",
            "LEFT",
            "RIGHT",
            "UP",
            "DOWN",
            "F1",
            "F2",
            "F3",
            "F4",
            "F5",
            "F6",
            "F7",
            "F8",
            "F9",
            "F10",
            "F11",
            "F12",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn key_to_code(&self, _name: &str) -> Result<KeyCode, KeyboardError> {
        Err(KeyboardError::NotReady)
    }

    fn send_key_event(&self, _event: KeyboardEvent) -> Result<(), KeyboardError> {
        Err(KeyboardError::NotReady)
    }
}

static KBD: LinuxKeyboardDevice = LinuxKeyboardDevice;
register_keyboard_device!(&KBD);
