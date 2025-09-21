//! X11 based platform integration for PlatynUI on Unix systems.
//!
//! Once implemented this crate will provide device shims built on XTest as well
//! as window management helpers for traditional Linux desktop environments.

/// Stub marker used while the Linux/X11 platform implementation is pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinuxX11PlatformStub;
