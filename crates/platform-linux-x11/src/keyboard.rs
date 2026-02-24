//! X11 keyboard device using XTest for key injection.
//!
//! Key names are resolved from a static lookup table; keysyms are mapped to
//! X11 keycodes via the current keyboard mapping ([`GetKeyboardMapping`]).
//! Characters not present in the active keymap are injected through dynamic
//! remapping of a spare (unmapped) keycode.
//!
//! The implementation supports:
//! - Named keys (modifiers, function keys, navigation, numpad)
//! - Single-character input (letters, digits, symbols)
//! - CapsLock-aware shift management for alphabetic characters
//! - Dynamic keycode remapping for characters outside the active layout

use crate::x11util::{X11Handle, connection, root_window_from};
use platynui_core::platform::{
    KeyCode, KeyState, KeyboardDevice, KeyboardError, KeyboardEvent, PlatformError, PlatformErrorKind,
};
use platynui_core::register_keyboard_device;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};
use tracing::{debug, trace};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as _, KeyButMask};
use x11rb::protocol::xtest;

// ---------------------------------------------------------------------------
//  X11 keysym constants (from X11/keysymdef.h)
// ---------------------------------------------------------------------------

// TTY function keys
const XK_BACKSPACE: u32 = 0xff08;
const XK_TAB: u32 = 0xff09;
const XK_RETURN: u32 = 0xff0d;
const XK_PAUSE: u32 = 0xff13;
const XK_SCROLL_LOCK: u32 = 0xff14;
const XK_SYS_REQ: u32 = 0xff15;
const XK_ESCAPE: u32 = 0xff1b;
const XK_DELETE: u32 = 0xffff;

// Cursor control & motion
const XK_HOME: u32 = 0xff50;
const XK_LEFT: u32 = 0xff51;
const XK_UP: u32 = 0xff52;
const XK_RIGHT: u32 = 0xff53;
const XK_DOWN: u32 = 0xff54;
const XK_PAGE_UP: u32 = 0xff55;
const XK_PAGE_DOWN: u32 = 0xff56;
const XK_END: u32 = 0xff57;

// Misc function keys
const XK_PRINT: u32 = 0xff61;
const XK_INSERT: u32 = 0xff63;
const XK_MENU: u32 = 0xff67;
const XK_HELP: u32 = 0xff6a;
const XK_BREAK: u32 = 0xff6b;
const XK_NUM_LOCK: u32 = 0xff7f;

// Keypad keys
const XK_KP_ENTER: u32 = 0xff8d;
const XK_KP_HOME: u32 = 0xff95;
const XK_KP_LEFT: u32 = 0xff96;
const XK_KP_UP: u32 = 0xff97;
const XK_KP_RIGHT: u32 = 0xff98;
const XK_KP_DOWN: u32 = 0xff99;
const XK_KP_PAGE_UP: u32 = 0xff9a;
const XK_KP_PAGE_DOWN: u32 = 0xff9b;
const XK_KP_END: u32 = 0xff9c;
const XK_KP_INSERT: u32 = 0xff9e;
const XK_KP_DELETE: u32 = 0xff9f;
const XK_KP_MULTIPLY: u32 = 0xffaa;
const XK_KP_ADD: u32 = 0xffab;
const XK_KP_SUBTRACT: u32 = 0xffad;
const XK_KP_DECIMAL: u32 = 0xffae;
const XK_KP_DIVIDE: u32 = 0xffaf;
const XK_KP_0: u32 = 0xffb0;
const XK_KP_1: u32 = 0xffb1;
const XK_KP_2: u32 = 0xffb2;
const XK_KP_3: u32 = 0xffb3;
const XK_KP_4: u32 = 0xffb4;
const XK_KP_5: u32 = 0xffb5;
const XK_KP_6: u32 = 0xffb6;
const XK_KP_7: u32 = 0xffb7;
const XK_KP_8: u32 = 0xffb8;
const XK_KP_9: u32 = 0xffb9;

// Function keys
const XK_F1: u32 = 0xffbe;
const XK_F2: u32 = 0xffbf;
const XK_F3: u32 = 0xffc0;
const XK_F4: u32 = 0xffc1;
const XK_F5: u32 = 0xffc2;
const XK_F6: u32 = 0xffc3;
const XK_F7: u32 = 0xffc4;
const XK_F8: u32 = 0xffc5;
const XK_F9: u32 = 0xffc6;
const XK_F10: u32 = 0xffc7;
const XK_F11: u32 = 0xffc8;
const XK_F12: u32 = 0xffc9;
const XK_F13: u32 = 0xffca;
const XK_F14: u32 = 0xffcb;
const XK_F15: u32 = 0xffcc;
const XK_F16: u32 = 0xffcd;
const XK_F17: u32 = 0xffce;
const XK_F18: u32 = 0xffcf;
const XK_F19: u32 = 0xffd0;
const XK_F20: u32 = 0xffd1;
const XK_F21: u32 = 0xffd2;
const XK_F22: u32 = 0xffd3;
const XK_F23: u32 = 0xffd4;
const XK_F24: u32 = 0xffd5;

// Modifier keys
const XK_SHIFT_L: u32 = 0xffe1;
const XK_SHIFT_R: u32 = 0xffe2;
const XK_CONTROL_L: u32 = 0xffe3;
const XK_CONTROL_R: u32 = 0xffe4;
const XK_CAPS_LOCK: u32 = 0xffe5;
const XK_META_L: u32 = 0xffe7;
const XK_META_R: u32 = 0xffe8;
const XK_ALT_L: u32 = 0xffe9;
const XK_ALT_R: u32 = 0xffea;
const XK_SUPER_L: u32 = 0xffeb;
const XK_SUPER_R: u32 = 0xffec;
const XK_HYPER_L: u32 = 0xffed;
const XK_HYPER_R: u32 = 0xffee;

// Latin-1 (keysym == codepoint for 0x20..=0x7e)
const XK_SPACE: u32 = 0x0020;

// ---------------------------------------------------------------------------
//  Named key table
// ---------------------------------------------------------------------------

/// Entry in the named key lookup table.
struct NamedKeyEntry {
    keysym: u32,
    is_modifier: bool,
}

/// `(name, keysym, is_modifier)` — names are matched case-insensitively.
const NAMED_KEYS: &[(&str, u32, bool)] = &[
    // Modifiers
    ("SHIFT", XK_SHIFT_L, true),
    ("LSHIFT", XK_SHIFT_L, true),
    ("RSHIFT", XK_SHIFT_R, true),
    ("CONTROL", XK_CONTROL_L, true),
    ("CTRL", XK_CONTROL_L, true),
    ("LCONTROL", XK_CONTROL_L, true),
    ("LCTRL", XK_CONTROL_L, true),
    ("RCONTROL", XK_CONTROL_R, true),
    ("RCTRL", XK_CONTROL_R, true),
    ("ALT", XK_ALT_L, true),
    ("LALT", XK_ALT_L, true),
    ("RALT", XK_ALT_R, true),
    ("ALTGR", XK_ALT_R, true),
    ("META", XK_META_L, true),
    ("LMETA", XK_META_L, true),
    ("RMETA", XK_META_R, true),
    ("SUPER", XK_SUPER_L, true),
    ("WIN", XK_SUPER_L, true),
    ("WINDOWS", XK_SUPER_L, true),
    ("LWIN", XK_SUPER_L, true),
    ("RWIN", XK_SUPER_R, true),
    ("HYPER", XK_HYPER_L, true),
    ("LHYPER", XK_HYPER_L, true),
    ("RHYPER", XK_HYPER_R, true),
    ("CAPSLOCK", XK_CAPS_LOCK, true),
    ("CAPS_LOCK", XK_CAPS_LOCK, true),
    ("NUMLOCK", XK_NUM_LOCK, true),
    ("NUM_LOCK", XK_NUM_LOCK, true),
    ("SCROLLLOCK", XK_SCROLL_LOCK, true),
    ("SCROLL_LOCK", XK_SCROLL_LOCK, true),
    // Navigation & editing
    ("RETURN", XK_RETURN, false),
    ("ENTER", XK_RETURN, false),
    ("TAB", XK_TAB, false),
    ("ESCAPE", XK_ESCAPE, false),
    ("ESC", XK_ESCAPE, false),
    ("BACKSPACE", XK_BACKSPACE, false),
    ("DELETE", XK_DELETE, false),
    ("DEL", XK_DELETE, false),
    ("INSERT", XK_INSERT, false),
    ("INS", XK_INSERT, false),
    ("HOME", XK_HOME, false),
    ("END", XK_END, false),
    ("PAGEUP", XK_PAGE_UP, false),
    ("PAGE_UP", XK_PAGE_UP, false),
    ("PGUP", XK_PAGE_UP, false),
    ("PAGEDOWN", XK_PAGE_DOWN, false),
    ("PAGE_DOWN", XK_PAGE_DOWN, false),
    ("PGDN", XK_PAGE_DOWN, false),
    ("SPACE", XK_SPACE, false),
    // Arrow keys
    ("LEFT", XK_LEFT, false),
    ("RIGHT", XK_RIGHT, false),
    ("UP", XK_UP, false),
    ("DOWN", XK_DOWN, false),
    ("ARROWLEFT", XK_LEFT, false),
    ("ARROWRIGHT", XK_RIGHT, false),
    ("ARROWUP", XK_UP, false),
    ("ARROWDOWN", XK_DOWN, false),
    // Function keys
    ("F1", XK_F1, false),
    ("F2", XK_F2, false),
    ("F3", XK_F3, false),
    ("F4", XK_F4, false),
    ("F5", XK_F5, false),
    ("F6", XK_F6, false),
    ("F7", XK_F7, false),
    ("F8", XK_F8, false),
    ("F9", XK_F9, false),
    ("F10", XK_F10, false),
    ("F11", XK_F11, false),
    ("F12", XK_F12, false),
    ("F13", XK_F13, false),
    ("F14", XK_F14, false),
    ("F15", XK_F15, false),
    ("F16", XK_F16, false),
    ("F17", XK_F17, false),
    ("F18", XK_F18, false),
    ("F19", XK_F19, false),
    ("F20", XK_F20, false),
    ("F21", XK_F21, false),
    ("F22", XK_F22, false),
    ("F23", XK_F23, false),
    ("F24", XK_F24, false),
    // Print / Pause / etc.
    ("PRINTSCREEN", XK_PRINT, false),
    ("PRINT", XK_PRINT, false),
    ("PRTSC", XK_PRINT, false),
    ("PAUSE", XK_PAUSE, false),
    ("BREAK", XK_BREAK, false),
    ("MENU", XK_MENU, false),
    ("APPS", XK_MENU, false),
    ("HELP", XK_HELP, false),
    ("SYSREQ", XK_SYS_REQ, false),
    // Numpad
    ("NUMPAD0", XK_KP_0, false),
    ("NUMPAD1", XK_KP_1, false),
    ("NUMPAD2", XK_KP_2, false),
    ("NUMPAD3", XK_KP_3, false),
    ("NUMPAD4", XK_KP_4, false),
    ("NUMPAD5", XK_KP_5, false),
    ("NUMPAD6", XK_KP_6, false),
    ("NUMPAD7", XK_KP_7, false),
    ("NUMPAD8", XK_KP_8, false),
    ("NUMPAD9", XK_KP_9, false),
    ("KP_0", XK_KP_0, false),
    ("KP_1", XK_KP_1, false),
    ("KP_2", XK_KP_2, false),
    ("KP_3", XK_KP_3, false),
    ("KP_4", XK_KP_4, false),
    ("KP_5", XK_KP_5, false),
    ("KP_6", XK_KP_6, false),
    ("KP_7", XK_KP_7, false),
    ("KP_8", XK_KP_8, false),
    ("KP_9", XK_KP_9, false),
    ("KP_ENTER", XK_KP_ENTER, false),
    ("KP_ADD", XK_KP_ADD, false),
    ("KP_SUBTRACT", XK_KP_SUBTRACT, false),
    ("KP_MULTIPLY", XK_KP_MULTIPLY, false),
    ("KP_DIVIDE", XK_KP_DIVIDE, false),
    ("KP_DECIMAL", XK_KP_DECIMAL, false),
    ("NUMPADADD", XK_KP_ADD, false),
    ("NUMPADSUBTRACT", XK_KP_SUBTRACT, false),
    ("NUMPADMULTIPLY", XK_KP_MULTIPLY, false),
    ("NUMPADDIVIDE", XK_KP_DIVIDE, false),
    ("NUMPADDECIMAL", XK_KP_DECIMAL, false),
    ("NUMPADENTER", XK_KP_ENTER, false),
    // Short-form aliases matching Windows VK_ names for cross-platform consistency
    ("ADD", XK_KP_ADD, false),
    ("SUBTRACT", XK_KP_SUBTRACT, false),
    ("MULTIPLY", XK_KP_MULTIPLY, false),
    ("DIVIDE", XK_KP_DIVIDE, false),
    ("DECIMAL", XK_KP_DECIMAL, false),
    ("KP_HOME", XK_KP_HOME, false),
    ("KP_LEFT", XK_KP_LEFT, false),
    ("KP_UP", XK_KP_UP, false),
    ("KP_RIGHT", XK_KP_RIGHT, false),
    ("KP_DOWN", XK_KP_DOWN, false),
    ("KP_PAGE_UP", XK_KP_PAGE_UP, false),
    ("KP_PAGE_DOWN", XK_KP_PAGE_DOWN, false),
    ("KP_END", XK_KP_END, false),
    ("KP_INSERT", XK_KP_INSERT, false),
    ("KP_DELETE", XK_KP_DELETE, false),
    // Symbol aliases (for characters problematic in <key> notation)
    ("PLUS", 0x002b, false),    // '+'
    ("MINUS", 0x002d, false),   // '-'
    ("LESS", 0x003c, false),    // '<'
    ("LT", 0x003c, false),      // '<'
    ("GREATER", 0x003e, false), // '>'
    ("GT", 0x003e, false),      // '>'
];

/// Build the named key lookup map (case-insensitive, keyed by uppercase name).
fn named_key_table() -> &'static HashMap<String, NamedKeyEntry> {
    static TABLE: LazyLock<HashMap<String, NamedKeyEntry>> = LazyLock::new(|| {
        NAMED_KEYS
            .iter()
            .map(|&(name, keysym, is_modifier)| (name.to_ascii_uppercase(), NamedKeyEntry { keysym, is_modifier }))
            .collect()
    });
    &TABLE
}

// ---------------------------------------------------------------------------
//  Platform key code (stored inside opaque `KeyCode`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct X11KeyCode(X11Key);

/// Describes how a key is produced on the X11 server.
#[derive(Clone, Debug)]
enum X11Key {
    /// Key exists in the keymap at a known keycode.
    ///
    /// For text characters that require Shift, `shift_required` is `true`.
    /// The auto-shift logic in [`send_event_direct`] will inject/release
    /// Shift automatically unless Ctrl or Alt is already held (chord mode).
    Direct { keycode: u8, shift_required: bool },

    /// Modifier key (Shift, Ctrl, Alt, etc.) — always sent as a raw
    /// press/release without automatic shift management.
    Modifier { keycode: u8 },

    /// Character whose keysym is not in the current keyboard mapping.
    ///
    /// At injection time, a spare (unmapped) keycode is temporarily remapped
    /// to this keysym via [`ChangeKeyboardMapping`], the key event is sent,
    /// and the mapping is restored.
    DynamicRemap { keysym: u32 },
}

// ---------------------------------------------------------------------------
//  Keyboard mapping cache
// ---------------------------------------------------------------------------

/// Snapshot of the X11 keyboard mapping, built from `GetKeyboardMapping`.
struct KeymapInfo {
    /// Keysyms per keycode (number of columns).
    keysyms_per_keycode: u8,
    /// Reverse map: keysym → `(keycode, column)`.
    ///
    /// Column 0 = unmodified, column 1 = Shift.
    /// Only the first mapping found for each keysym is kept (lowest keycode).
    keysym_to_keycode: HashMap<u32, (u8, u8)>,
    /// A keycode that has no keysym binding — used for dynamic remapping.
    spare_keycode: Option<u8>,
    /// Keycode for `Shift_L` (used for auto-shift injection).
    shift_keycode: Option<u8>,
}

impl KeymapInfo {
    /// Load the keyboard mapping from the X11 server.
    fn load(guard: &X11Handle) -> Result<Self, KeyboardError> {
        let setup = guard.conn.setup();
        let min_kc = setup.min_keycode;
        let max_kc = setup.max_keycode;
        let count = max_kc - min_kc + 1;

        let reply = guard.conn.get_keyboard_mapping(min_kc, count).map_err(to_kb)?.reply().map_err(to_kb)?;

        let kpk = reply.keysyms_per_keycode;
        let keysyms = &reply.keysyms;

        let mut map: HashMap<u32, (u8, u8)> = HashMap::with_capacity(keysyms.len() / 2);
        let mut spare = None;

        for kc_offset in 0..u16::from(count) {
            let kc = min_kc.wrapping_add(kc_offset as u8);
            let base = kc_offset as usize * usize::from(kpk);
            let mut all_empty = true;

            // Only inspect columns 0 (unmodified) and 1 (Shift) for the
            // reverse map.  Higher columns (Mode_switch/AltGr) are skipped
            // to keep the logic simple; characters only reachable via AltGr
            // fall through to the DynamicRemap path.
            for col in 0..usize::from(kpk).min(2) {
                let ks = keysyms[base + col];
                if ks != 0 {
                    all_empty = false;
                    // First-keycode-wins: preserve the lowest keycode for
                    // each keysym so that standard keys take priority over
                    // duplicates on the numpad, etc.
                    map.entry(ks).or_insert((kc, col as u8));
                }
            }

            // Check remaining columns for emptiness (for spare detection).
            if all_empty {
                for col in 2..usize::from(kpk) {
                    if keysyms[base + col] != 0 {
                        all_empty = false;
                        break;
                    }
                }
            }

            if all_empty && spare.is_none() {
                spare = Some(kc);
            }
        }

        let shift_keycode = map.get(&XK_SHIFT_L).map(|(kc, _)| *kc);

        debug!(
            keycodes = count,
            keysyms_per_keycode = kpk,
            entries = map.len(),
            spare = ?spare,
            shift_keycode = ?shift_keycode,
            "X11 keyboard mapping loaded",
        );

        Ok(Self { keysyms_per_keycode: kpk, keysym_to_keycode: map, spare_keycode: spare, shift_keycode })
    }

    /// Look up the keycode and column for a keysym.
    ///
    /// Returns `(keycode, column)` where column 0 = unmodified and 1 = Shift.
    fn find_keycode(&self, keysym: u32) -> Option<(u8, u8)> {
        self.keysym_to_keycode.get(&keysym).copied()
    }
}

// ---------------------------------------------------------------------------
//  Global keyboard state
// ---------------------------------------------------------------------------

struct KeyboardState {
    keymap: Option<KeymapInfo>,
}

fn keyboard_state() -> &'static Mutex<KeyboardState> {
    static STATE: OnceLock<Mutex<KeyboardState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(KeyboardState { keymap: None }))
}

/// Ensure the keymap is loaded, returning a reference to it.
///
/// Acquires and releases the X11 connection lock to fetch the mapping.
/// The resulting `KeymapInfo` is stored in global state for reuse.
fn ensure_keymap() -> Result<(), KeyboardError> {
    let mut state = keyboard_state().lock().map_err(|_| {
        KeyboardError::Platform(PlatformError::new(PlatformErrorKind::OperationFailed, "keyboard state lock poisoned"))
    })?;

    if state.keymap.is_some() {
        return Ok(());
    }

    let guard = connection().map_err(KeyboardError::Platform)?;
    let keymap = KeymapInfo::load(&guard)?;
    state.keymap = Some(keymap);
    Ok(())
}

// ---------------------------------------------------------------------------
//  Keysym conversion helpers
// ---------------------------------------------------------------------------

/// Convert a Unicode character to its corresponding X11 keysym.
///
/// Control characters (`\n`, `\t`, `\r`, `\x08`, `\x1b`, `\x7f`) are mapped
/// to their corresponding X11 TTY function keysyms (Return, Tab, etc.)
/// rather than producing unusable Unicode keysyms.
///
/// For Latin-1 printable characters (`0x20..=0x7e`, `0xa0..=0xff`), the
/// keysym equals the code point.  For other Unicode characters, the keysym
/// is `0x0100_0000 | codepoint`.
fn char_to_keysym(ch: char) -> u32 {
    // Map common control characters to their X11 TTY function keysyms.
    // These occur when the keyboard_sequence parser passes '\n', '\t', etc.
    // as individual characters to key_to_code.
    match ch {
        '\n' | '\r' => XK_RETURN,
        '\t' => XK_TAB,
        '\u{08}' => XK_BACKSPACE, // '\b' = backspace
        '\u{1b}' => XK_ESCAPE,    // ESC
        '\u{7f}' => XK_DELETE,    // DEL
        _ => {
            let cp = ch as u32;
            match cp {
                0x0020..=0x007e | 0x00a0..=0x00ff => cp,
                _ => 0x0100_0000 | cp,
            }
        }
    }
}

/// Check whether CapsLock is currently active by querying the X11 modifier
/// mask.
fn is_caps_lock_on(guard: &X11Handle) -> bool {
    let root = root_window_from(guard);
    guard
        .conn
        .query_pointer(root)
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| u32::from(r.mask) & u32::from(KeyButMask::LOCK) != 0)
        .unwrap_or(false)
}

/// Return the current modifier mask from `QueryPointer`.
fn query_modifier_mask(guard: &X11Handle) -> u32 {
    let root = root_window_from(guard);
    guard.conn.query_pointer(root).ok().and_then(|c| c.reply().ok()).map(|r| u32::from(r.mask)).unwrap_or(0)
}

// ---------------------------------------------------------------------------
//  XTest injection helpers
// ---------------------------------------------------------------------------

/// XTest event type codes.
const XTEST_KEY_PRESS: u8 = 2;
const XTEST_KEY_RELEASE: u8 = 3;

/// Inject a raw key event via XTest.
fn inject_key(guard: &X11Handle, type_code: u8, keycode: u8, root: u32) -> Result<(), KeyboardError> {
    xtest::fake_input(&guard.conn, type_code, keycode, 0, root, 0, 0, 0).map_err(to_kb)?;
    guard.conn.flush().map_err(to_kb)?;
    Ok(())
}

/// Sync with the X server — ensures all preceding requests have been
/// processed.  Used after `ChangeKeyboardMapping` before sending key events.
fn x_sync(guard: &X11Handle) -> Result<(), KeyboardError> {
    guard.conn.get_input_focus().map_err(to_kb)?.reply().map_err(to_kb)?;
    Ok(())
}

// ---------------------------------------------------------------------------
//  Key event dispatching
// ---------------------------------------------------------------------------

/// Send a direct key event, automatically managing Shift for text characters.
///
/// When `shift_required` is `true` and no Ctrl/Alt modifier is currently
/// held (indicating a chord), Shift is injected before press and released
/// after release.  This matches the Windows platform behaviour for typed
/// characters that require Shift.
fn send_event_direct(
    guard: &X11Handle,
    keycode: u8,
    shift_required: bool,
    state: KeyState,
    root: u32,
    shift_keycode: Option<u8>,
) -> Result<(), KeyboardError> {
    let ctrl_alt_mask = u32::from(KeyButMask::CONTROL) | u32::from(KeyButMask::MOD1);

    match state {
        KeyState::Press => {
            if shift_required {
                let mods = query_modifier_mask(guard);
                let in_chord = mods & ctrl_alt_mask != 0;
                if !in_chord && let Some(shift_kc) = shift_keycode {
                    trace!(shift_kc, "auto-injecting Shift press");
                    inject_key(guard, XTEST_KEY_PRESS, shift_kc, root)?;
                }
            }
            inject_key(guard, XTEST_KEY_PRESS, keycode, root)?;
        }
        KeyState::Release => {
            inject_key(guard, XTEST_KEY_RELEASE, keycode, root)?;
            if shift_required {
                let mods = query_modifier_mask(guard);
                let in_chord = mods & ctrl_alt_mask != 0;
                if !in_chord && let Some(shift_kc) = shift_keycode {
                    trace!(shift_kc, "auto-releasing Shift");
                    inject_key(guard, XTEST_KEY_RELEASE, shift_kc, root)?;
                }
            }
        }
    }

    Ok(())
}

/// Send a key event for a dynamically remapped keysym.
///
/// Temporarily remaps a spare keycode to the desired keysym, sends the
/// event, and restores the original (empty) mapping.
fn send_event_remap(
    guard: &X11Handle,
    keysym: u32,
    state: KeyState,
    root: u32,
    keymap: &KeymapInfo,
) -> Result<(), KeyboardError> {
    let spare = keymap.spare_keycode.ok_or_else(|| {
        KeyboardError::UnsupportedKey(format!("no spare keycode for dynamic remap of keysym {keysym:#x}"))
    })?;
    let kpk = keymap.keysyms_per_keycode;

    match state {
        KeyState::Press => {
            // Build the new keysym list: desired keysym at column 0, zeros elsewhere.
            let mut new_keysyms = vec![0u32; usize::from(kpk)];
            new_keysyms[0] = keysym;

            trace!(spare, keysym, "remap spare keycode for press");
            guard.conn.change_keyboard_mapping(1, spare, kpk, &new_keysyms).map_err(to_kb)?;
            x_sync(guard)?;
            inject_key(guard, XTEST_KEY_PRESS, spare, root)?;
        }
        KeyState::Release => {
            inject_key(guard, XTEST_KEY_RELEASE, spare, root)?;

            // Restore the spare keycode to unmapped (all zeros).
            let zero_keysyms = vec![0u32; usize::from(kpk)];
            trace!(spare, keysym, "restoring spare keycode after release");
            guard.conn.change_keyboard_mapping(1, spare, kpk, &zero_keysyms).map_err(to_kb)?;
            x_sync(guard)?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
//  LinuxKeyboardDevice
// ---------------------------------------------------------------------------

pub struct LinuxKeyboardDevice;

impl KeyboardDevice for LinuxKeyboardDevice {
    fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
        ensure_keymap()?;

        let state = keyboard_state().lock().map_err(|_| {
            KeyboardError::Platform(PlatformError::new(
                PlatformErrorKind::OperationFailed,
                "keyboard state lock poisoned",
            ))
        })?;
        let keymap = state.keymap.as_ref().ok_or(KeyboardError::NotReady)?;

        // 1) Named key lookup (case-insensitive)
        let upper = name.to_ascii_uppercase();
        if let Some(entry) = named_key_table().get(&upper) {
            if let Some((kc, _col)) = keymap.find_keycode(entry.keysym) {
                return if entry.is_modifier {
                    trace!(name, keysym = entry.keysym, keycode = kc, "resolved modifier key");
                    Ok(KeyCode::new(X11KeyCode(X11Key::Modifier { keycode: kc })))
                } else {
                    // Named non-modifier keys (ENTER, TAB, F1, etc.) do not
                    // require auto-shift — the keysym is directly at column 0.
                    let shift = _col == 1;
                    trace!(name, keysym = entry.keysym, keycode = kc, shift, "resolved named key");
                    Ok(KeyCode::new(X11KeyCode(X11Key::Direct { keycode: kc, shift_required: shift })))
                };
            }
            // Keysym not in keymap — try dynamic remap.
            if keymap.spare_keycode.is_some() {
                debug!(name, keysym = entry.keysym, "named key not in keymap — will use dynamic remap");
                return Ok(KeyCode::new(X11KeyCode(X11Key::DynamicRemap { keysym: entry.keysym })));
            }
            return Err(KeyboardError::UnsupportedKey(name.to_owned()));
        }

        // 2) Single character
        let mut chars = name.chars();
        if let Some(ch) = chars.next()
            && chars.next().is_none()
        {
            return Self::resolve_char(ch, keymap);
        }

        Err(KeyboardError::UnsupportedKey(name.to_owned()))
    }

    fn start_input(&self) -> Result<(), KeyboardError> {
        // Refresh the keyboard mapping at the start of each input sequence
        // to pick up any layout changes.
        let guard = connection().map_err(KeyboardError::Platform)?;
        let keymap = KeymapInfo::load(&guard)?;
        drop(guard);

        let mut state = keyboard_state().lock().map_err(|_| {
            KeyboardError::Platform(PlatformError::new(
                PlatformErrorKind::OperationFailed,
                "keyboard state lock poisoned",
            ))
        })?;
        state.keymap = Some(keymap);
        debug!("keyboard input session started — keymap refreshed");
        Ok(())
    }

    fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError> {
        let Some(x11kc) = event.code.downcast_ref::<X11KeyCode>() else {
            return Err(KeyboardError::UnsupportedKey("foreign key code".to_string()));
        };

        // Read keymap info without holding the lock during X11 calls.
        let (shift_keycode, spare_keycode, keysyms_per_keycode) = {
            let state = keyboard_state().lock().map_err(|_| {
                KeyboardError::Platform(PlatformError::new(
                    PlatformErrorKind::OperationFailed,
                    "keyboard state lock poisoned",
                ))
            })?;
            let km = state.keymap.as_ref().ok_or(KeyboardError::NotReady)?;
            (km.shift_keycode, km.spare_keycode, km.keysyms_per_keycode)
        };

        let guard = connection().map_err(KeyboardError::Platform)?;
        let root = root_window_from(&guard);

        match &x11kc.0 {
            X11Key::Direct { keycode, shift_required } => {
                trace!(keycode, shift_required, state = ?event.state, "send direct key");
                send_event_direct(&guard, *keycode, *shift_required, event.state, root, shift_keycode)?;
            }
            X11Key::Modifier { keycode } => {
                let type_code = match event.state {
                    KeyState::Press => XTEST_KEY_PRESS,
                    KeyState::Release => XTEST_KEY_RELEASE,
                };
                trace!(keycode, state = ?event.state, "send modifier key");
                inject_key(&guard, type_code, *keycode, root)?;
            }
            X11Key::DynamicRemap { keysym } => {
                // Build a minimal KeymapInfo view for the remap function.
                let mini =
                    KeymapInfo { keysyms_per_keycode, keysym_to_keycode: HashMap::new(), spare_keycode, shift_keycode };
                trace!(keysym, state = ?event.state, "send dynamic-remap key");
                send_event_remap(&guard, *keysym, event.state, root, &mini)?;
            }
        }

        Ok(())
    }

    fn end_input(&self) -> Result<(), KeyboardError> {
        debug!("keyboard input session ended");
        Ok(())
    }

    fn known_key_names(&self) -> Vec<String> {
        let mut names: Vec<String> = named_key_table().keys().cloned().collect();
        for ch in 'A'..='Z' {
            names.push(ch.to_string());
        }
        for ch in '0'..='9' {
            names.push(ch.to_string());
        }
        names.sort_unstable();
        names.dedup();
        names
    }
}

impl LinuxKeyboardDevice {
    /// Resolve a single character to a `KeyCode`.
    ///
    /// Accounts for CapsLock: when CapsLock is active and the character is
    /// ASCII-alphabetic, the shift requirement is inverted so the correct
    /// case is produced.
    fn resolve_char(ch: char, keymap: &KeymapInfo) -> Result<KeyCode, KeyboardError> {
        let keysym = char_to_keysym(ch);

        // Try direct lookup in the keymap.
        if let Some((kc, col)) = keymap.find_keycode(keysym) {
            let mut shift = col == 1;

            // CapsLock compensation for ASCII letters.
            if ch.is_ascii_alphabetic() {
                let guard = connection().map_err(KeyboardError::Platform)?;
                if is_caps_lock_on(&guard) {
                    shift = !shift;
                }
            }

            trace!(ch = %ch, keysym, keycode = kc, shift, "resolved character");
            return Ok(KeyCode::new(X11KeyCode(X11Key::Direct { keycode: kc, shift_required: shift })));
        }

        // For uppercase ASCII letters not directly in the keymap, try
        // looking up the lowercase equivalent with Shift.
        if ch.is_ascii_uppercase() {
            let lower = ch.to_ascii_lowercase();
            let lower_ks = char_to_keysym(lower);
            if let Some((kc, _)) = keymap.find_keycode(lower_ks) {
                let guard = connection().map_err(KeyboardError::Platform)?;
                let caps_on = is_caps_lock_on(&guard);
                // With CapsLock off, need Shift; with CapsLock on, no Shift needed.
                let shift = !caps_on;
                trace!(ch = %ch, keysym, keycode = kc, shift, "resolved uppercase via lowercase");
                return Ok(KeyCode::new(X11KeyCode(X11Key::Direct { keycode: kc, shift_required: shift })));
            }
        }

        // Dynamic remap fallback for characters outside the active layout.
        if keymap.spare_keycode.is_some() {
            debug!(ch = %ch, keysym, "character not in keymap — will use dynamic remap");
            return Ok(KeyCode::new(X11KeyCode(X11Key::DynamicRemap { keysym })));
        }

        Err(KeyboardError::UnsupportedKey(ch.to_string()))
    }
}

// ---------------------------------------------------------------------------
//  Error conversion helper
// ---------------------------------------------------------------------------

fn to_kb<E: std::fmt::Display>(e: E) -> KeyboardError {
    KeyboardError::Platform(PlatformError::new(PlatformErrorKind::OperationFailed, format!("x11 keyboard: {e}")))
}

// ---------------------------------------------------------------------------
//  Registration
// ---------------------------------------------------------------------------

static KBD: LinuxKeyboardDevice = LinuxKeyboardDevice;
register_keyboard_device!(&KBD);

// ---------------------------------------------------------------------------
//  Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_to_keysym_ascii() {
        assert_eq!(char_to_keysym('a'), 0x61);
        assert_eq!(char_to_keysym('A'), 0x41);
        assert_eq!(char_to_keysym('0'), 0x30);
        assert_eq!(char_to_keysym(' '), 0x20);
        assert_eq!(char_to_keysym('~'), 0x7e);
    }

    #[test]
    fn char_to_keysym_control_characters() {
        assert_eq!(char_to_keysym('\n'), XK_RETURN);
        assert_eq!(char_to_keysym('\r'), XK_RETURN);
        assert_eq!(char_to_keysym('\t'), XK_TAB);
        assert_eq!(char_to_keysym('\u{08}'), XK_BACKSPACE);
        assert_eq!(char_to_keysym('\u{1b}'), XK_ESCAPE);
        assert_eq!(char_to_keysym('\u{7f}'), XK_DELETE);
    }

    #[test]
    fn char_to_keysym_latin1() {
        assert_eq!(char_to_keysym('\u{00e4}'), 0xe4); // ä
        assert_eq!(char_to_keysym('\u{00a0}'), 0xa0); // NBSP
        assert_eq!(char_to_keysym('\u{00ff}'), 0xff); // ÿ
    }

    #[test]
    fn char_to_keysym_unicode() {
        assert_eq!(char_to_keysym('€'), 0x0100_0000 | 0x20ac);
        assert_eq!(char_to_keysym('日'), 0x0100_0000 | 0x65e5);
    }

    #[test]
    fn named_key_table_contains_common_keys() {
        let table = named_key_table();
        assert!(table.contains_key("ENTER"));
        assert!(table.contains_key("ESCAPE"));
        assert!(table.contains_key("TAB"));
        assert!(table.contains_key("SHIFT"));
        assert!(table.contains_key("CTRL"));
        assert!(table.contains_key("ALT"));
        assert!(table.contains_key("F1"));
        assert!(table.contains_key("F12"));
        assert!(table.contains_key("SPACE"));
        assert!(table.contains_key("BACKSPACE"));
    }

    #[test]
    fn named_key_table_modifiers_flagged() {
        let table = named_key_table();
        assert!(table.get("SHIFT").unwrap().is_modifier);
        assert!(table.get("CTRL").unwrap().is_modifier);
        assert!(table.get("ALT").unwrap().is_modifier);
        assert!(table.get("CAPSLOCK").unwrap().is_modifier);
        assert!(!table.get("ENTER").unwrap().is_modifier);
        assert!(!table.get("F1").unwrap().is_modifier);
        assert!(!table.get("SPACE").unwrap().is_modifier);
    }

    #[test]
    fn named_key_table_symbol_aliases() {
        let table = named_key_table();
        assert_eq!(table.get("PLUS").unwrap().keysym, 0x002b);
        assert_eq!(table.get("LESS").unwrap().keysym, 0x003c);
        assert_eq!(table.get("GREATER").unwrap().keysym, 0x003e);
        assert_eq!(table.get("MINUS").unwrap().keysym, 0x002d);
    }

    #[test]
    fn named_key_aliases_resolve_same_keysym() {
        let table = named_key_table();
        assert_eq!(table.get("ENTER").unwrap().keysym, table.get("RETURN").unwrap().keysym);
        assert_eq!(table.get("ESC").unwrap().keysym, table.get("ESCAPE").unwrap().keysym);
        assert_eq!(table.get("DEL").unwrap().keysym, table.get("DELETE").unwrap().keysym);
        assert_eq!(table.get("INS").unwrap().keysym, table.get("INSERT").unwrap().keysym);
        assert_eq!(table.get("PGUP").unwrap().keysym, table.get("PAGEUP").unwrap().keysym);
        assert_eq!(table.get("PGDN").unwrap().keysym, table.get("PAGEDOWN").unwrap().keysym);
    }

    #[test]
    fn known_key_names_includes_chars_and_named() {
        let device = LinuxKeyboardDevice;
        let names = device.known_key_names();
        // Should contain named keys
        assert!(names.contains(&"ENTER".to_string()));
        assert!(names.contains(&"SHIFT".to_string()));
        // Should contain single characters
        assert!(names.contains(&"A".to_string()));
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"0".to_string()));
        // Should be sorted
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }
}
