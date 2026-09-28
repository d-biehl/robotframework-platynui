//! The `app:Application` node grouping one JVM's windows.
//!
//! Not decoration: the Swing acceptance suites set their scoped root to
//! `/app:Application[@ProcessId=<pid>]`, and every provider presents its
//! processes this way, so a backend that skipped it would make the same
//! application addressable differently depending on which channel served it.
//!
//! Its metadata comes from the JVM itself (`agent/process`), not from a host-side
//! process query. That is both cheaper and better: the host would need a
//! per-platform process API to learn the same things and would *still* not know
//! the main class — which is the only one of these facts a user recognises their
//! application by, since a JVM's executable is always `java`.

use super::node::{AgentNode, TECHNOLOGY};
use super::session::AgentSession;
use platynui_core::platform::WindowManager;
use platynui_core::ui::attribute_names::{application, common};
use platynui_core::ui::{Namespace, PatternName, RuntimeId, UiAttribute, UiNode, UiValue};
use serde::Deserialize;
use serde_json::json;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tracing::debug;

/// What the agent reports about its own process.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ProcessFacts {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "executablePath", default)]
    pub executable_path: Option<String>,
    #[serde(rename = "commandLine", default)]
    pub command_line: Option<String>,
    #[serde(rename = "userName", default)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub architecture: Option<String>,
    #[serde(rename = "vmName", default)]
    pub vm_name: Option<String>,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<String>,
    #[serde(rename = "startTimeMillis", default)]
    pub start_time_millis: Option<i64>,
}

impl ProcessFacts {
    /// Reads the facts once per session; they do not change while a process runs.
    pub(crate) fn read(session: &AgentSession) -> Self {
        match session.call("agent/process", json!({})) {
            Ok(result) => serde_json::from_value(result).unwrap_or_else(|error| {
                debug!(pid = session.pid(), %error, "unreadable process facts");
                Self::default()
            }),
            Err(error) => {
                debug!(pid = session.pid(), %error, "process facts unavailable");
                Self::default()
            }
        }
    }
}

pub(crate) struct AgentAppNode {
    session: Arc<AgentSession>,
    /// The JVM's process, recorded at creation so that a later process with the
    /// same pid does not count; `None` when no process had the pid by then.
    process: Option<platynui_process::ProcessIdentity>,
    facts: ProcessFacts,
    window_manager: Option<Arc<dyn WindowManager>>,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    self_weak: OnceLock<Weak<dyn UiNode>>,
    runtime_id: OnceLock<RuntimeId>,
}

impl AgentAppNode {
    pub(crate) fn new(
        session: Arc<AgentSession>,
        facts: ProcessFacts,
        window_manager: Option<Arc<dyn WindowManager>>,
        parent: Option<&Arc<dyn UiNode>>,
    ) -> Arc<Self> {
        let process = platynui_process::ProcessIdentity::capture(session.pid());
        let node = Arc::new(Self {
            session,
            process,
            facts,
            window_manager,
            parent: Mutex::new(parent.map(Arc::downgrade)),
            self_weak: OnceLock::new(),
            runtime_id: OnceLock::new(),
        });
        let erased: Arc<dyn UiNode> = node.clone();
        let _ = node.self_weak.set(Arc::downgrade(&erased));
        node
    }
}

impl UiNode for AgentAppNode {
    fn namespace(&self) -> Namespace {
        Namespace::App
    }

    #[allow(clippy::unnecessary_literal_bound)] // signature fixed by the UiNode trait
    fn role(&self) -> &str {
        "Application"
    }

    fn name(&self) -> String {
        self.facts.name.clone().unwrap_or_default()
    }

    fn runtime_id(&self) -> &RuntimeId {
        self.runtime_id.get_or_init(|| RuntimeId::from(format!("agent/app/{}", self.session.pid())))
    }

    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent
            .lock()
            .unwrap_or_else(|poisoned| {
                self.parent.clear_poison();
                poisoned.into_inner()
            })
            .clone()
    }

    fn has_children(&self) -> bool {
        // An app node exists only because at least one window was seen for this
        // JVM, so claiming children is cheaper than proving them.
        true
    }

    /// Valid while the JVM's process runs and its agent session is usable: a
    /// closed or degraded session serves nothing, so its node must not claim to
    /// be fine (`java-provider`, *Node validity is answered, not assumed*).
    /// Every part is local state; nothing calls into the JVM.
    fn is_valid(&self) -> bool {
        !self.session.is_closed()
            && !self.session.is_degraded()
            && self.process.as_ref().is_some_and(|process| !process.check().has_ended())
    }

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        let parent = self.self_weak.get().and_then(Weak::upgrade);
        let windows = super::backend::read_windows(&self.session);
        let session = Arc::clone(&self.session);
        let window_manager = self.window_manager.clone();
        Box::new(windows.into_iter().map(move |window| {
            let node = AgentNode::new(Arc::clone(&session), window, None, window_manager.clone(), parent.as_ref());
            // A window keeps its application node alive.
            if let Some(parent) = &parent {
                node.hold_parent(Arc::clone(parent));
            }
            node as Arc<dyn UiNode>
        }))
    }

    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        let mut attrs: Vec<Arc<dyn UiAttribute>> = vec![
            literal(Namespace::Control, common::ROLE, UiValue::from("Application")),
            literal(Namespace::Control, common::NAME, UiValue::from(self.name())),
            literal(Namespace::Control, common::RUNTIME_ID, UiValue::from(self.runtime_id().as_str())),
            literal(Namespace::Control, common::TECHNOLOGY, UiValue::from(TECHNOLOGY)),
            literal(Namespace::Control, application::PROCESS_ID, UiValue::from(i64::from(self.session.pid()))),
        ];
        push_optional(&mut attrs, application::PROCESS_NAME, self.facts.name.as_deref());
        push_optional(&mut attrs, application::EXECUTABLE_PATH, self.facts.executable_path.as_deref());
        push_optional(&mut attrs, application::COMMAND_LINE, self.facts.command_line.as_deref());
        push_optional(&mut attrs, application::USER_NAME, self.facts.user_name.as_deref());
        push_optional(&mut attrs, application::ARCHITECTURE, self.facts.architecture.as_deref());
        if let Some(start) = self.facts.start_time_millis {
            attrs.push(literal(Namespace::Control, application::START_TIME, UiValue::from(start)));
        }
        // Which JVM and which agent version served this process — the first two
        // questions asked when a Java run behaves differently than expected.
        if let Some(vm) = self.facts.vm_name.as_deref() {
            attrs.push(native("VmName", UiValue::from(vm.to_owned())));
        }
        if let Some(version) = self.facts.java_version.as_deref() {
            attrs.push(native("JavaVersion", UiValue::from(version.to_owned())));
        }
        attrs.push(native("AgentVersion", UiValue::from(self.session.version().to_owned())));
        attrs.push(native("AgentToolkits", UiValue::from(self.session.toolkits().join(","))));
        Box::new(attrs.into_iter())
    }

    fn supported_patterns(&self) -> Vec<PatternName> {
        Vec::new()
    }

    fn invalidate(&self) {}
}

fn push_optional(attrs: &mut Vec<Arc<dyn UiAttribute>>, name: &'static str, value: Option<&str>) {
    if let Some(text) = value.filter(|text| !text.is_empty()) {
        attrs.push(literal(Namespace::Control, name, UiValue::from(text.to_owned())));
    }
}

fn literal(namespace: Namespace, name: &'static str, value: UiValue) -> Arc<dyn UiAttribute> {
    Arc::new(Fixed { namespace, name, value })
}

fn native(name: &'static str, value: UiValue) -> Arc<dyn UiAttribute> {
    Arc::new(Fixed { namespace: Namespace::Native, name, value })
}

struct Fixed {
    namespace: Namespace,
    name: &'static str,
    value: UiValue,
}

impl UiAttribute for Fixed {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        self.name
    }

    fn value(&self) -> UiValue {
        self.value.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentAppNode, ProcessFacts};
    use crate::agent::session::AgentSession;
    use platynui_core::ui::UiNode;
    use std::sync::Arc;

    /// A session for this test process, so that the process check alone would
    /// find the node valid.
    fn session() -> Arc<AgentSession> {
        Arc::new(AgentSession::unconnected(std::process::id()))
    }

    fn node(session: &Arc<AgentSession>) -> Arc<AgentAppNode> {
        AgentAppNode::new(Arc::clone(session), ProcessFacts::default(), None, None)
    }

    #[test]
    fn a_closed_session_makes_the_application_node_invalid() {
        let session = session();
        let app = node(&session);
        assert!(app.is_valid(), "the process runs and the session is open");

        session.close();
        assert!(!app.is_valid(), "a closed session serves nothing");
    }

    #[test]
    fn a_degraded_session_makes_the_application_node_invalid_until_it_recovers() {
        let session = session();
        let app = node(&session);

        session.set_degraded(true);
        assert!(!app.is_valid(), "a degraded agent must not claim its nodes are fine");

        session.set_degraded(false);
        assert!(app.is_valid(), "the node is valid again once the agent answers");
    }

    #[test]
    fn an_ended_process_makes_the_application_node_invalid() {
        // Far above any pid a system hands out, so no process has it.
        let session = Arc::new(AgentSession::unconnected(0x3FFF_FFFC));
        let app = node(&session);
        assert!(!app.is_valid(), "the session is open, but its process is gone");
    }

    /// Every call of this session fails, because no agent answers for it. A
    /// validity check that called into the JVM would count a failure.
    #[test]
    fn the_validity_check_does_not_call_into_the_jvm() {
        let session = session();
        let app = node(&session);
        for _ in 0..10 {
            let _ = app.is_valid();
        }
        assert_eq!(session.failure_count(), 0, "is_valid called into the agent");
        assert!(!session.is_degraded());
    }
}
