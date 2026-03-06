#[cfg(target_os = "linux")]
mod reverse_lookup;

#[cfg(target_os = "linux")]
pub use reverse_lookup::{KeyAction, KeyCombination, KeymapLookup, modifier_bit};
#[cfg(target_os = "linux")]
pub use xkbcommon::xkb;
