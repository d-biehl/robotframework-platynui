//! Inspector-internal global modifier reading for the live picker.
//!
//! The picker must know whether the activation modifiers are held while the
//! Inspector is **not** focused (the user is hovering another app), so egui's
//! own focus-bound input cannot supply this. Each platform reads the current
//! global modifier state directly — the PlatynUI compositor via
//! `get_modifiers` on its control socket, X11 via `XQueryPointer`'s modifier
//! mask, Windows via `GetAsyncKeyState`. This lives in the Inspector (not a
//! shared platform trait) because it is an interactive Inspector concern only.
//!
//! Platforms without a reader (macOS today, generic Wayland without a
//! PlatynUI control socket) return `None` from [`ModifierReader::new`], which
//! the picker treats as "unavailable" and greys itself out.

use crate::viewmodel::picker::Modifiers;

#[cfg(target_os = "linux")]
pub use linux::ModifierReader;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub use unsupported::ModifierReader;
#[cfg(target_os = "windows")]
pub use windows_impl::ModifierReader;

#[cfg(target_os = "linux")]
mod linux {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;
    use std::time::Duration;

    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, KeyButMask, Window};
    use x11rb::rust_connection::RustConnection;

    use super::Modifiers;

    /// Reads the global modifier state. Two backends, probed in this order:
    ///
    /// 1. **PlatynUI compositor** — the session's control socket answers
    ///    `get_modifiers` with the seat keyboard's state. This MUST be probed
    ///    before X11: a compositor session may still carry the host's X11
    ///    `DISPLAY`, and the X11 reader would then silently bind the *host*
    ///    seat and never see modifiers held on the compositor seat.
    /// 2. **X11** — `XQueryPointer`'s modifier mask on the root window.
    ///
    /// Neither present (generic Wayland) → `None`, picker unavailable.
    pub enum ModifierReader {
        Compositor(CompositorModifierReader),
        // Boxed: the X11 connection is large, the enum is held for the
        // Inspector's lifetime either way.
        X11(Box<X11ModifierReader>),
    }

    impl ModifierReader {
        pub fn new() -> Option<Self> {
            if let Some(reader) = CompositorModifierReader::probe() {
                tracing::info!("live picker modifier source: PlatynUI compositor control socket");
                return Some(Self::Compositor(reader));
            }
            if let Some(reader) = X11ModifierReader::new() {
                tracing::info!("live picker modifier source: X11");
                return Some(Self::X11(Box::new(reader)));
            }
            None
        }

        pub fn read(&self) -> Option<Modifiers> {
            match self {
                Self::Compositor(reader) => reader.read(),
                Self::X11(reader) => reader.read(),
            }
        }
    }

    /// Polls the seat keyboard's modifier state from the PlatynUI compositor
    /// (`get_modifiers` over the control socket). One-shot connection per
    /// poll, like the platform's other control-socket clients — the picker
    /// polls at UI tick rate, which a local Unix socket handles easily.
    pub struct CompositorModifierReader {
        socket_path: PathBuf,
    }

    impl CompositorModifierReader {
        /// The PlatynUI control socket (`$PLATYNUI_CONTROL_SOCKET`, else
        /// `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY.control`), if it answers `ping`.
        fn probe() -> Option<Self> {
            let socket_path = if let Ok(path) = std::env::var("PLATYNUI_CONTROL_SOCKET") {
                PathBuf::from(path)
            } else {
                let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
                let display = std::env::var("WAYLAND_DISPLAY").ok()?;
                PathBuf::from(runtime_dir).join(format!("{display}.control"))
            };
            let reader = Self { socket_path };
            let answers_ping = reader
                .request(r#"{"command":"ping"}"#)
                .and_then(|value| value.get("status").and_then(serde_json::Value::as_str).map(|s| s == "ok"))
                .unwrap_or(false);
            answers_ping.then_some(reader)
        }

        fn request(&self, command: &str) -> Option<serde_json::Value> {
            let mut stream = UnixStream::connect(&self.socket_path).ok()?;
            stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
            stream.set_write_timeout(Some(Duration::from_secs(2))).ok()?;
            writeln!(stream, "{command}").ok()?;
            stream.flush().ok()?;
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).ok()?;
            serde_json::from_str(line.trim()).ok()
        }

        pub fn read(&self) -> Option<Modifiers> {
            let value = self.request(r#"{"command":"get_modifiers"}"#)?;
            if value.get("status").and_then(serde_json::Value::as_str) != Some("ok") {
                return None;
            }
            let flag = |key: &str| value.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false);
            Some(Modifiers { ctrl: flag("ctrl"), alt: flag("alt"), shift: flag("shift") })
        }
    }

    /// Reads the global modifier state from the X server. Holds its own
    /// connection so it works regardless of egui focus.
    pub struct X11ModifierReader {
        conn: RustConnection,
        root: Window,
    }

    impl X11ModifierReader {
        /// Connects to the X server. Returns `None` when there is no X11
        /// display, so the picker stays disabled there.
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
