//! Failures of the Java-agent transport.
//!
//! Internal to this crate by design (see `dev-docs/error-handling.md`): the
//! consuming provider maps these onto `ProviderError` at its own boundary. The
//! variants exist to keep failures **distinguishable where the remedies
//! differ** — "the attach never reached the target" and "the target refused the
//! agent" need different things from the operator, and collapsing them into one
//! string would make the diagnostic useless.

use std::path::PathBuf;

/// A failure while injecting into, discovering, or talking to a JVM agent.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// The process exists but is not a JVM — attaching would be meaningless.
    #[error("process {pid} does not run a JVM")]
    NotAJvm {
        /// The target process id.
        pid: u32,
    },

    /// The process is gone, or its state cannot be determined.
    #[error("process {pid} is not available: {details}")]
    ProcessUnavailable {
        /// The target process id.
        pid: u32,
        /// What the probe reported.
        details: String,
    },

    /// The attach handshake with the target JVM failed — the agent never ran.
    ///
    /// Causes range from "attach is disabled in that JVM"
    /// (`-XX:+DisableAttachMechanism`) through permission denials to the JVM
    /// not answering its attach protocol at all.
    #[error("attach to process {pid} failed: {details}")]
    AttachFailed {
        /// The target process id.
        pid: u32,
        /// What the attach protocol reported.
        details: String,
    },

    /// The attach reached the target, but the JVM refused to initialise the
    /// agent — a sandboxed-JNLP `SecurityManager`, or dynamic agent loading
    /// disallowed (JEP 451). Distinct from [`Self::AttachFailed`] because the
    /// remedy is inside the target, not on the PlatynUI side.
    #[error("process {pid} refused to load the agent: {details}")]
    AgentRefused {
        /// The target process id.
        pid: u32,
        /// What the target reported.
        details: String,
    },

    /// No agent is reachable in that process.
    #[error("no PlatynUI agent in process {pid}")]
    NoAgent {
        /// The target process id.
        pid: u32,
    },

    /// The agent JAR could not be resolved or does not exist.
    #[error("the PlatynUI agent JAR is unavailable: {details}")]
    JarUnavailable {
        /// What was tried, and the remedy.
        details: String,
        /// The path that was checked, when one was known.
        path: Option<PathBuf>,
    },

    /// The agent in that JVM was built from a different PlatynUI version.
    ///
    /// Fatal by design: an agent cannot be unloaded, so the only remedy is
    /// restarting the application. Interoperating across versions would only
    /// surface the mismatch later, in the middle of a test.
    #[error(
        "agent version {agent} in process {pid} cannot serve client version {client} — \
         restart the application to load the matching agent"
    )]
    VersionMismatch {
        /// The target process id.
        pid: u32,
        /// The version the agent reports.
        agent: String,
        /// The version this client was built as.
        client: String,
    },

    /// The agent rejected the connection token.
    #[error("the agent in process {pid} rejected the connection token")]
    Unauthenticated {
        /// The target process id.
        pid: u32,
    },

    /// A call did not complete within its deadline.
    #[error("agent call '{method}' did not answer within {timeout_ms} ms")]
    Timeout {
        /// The RPC method that was waiting.
        method: String,
        /// The deadline that elapsed.
        timeout_ms: u64,
    },

    /// The connection broke, or could not be established.
    #[error("agent transport failure: {details}")]
    Transport {
        /// What the socket or file layer reported.
        details: String,
    },

    /// The agent answered something this client cannot make sense of.
    #[error("agent protocol error: {details}")]
    Protocol {
        /// What was expected and what arrived.
        details: String,
    },

    /// The agent reported a JSON-RPC error for a call.
    #[error("agent call '{method}' failed ({code}): {message}")]
    Call {
        /// The RPC method that failed.
        method: String,
        /// The JSON-RPC error code.
        code: i64,
        /// The agent's message.
        message: String,
    },
}
