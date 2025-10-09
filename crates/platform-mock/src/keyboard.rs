use once_cell::sync::Lazy;
use platynui_core::platform::{KeyCode, KeyState, KeyboardDevice, KeyboardError, KeyboardEvent};
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyboardLogEntry {
    StartInput,
    EndInput,
    Press(String),
    Release(String),
}

#[derive(Debug)]
struct KeyboardState {
    started: bool,
    log: Vec<KeyboardLogEntry>,
}

impl KeyboardState {
    const fn new() -> Self {
        Self { started: false, log: Vec::new() }
    }

    fn push(&mut self, entry: KeyboardLogEntry) {
        self.log.push(entry);
    }
}

#[derive(Debug)]
enum MockKeyKind {
    Named(&'static str),
    Character(char),
}

#[derive(Debug)]
struct MockKeyCode {
    kind: MockKeyKind,
}

impl MockKeyCode {
    fn named(name: &'static str) -> Self {
        Self { kind: MockKeyKind::Named(name) }
    }

    fn character(ch: char) -> Self {
        Self { kind: MockKeyKind::Character(ch) }
    }

    fn display_name(&self) -> String {
        match &self.kind {
            MockKeyKind::Named(name) => (*name).to_string(),
            MockKeyKind::Character(ch) => ch.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct MockKeyboardDevice {
    state: Mutex<KeyboardState>,
}

impl MockKeyboardDevice {
    const fn new() -> Self {
        Self { state: Mutex::new(KeyboardState::new()) }
    }
}

impl KeyboardDevice for MockKeyboardDevice {
    fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
        if let Some(code) = resolve_named_key(name) {
            return Ok(code);
        }

        let mut chars = name.chars();
        if let Some(ch) = chars.next()
            && chars.next().is_none()
        {
            return Ok(KeyCode::new(MockKeyCode::character(ch)));
        }

        Err(KeyboardError::UnsupportedKey(name.to_owned()))
    }

    fn start_input(&self) -> Result<(), KeyboardError> {
        let mut state = self.state.lock().unwrap();
        if state.started {
            return Err(KeyboardError::InputInProgress);
        }
        state.started = true;
        state.push(KeyboardLogEntry::StartInput);
        println!("mock-keyboard: start");
        Ok(())
    }

    fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError> {
        let mut state = self.state.lock().unwrap();
        if !state.started {
            return Err(KeyboardError::NotReady);
        }
        let Some(mock) = event.code.downcast_ref::<MockKeyCode>() else {
            return Err(KeyboardError::UnsupportedKey("foreign key code".to_string()));
        };
        let name = mock.display_name();
        match event.state {
            KeyState::Press => {
                println!("mock-keyboard: press {name}");
                state.push(KeyboardLogEntry::Press(name));
            }
            KeyState::Release => {
                println!("mock-keyboard: release {name}");
                state.push(KeyboardLogEntry::Release(name));
            }
        }
        Ok(())
    }

    fn end_input(&self) -> Result<(), KeyboardError> {
        let mut state = self.state.lock().unwrap();
        if !state.started {
            return Err(KeyboardError::NotReady);
        }
        state.started = false;
        state.push(KeyboardLogEntry::EndInput);
        println!("mock-keyboard: end");
        Ok(())
    }
}

pub static MOCK_KEYBOARD: MockKeyboardDevice = MockKeyboardDevice::new();

pub fn reset_keyboard_state() {
    let mut state = MOCK_KEYBOARD.state.lock().unwrap();
    *state = KeyboardState::new();
}

pub fn take_keyboard_log() -> Vec<KeyboardLogEntry> {
    let mut state = MOCK_KEYBOARD.state.lock().unwrap();
    let log = state.log.clone();
    state.log.clear();
    log
}

// Expose device reference for explicit injection in tests/integration code.
// Test helpers for exposing internal state

struct NamedKey {
    canonical: &'static str,
    aliases: &'static [&'static str],
}

const NAMED_KEYS: &[NamedKey] = &[
    NamedKey { canonical: "Control", aliases: &["Control", "Ctrl"] },
    NamedKey { canonical: "Shift", aliases: &["Shift"] },
    NamedKey { canonical: "Alt", aliases: &["Alt", "Menu"] },
    NamedKey { canonical: "Option", aliases: &["Option"] },
    NamedKey { canonical: "Command", aliases: &["Command", "Cmd"] },
    NamedKey { canonical: "Windows", aliases: &["Windows", "Win"] },
    NamedKey { canonical: "Super", aliases: &["Super"] },
    NamedKey { canonical: "Meta", aliases: &["Meta"] },
    NamedKey { canonical: "Enter", aliases: &["Enter", "Return"] },
    NamedKey { canonical: "Escape", aliases: &["Escape", "Esc"] },
    NamedKey { canonical: "Space", aliases: &["Space"] },
    NamedKey { canonical: "Tab", aliases: &["Tab"] },
    NamedKey { canonical: "Backspace", aliases: &["Backspace"] },
    NamedKey { canonical: "Delete", aliases: &["Delete", "Del"] },
    NamedKey { canonical: "CapsLock", aliases: &["CapsLock"] },
    NamedKey { canonical: "ArrowUp", aliases: &["ArrowUp", "Up"] },
    NamedKey { canonical: "ArrowDown", aliases: &["ArrowDown", "Down"] },
    NamedKey { canonical: "ArrowLeft", aliases: &["ArrowLeft", "Left"] },
    NamedKey { canonical: "ArrowRight", aliases: &["ArrowRight", "Right"] },
    NamedKey { canonical: "Home", aliases: &["Home"] },
    NamedKey { canonical: "End", aliases: &["End"] },
    NamedKey { canonical: "PageUp", aliases: &["PageUp", "PgUp"] },
    NamedKey { canonical: "PageDown", aliases: &["PageDown", "PgDn"] },
    NamedKey { canonical: "Insert", aliases: &["Insert", "Ins"] },
    NamedKey { canonical: "PrintScreen", aliases: &["PrintScreen", "PrtSc"] },
    NamedKey { canonical: "Pause", aliases: &["Pause"] },
    NamedKey { canonical: "ScrollLock", aliases: &["ScrollLock"] },
    NamedKey { canonical: "F1", aliases: &["F1"] },
    NamedKey { canonical: "F2", aliases: &["F2"] },
    NamedKey { canonical: "F3", aliases: &["F3"] },
    NamedKey { canonical: "F4", aliases: &["F4"] },
    NamedKey { canonical: "F5", aliases: &["F5"] },
    NamedKey { canonical: "F6", aliases: &["F6"] },
    NamedKey { canonical: "F7", aliases: &["F7"] },
    NamedKey { canonical: "F8", aliases: &["F8"] },
    NamedKey { canonical: "F9", aliases: &["F9"] },
    NamedKey { canonical: "F10", aliases: &["F10"] },
    NamedKey { canonical: "F11", aliases: &["F11"] },
    NamedKey { canonical: "F12", aliases: &["F12"] },
];

static NAMED_LOOKUP: Lazy<Vec<(&'static str, KeyCode)>> = Lazy::new(|| {
    NAMED_KEYS.iter().map(|entry| (entry.canonical, KeyCode::new(MockKeyCode::named(entry.canonical)))).collect()
});

fn resolve_named_key(input: &str) -> Option<KeyCode> {
    NAMED_KEYS.iter().enumerate().find_map(|(idx, entry)| {
        if entry.aliases.iter().any(|alias| alias.eq_ignore_ascii_case(input)) {
            return Some(NAMED_LOOKUP[idx].1.clone());
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use platynui_core::platform::{KeyState, KeyboardEvent, keyboard_devices};
    use rstest::rstest;
    use std::sync::Mutex;

    // Serialize tests that touch the global MOCK_KEYBOARD state to avoid races.
    static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    #[rstest]
    fn keyboard_device_not_auto_registered() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_keyboard_state();
        // Mock keyboard should NOT auto-register
        let devices: Vec<_> = keyboard_devices().collect();
        // Mock device should NOT be in the registry
        let mock_in_registry =
            devices.iter().any(|device| std::ptr::eq(*device, &MOCK_KEYBOARD as &dyn KeyboardDevice));
        assert!(!mock_in_registry, "Mock keyboard device should not be auto-registered");

        // Use direct reference for testing the device itself
        let device = &MOCK_KEYBOARD;
        let control = device.key_to_code("Control").unwrap();
        assert!(control.downcast_ref::<MockKeyCode>().is_some());
    }

    #[rstest]
    fn key_to_code_accepts_named_and_chars() {
        let _guard = TEST_LOCK.lock().unwrap();
        // Use direct reference to mock keyboard instead of registry lookup
        let device = &MOCK_KEYBOARD;
        let control = device.key_to_code("ctrl").expect("ctrl resolves");
        assert!(control.downcast_ref::<MockKeyCode>().is_some());
        let letter = device.key_to_code("a").expect("character resolves");
        assert!(letter.downcast_ref::<MockKeyCode>().is_some());
        assert!(device.key_to_code("<unknown>").is_err());
    }

    #[rstest]
    fn keyboard_logs_events() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_keyboard_state();
        // Use direct reference to mock keyboard instead of registry lookup
        let device = &MOCK_KEYBOARD;
        device.start_input().unwrap();
        let ctrl = device.key_to_code("Control").unwrap();
        let letter = device.key_to_code("A").unwrap();
        device.send_key_event(KeyboardEvent { code: ctrl.clone(), state: KeyState::Press }).unwrap();
        device.send_key_event(KeyboardEvent { code: letter.clone(), state: KeyState::Press }).unwrap();
        device.send_key_event(KeyboardEvent { code: letter, state: KeyState::Release }).unwrap();
        device.send_key_event(KeyboardEvent { code: ctrl, state: KeyState::Release }).unwrap();
        device.end_input().unwrap();

        let log = take_keyboard_log();
        assert_eq!(
            log,
            vec![
                KeyboardLogEntry::StartInput,
                KeyboardLogEntry::Press("Control".into()),
                KeyboardLogEntry::Press("A".into()),
                KeyboardLogEntry::Release("A".into()),
                KeyboardLogEntry::Release("Control".into()),
                KeyboardLogEntry::EndInput,
            ]
        );
    }

    #[rstest]
    fn start_input_is_guarded() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_keyboard_state();
        // Use direct reference to mock keyboard instead of registry lookup
        let device = &MOCK_KEYBOARD;
        device.start_input().unwrap();
        let err = device.start_input().expect_err("second start fails");
        assert!(matches!(err, KeyboardError::InputInProgress));
        device.end_input().unwrap();
    }
}
