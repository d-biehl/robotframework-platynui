//! One connection to one agent, and the degraded state around it.
//!
//! # Why a session and not just the client
//!
//! [`AgentClient`] needs `&mut self` per call — calls on one connection are
//! strictly sequential, and the client enforces that by construction. Nodes,
//! though, are shared `Arc`s read from whatever thread the runtime happens to
//! be on. The session is where those two facts are reconciled: one mutex around
//! the connection, so concurrency serialises at the socket while the agent
//! itself serialises on the toolkit thread.
//!
//! # Degraded is a state, not an error
//!
//! A JVM whose toolkit thread is wedged answers nothing, and every call against
//! it costs a full deadline. Paying that per node, per attribute, on every
//! enumeration pass is how one sick application makes a whole test run look
//! hung. So the first bounded failures mark the session degraded, and while it
//! is degraded calls fail immediately instead of waiting — until a probe,
//! rate-limited, finds the agent answering again.
//!
//! What a degraded session must never do is claim its nodes are fine:
//! `UiNode::is_valid` answers **false**, so a consumer holding a scoped root
//! re-resolves rather than pinning an element in a process that may be gone.

use platynui_java_agent::{AgentClient, AgentError, ClientConfig, HandshakeInfo, handshake};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Consecutive bounded failures before a session is considered degraded. One
/// timeout is a busy toolkit thread; three in a row is a JVM that is not coming
/// back on its own.
const DEGRADED_THRESHOLD: u64 = 3;

/// Shortest gap between two probes of a degraded session.
const PROBE_INTERVAL: Duration = Duration::from_secs(2);

/// A live agent connection.
pub(crate) struct AgentSession {
    pid: u32,
    /// The agent's own version, for diagnostics.
    version: String,
    toolkits: Vec<String>,
    /// `None` means the connection has to be rebuilt before the next call; see
    /// [`AgentSession::leaves_the_stream_unusable`].
    client: Mutex<Option<AgentClient>>,
    config: ClientConfig,
    failures: AtomicU64,
    degraded: AtomicBool,
    /// Monotonic millis since session start, so a probe can be rate-limited
    /// without a second clock.
    started: Instant,
    last_probe_ms: AtomicU64,
    closed: AtomicBool,
}

impl AgentSession {
    /// Connects to the agent described by `info`.
    ///
    /// # Errors
    ///
    /// Whatever [`AgentClient::connect`] reports — notably a version mismatch,
    /// which is fatal for that JVM by design: an agent cannot be unloaded, so
    /// the only remedy is restarting the application.
    pub fn connect(info: &HandshakeInfo, config: ClientConfig) -> Result<Arc<Self>, AgentError> {
        let client = AgentClient::connect(info, config)?;
        let version = client.info().agent_version.clone();
        let toolkits = client.info().toolkits.clone();
        Ok(Arc::new(Self {
            pid: info.pid,
            version,
            toolkits,
            client: Mutex::new(Some(client)),
            config,
            failures: AtomicU64::new(0),
            degraded: AtomicBool::new(false),
            started: Instant::now(),
            last_probe_ms: AtomicU64::new(0),
            closed: AtomicBool::new(false),
        }))
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn toolkits(&self) -> &[String] {
        &self.toolkits
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded.load(Ordering::Acquire)
    }

    /// Marks the session unusable. Idempotent; a closed session never recovers,
    /// because the reason to close it is that the provider is going away.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        if let Ok(mut slot) = self.client.lock() {
            *slot = None;
        }
    }

    /// Calls `method` on the agent.
    ///
    /// # Errors
    ///
    /// [`AgentError::Timeout`] and friends from the client, and
    /// [`AgentError::Transport`] with a "degraded" note when the session is
    /// short-circuiting instead of paying another deadline.
    pub fn call(&self, method: &str, params: Value) -> Result<Value, AgentError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(AgentError::Transport { details: format!("the session for process {} is closed", self.pid) });
        }
        if self.is_degraded() && !self.probe_is_due() {
            return Err(AgentError::Transport {
                details: format!("the agent in process {} is degraded; not waiting for '{method}'", self.pid),
            });
        }
        let mut slot = self.client.lock().unwrap_or_else(|poisoned| {
            // A panic while holding the connection leaves it in an unknown state.
            // Dropping it is the same remedy as for a desynchronised stream.
            self.client.clear_poison();
            let mut guard = poisoned.into_inner();
            *guard = None;
            guard
        });
        if slot.is_none() {
            match self.reconnect() {
                Ok(client) => *slot = Some(client),
                Err(error) => {
                    self.note_failure(method, &error);
                    return Err(error);
                }
            }
        }
        let client = slot.as_mut().expect("a connection was just established");
        match client.call(method, params) {
            Ok(result) => {
                self.note_success();
                Ok(result)
            }
            Err(error) => {
                if Self::leaves_the_stream_unusable(&error) {
                    // Not paranoia — necessity. A call that hit its deadline was
                    // *abandoned*, not cancelled: the agent may still write that
                    // answer later, and the next call would read it as its own.
                    // The client detects the crossed ids and reports a protocol
                    // error, but it cannot un-cross them, so a timed-out
                    // connection never works again. Dropping it here is what makes
                    // recovery possible at all: the next call builds a fresh one.
                    debug!(pid = self.pid, method, "dropping the connection; it can no longer be trusted");
                    *slot = None;
                }
                self.note_failure(method, &error);
                Err(error)
            }
        }
    }

    /// Whether an error leaves the connection unusable for further calls.
    ///
    /// A JSON-RPC error from the agent does not — the exchange completed, the
    /// answer was simply "no". Everything else means the request and its response
    /// are no longer in step.
    fn leaves_the_stream_unusable(error: &AgentError) -> bool {
        matches!(error, AgentError::Timeout { .. } | AgentError::Protocol { .. } | AgentError::Transport { .. })
    }

    /// Rebuilds the connection from the agent's current handshake file.
    ///
    /// Read fresh rather than remembered: the file is the authority on the port,
    /// and if the JVM is gone it is gone from there too, which turns "reconnect"
    /// into an honest [`AgentError::NoAgent`] instead of a retry against a port
    /// that may since belong to something else.
    fn reconnect(&self) -> Result<AgentClient, AgentError> {
        let info = handshake::for_pid(self.pid)?.ok_or(AgentError::NoAgent { pid: self.pid })?;
        let client = AgentClient::connect(&info, self.config)?;
        debug!(pid = self.pid, "reconnected to the agent");
        Ok(client)
    }

    /// Whether the element behind `id` is still live in the target JVM.
    ///
    /// The load-bearing answer behind `UiNode::is_valid`. An unreachable or
    /// degraded agent answers **false**: a JVM that died took its nodes with
    /// it, and for one that is merely wedged, forcing a re-resolve is the
    /// recoverable direction — an optimistic `true` would pin a dead element
    /// for as long as the consumer holds it.
    pub fn is_element_live(&self, id: u64) -> bool {
        match self.call("element/live", json!({ "id": id })) {
            Ok(result) => result.get("live").and_then(Value::as_bool).unwrap_or(false),
            Err(error) => {
                debug!(pid = self.pid, element = id, %error, "liveness unanswerable; reporting invalid");
                false
            }
        }
    }

    fn note_success(&self) {
        self.failures.store(0, Ordering::Release);
        if self.degraded.swap(false, Ordering::AcqRel) {
            debug!(pid = self.pid, "the agent is answering again");
        }
    }

    fn note_failure(&self, method: &str, error: &AgentError) {
        let failures = self.failures.fetch_add(1, Ordering::AcqRel) + 1;
        if failures >= DEGRADED_THRESHOLD && !self.degraded.swap(true, Ordering::AcqRel) {
            warn!(
                pid = self.pid,
                method,
                %error,
                "the agent stopped answering; treating this JVM as degraded so its calls fail fast"
            );
        } else {
            debug!(pid = self.pid, method, %error, failures, "agent call failed");
        }
    }

    /// Whether a degraded session may pay for one more call, to find out if the
    /// agent came back. Rate-limited so recovery costs one deadline every
    /// [`PROBE_INTERVAL`] rather than one per node.
    fn probe_is_due(&self) -> bool {
        let now = self.elapsed_ms();
        let last = self.last_probe_ms.load(Ordering::Acquire);
        let interval = u64::try_from(PROBE_INTERVAL.as_millis()).unwrap_or(u64::MAX);
        if now.saturating_sub(last) < interval {
            return false;
        }
        // Only one thread gets the probe slot; the rest keep failing fast.
        self.last_probe_ms.compare_exchange(last, now, Ordering::AcqRel, Ordering::Acquire).is_ok()
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

// The degradation policy is what keeps one sick JVM from stalling a whole run,
// so both halves of it are pinned at compile time rather than left to a comment:
// a single busy-toolkit timeout must not condemn a JVM, and recovery must cost
// one deadline per interval rather than one per node.
const _: () = assert!(DEGRADED_THRESHOLD > 1);
const _: () = assert!(PROBE_INTERVAL.as_millis() >= 1_000);
