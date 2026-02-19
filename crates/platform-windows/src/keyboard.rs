use platynui_core::platform::{
    KeyCode, KeyState, KeyboardDevice, KeyboardError, KeyboardEvent, register_keyboard_device,
};

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, MapVirtualKeyW, SendInput, VIRTUAL_KEY, VK_CAPITAL, VK_CONTROL, VK_MENU,
    VK_SHIFT, VkKeyScanW,
};

use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug)]
enum WinKey {
    Vk(u16),      // Virtual-Key
    Unicode(u16), // UTF-16 code unit
    CharMapped { vk: u16, shift: bool, ctrl: bool, alt: bool },
}

#[derive(Clone, Debug)]
struct WinKeyCode(WinKey);

impl WinKeyCode {
    fn from_vk(vk: u16) -> KeyCode {
        KeyCode::new(WinKeyCode(WinKey::Vk(vk)))
    }
    fn from_unicode(ch: u16) -> KeyCode {
        KeyCode::new(WinKeyCode(WinKey::Unicode(ch)))
    }
}

pub struct WindowsKeyboardDevice;

impl WindowsKeyboardDevice {
    fn is_extended_vk(vk: u16) -> bool {
        // Extended keys typically include: right Alt/Control, navigation cluster, arrow keys,
        // numpad divide, numlock, insert/delete/home/end/page up/down, and right Windows/Menu.
        match vk {
            // Right modifiers
            0xA5 /* VK_RMENU */ | 0xA3 /* VK_RCONTROL */ |
            // Navigation cluster
            0x2D /* VK_INSERT */ | 0x2E /* VK_DELETE */ | 0x24 /* VK_HOME */ | 0x23 /* VK_END */ |
            0x21 /* VK_PRIOR */ | 0x22 /* VK_NEXT */ |
            // Arrows
            0x25 /* VK_LEFT */ | 0x26 /* VK_UP */ | 0x27 /* VK_RIGHT */ | 0x28 /* VK_DOWN */ |
            // Numpad divide and NumLock
            0x6F /* VK_DIVIDE */ | 0x90 /* VK_NUMLOCK */ |
            // Windows/menu
            0x5B /* VK_LWIN */ | 0x5C /* VK_RWIN */ | 0x5D /* VK_APPS */ => true,
            _ => false,
        }
    }
    fn name_to_entry(name: &str) -> Option<&KeyName> {
        let upper = name.trim().to_ascii_uppercase();
        VK_MAP.get(&upper)
    }

    fn send_vk(state: KeyState, vk: u16) -> Result<(), KeyboardError> {
        // Prefer sending VK; include scan code for better app compatibility
        let sc = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
        let mut flags: KEYBD_EVENT_FLAGS = match state {
            KeyState::Press => KEYBD_EVENT_FLAGS(0),
            KeyState::Release => KEYEVENTF_KEYUP,
        };
        if Self::is_extended_vk(vk) {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT { wVk: VIRTUAL_KEY(vk), wScan: sc, dwFlags: flags, time: 0, dwExtraInfo: 0 },
            },
        };
        let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        if sent == 0 { Err(KeyboardError::NotReady) } else { Ok(()) }
    }

    fn send_unicode(state: KeyState, ch: u16) -> Result<(), KeyboardError> {
        let flags = match state {
            KeyState::Press => KEYEVENTF_UNICODE,
            KeyState::Release => KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
        };
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0), wScan: ch, dwFlags: flags, time: 0, dwExtraInfo: 0 },
            },
        };
        let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        if sent == 0 { Err(KeyboardError::NotReady) } else { Ok(()) }
    }

    fn current_capslock() -> bool {
        unsafe { (GetKeyState(VK_CAPITAL.0 as i32) & 0x0001) != 0 }
    }

    fn is_key_down(vk: VIRTUAL_KEY) -> bool {
        unsafe { (GetKeyState(vk.0 as i32) & (0x8000u16 as i16)) != 0 }
    }

    #[inline]
    fn char_to_keycode(ch_u16: u16) -> KeyCode {
        // Try layout-aware mapping first
        unsafe {
            let res = VkKeyScanW(ch_u16);
            if res != -1i16 {
                let vk = (res & 0xFF) as u16;
                let shift = ((res >> 8) & 1) != 0;
                let ctrl = ((res >> 9) & 1) != 0;
                let alt = ((res >> 10) & 1) != 0;
                return KeyCode::new(WinKeyCode(WinKey::CharMapped { vk, shift, ctrl, alt }));
            }
        }
        // Fallback to Unicode injection
        WinKeyCode::from_unicode(ch_u16)
    }
}

impl KeyboardDevice for WindowsKeyboardDevice {
    fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
        // 1) Benannte Auflösung aus einer gemeinsamen Map: VK(..) oder Char(..)
        if let Some(entry) = Self::name_to_entry(name) {
            return Ok(match entry {
                KeyName::Vk(vk) => WinKeyCode::from_vk(*vk),
                KeyName::Char(ch) => Self::char_to_keycode(*ch as u16),
            });
        }

        // 2) Einzelnes Zeichen → VkKeyScanW (mit Shift/Alt/Ctrl), Fallback Unicode
        if name.chars().count() == 1 {
            // SAFETY: count() == 1 guarantees next() returns Some
            let ch_u16 = name.chars().next().unwrap_or_default() as u16;
            // CapsLock beeinflusst nur Buchstaben: invertiere SHIFT bei aktivem CapsLock
            if ((ch_u16 as u8 as char).is_ascii_alphabetic()) && Self::current_capslock() {
                // Use VkKeyScanW first, then flip shift bit
                unsafe {
                    let res = VkKeyScanW(ch_u16);
                    if res != -1i16 {
                        let vk = (res & 0xFF) as u16;
                        let mut shift = ((res >> 8) & 1) != 0;
                        let ctrl = ((res >> 9) & 1) != 0;
                        let alt = ((res >> 10) & 1) != 0;
                        shift = !shift;
                        return Ok(KeyCode::new(WinKeyCode(WinKey::CharMapped { vk, shift, ctrl, alt })));
                    }
                }
            }
            return Ok(Self::char_to_keycode(ch_u16));
        }
        Err(KeyboardError::UnsupportedKey(name.to_owned()))
    }

    fn start_input(&self) -> Result<(), KeyboardError> {
        Ok(())
    }

    fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError> {
        let Some(wk) = event.code.downcast_ref::<WinKeyCode>() else {
            return Err(KeyboardError::UnsupportedKey("foreign key code".to_string()));
        };
        match wk.0 {
            WinKey::Vk(vk) => Self::send_vk(event.state, vk),
            WinKey::Unicode(ch) => Self::send_unicode(event.state, ch),
            WinKey::CharMapped { vk, shift, ctrl, alt } => {
                match event.state {
                    KeyState::Press => {
                        // Avoid injecting SHIFT with Ctrl/Alt (shortcut contexts)
                        let ctrl_down_now = Self::is_key_down(VK_CONTROL);
                        let alt_down_now = Self::is_key_down(VK_MENU);
                        let altgr = ctrl && alt;
                        if altgr {
                            // Right Alt (AltGr) erzeugen, nicht Ctrl+Alt
                            Self::send_vk(KeyState::Press, 0xA5 /* VK_RMENU */)?;
                        } else {
                            if ctrl {
                                Self::send_vk(KeyState::Press, VK_CONTROL.0)?;
                            }
                            if alt {
                                Self::send_vk(KeyState::Press, VK_MENU.0)?;
                            }
                        }
                        if shift && !(ctrl_down_now || alt_down_now || altgr) {
                            Self::send_vk(KeyState::Press, VK_SHIFT.0)?;
                        }
                        Self::send_vk(KeyState::Press, vk)
                    }
                    KeyState::Release => {
                        let r = Self::send_vk(KeyState::Release, vk);
                        let ctrl_down_now = Self::is_key_down(VK_CONTROL);
                        let alt_down_now = Self::is_key_down(VK_MENU);
                        let altgr = ctrl && alt;
                        if shift && !(ctrl_down_now || alt_down_now || altgr) {
                            let _ = Self::send_vk(KeyState::Release, VK_SHIFT.0);
                        }
                        if altgr {
                            let _ = Self::send_vk(KeyState::Release, 0xA5_u16);
                        } else {
                            if alt {
                                let _ = Self::send_vk(KeyState::Release, VK_MENU.0);
                            }
                            if ctrl {
                                let _ = Self::send_vk(KeyState::Release, VK_CONTROL.0);
                            }
                        }
                        r
                    }
                }
            }
        }
    }

    fn end_input(&self) -> Result<(), KeyboardError> {
        Ok(())
    }

    fn known_key_names(&self) -> Vec<String> {
        let mut names: Vec<String> = VK_MAP.keys().cloned().collect();
        // Also advertise letters A..Z and digits 0..9 as acceptable character names
        for ch in 'A'..='Z' {
            let s = ch.to_string();
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&s)) {
                names.push(s);
            }
        }
        for ch in '0'..='9' {
            let s = ch.to_string();
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&s)) {
                names.push(s);
            }
        }
        names.sort_unstable();
        names
    }
}

static DEVICE: WindowsKeyboardDevice = WindowsKeyboardDevice;

register_keyboard_device!(&DEVICE);

// Global VK_* name → VK code mapping (exact VK_* strings only)
#[derive(Clone, Copy, Debug)]
enum KeyName {
    Vk(u16),
    Char(char),
}

static VK_MAP: LazyLock<HashMap<String, KeyName>> = LazyLock::new(|| {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    let mut m: HashMap<String, KeyName> = HashMap::new();
    // Insert only WITHOUT the VK_ prefix (we don't need VK_* names)
    macro_rules! ins {
        ($name:ident) => {{
            let full = stringify!($name);
            if let Some(no) = full.strip_prefix("VK_") {
                m.insert(no.to_string(), KeyName::Vk($name.0 as u16));
            }
        }};
    }
    // Mouse/cancel
    ins!(VK_LBUTTON);
    ins!(VK_RBUTTON);
    ins!(VK_CANCEL);
    ins!(VK_MBUTTON);
    ins!(VK_XBUTTON1);
    ins!(VK_XBUTTON2);
    // 0-9, A-Z use ASCII codes directly: VK_0..VK_9 and VK_A..VK_Z are not defined as separate constants here;
    // they are implied by character codes; we rely on single-char Unicode path for these when not using VK_ names.
    // Editing/navigation
    ins!(VK_BACK);
    ins!(VK_TAB);
    ins!(VK_CLEAR);
    ins!(VK_RETURN);
    ins!(VK_SHIFT);
    ins!(VK_CONTROL);
    ins!(VK_MENU);
    ins!(VK_PAUSE);
    ins!(VK_CAPITAL);
    ins!(VK_ESCAPE);
    ins!(VK_SPACE);
    ins!(VK_PRIOR);
    ins!(VK_NEXT);
    ins!(VK_END);
    ins!(VK_HOME);
    ins!(VK_LEFT);
    ins!(VK_UP);
    ins!(VK_RIGHT);
    ins!(VK_DOWN);
    ins!(VK_SELECT);
    ins!(VK_PRINT);
    ins!(VK_EXECUTE);
    ins!(VK_SNAPSHOT);
    ins!(VK_INSERT);
    ins!(VK_DELETE);
    ins!(VK_HELP);
    // IME / conversion
    ins!(VK_KANA);
    ins!(VK_HANGUL);
    ins!(VK_JUNJA);
    ins!(VK_FINAL);
    ins!(VK_HANJA);
    ins!(VK_KANJI);
    ins!(VK_CONVERT);
    ins!(VK_NONCONVERT);
    ins!(VK_ACCEPT);
    ins!(VK_MODECHANGE);
    // IME toggles
    ins!(VK_IME_ON);
    ins!(VK_IME_OFF);
    // Windows keys/apps
    ins!(VK_LWIN);
    ins!(VK_RWIN);
    ins!(VK_APPS);
    ins!(VK_SLEEP);
    // Numpad and operations
    ins!(VK_NUMPAD0);
    ins!(VK_NUMPAD1);
    ins!(VK_NUMPAD2);
    ins!(VK_NUMPAD3);
    ins!(VK_NUMPAD4);
    ins!(VK_NUMPAD5);
    ins!(VK_NUMPAD6);
    ins!(VK_NUMPAD7);
    ins!(VK_NUMPAD8);
    ins!(VK_NUMPAD9);
    ins!(VK_MULTIPLY);
    ins!(VK_ADD);
    ins!(VK_SEPARATOR);
    ins!(VK_SUBTRACT);
    ins!(VK_DECIMAL);
    ins!(VK_DIVIDE);
    // Function keys
    ins!(VK_F1);
    ins!(VK_F2);
    ins!(VK_F3);
    ins!(VK_F4);
    ins!(VK_F5);
    ins!(VK_F6);
    ins!(VK_F7);
    ins!(VK_F8);
    ins!(VK_F9);
    ins!(VK_F10);
    ins!(VK_F11);
    ins!(VK_F12);
    ins!(VK_F13);
    ins!(VK_F14);
    ins!(VK_F15);
    ins!(VK_F16);
    ins!(VK_F17);
    ins!(VK_F18);
    ins!(VK_F19);
    ins!(VK_F20);
    ins!(VK_F21);
    ins!(VK_F22);
    ins!(VK_F23);
    ins!(VK_F24);
    // Lock/modifier variants
    ins!(VK_NUMLOCK);
    ins!(VK_SCROLL);
    ins!(VK_LSHIFT);
    ins!(VK_RSHIFT);
    ins!(VK_LCONTROL);
    ins!(VK_RCONTROL);
    ins!(VK_LMENU);
    ins!(VK_RMENU);
    // Browser/media/launch
    ins!(VK_BROWSER_BACK);
    ins!(VK_BROWSER_FORWARD);
    ins!(VK_BROWSER_REFRESH);
    ins!(VK_BROWSER_STOP);
    ins!(VK_BROWSER_SEARCH);
    ins!(VK_BROWSER_FAVORITES);
    ins!(VK_BROWSER_HOME);
    ins!(VK_VOLUME_MUTE);
    ins!(VK_VOLUME_DOWN);
    ins!(VK_VOLUME_UP);
    ins!(VK_MEDIA_NEXT_TRACK);
    ins!(VK_MEDIA_PREV_TRACK);
    ins!(VK_MEDIA_STOP);
    ins!(VK_MEDIA_PLAY_PAUSE);
    ins!(VK_LAUNCH_MAIL);
    ins!(VK_LAUNCH_MEDIA_SELECT);
    ins!(VK_LAUNCH_APP1);
    ins!(VK_LAUNCH_APP2);
    // Navigation keys (Windows 10+)
    ins!(VK_NAVIGATION_VIEW);
    ins!(VK_NAVIGATION_MENU);
    ins!(VK_NAVIGATION_UP);
    ins!(VK_NAVIGATION_DOWN);
    ins!(VK_NAVIGATION_LEFT);
    ins!(VK_NAVIGATION_RIGHT);
    ins!(VK_NAVIGATION_ACCEPT);
    ins!(VK_NAVIGATION_CANCEL);
    // OEM and other specials
    ins!(VK_OEM_1);
    ins!(VK_OEM_PLUS);
    ins!(VK_OEM_COMMA);
    ins!(VK_OEM_MINUS);
    ins!(VK_OEM_PERIOD);
    ins!(VK_OEM_2);
    ins!(VK_OEM_3);
    ins!(VK_OEM_4);
    ins!(VK_OEM_5);
    ins!(VK_OEM_6);
    ins!(VK_OEM_7);
    ins!(VK_OEM_8);
    ins!(VK_OEM_AX);
    ins!(VK_OEM_102);
    ins!(VK_OEM_NEC_EQUAL);
    ins!(VK_OEM_FJ_JISHO);
    ins!(VK_OEM_FJ_MASSHOU);
    ins!(VK_OEM_FJ_TOUROKU);
    ins!(VK_OEM_FJ_LOYA);
    ins!(VK_OEM_FJ_ROYA);
    // Brazilian ABNT keys
    ins!(VK_ABNT_C1);
    ins!(VK_ABNT_C2);
    ins!(VK_ICO_HELP);
    ins!(VK_ICO_00);
    ins!(VK_PROCESSKEY);
    ins!(VK_PACKET);
    ins!(VK_ATTN);
    ins!(VK_CRSEL);
    ins!(VK_EXSEL);
    ins!(VK_EREOF);
    ins!(VK_PLAY);
    ins!(VK_ZOOM);
    ins!(VK_NONAME);
    ins!(VK_PA1);
    ins!(VK_OEM_CLEAR);
    // Japanese DBE (IME) keys
    ins!(VK_DBE_ALPHANUMERIC);
    ins!(VK_DBE_KATAKANA);
    ins!(VK_DBE_HIRAGANA);
    ins!(VK_DBE_SBCSCHAR);
    ins!(VK_DBE_DBCSCHAR);
    ins!(VK_DBE_ROMAN);
    ins!(VK_DBE_NOROMAN);
    ins!(VK_DBE_ENTERWORDREGISTERMODE);
    ins!(VK_DBE_ENTERIMECONFIGMODE);
    ins!(VK_DBE_FLUSHSTRING);
    ins!(VK_DBE_CODEINPUT);
    ins!(VK_DBE_NOCODEINPUT);
    ins!(VK_DBE_DETERMINESTRING);
    ins!(VK_DBE_ENTERDLGCONVERSIONMODE);

    // Gamepad (Windows 10+)
    ins!(VK_GAMEPAD_A);
    ins!(VK_GAMEPAD_B);
    ins!(VK_GAMEPAD_X);
    ins!(VK_GAMEPAD_Y);
    ins!(VK_GAMEPAD_RIGHT_SHOULDER);
    ins!(VK_GAMEPAD_LEFT_SHOULDER);
    ins!(VK_GAMEPAD_LEFT_TRIGGER);
    ins!(VK_GAMEPAD_RIGHT_TRIGGER);
    ins!(VK_GAMEPAD_DPAD_UP);
    ins!(VK_GAMEPAD_DPAD_DOWN);
    ins!(VK_GAMEPAD_DPAD_LEFT);
    ins!(VK_GAMEPAD_DPAD_RIGHT);
    ins!(VK_GAMEPAD_MENU);
    ins!(VK_GAMEPAD_VIEW);
    ins!(VK_GAMEPAD_LEFT_THUMBSTICK_BUTTON);
    ins!(VK_GAMEPAD_RIGHT_THUMBSTICK_BUTTON);
    ins!(VK_GAMEPAD_LEFT_THUMBSTICK_UP);
    ins!(VK_GAMEPAD_LEFT_THUMBSTICK_DOWN);
    ins!(VK_GAMEPAD_LEFT_THUMBSTICK_RIGHT);
    ins!(VK_GAMEPAD_LEFT_THUMBSTICK_LEFT);
    ins!(VK_GAMEPAD_RIGHT_THUMBSTICK_UP);
    ins!(VK_GAMEPAD_RIGHT_THUMBSTICK_DOWN);
    ins!(VK_GAMEPAD_RIGHT_THUMBSTICK_RIGHT);
    ins!(VK_GAMEPAD_RIGHT_THUMBSTICK_LEFT);

    // Intentionally NOT inserting VK_0..VK_9 / VK_A..VK_Z here. They are handled via single-char Unicode path.

    // Symbol aliases that map to characters and should be resolved layout-aware via VkKeyScanW
    m.insert("PLUS".to_string(), KeyName::Char('+'));
    m.insert("MINUS".to_string(), KeyName::Char('-'));
    m.insert("LESS".to_string(), KeyName::Char('<'));
    m.insert("LT".to_string(), KeyName::Char('<'));
    m.insert("GREATER".to_string(), KeyName::Char('>'));
    m.insert("GT".to_string(), KeyName::Char('>'));

    // Common abbreviations/synonyms
    let mut alias = |key: &str, vk: VIRTUAL_KEY| {
        m.insert(key.to_string(), KeyName::Vk(vk.0));
    };
    alias("CTRL", VK_CONTROL);
    alias("CONTROL", VK_CONTROL);
    alias("ALT", VK_MENU);
    alias("WIN", VK_LWIN);
    alias("WINDOWS", VK_LWIN);
    alias("ENTER", VK_RETURN);
    alias("ESC", VK_ESCAPE);
    alias("ESCAPE", VK_ESCAPE);
    alias("PAGEUP", VK_PRIOR);
    alias("PGUP", VK_PRIOR);
    alias("PAGEDOWN", VK_NEXT);
    alias("PGDN", VK_NEXT);
    alias("BACKSPACE", VK_BACK);
    alias("BS", VK_BACK);
    alias("PRINTSCREEN", VK_SNAPSHOT);
    alias("PRTSC", VK_SNAPSHOT);
    alias("CAPSLOCK", VK_CAPITAL);
    // AltGr / RightAlt synonyms
    alias("ALTGR", VK_RMENU);
    alias("RALT", VK_RMENU);
    alias("RIGHTALT", VK_RMENU);
    alias("LALT", VK_MENU);
    alias("LEFTALT", VK_MENU);
    // Shift Left/Right
    alias("LSHIFT", VK_LSHIFT);
    alias("LEFTSHIFT", VK_LSHIFT);
    alias("RSHIFT", VK_RSHIFT);
    alias("RIGHTSHIFT", VK_RSHIFT);
    // Control Left/Right
    alias("LCTRL", VK_LCONTROL);
    alias("LEFTCTRL", VK_LCONTROL);
    alias("RCTRL", VK_RCONTROL);
    alias("RIGHTCTRL", VK_RCONTROL);
    alias("LEFTCONTROL", VK_LCONTROL);
    alias("RIGHTCONTROL", VK_RCONTROL);
    // Windows key aliases
    alias("LEFTWIN", VK_LWIN);
    alias("RIGHTWIN", VK_RWIN);

    // Normalize keys to uppercase for lookups
    let mut upper_map: HashMap<String, KeyName> = HashMap::new();
    for (k, v) in m.into_iter() {
        upper_map.insert(k.to_ascii_uppercase(), v);
    }
    upper_map
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_named_keys_and_fkeys() {
        let dev = WindowsKeyboardDevice;
        assert!(dev.key_to_code("Ctrl").is_ok());
        assert!(dev.key_to_code("Escape").is_ok());
        assert!(dev.key_to_code("F1").is_ok());
        assert!(dev.key_to_code("F24").is_ok());
    }

    #[test]
    fn maps_letter_name_to_vk() {
        let dev = WindowsKeyboardDevice;
        let kc = dev.key_to_code("A").unwrap();
        let wk = kc.downcast_ref::<WinKeyCode>().unwrap();
        match wk.0 {
            WinKey::Vk(vk) => assert_eq!(vk, 'A' as u16),
            WinKey::CharMapped { vk, .. } => assert_eq!(vk, 'A' as u16),
            other => panic!("expected Vk or CharMapped mapping for 'A', got {:?}", other),
        }
    }

    #[test]
    fn unicode_for_non_ascii() {
        let dev = WindowsKeyboardDevice;
        let kc = dev.key_to_code("ä").unwrap();
        let wk = kc.downcast_ref::<WinKeyCode>().unwrap();
        match wk.0 {
            WinKey::Unicode(code) => assert_eq!(code, 'ä' as u16),
            WinKey::CharMapped { .. } => {}
            other => panic!("expected Unicode fallback or CharMapped for 'ä', got {:?}", other),
        }
    }
}
