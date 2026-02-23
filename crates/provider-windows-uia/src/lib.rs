//! Windows UIAutomation based UiTree provider.
//!
//! Design decisions and scope are documented in `docs/architecture.md` §10.2.
//! This module provides a compilable skeleton that wires the provider factory
//! and exposes lazy iterators for children/attributes. On non‑Windows targets,
//! the crate builds as a no‑op. On Windows, the implementation will be
//! incrementally expanded to call into UIAutomation via a dedicated STA actor.
#[cfg(windows)]
mod com;
#[cfg(windows)]
pub mod error;
#[cfg(windows)]
mod map;
#[cfg(windows)]
mod node;
#[cfg(windows)]
mod provider;

#[cfg(windows)]
pub use provider::*;
