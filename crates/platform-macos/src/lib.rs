//! macOS specific platform integration for PlatynUI.
//!
//! This crate will wrap AppKit/CoreGraphics primitives for window management,
//! devices and highlight handling on Apple platforms.

/// Temporary marker type for the macOS platform crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacOsPlatformStub;
