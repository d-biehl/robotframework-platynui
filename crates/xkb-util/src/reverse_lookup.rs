use std::collections::HashMap;
use std::ffi::OsStr;

use xkbcommon::xkb::{self, Keycode, Keymap, Keysym, ModIndex, ModMask};

/// A key combination: physical keycode plus modifier mask needed to produce a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCombination {
    /// XKB keycode (evdev + 8).
    pub keycode: Keycode,
    /// XKB layout index.
    pub layout: u32,
    /// Shift level within the layout.
    pub level: u32,
    /// Modifier mask required (bitfield using [`modifier_bit`] constants).
    pub modifiers: u32,
}

/// How to type a character on the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// A single key press (with optional modifiers).
    Simple(KeyCombination),
    /// A dead key followed by a base key to produce a character.
    ///
    /// For standalone accents (e.g. `´`), the base key is Space.
    /// For composed characters (e.g. `à`), the base key is the letter (`a`).
    Compose {
        /// The dead key to press first (e.g. `dead_grave`).
        dead_key: KeyCombination,
        /// The key to press after the dead key (e.g. `a` for `à`, Space for standalone accent).
        base_key: KeyCombination,
    },
}

/// Well-known modifier bit positions (matching typical XKB keymaps).
pub mod modifier_bit {
    /// Shift modifier.
    pub const SHIFT: u32 = 1 << 0;
    /// Caps Lock.
    pub const CAPS_LOCK: u32 = 1 << 1;
    /// Control modifier.
    pub const CONTROL: u32 = 1 << 2;
    /// Alt / Mod1.
    pub const ALT: u32 = 1 << 3;
    /// Mod2 (typically Num Lock).
    pub const NUM_LOCK: u32 = 1 << 4;
    /// Mod3.
    pub const MOD3: u32 = 1 << 5;
    /// Super / Logo / Mod4.
    pub const LOGO: u32 = 1 << 6;
    /// ISO Level3 Shift (`AltGr`).
    pub const LEVEL3_SHIFT: u32 = 1 << 7;
}

/// Modifier names in XKB discovery order, paired with the bit we assign.
const MOD_NAMES: &[(&str, u32)] = &[
    ("Shift", modifier_bit::SHIFT),
    // CapsLock is discovered by index but not typically needed for text input;
    // we skip it in the typed-output lookup to prefer Shift combos.
    ("Control", modifier_bit::CONTROL),
    ("Mod1", modifier_bit::ALT),
    ("Mod4", modifier_bit::LOGO),
    ("ISO_Level3_Shift", modifier_bit::LEVEL3_SHIFT),
    // Mod5 is often the real modifier backing ISO_Level3_Shift (AltGr);
    // xkbcommon may not resolve the virtual modifier name, so also try the real one.
    ("Mod5", modifier_bit::LEVEL3_SHIFT),
];

/// Reverse lookup table: given a keymap, find the keycode + modifiers for any character.
#[derive(Debug)]
pub struct KeymapLookup {
    /// `char` → `KeyAction`, preferring simple over compose, fewer modifiers, lower level.
    map: HashMap<char, KeyAction>,
    /// Modifier index → bit mapping discovered from the specific keymap.
    mod_index_to_bit: HashMap<ModIndex, u32>,
}

impl KeymapLookup {
    /// Build the reverse lookup table from an XKB keymap string (text v1 format).
    ///
    /// This is the preferred constructor when you receive a keymap string from
    /// an external source (e.g. an EIS keyboard device or a Wayland compositor).
    ///
    /// # Errors
    ///
    /// Returns an error if the keymap string cannot be parsed.
    pub fn from_string(keymap_string: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_string(
            &context,
            keymap_string.to_string(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or("failed to parse XKB keymap string")?;
        Ok(Self::new(&keymap))
    }

    /// Build the reverse lookup table for the given keymap.
    ///
    /// For each character that can be produced by a key + modifier combination in the
    /// keymap, stores the preferred (fewest modifiers) way to type it.
    ///
    /// Uses `key_get_mods_for_level` to query the exact modifier masks from XKB
    /// rather than guessing from the level index, so all key types (four-level,
    /// eight-level, etc.) are handled correctly.
    ///
    /// Dead keys and compose sequences (e.g. `dead_grave` + a → à) are resolved
    /// using the system compose table (via libxkbcommon's compose API).
    /// When no compose table is available, dead keys and composed characters
    /// are silently omitted from the lookup.
    pub fn new(keymap: &Keymap) -> Self {
        let mod_index_to_bit = discover_mod_bits(keymap);
        let mut map: HashMap<char, KeyAction> = HashMap::new();

        // Per-keysym best combo, deduplicated for the compose phase.
        let mut dead_syms: HashMap<u32, (Keysym, KeyCombination)> = HashMap::new();
        let mut base_syms: HashMap<u32, (Keysym, KeyCombination)> = HashMap::new();

        // Phase 1: Scan all keys and collect simple entries + dead/base keysyms.
        keymap.key_for_each(|km, keycode| {
            let num_layouts = km.num_layouts_for_key(keycode);
            for layout in 0..num_layouts {
                let num_levels = km.num_levels_for_key(keycode, layout);
                for level in 0..num_levels {
                    let syms = km.key_get_syms_by_level(keycode, layout, level);

                    let mut masks_buf = [ModMask::default(); 16];
                    let num_masks = km.key_get_mods_for_level(keycode, layout, level, &mut masks_buf);
                    let masks = &masks_buf[..num_masks];

                    for &sym in syms {
                        let modifiers = masks
                            .iter()
                            .filter_map(|&xkb_mask| xkb_mask_to_bits(xkb_mask, &mod_index_to_bit))
                            .min_by_key(|bits| bits.count_ones())
                            .unwrap_or(0);

                        let combo = KeyCombination { keycode, layout, level, modifiers };

                        if is_dead_keysym(sym) {
                            dead_syms
                                .entry(sym.raw())
                                .and_modify(|(_, existing)| {
                                    if is_preferred_combo(&combo, existing) {
                                        *existing = combo;
                                    }
                                })
                                .or_insert((sym, combo));
                        } else if let Some(ch) = keysym_to_char(sym) {
                            base_syms
                                .entry(sym.raw())
                                .and_modify(|(_, existing)| {
                                    if is_preferred_combo(&combo, existing) {
                                        *existing = combo;
                                    }
                                })
                                .or_insert((sym, combo));

                            insert_if_preferred(&mut map, ch, KeyAction::Simple(combo));
                        }
                    }
                }
            }
        });

        // Phase 2: Resolve compose sequences (dead key + base key → composed char).
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        if let Some(mut compose_state) = create_compose_state(&context) {
            for &(dead_sym, dead_combo) in dead_syms.values() {
                for &(base_sym, base_combo) in base_syms.values() {
                    compose_state.reset();
                    compose_state.feed(dead_sym);
                    compose_state.feed(base_sym);
                    if compose_state.status() == xkb::compose::Status::Composed
                        && let Some(ch) = compose_state.utf8().and_then(|s| s.chars().next())
                    {
                        let action = KeyAction::Compose { dead_key: dead_combo, base_key: base_combo };
                        insert_if_preferred(&mut map, ch, action);
                    }
                }
            }
        }

        tracing::debug!(entries = map.len(), "built XKB reverse lookup table");
        Self { map, mod_index_to_bit }
    }

    /// Look up how to type `ch` on this keymap.
    ///
    /// Returns `None` if the character cannot be produced by any key combination.
    #[must_use]
    pub fn lookup(&self, ch: char) -> Option<&KeyAction> {
        self.map.get(&ch)
    }

    /// Number of characters in the lookup table.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the lookup table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterate all entries.
    pub fn iter(&self) -> impl Iterator<Item = (&char, &KeyAction)> {
        self.map.iter()
    }

    /// Returns the evdev keycode (XKB keycode − 8) for a `KeyCombination`.
    #[must_use]
    pub fn evdev_keycode(combo: &KeyCombination) -> u32 {
        combo.keycode.raw() - 8
    }

    /// Returns human-readable modifier names for a modifier mask.
    #[must_use]
    pub fn modifier_names(modifiers: u32) -> Vec<&'static str> {
        let mut names = Vec::new();
        if modifiers & modifier_bit::SHIFT != 0 {
            names.push("Shift");
        }
        if modifiers & modifier_bit::CONTROL != 0 {
            names.push("Control");
        }
        if modifiers & modifier_bit::ALT != 0 {
            names.push("Alt");
        }
        if modifiers & modifier_bit::LOGO != 0 {
            names.push("Super");
        }
        if modifiers & modifier_bit::LEVEL3_SHIFT != 0 {
            names.push("AltGr");
        }
        names
    }

    /// The modifier-index-to-bit mapping discovered from the keymap.
    #[must_use]
    pub fn mod_index_to_bit(&self) -> &HashMap<ModIndex, u32> {
        &self.mod_index_to_bit
    }
}

/// Convert a keysym to a Rust `char` using libxkbcommon's `keysym_to_utf32`.
fn keysym_to_char(sym: Keysym) -> Option<char> {
    let cp = xkb::keysym_to_utf32(sym);
    if cp == 0 { None } else { char::from_u32(cp) }
}

/// Check whether a keysym is a dead key (XKB dead key range 0xfe50–0xfe93).
fn is_dead_keysym(sym: Keysym) -> bool {
    (0xfe50..=0xfe93).contains(&sym.raw())
}

/// Try to create a compose state from the system locale.
///
/// Returns `None` if no compose table is available for the current locale.
fn create_compose_state(context: &xkb::Context) -> Option<xkb::compose::State> {
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_CTYPE"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_else(|_| "C".to_string());
    let table =
        xkb::compose::Table::new_from_locale(context, OsStr::new(&locale), xkb::compose::COMPILE_NO_FLAGS).ok()?;
    Some(xkb::compose::State::new(&table, xkb::compose::STATE_NO_FLAGS))
}

/// Discover the modifier index → bit mapping for this keymap.
fn discover_mod_bits(keymap: &Keymap) -> HashMap<ModIndex, u32> {
    let mut index_to_bit = HashMap::new();
    for &(name, bit) in MOD_NAMES {
        let idx = keymap.mod_get_index(name);
        if idx != xkb::MOD_INVALID {
            index_to_bit.insert(idx, bit);
        }
    }
    index_to_bit
}

/// Convert an XKB `ModMask` (where bit N = modifier index N) to our
/// [`modifier_bit`] scheme using the index→bit mapping from the keymap.
///
/// Returns `None` if the mask contains any modifier indices that are not in
/// our mapping — this means we cannot fully represent that combination, so
/// callers should skip it and prefer a mask we *can* express.
fn xkb_mask_to_bits(xkb_mask: ModMask, mod_index_to_bit: &HashMap<ModIndex, u32>) -> Option<u32> {
    if xkb_mask == 0 {
        return Some(0);
    }
    let mut bits = 0u32;
    let mut remaining = xkb_mask;
    let mut idx: ModIndex = 0;
    while remaining != 0 {
        if remaining & 1 != 0 {
            let Some(&bit) = mod_index_to_bit.get(&idx) else {
                return None; // unmapped modifier — skip this mask
            };
            bits |= bit;
        }
        remaining >>= 1;
        idx += 1;
    }
    Some(bits)
}

/// Insert an action into the map if it is preferred over any existing entry.
fn insert_if_preferred(map: &mut HashMap<char, KeyAction>, ch: char, action: KeyAction) {
    map.entry(ch)
        .and_modify(|existing| {
            if is_preferred_action(&action, existing) {
                *existing = action;
            }
        })
        .or_insert(action);
}

/// Returns `true` if `candidate` is preferred over `existing`.
///
/// Simple is always preferred over Compose (direct keypress beats two-key
/// sequence). Within the same variant, fewer total modifiers → lower level
/// → lower keycode wins.
fn is_preferred_action(candidate: &KeyAction, existing: &KeyAction) -> bool {
    match (candidate, existing) {
        (KeyAction::Simple(_), KeyAction::Compose { .. }) => true,
        (KeyAction::Compose { .. }, KeyAction::Simple(_)) => false,
        (KeyAction::Simple(c), KeyAction::Simple(e)) => is_preferred_combo(c, e),
        (KeyAction::Compose { dead_key: cd, base_key: cb }, KeyAction::Compose { dead_key: ed, base_key: eb }) => {
            let c_mods = cd.modifiers.count_ones() + cb.modifiers.count_ones();
            let e_mods = ed.modifiers.count_ones() + eb.modifiers.count_ones();
            if c_mods != e_mods {
                return c_mods < e_mods;
            }
            if cd.level != ed.level {
                return cd.level < ed.level;
            }
            if cb.level != eb.level {
                return cb.level < eb.level;
            }
            cd.keycode.raw() < ed.keycode.raw()
        }
    }
}

/// Returns `true` if `candidate` combo is preferred over `existing`.
///
/// Fewer modifiers → lower level → lower keycode.
fn is_preferred_combo(candidate: &KeyCombination, existing: &KeyCombination) -> bool {
    let c_mod = candidate.modifiers.count_ones();
    let e_mod = existing.modifiers.count_ones();
    if c_mod != e_mod {
        return c_mod < e_mod;
    }
    if candidate.level != existing.level {
        return candidate.level < existing.level;
    }
    candidate.keycode.raw() < existing.keycode.raw()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_keymap(layout: &str, variant: &str) -> Keymap {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        xkb::Keymap::new_from_names(&context, "", "", layout, variant, None, xkb::KEYMAP_COMPILE_NO_FLAGS)
            .expect("failed to create keymap")
    }

    /// Helper: extract the `KeyCombination` from a `Simple` action; panics otherwise.
    fn expect_simple(action: &KeyAction) -> &KeyCombination {
        match action {
            KeyAction::Simple(c) => c,
            other @ KeyAction::Compose { .. } => panic!("expected Simple, got {other:?}"),
        }
    }

    #[test]
    fn lookup_ascii_letters_us() {
        let keymap = make_keymap("us", "");
        let lookup = KeymapLookup::new(&keymap);

        // 'a' should be available and require no modifiers.
        let combo = expect_simple(lookup.lookup('a').expect("'a' not found"));
        assert_eq!(combo.modifiers, 0, "'a' should need no modifiers");
        assert_eq!(KeymapLookup::evdev_keycode(combo), 30, "evdev KEY_A = 30");

        // 'A' should require Shift.
        let combo = expect_simple(lookup.lookup('A').expect("'A' not found"));
        assert_ne!(combo.modifiers & modifier_bit::SHIFT, 0, "'A' needs Shift");
    }

    #[test]
    fn lookup_digits_us() {
        let keymap = make_keymap("us", "");
        let lookup = KeymapLookup::new(&keymap);

        let combo = expect_simple(lookup.lookup('1').expect("'1' not found"));
        assert_eq!(combo.modifiers, 0);
        assert_eq!(KeymapLookup::evdev_keycode(combo), 2, "evdev KEY_1 = 2");

        let combo = expect_simple(lookup.lookup('!').expect("'!' not found"));
        assert_ne!(combo.modifiers & modifier_bit::SHIFT, 0, "'!' needs Shift");
    }

    #[test]
    fn lookup_german_umlauts() {
        let keymap = make_keymap("de", "");
        let lookup = KeymapLookup::new(&keymap);

        for &ch in &['ü', 'ö', 'ä', 'ß'] {
            let combo = expect_simple(lookup.lookup(ch).unwrap_or_else(|| panic!("'{ch}' not found in de layout")));
            assert_eq!(combo.modifiers, 0, "'{ch}' should need no modifiers on de layout");
        }

        // Capital umlauts need Shift.
        for &ch in &['Ü', 'Ö', 'Ä'] {
            let combo = expect_simple(lookup.lookup(ch).unwrap_or_else(|| panic!("'{ch}' not found in de layout")));
            assert_ne!(combo.modifiers & modifier_bit::SHIFT, 0, "'{ch}' needs Shift");
        }
    }

    #[test]
    fn lookup_altgr_us_intl() {
        // US international layout has AltGr combos for accented characters.
        let keymap = make_keymap("us", "intl");
        let lookup = KeymapLookup::new(&keymap);

        // é is available via AltGr+e on us(intl) → Simple with LEVEL3_SHIFT.
        if let Some(action) = lookup.lookup('é') {
            let combo = expect_simple(action);
            assert_ne!(combo.modifiers & modifier_bit::LEVEL3_SHIFT, 0, "'é' needs AltGr");
        }
    }

    #[test]
    fn lookup_table_not_empty() {
        let keymap = make_keymap("us", "");
        let lookup = KeymapLookup::new(&keymap);
        // A US keymap should have at least all ASCII printable characters.
        assert!(lookup.len() >= 95, "expected at least 95 entries, got {}", lookup.len());
    }

    #[test]
    fn evdev_keycode_offset() {
        let combo = KeyCombination { keycode: Keycode::new(38), layout: 0, level: 0, modifiers: 0 };
        assert_eq!(KeymapLookup::evdev_keycode(&combo), 30, "XKB 38 - 8 = evdev 30");
    }

    #[test]
    fn from_string_roundtrip() {
        // Serialize a keymap to string, then parse it back via from_string.
        let keymap = make_keymap("us", "");
        let keymap_string = keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
        let lookup = KeymapLookup::from_string(&keymap_string).expect("from_string failed");
        assert!(lookup.len() >= 95, "expected at least 95 entries, got {}", lookup.len());
        let combo = expect_simple(lookup.lookup('a').expect("'a' not found"));
        assert_eq!(combo.modifiers, 0);
    }

    #[test]
    fn lookup_dead_key_german() {
        // German layout has dead_acute on the acute/grave key.
        // Standalone accents should be Compose { dead_key, base_key = Space }.
        let keymap = make_keymap("de", "");
        let lookup = KeymapLookup::new(&keymap);

        // ´ and ^ are dead keys resolved via compose (dead + Space).
        for &ch in &['´', '^'] {
            if let Some(action) = lookup.lookup(ch) {
                assert!(
                    matches!(action, KeyAction::Compose { .. }),
                    "'{ch}' should be a Compose action, got {action:?}"
                );
            }
        }

        // Regular characters should be Simple.
        let action = lookup.lookup('a').expect("'a' not found");
        assert!(matches!(action, KeyAction::Simple(_)), "'a' should be Simple");
    }

    #[test]
    fn lookup_composed_accents_german() {
        // German layout: dead_grave + a → à, dead_acute + e → é (via compose table).
        let keymap = make_keymap("de", "");
        let lookup = KeymapLookup::new(&keymap);

        // These composed characters should be available via Compose sequences.
        for &ch in &['à', 'è', 'é', 'â', 'ê'] {
            if let Some(action) = lookup.lookup(ch) {
                assert!(matches!(action, KeyAction::Compose { .. }), "'{ch}' should be Compose, got {action:?}");
            }
            // (Silently pass if compose table is not available in test env.)
        }
    }
}
