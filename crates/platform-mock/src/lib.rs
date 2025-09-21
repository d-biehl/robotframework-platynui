//! In-memory mock platform implementation for PlatynUI tests.
//!
//! The real implementation will expose deterministic devices and window
//! management primitives so integration tests can run without native APIs.

/// Marker struct ensuring the mock platform crate compiles while features are developed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MockPlatformStub;
