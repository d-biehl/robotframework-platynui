//! Windows specific platform integration for PlatynUI.
//!
//! This crate wires native Windows device abstractions (pointer, keyboard,
//! highlight, screenshot) and UIAutomation helpers into the runtime via the
//! shared registration macros provided by `platynui-core`.

#[cfg(target_os = "windows")]
mod desktop;
#[cfg(target_os = "windows")]
mod init;
#[cfg(target_os = "windows")]
mod pointer;

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowsPlatformStub;
