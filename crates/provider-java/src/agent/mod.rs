//! The in-JVM agent backend: where the agent's answers become PlatynUI nodes.
//!
//! `crates/java-agent` ends where this begins — the agent is in the JVM,
//! reachable and bounded, and it knows nothing about UI nodes. What this module
//! adds is the half PlatynUI is actually about: the mapping onto
//! `UiNode`/role/attribute/pattern/`RuntimeId`, so a Java application is queried
//! with the same XPath and appears in the same Inspector as any other provider.
//!
//! The split inside is deliberate and follows what can be tested how:
//!
//! - [`element`] — the wire payload and the vocabulary mapping. Pure data and
//!   pure functions, so CI covers the mapping against recorded payloads without
//!   a JVM anywhere near it.
//! - [`session`] — one connection to one agent, with the degraded state that
//!   turns a wedged JVM into bounded errors.
//! - [`node`] — the `UiNode` implementation, including the validity answer that
//!   scoped-root reuse depends on.
//! - [`backend`] — discovery of agent-carrying JVMs, automatic attachment, and
//!   the [`crate::backend::JavaBackend`] implementation the router talks to.

pub(crate) mod app;
pub(crate) mod backend;
pub(crate) mod element;
pub(crate) mod node;
pub(crate) mod session;
pub(crate) mod window_handle;

pub(crate) use backend::{AgentBackend, BACKEND_ID};
