//! Getting the PlatynUI agent into a JVM, finding it, and talking to it.
//!
//! Everything here is **toolkit-neutral and provider-neutral**: this crate ends
//! where a toolkit adapter begins — the agent is in the JVM and answers. It
//! depends on no other PlatynUI crate, so it can be built and verified on its
//! own.
//!
//! # The three pieces
//!
//! - [`attach`] — the JVM attach protocol, spoken natively. **The primary
//!   injection path**: Java applications are launched by scripts, installers or
//!   Web Start, so the launch line is typically not PlatynUI's to change, and
//!   the Inspector's core use is looking into an application that is *already
//!   running*, where `-javaagent` is impossible by definition. Implemented
//!   directly rather than through a JDK tool or a bundled `jattach`, so the
//!   test host needs no JDK and ships no unsigned foreign binary.
//! - [`handshake`] — discovery, authentication and rendezvous through the
//!   per-user handshake file the agent publishes.
//! - [`client`] — the newline-delimited JSON-RPC connection, with a deadline on
//!   every read.
//!
//! # Control plane and data plane are not the same channel
//!
//! Injection happens **once**; all running traffic goes over the agent's own
//! loopback connection. The JVM attach mechanism is a command channel only, and
//! on Windows every attach means `OpenProcess` + `CreateRemoteThread` into the
//! target — EDR-visible, and not something to repeat per read. (JAB fuses the
//! two into Win32 messaging, which is why it is zero-config but Windows-only
//! and blocks its callers on a hung JVM. We keep the zero-config rendezvous and
//! drop the coupling.)

// Backtick-pedantry on prose that is full of product and API names adds noise
// without catching bugs (same call as `provider-jab`):
#![allow(clippy::doc_markdown)]

pub mod attach;
pub mod client;
pub mod discovery;
pub mod error;
pub mod handshake;
pub mod jvm;
pub mod paths;

pub use client::{AgentClient, AgentInfo, ClientConfig, Notification};
pub use discovery::{AgentPackage, resolve_agent_jar};
pub use error::AgentError;
pub use handshake::HandshakeInfo;

/// The agent's JSON-RPC code for "handshake required, or wrong token".
pub(crate) const UNAUTHENTICATED: i64 = -32001;
/// The agent's JSON-RPC code for a provider↔agent version mismatch.
pub(crate) const VERSION_MISMATCH: i64 = -32002;
/// The agent's JSON-RPC code for a call abandoned at its agent-side deadline.
pub(crate) const DEADLINE_EXCEEDED: i64 = -32003;

/// Connects to the agent in `pid`, if one is there.
///
/// The whole normal path in one call: read the handshake file, ignore it if it
/// is stale, connect, authenticate, pin the version.
///
/// # Errors
///
/// [`AgentError::NoAgent`] when that JVM carries no agent — a question worth
/// asking before deciding to attach — and the connection errors of
/// [`AgentClient::connect`] otherwise.
pub fn connect(pid: u32, config: ClientConfig) -> Result<AgentClient, AgentError> {
    let info = handshake::for_pid(pid)?.ok_or(AgentError::NoAgent { pid })?;
    AgentClient::connect(&info, config)
}
