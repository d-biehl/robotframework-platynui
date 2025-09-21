//! Core orchestration layer for PlatynUI.
//!
//! This crate will host the runtime responsible for wiring UiTree providers,
//! platform abstractions (devices, window management) and the XPath evaluation
//! pipeline described in `docs/architekturkonzept_runtime.md`.

// Placeholder module to avoid an empty crate while the real implementation is
// still under construction.
pub mod placeholder {
    //! Temporary marker types until the runtime logic lands.

    /// Marker type that indicates the runtime crate was linked successfully.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct RuntimeStub;
}
