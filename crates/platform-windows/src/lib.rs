//! Windows specific platform integration for PlatynUI.
//!
//! This crate wires native Windows device abstractions (pointer, keyboard,
//! highlight, screenshot) plus desktop-info and window-manager helpers into a
//! per-runtime [`PlatformBundle`](platynui_core::platform::PlatformBundle).
//!
//! Rather than registering process-global device singletons, the crate exposes
//! [`create_windows_bundle`] behind a registered
//! [`PlatformFactory`](platynui_core::platform::PlatformFactory): each runtime
//! builds its own bundle and drops it on shutdown. The only deliberately
//! process-global state is the one-time DPI-awareness setting (see the `init`
//! module) and a handful of server-stable caches (the keyboard VK map, monitor
//! friendly names).

#[cfg(target_os = "windows")]
mod desktop;
#[cfg(target_os = "windows")]
mod factory;
#[cfg(target_os = "windows")]
mod highlight;
#[cfg(target_os = "windows")]
mod init;
#[cfg(target_os = "windows")]
mod keyboard;
#[cfg(target_os = "windows")]
mod pointer;
#[cfg(target_os = "windows")]
mod screenshot;
#[cfg(target_os = "windows")]
mod window_manager;

#[cfg(target_os = "windows")]
pub use factory::create_windows_bundle;

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowsPlatformStub;
