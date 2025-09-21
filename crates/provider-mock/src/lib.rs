//! Deterministic mock UiTree provider for testing the runtime.
//!
//! Eventually this crate will expose scriptable trees and predictable RuntimeId
//! assignments to support XPath unit and integration tests.

/// Stub marker type for den Mock-Provider während der Scaffold-Phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MockProviderStub;
