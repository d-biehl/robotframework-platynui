//! Out-of-process UiTree provider based on JSON-RPC 2.0.
//!
//! This crate will speak the provider handshake inspired by LSP/MCP and forward
//! tree queries to external processes communicating over pipes or local sockets.

/// Stub type until the JSON-RPC transport and protocol glue is implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JsonRpcProviderStub;
