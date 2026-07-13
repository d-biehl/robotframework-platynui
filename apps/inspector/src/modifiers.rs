//! Inspector-internal global modifier reading for the live picker.
//!
//! The picker must know whether the activation modifiers are held while the
//! Inspector is **not** focused (the user is hovering another app), so egui's
//! own focus-bound input cannot supply this. Each platform reads the current
//! global modifier state directly — X11 via `XQueryPointer`'s modifier mask,
//! Windows via `GetAsyncKeyState`. This lives in the Inspector (not a shared
//! platform trait) because it is an interactive Inspector concern only.
//!
//! Platforms without a reader (macOS today, generic Wayland) return `None` from
//! [`ModifierReader::new`], which the picker treats as "unavailable" and greys
//! itself out.

use crate::viewmodel::picker::Modifiers;

#[cfg(target_os = "linux")]
pub use linux::ModifierReader;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub use unsupported::ModifierReader;
#[cfg(target_os = "windows")]
pub use windows_impl::ModifierReader;

#[cfg(target_os = "linux")]
mod linux {
    use super::Modifiers;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, KeyButMask, Window};
    use x11rb::rust_connection::RustConnection;

    /// Reads the global modifier state from the X server. Holds its own
    /// connection so it works regardless of egui focus.
    pub struct ModifierReader {
        conn: RustConnection,
        root: Window,
    }

    impl ModifierReader {
        /// Connects to the X server. Returns `None` when there is no X11
        /// display (e.g. a pure Wayland session with no XWayland), so the
        /// picker stays disabled there.
        pub fn new() -> Option<Self> {
            let (conn, screen_num) = x11rb::connect(None).ok()?;
            let root = conn.setup().roots.get(screen_num)?.root;
            Some(Self { conn, root })
        }

        pub fn read(&self) -> Option<Modifiers> {
            let reply = self.conn.query_pointer(self.root).ok()?.reply().ok()?;
            let mask = reply.mask;
            Some(Modifiers {
                ctrl: mask.contains(KeyButMask::CONTROL),
                alt: mask.contains(KeyButMask::MOD1),
                shift: mask.contains(KeyButMask::SHIFT),
            })
        }
    }
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::Modifiers;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_MENU, VK_SHIFT};

    pub struct ModifierReader;

    impl ModifierReader {
        pub fn new() -> Option<Self> {
            Some(Self)
        }

        pub fn read(&self) -> Option<Modifiers> {
            fn down(vk: VIRTUAL_KEY) -> bool {
                // The high-order bit of GetAsyncKeyState marks the key as down.
                (unsafe { GetAsyncKeyState(i32::from(vk.0)) } as u16 & 0x8000) != 0
            }
            Some(Modifiers { ctrl: down(VK_CONTROL), alt: down(VK_MENU), shift: down(VK_SHIFT) })
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod unsupported {
    use super::Modifiers;

    pub struct ModifierReader;

    impl ModifierReader {
        pub fn new() -> Option<Self> {
            None
        }

        pub fn read(&self) -> Option<Modifiers> {
            None
        }
    }
}
