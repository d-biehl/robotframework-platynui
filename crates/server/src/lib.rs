//! JSON-RPC facing server façade for PlatynUI.
//!
//! The final implementation will expose the runtime functionality through a
//! transport inspired by Language Server Protocol / MCP. For now we keep a
//! stub so downstream crates can already depend on this package.

/// Marker type that keeps the server crate non-empty during bootstrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServerStub;
