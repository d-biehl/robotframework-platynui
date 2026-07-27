//! The agent backend: which JVMs it serves, and how it gets into them.
//!
//! # Discovery costs a stat, not a scan
//!
//! Finding agent-carrying JVMs is a directory read of the per-user handshake
//! directory — no process enumeration, no platform accessibility API, nothing
//! that has to exist on a given windowing system. That is why this backend is
//! platform-neutral in substance even while the crate around it is still
//! Windows-gated: `java-provider-linux` only has to remove the gate.
//!
//! # Automatic attachment, on by default
//!
//! Attaching to an application somebody else launched is the normal case, not an
//! exception: Java applications are started by scripts, installers and Web Start,
//! and the Inspector's core use is looking into something already running, where
//! `-javaagent` is impossible by definition. So a Java window whose JVM carries no
//! agent gets one injected. The deliberate consent is installing the agent
//! package, not flipping a flag — `providers.java.agent.auto_attach = false`
//! narrows the backend to `-javaagent`-launched targets for anyone who needs that.

use super::app::{AgentAppNode, ProcessFacts};
use super::element::Element;
use super::node::AgentNode;
use super::session::AgentSession;
use crate::backend::{Enumeration, ForeignWindows, JavaBackend};
use platynui_core::config::ConfigMap;
use platynui_core::platform::WindowManager;
use platynui_core::provider::ProviderError;
use platynui_core::types::Point;
use platynui_core::ui::UiNode;
use platynui_java_agent::{AgentError, ClientConfig, attach, handshake, resolve_agent_jar};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Backend id and config sub-map: `providers.java.agent.*`.
pub(crate) const BACKEND_ID: &str = "agent";

/// Default per-call deadline (`providers.java.agent.call_timeout_ms`).
const DEFAULT_CALL_TIMEOUT_MS: u64 = 5_000;
/// How long one automatic attach may take before it is abandoned.
const ATTACH_TIMEOUT: Duration = attach::DEFAULT_ATTACH_TIMEOUT;

/// How often one JVM may be attached to before the backend gives up on it.
///
/// Not once: an attach can fail transiently — the target's attach listener may not
/// exist yet the moment its first window becomes visible — and a single failure
/// would then cost that application the agent for the whole session. Not
/// unbounded either: a JVM that structurally refuses (no permission, a security
/// manager) must not be attacked once per enumeration pass for as long as it runs.
const MAX_ATTACH_ATTEMPTS: u32 = 3;

/// How long a freshly injected agent gets to publish its handshake file.
///
/// Waited for on purpose. The alternative is that the enumeration which triggered
/// the attach still answers from the weaker backend, and the caller has to
/// enumerate a second time to see the better one — in the Inspector, pressing
/// refresh twice. The agent publishes the file only after its toolkit adapter is
/// installed, so "file present" also means "the tree methods answer".
///
/// Paid once per process, and the measured latency is well under a second; the
/// budget is generous because the cost of overrunning it is one stale pass, while
/// the cost of giving up too early is that same stale pass every time.
const ATTACH_READY_TIMEOUT: Duration = Duration::from_secs(3);
const ATTACH_READY_POLL: Duration = Duration::from_millis(50);

/// One JVM's serving state.
struct Served {
    session: Arc<AgentSession>,
    facts: ProcessFacts,
}

pub(crate) struct AgentBackend {
    enabled: bool,
    auto_attach: bool,
    /// Explicit agent JAR (`providers.java.agent.jar`); otherwise discovered.
    jar: Option<PathBuf>,
    client_config: ClientConfig,
    /// Live sessions by pid. Kept across passes: injection happens once and the
    /// connection is what carries all subsequent traffic.
    sessions: Mutex<HashMap<u32, Arc<Served>>>,
    /// How often an attach has been attempted per pid, so a JVM that cannot take
    /// the agent is not attacked once per enumeration pass — while a JVM that
    /// merely was not ready yet still gets another chance.
    attach_attempts: Mutex<HashMap<u32, u32>>,
    window_manager: Mutex<Option<Arc<dyn WindowManager>>>,
    foreign: Option<Arc<ForeignWindows>>,
    /// Guards the one-off stale-file cleanup; see `prune_stale_handshakes_once`.
    pruned: AtomicBool,
    is_shutdown: AtomicBool,
}

impl AgentBackend {
    /// Build the backend from its sub-map of the Java provider's config, plus its
    /// view of what stronger backends serve.
    pub(crate) fn from_config(settings: Option<&ConfigMap>, foreign: Option<Arc<ForeignWindows>>) -> Self {
        let enabled = settings.and_then(|agent| agent.get_bool("enabled")).unwrap_or(true);
        // Default **on**: see the module docs on where the consent actually lives.
        let auto_attach = settings.and_then(|agent| agent.get_bool("auto_attach")).unwrap_or(true);
        let jar = settings.and_then(|agent| agent.get_str("jar")).map(PathBuf::from);
        let call_timeout_ms = settings
            .and_then(|agent| agent.get_i64("call_timeout_ms"))
            .and_then(|ms| u64::try_from(ms).ok())
            .filter(|ms| *ms > 0)
            .unwrap_or(DEFAULT_CALL_TIMEOUT_MS);
        Self {
            enabled,
            auto_attach,
            jar,
            client_config: ClientConfig {
                call_timeout: Duration::from_millis(call_timeout_ms),
                ..ClientConfig::default()
            },
            sessions: Mutex::new(HashMap::new()),
            attach_attempts: Mutex::new(HashMap::new()),
            window_manager: Mutex::new(None),
            foreign,
            pruned: AtomicBool::new(false),
            is_shutdown: AtomicBool::new(false),
        }
    }

    /// Deletes handshake files left behind by JVMs that are gone — once per
    /// backend instance.
    ///
    /// A killed JVM never runs its shutdown hook, so its file survives it.
    /// Correctness never depended on cleaning up (`handshake::for_pid` ignores a
    /// file whose process is dead, because the port may since belong to something
    /// else entirely), but *cost* now does: discovery reads this directory on every
    /// enumeration pass, so junk accumulated over a machine's lifetime would be
    /// re-read forever.
    ///
    /// Once, not per pass: this writes, and the files it would find on a second
    /// look are the ones a still-running JVM will clean up itself.
    fn prune_stale_handshakes_once(&self) {
        if self.pruned.swap(true, Ordering::AcqRel) {
            return;
        }
        let removed = handshake::remove_stale();
        if !removed.is_empty() {
            debug!(count = removed.len(), "removed handshake files of JVMs that are gone");
        }
    }

    fn window_manager(&self) -> Option<Arc<dyn WindowManager>> {
        self.window_manager.lock().expect("window manager mutex poisoned").clone()
    }

    /// The session for `pid`, connecting on first sight and reusing afterwards.
    fn session_for(&self, info: &handshake::HandshakeInfo) -> Option<Arc<Served>> {
        let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
        if let Some(existing) = sessions.get(&info.pid) {
            return Some(Arc::clone(existing));
        }
        match AgentSession::connect(info, self.client_config) {
            Ok(session) => {
                let facts = ProcessFacts::read(&session);
                info!(
                    pid = info.pid,
                    agent = %info.agent_version,
                    toolkits = ?info.toolkits,
                    "connected to a PlatynUI agent"
                );
                let served = Arc::new(Served { session, facts });
                sessions.insert(info.pid, Arc::clone(&served));
                Some(served)
            }
            Err(error) => {
                // A version mismatch is the one that needs saying out loud: an
                // agent cannot be unloaded, so the only remedy is restarting the
                // application, and silence here would look like "no Java support".
                if matches!(&error, AgentError::VersionMismatch { .. }) {
                    warn!(pid = info.pid, %error, "cannot serve this JVM");
                } else {
                    debug!(pid = info.pid, %error, "agent unreachable");
                }
                None
            }
        }
    }

    /// Drops sessions whose JVM is gone, so a long run does not accumulate dead
    /// connections and a recycled pid cannot inherit one.
    fn retire_dead_sessions(&self, live: &HashSet<u32>) {
        let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
        sessions.retain(|pid, served| {
            let keep = live.contains(pid);
            if !keep {
                debug!(pid, "the agent's handshake file is gone; dropping the session");
                served.session.close();
            }
            keep
        });
    }

    /// Injects the agent into JVMs that have none.
    ///
    /// The trigger is deliberately **"this JVM carries no agent"** and not "no
    /// backend can serve this window". A window the Access Bridge serves happily
    /// still has a higher-fidelity representation available — correct table cells,
    /// real identity, `Component.getName()` — and taking it is the entire point.
    /// Gating on unserved windows would have limited automatic attachment to JVMs
    /// that were already broken, which is the opposite of the intent.
    ///
    /// Only ever attempted once per process: a JVM that refuses (no permission, a
    /// security manager, an incompatible layout) would otherwise be attacked on
    /// every enumeration pass, turning one unusable target into a permanent cost
    /// for the whole session.
    fn attach_to_agentless(&self, java_processes: &[u32]) -> Vec<u32> {
        if !self.auto_attach || java_processes.is_empty() {
            return Vec::new();
        }
        let candidates: Vec<u32> = {
            let sessions = self.sessions.lock().expect("sessions mutex poisoned");
            let mut attempts = self.attach_attempts.lock().expect("attach log mutex poisoned");
            java_processes
                .iter()
                .copied()
                .filter(|pid| !sessions.contains_key(pid))
                // A handshake file means an agent is already in there, whether or
                // not this backend has connected to it yet.
                .filter(|pid| !handshake::agent_present(*pid))
                .filter(|pid| {
                    let attempt = attempts.entry(*pid).or_insert(0);
                    *attempt += 1;
                    *attempt <= MAX_ATTACH_ATTEMPTS
                })
                .collect()
        };
        if candidates.is_empty() {
            return Vec::new();
        }
        let jar = match resolve_agent_jar(self.jar.as_deref()) {
            Ok(jar) => jar,
            Err(error) => {
                debug!(%error, "no agent JAR available; automatic attachment is off for this session");
                return Vec::new();
            }
        };
        let mut injected = Vec::new();
        for pid in candidates {
            match attach::load_agent(pid, &jar, None, ATTACH_TIMEOUT) {
                Ok(()) => injected.push(pid),
                Err(error) => debug!(pid, %error, "automatic attachment failed"),
            }
        }
        Self::await_readiness(&mut injected);
        injected
    }

    /// Waits until the freshly injected agents are reachable, dropping the ones
    /// that never showed up.
    ///
    /// Reporting a pid the caller cannot serve yet would be worse than reporting
    /// none: the router re-enumerates on the strength of this answer, and an
    /// agent that is still starting would make that second sweep as stale as the
    /// first while costing the same.
    fn await_readiness(injected: &mut Vec<u32>) {
        if injected.is_empty() {
            return;
        }
        let deadline = Instant::now() + ATTACH_READY_TIMEOUT;
        let mut pending: Vec<u32> = injected.clone();
        loop {
            pending.retain(|pid| !handshake::agent_present(*pid));
            if pending.is_empty() || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(ATTACH_READY_POLL);
        }
        for pid in &pending {
            debug!(pid, "the injected agent did not publish a handshake in time; it serves from a later pass");
        }
        injected.retain(|pid| !pending.contains(pid));
        for pid in injected.iter() {
            info!(pid, "injected the PlatynUI agent into a running JVM; it serves this enumeration already");
        }
    }
}

/// The JVM's top-level windows, or an empty list when the agent will not answer.
///
/// Shared with the `app:Application` node, whose children are the same windows
/// seen from one level down.
pub(crate) fn read_windows(session: &AgentSession) -> Vec<Element> {
    match session.call("ui/windows", json!({})) {
        Ok(result) => serde_json::from_value(result["windows"].clone()).unwrap_or_else(|error| {
            debug!(pid = session.pid(), %error, "unreadable window list");
            Vec::new()
        }),
        Err(error) => {
            debug!(pid = session.pid(), %error, "window list unavailable");
            Vec::new()
        }
    }
}

impl JavaBackend for AgentBackend {
    fn id(&self) -> &'static str {
        BACKEND_ID
    }

    fn enumerate(&self, parent: &Arc<dyn UiNode>) -> Enumeration {
        self.prune_stale_handshakes_once();
        if self.is_shutdown.load(Ordering::Acquire) || !self.enabled {
            return Enumeration::default();
        }
        let agents = handshake::discover();
        let live: HashSet<u32> = agents.iter().map(|info| info.pid).collect();
        self.retire_dead_sessions(&live);

        let window_manager = self.window_manager();
        let mut pass = Enumeration::default();
        for info in &agents {
            let Some(served) = self.session_for(info) else {
                continue;
            };
            let windows = read_windows(&served.session);
            if windows.is_empty() {
                // An agent that is in the JVM but has no window to show is not a
                // failure: a `-javaagent` target reaches this state before its UI
                // exists, and it will have one on a later pass.
                continue;
            }
            let mut served_any = false;
            for element in windows {
                let handle = element.window.as_ref().and_then(|window| window.handle).filter(|handle| *handle != 0);
                // A window a stronger backend serves is not this backend's, even
                // though it could serve it — the router's order decides, not
                // capability.
                if let Some(raw) = handle
                    && self.foreign.as_ref().is_some_and(|foreign| foreign.is_foreign(raw))
                {
                    continue;
                }
                if let Some(raw) = handle {
                    pass.served_windows.push(raw);
                }
                served_any = true;
                pass.nodes.push(AgentNode::new(
                    Arc::clone(&served.session),
                    element,
                    None,
                    window_manager.clone(),
                    Some(parent),
                ) as Arc<dyn UiNode>);
            }
            if served_any {
                pass.nodes.push(AgentAppNode::new(
                    Arc::clone(&served.session),
                    served.facts.clone(),
                    window_manager.clone(),
                    Some(parent),
                ) as Arc<dyn UiNode>);
            }
        }
        pass
    }

    fn element_at_point(&self, point: Point) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
        if self.is_shutdown.load(Ordering::Acquire) || !self.enabled {
            return Err(unsupported("backend disabled or shut down"));
        }
        let sessions: Vec<Arc<Served>> =
            self.sessions.lock().expect("sessions mutex poisoned").values().map(Arc::clone).collect();
        if sessions.is_empty() {
            return Err(unsupported("no agent-served JVM"));
        }
        let window_manager = self.window_manager();
        for served in sessions {
            if served.session.is_degraded() {
                continue;
            }
            let answer = match served.session.call("ui/at_point", json!({ "x": point.x(), "y": point.y() })) {
                Ok(answer) => answer,
                Err(error) => {
                    debug!(pid = served.session.pid(), %error, "hit-test unavailable for this JVM");
                    continue;
                }
            };
            let chain: Vec<Element> = serde_json::from_value(answer["chain"].clone()).map_err(|error| {
                ProviderError::CommunicationFailure {
                    channel: "java-agent",
                    details: Some(format!("unreadable hit-test chain: {error}")),
                }
            })?;
            if chain.is_empty() {
                // No window of this JVM covers the point; another one may.
                continue;
            }
            return Ok(Some(build_chain(&served, chain, window_manager.as_ref())));
        }
        // Not this backend's point: the abstention is what lets the router try
        // the Access Bridge and then the platform's native provider.
        Err(unsupported("no agent-served window at point"))
    }

    fn set_window_manager(&self, window_manager: Arc<dyn WindowManager>) {
        let mut slot = self.window_manager.lock().expect("window manager mutex poisoned");
        if slot.is_none() {
            *slot = Some(window_manager);
        }
    }

    fn shutdown(&self) {
        if self.is_shutdown.swap(true, Ordering::AcqRel) {
            return;
        }
        info!("Java agent backend shutting down");
        let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
        for served in sessions.values() {
            served.session.close();
        }
        sessions.clear();
    }
}

impl AgentBackend {
    /// Offers the agent to the JVMs behind the Java windows this pass saw.
    ///
    /// Called by the router rather than from `enumerate`, because this backend
    /// does not enumerate windows at all — it finds *agents*. Which processes have
    /// a Java window is what another backend discovered, and on a platform with no
    /// window-enumerating backend the list is simply empty, which is the honest
    /// answer there.
    /// Returns the pids that gained a **reachable** agent, so the router knows its
    /// sweep is stale for them.
    pub(crate) fn consider_attaching(&self, java_processes: &[u32]) -> Vec<u32> {
        if self.is_shutdown.load(Ordering::Acquire) || !self.enabled {
            return Vec::new();
        }
        self.attach_to_agentless(java_processes)
    }
}

/// Builds the hit-test chain into linked nodes, outermost first, and returns the
/// deepest one — with the chain above it kept alive by the node itself, since a
/// hit-test result has no other owner.
fn build_chain(
    served: &Served,
    chain: Vec<Element>,
    window_manager: Option<&Arc<dyn WindowManager>>,
) -> Arc<dyn UiNode> {
    let owned = window_manager.cloned();
    let app: Arc<dyn UiNode> =
        AgentAppNode::new(Arc::clone(&served.session), served.facts.clone(), owned.clone(), None);
    let mut parent: Arc<dyn UiNode> = app;
    let mut parent_role: Option<String> = None;
    for element in chain {
        let role = element.role.clone();
        let node =
            AgentNode::new(Arc::clone(&served.session), element, parent_role.clone(), owned.clone(), Some(&parent));
        node.hold_parent(Arc::clone(&parent));
        parent_role = Some(role);
        parent = node;
    }
    parent
}

fn unsupported(details: &str) -> ProviderError {
    ProviderError::UnsupportedOperation { operation: "element_at_point", details: Some(details.into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_enabled_with_automatic_attachment_on() {
        let backend = AgentBackend::from_config(None, None);
        assert!(backend.enabled);
        assert!(backend.auto_attach, "attaching to a running application is the normal path");
        assert_eq!(backend.client_config.call_timeout, Duration::from_millis(DEFAULT_CALL_TIMEOUT_MS));
        assert!(backend.jar.is_none());
    }

    #[test]
    fn the_backend_reads_its_own_settings() {
        let settings = ConfigMap::new()
            .with("enabled", false)
            .with("auto_attach", false)
            .with("jar", "C:\\agents\\platynui-agent.jar")
            .with("call_timeout_ms", 750_i64);
        let backend = AgentBackend::from_config(Some(&settings), None);
        assert!(!backend.enabled);
        assert!(!backend.auto_attach);
        assert_eq!(backend.jar.as_deref(), Some(std::path::Path::new("C:\\agents\\platynui-agent.jar")));
        assert_eq!(backend.client_config.call_timeout, Duration::from_millis(750));
    }

    #[test]
    fn an_invalid_timeout_falls_back_to_the_default() {
        let settings = ConfigMap::new().with("call_timeout_ms", -1_i64);
        let backend = AgentBackend::from_config(Some(&settings), None);
        assert_eq!(backend.client_config.call_timeout, Duration::from_millis(DEFAULT_CALL_TIMEOUT_MS));
    }

    /// A disabled backend must cost nothing: no directory scan, no connection,
    /// and an abstention on hit-testing so the other backends still get their turn.
    #[test]
    fn a_disabled_backend_is_inert() {
        use platynui_core::ui::{Namespace, PatternName, RuntimeId, UiAttribute};
        use std::sync::Weak;

        struct DesktopStub(RuntimeId);
        #[allow(clippy::unnecessary_literal_bound)] // signatures fixed by the UiNode trait
        impl UiNode for DesktopStub {
            fn namespace(&self) -> Namespace {
                Namespace::Control
            }
            fn role(&self) -> &str {
                "Desktop"
            }
            fn name(&self) -> String {
                "Desktop".into()
            }
            fn runtime_id(&self) -> &RuntimeId {
                &self.0
            }
            fn parent(&self) -> Option<Weak<dyn UiNode>> {
                None
            }
            fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
                Box::new(std::iter::empty())
            }
            fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
                Box::new(std::iter::empty())
            }
            fn supported_patterns(&self) -> Vec<PatternName> {
                Vec::new()
            }
            fn invalidate(&self) {}
        }

        let settings = ConfigMap::new().with("enabled", false);
        let backend = AgentBackend::from_config(Some(&settings), None);
        let desktop: Arc<dyn UiNode> = Arc::new(DesktopStub(RuntimeId::from("desktop")));
        let pass = backend.enumerate(&desktop);
        assert!(pass.nodes.is_empty());
        assert!(pass.served_windows.is_empty());
        assert!(matches!(
            backend.element_at_point(Point::new(1.0, 1.0)),
            Err(ProviderError::UnsupportedOperation { .. })
        ));
        // And it must not attach either: the kill switch means "not this channel",
        // not "this channel without the tree".
        assert!(backend.consider_attaching(&[4242]).is_empty());
        assert!(backend.attach_attempts.lock().expect("log").is_empty());
    }
}
