//! UIAutomation based UiTree provider for Windows.
//!
//! The provider will translate Microsoft UIA structures into the PlatynUI
//! normalized tree and expose attributes/patterns as documented in the
//! architecture concept.

/// Stub type kept until the Windows UIA bridge is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowsUiaProviderStub;
