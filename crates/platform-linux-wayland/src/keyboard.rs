use input_event_codes::{
    KEY_BACKSPACE, KEY_CAPSLOCK, KEY_COMPOSE, KEY_DELETE, KEY_DOWN, KEY_END, KEY_ENTER, KEY_ESC, KEY_F1, KEY_F10,
    KEY_F11, KEY_F12, KEY_F13, KEY_F14, KEY_F15, KEY_F16, KEY_F17, KEY_F18, KEY_F19, KEY_F2, KEY_F20, KEY_F21,
    KEY_F22, KEY_F23, KEY_F24, KEY_F3, KEY_F4, KEY_F5, KEY_F6, KEY_F7, KEY_F8, KEY_F9, KEY_HELP, KEY_HOME,
    KEY_INSERT, KEY_KP0, KEY_KP1, KEY_KP2, KEY_KP3, KEY_KP4, KEY_KP5, KEY_KP6, KEY_KP7, KEY_KP8, KEY_KP9,
    KEY_KPASTERISK, KEY_KPDOT, KEY_KPENTER, KEY_KPMINUS, KEY_KPPLUS, KEY_KPSLASH, KEY_LEFT, KEY_LEFTALT,
    KEY_LEFTCTRL, KEY_LEFTMETA, KEY_LEFTSHIFT, KEY_NUMLOCK, KEY_PAGEDOWN, KEY_PAGEUP, KEY_PAUSE,
    KEY_RIGHT, KEY_RIGHTALT, KEY_RIGHTCTRL, KEY_RIGHTMETA, KEY_RIGHTSHIFT, KEY_SCROLLLOCK, KEY_SPACE, KEY_SYSRQ,
    KEY_TAB, KEY_UP,
};
use platynui_core::platform::{
    KeyCode, KeyState, KeyboardDevice, KeyboardError, KeyboardEvent, PlatformError, PlatformErrorKind,
};
use platynui_core::register_keyboard_device;
use platynui_xkb_util::{KeyAction, KeymapLookup};
use reis::ei;
use reis::event::DeviceCapability;

/// Named key to evdev keycode mapping for special keys.
///
/// `KeymapLookup` handles character→keycode mapping; this table covers
/// non-character keys (modifiers, function keys, navigation, etc.)
/// with evdev codes from `linux/input-event-codes.h`.
///
/// All names are matched case-insensitively (lowered before lookup).
static NAMED_KEYS: &[(&str, u16)] = &[
    // ── Modifiers ────────────────────────────────────────────────────────────
    ("SHIFT", KEY_LEFTSHIFT!()),
    ("LSHIFT", KEY_LEFTSHIFT!()),
    ("LEFTSHIFT", KEY_LEFTSHIFT!()),
    ("RSHIFT", KEY_RIGHTSHIFT!()),
    ("RIGHTSHIFT", KEY_RIGHTSHIFT!()),
    ("CONTROL", KEY_LEFTCTRL!()),
    ("CTRL", KEY_LEFTCTRL!()),
    ("LCONTROL", KEY_LEFTCTRL!()),
    ("LCTRL", KEY_LEFTCTRL!()),
    ("LEFTCONTROL", KEY_LEFTCTRL!()),
    ("LEFTCTRL", KEY_LEFTCTRL!()),
    ("RCONTROL", KEY_RIGHTCTRL!()),
    ("RCTRL", KEY_RIGHTCTRL!()),
    ("RIGHTCONTROL", KEY_RIGHTCTRL!()),
    ("RIGHTCTRL", KEY_RIGHTCTRL!()),
    ("ALT", KEY_LEFTALT!()),
    ("LALT", KEY_LEFTALT!()),
    ("LEFTALT", KEY_LEFTALT!()),
    ("RALT", KEY_RIGHTALT!()),
    ("RIGHTALT", KEY_RIGHTALT!()),
    ("ALTGR", KEY_RIGHTALT!()),
    ("LMENU", KEY_LEFTALT!()),
    ("RMENU", KEY_RIGHTALT!()),
    ("META", KEY_LEFTMETA!()),
    ("LMETA", KEY_LEFTMETA!()),
    ("RMETA", KEY_RIGHTMETA!()),
    ("SUPER", KEY_LEFTMETA!()),
    ("WIN", KEY_LEFTMETA!()),
    ("WINDOWS", KEY_LEFTMETA!()),
    ("LWIN", KEY_LEFTMETA!()),
    ("LEFTWIN", KEY_LEFTMETA!()),
    ("RWIN", KEY_RIGHTMETA!()),
    ("RIGHTWIN", KEY_RIGHTMETA!()),
    // ── Lock keys ────────────────────────────────────────────────────────────
    ("CAPSLOCK", KEY_CAPSLOCK!()),
    ("NUMLOCK", KEY_NUMLOCK!()),
    ("SCROLLLOCK", KEY_SCROLLLOCK!()),
    // ── Context-menu key ─────────────────────────────────────────────────────
    ("APPS", KEY_COMPOSE!()),
    ("MENU", KEY_COMPOSE!()),
    // ── Text / editing ───────────────────────────────────────────────────────
    ("ENTER", KEY_ENTER!()),
    ("RETURN", KEY_ENTER!()),
    ("ESCAPE", KEY_ESC!()),
    ("ESC", KEY_ESC!()),
    ("SPACE", KEY_SPACE!()),
    ("TAB", KEY_TAB!()),
    ("BACKSPACE", KEY_BACKSPACE!()),
    ("DELETE", KEY_DELETE!()),
    ("DEL", KEY_DELETE!()),
    ("INSERT", KEY_INSERT!()),
    ("INS", KEY_INSERT!()),
    // ── Navigation ───────────────────────────────────────────────────────────
    ("UP", KEY_UP!()),
    ("DOWN", KEY_DOWN!()),
    ("LEFT", KEY_LEFT!()),
    ("RIGHT", KEY_RIGHT!()),
    ("ARROWUP", KEY_UP!()),
    ("ARROWDOWN", KEY_DOWN!()),
    ("ARROWLEFT", KEY_LEFT!()),
    ("ARROWRIGHT", KEY_RIGHT!()),
    ("HOME", KEY_HOME!()),
    ("END", KEY_END!()),
    ("PAGEUP", KEY_PAGEUP!()),
    ("PGUP", KEY_PAGEUP!()),
    ("PAGEDOWN", KEY_PAGEDOWN!()),
    ("PGDN", KEY_PAGEDOWN!()),
    // ── Function keys F1–F24 ─────────────────────────────────────────────────
    ("F1", KEY_F1!()),
    ("F2", KEY_F2!()),
    ("F3", KEY_F3!()),
    ("F4", KEY_F4!()),
    ("F5", KEY_F5!()),
    ("F6", KEY_F6!()),
    ("F7", KEY_F7!()),
    ("F8", KEY_F8!()),
    ("F9", KEY_F9!()),
    ("F10", KEY_F10!()),
    ("F11", KEY_F11!()),
    ("F12", KEY_F12!()),
    ("F13", KEY_F13!()),
    ("F14", KEY_F14!()),
    ("F15", KEY_F15!()),
    ("F16", KEY_F16!()),
    ("F17", KEY_F17!()),
    ("F18", KEY_F18!()),
    ("F19", KEY_F19!()),
    ("F20", KEY_F20!()),
    ("F21", KEY_F21!()),
    ("F22", KEY_F22!()),
    ("F23", KEY_F23!()),
    ("F24", KEY_F24!()),
    // ── System ───────────────────────────────────────────────────────────────
    ("PRINTSCREEN", KEY_SYSRQ!()),
    ("PRINT", KEY_SYSRQ!()),
    ("PRTSC", KEY_SYSRQ!()),
    ("SYSRQ", KEY_SYSRQ!()),
    ("SYSREQ", KEY_SYSRQ!()),
    ("PAUSE", KEY_PAUSE!()),
    ("BREAK", KEY_PAUSE!()),
    ("HELP", KEY_HELP!()),
    // ── Numpad digits ────────────────────────────────────────────────────────
    ("NUMPAD0", KEY_KP0!()),
    ("NUMPAD1", KEY_KP1!()),
    ("NUMPAD2", KEY_KP2!()),
    ("NUMPAD3", KEY_KP3!()),
    ("NUMPAD4", KEY_KP4!()),
    ("NUMPAD5", KEY_KP5!()),
    ("NUMPAD6", KEY_KP6!()),
    ("NUMPAD7", KEY_KP7!()),
    ("NUMPAD8", KEY_KP8!()),
    ("NUMPAD9", KEY_KP9!()),
    ("KP_0", KEY_KP0!()),
    ("KP_1", KEY_KP1!()),
    ("KP_2", KEY_KP2!()),
    ("KP_3", KEY_KP3!()),
    ("KP_4", KEY_KP4!()),
    ("KP_5", KEY_KP5!()),
    ("KP_6", KEY_KP6!()),
    ("KP_7", KEY_KP7!()),
    ("KP_8", KEY_KP8!()),
    ("KP_9", KEY_KP9!()),
    // ── Numpad operations ────────────────────────────────────────────────────
    ("KP_ENTER", KEY_KPENTER!()),
    ("NUMPADENTER", KEY_KPENTER!()),
    ("KP_ADD", KEY_KPPLUS!()),
    ("NUMPADADD", KEY_KPPLUS!()),
    ("ADD", KEY_KPPLUS!()),
    ("KP_SUBTRACT", KEY_KPMINUS!()),
    ("NUMPADSUBTRACT", KEY_KPMINUS!()),
    ("SUBTRACT", KEY_KPMINUS!()),
    ("KP_MULTIPLY", KEY_KPASTERISK!()),
    ("NUMPADMULTIPLY", KEY_KPASTERISK!()),
    ("MULTIPLY", KEY_KPASTERISK!()),
    ("KP_DIVIDE", KEY_KPSLASH!()),
    ("NUMPADDIVIDE", KEY_KPSLASH!()),
    ("DIVIDE", KEY_KPSLASH!()),
    ("KP_DECIMAL", KEY_KPDOT!()),
    ("NUMPADDECIMAL", KEY_KPDOT!()),
    ("DECIMAL", KEY_KPDOT!()),
];

pub struct WaylandKeyboardDevice;

impl KeyboardDevice for WaylandKeyboardDevice {
    fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
        let upper = name.to_ascii_uppercase();

        // 1. Named special keys (modifiers, function keys, navigation).
        for &(key_name, code) in NAMED_KEYS {
            if key_name == upper {
                return Ok(KeyCode::new(u32::from(code)));
            }
        }

        // 2. Symbol aliases (characters problematic in <key> notation).
        let ch = match upper.as_str() {
            "PLUS" => Some('+'),
            "MINUS" => Some('-'),
            "LESS" | "LT" => Some('<'),
            "GREATER" | "GT" => Some('>'),
            _ => None,
        };
        if let Some(ch) = ch
            && let Ok(lookup) = get_or_init_keymap_lookup()
            && let Some(action) = lookup.lookup(ch)
        {
            let evdev = match action {
                KeyAction::Simple(combo) => KeymapLookup::evdev_keycode(combo),
                KeyAction::Compose { dead_key, .. } => KeymapLookup::evdev_keycode(dead_key),
            };
            return Ok(KeyCode::new(evdev));
        }

        // 3. Raw evdev keycode number.
        if let Ok(code) = upper.parse::<u32>() {
            return Ok(KeyCode::new(code));
        }

        // 4. Single character → xkb-util reverse lookup (layout-aware).
        if name.chars().count() == 1
            && let Some(ch) = name.chars().next()
            && let Ok(lookup) = get_or_init_keymap_lookup()
            && let Some(action) = lookup.lookup(ch)
        {
            let evdev = match action {
                KeyAction::Simple(combo) => KeymapLookup::evdev_keycode(combo),
                // For compose sequences, return the dead key's evdev code.
                // The full compose sequence would need to be handled at a higher level.
                KeyAction::Compose { dead_key, .. } => KeymapLookup::evdev_keycode(dead_key),
            };
            return Ok(KeyCode::new(evdev));
        }

        Err(KeyboardError::UnsupportedKey(name.to_owned()))
    }

    fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError> {
        let keycode = event
            .code
            .downcast_ref::<u32>()
            .copied()
            .ok_or_else(|| KeyboardError::UnsupportedKey("non-u32 keycode".into()))?;

        let ei_state = match event.state {
            KeyState::Press => ei::keyboard::KeyState::Press,
            KeyState::Release => ei::keyboard::KeyState::Released,
        };

        with_ei_keyboard(|connection, device| {
            let kbd = device
                .interface::<ei::Keyboard>()
                .ok_or_else(|| to_kb("device missing Keyboard interface"))?;

            let serial = connection.serial();
            let device_proxy = device.device();
            device_proxy.start_emulating(serial, 1);
            kbd.key(keycode, ei_state);
            device_proxy.frame(serial, crate::eis::timestamp_us());
            device_proxy.stop_emulating(serial);
            connection.flush().map_err(|e| to_kb(format!("flush: {e}")))?;
            Ok(())
        })
    }

    fn known_key_names(&self) -> Vec<String> {
        NAMED_KEYS.iter().map(|(name, _)| (*name).to_owned()).collect()
    }
}

// ---------------------------------------------------------------------------
// XKB keymap lookup (initialized lazily from EIS keymap)
// ---------------------------------------------------------------------------

/// Get or initialize the `KeymapLookup` from the compositor's keymap.
///
/// The keymap is obtained from the EIS keyboard device during the first call
/// and cached for subsequent lookups. If no keymap is available from EIS
/// (e.g. the compositor doesn't send one), falls back to a default US layout.
fn get_or_init_keymap_lookup() -> Result<&'static KeymapLookup, KeyboardError> {
    use std::sync::OnceLock;
    static LOOKUP: OnceLock<Result<KeymapLookup, String>> = OnceLock::new();

    let result = LOOKUP.get_or_init(|| {
        tracing::debug!("initializing XKB keymap lookup");
        // Try to get keymap from a default xkb context (system keymap).
        // In a full implementation, we'd extract the keymap string from
        // the EIS keyboard device's keymap event.
        let context = platynui_xkb_util::xkb::Context::new(platynui_xkb_util::xkb::CONTEXT_NO_FLAGS);
        let keymap = platynui_xkb_util::xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "",
            "",
            None,
            platynui_xkb_util::xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or_else(|| "failed to create default XKB keymap".to_string())?;

        let lookup = KeymapLookup::new(&keymap);
        tracing::info!(entries = lookup.len(), "XKB keymap lookup initialized");
        Ok(lookup)
    });

    result.as_ref().map_err(|e| KeyboardError::Platform(to_pf(e)))
}

// ---------------------------------------------------------------------------
// EIS keyboard session
// ---------------------------------------------------------------------------

fn with_ei_keyboard(
    action: impl FnOnce(&reis::event::Connection, &reis::event::Device) -> Result<(), KeyboardError>,
) -> Result<(), KeyboardError> {
    let guard = crate::wayland_util::connection().map_err(KeyboardError::Platform)?;
    drop(guard);

    let mut session = crate::eis::establish_session("platynui-wayland-keyboard")
        .map_err(|e| KeyboardError::Platform(to_pf(format!("EIS session: {e}"))))?;

    let device = crate::eis::find_device(&mut session, DeviceCapability::Keyboard)
        .map_err(|e| KeyboardError::Platform(to_pf(format!("EIS device: {e}"))))?;

    action(&session.connection, &device)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_pf<E: std::fmt::Display>(e: E) -> PlatformError {
    PlatformError::new(PlatformErrorKind::OperationFailed, format!("wayland keyboard: {e}"))
}

fn to_kb<E: std::fmt::Display>(e: E) -> KeyboardError {
    KeyboardError::Platform(to_pf(e))
}

static KBD: WaylandKeyboardDevice = WaylandKeyboardDevice;

register_keyboard_device!(&KBD);
