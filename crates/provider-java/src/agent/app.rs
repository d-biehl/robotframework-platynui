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
        let node = Arc::new(Self {
            session,
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

    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        let parent = self.self_weak.get().and_then(Weak::upgrade);
        let windows = super::backend::read_windows(&self.session);
        let session = Arc::clone(&self.session);
        let window_manager = self.window_manager.clone();
        Box::new(windows.into_iter().map(move |window| {
            AgentNode::new(Arc::clone(&session), window, None, window_manager.clone(), parent.as_ref())
                as Arc<dyn UiNode>
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

    fn doc_order_key(&self) -> Option<u64> {
        Some(u64::from(self.session.pid()))
    }
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
