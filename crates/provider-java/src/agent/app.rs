//! The `app:Application` node grouping one JVM's windows.
//!
//! Not decoration: the Swing acceptance suites set their scoped root to
//! `/app:Application[@ProcessId=<pid>]`, and every provider presents its
//! processes this way, so a backend that skipped it would make the same
//! application addressable differently depending on which channel served it.
//!
//! Its process attributes come from the platform, read through the process
//! identity the node recorded at creation (`platynui-process`), the same reader
//! every provider uses. So the agent and the Access Bridge report one process
//! identically, and a node never reports a process that received its pid. What
//! the JVM says about itself (`agent/process`) is no source for them: its
//! `user.name` can be overridden, and its `java.home` names another launcher
//! than the one that was started. It is used only for what the platform does not
//! know — the main class as the display name, which is what a user recognises
//! their application by, and the JVM's name and version.

use super::node::{AgentNode, TECHNOLOGY, View};
use super::session::AgentSession;
use platynui_core::platform::WindowManager;
use platynui_core::ui::attribute_names::{application, common};
use platynui_core::ui::{Namespace, PatternName, RuntimeId, UiAttribute, UiNode, UiValue};
use platynui_process::ProcessAttribute;
use serde::Deserialize;
use serde_json::json;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tracing::debug;

/// What the agent reports about its own JVM, as far as the node uses it. The
/// agent sends its process facts as well; serde ignores them, because the
/// process attributes come from the platform.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ProcessFacts {
    /// The main class's simple name, or the jar's file name: the display name.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "vmName", default)]
    pub vm_name: Option<String>,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<String>,
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

    fn control_attributes(&self) -> Vec<Arc<dyn UiAttribute>> {
        vec![
            literal(Namespace::Control, common::ROLE, UiValue::from("Application")),
            literal(Namespace::Control, common::NAME, UiValue::from(self.name())),
            literal(Namespace::Control, common::RUNTIME_ID, UiValue::from(self.runtime_id().as_str())),
            literal(Namespace::Control, common::TECHNOLOGY, UiValue::from(TECHNOLOGY)),
            literal(Namespace::Control, application::PROCESS_ID, UiValue::from(i64::from(self.session.pid()))),
        ]
    }

    /// Which JVM and which agent version served this process — the first two
    /// questions asked when a Java run behaves differently than expected.
    fn native_attributes(&self) -> Vec<Arc<dyn UiAttribute>> {
        let mut attrs = Vec::new();
        if let Some(vm) = self.facts.vm_name.as_deref() {
            attrs.push(native("VmName", UiValue::from(vm.to_owned())));
        }
        if let Some(version) = self.facts.java_version.as_deref() {
            attrs.push(native("JavaVersion", UiValue::from(version.to_owned())));
        }
        attrs.push(native("AgentVersion", UiValue::from(self.session.version().to_owned())));
        attrs.push(native("AgentToolkits", UiValue::from(self.session.toolkits().join(","))));
        attrs
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
            let node = AgentNode::new(
                Arc::clone(&session),
                View::Application,
                window,
                None,
                window_manager.clone(),
                parent.as_ref(),
            );
            // A window keeps its application node alive.
            if let Some(parent) = &parent {
                node.hold_parent(Arc::clone(parent));
            }
            node as Arc<dyn UiNode>
        }))
    }

    /// The process attributes are read once per listing, through the recorded
    /// identity; only those that were read are listed.
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        let mut attrs = self.control_attributes();
        if let Some(process) = &self.process {
            attrs.extend(process.read_all().iter().map(|(attribute, value)| {
                literal(Namespace::App, process_attribute_name(attribute), UiValue::from(value))
            }));
        }
        attrs.extend(self.native_attributes());
        Box::new(attrs.into_iter())
    }

    /// A lookup by name reads only what it names: one process attribute, or
    /// none for any other name.
    fn attribute(&self, namespace: Namespace, name: &str) -> Option<Arc<dyn UiAttribute>> {
        let others = match namespace {
            Namespace::App => {
                let attribute = process_attribute(name)?;
                let value = self.process.as_ref()?.read(attribute)?;
                return Some(literal(Namespace::App, process_attribute_name(attribute), UiValue::from(value)));
            }
            Namespace::Control => self.control_attributes(),
            Namespace::Native => self.native_attributes(),
            Namespace::Item => return None,
        };
        others.into_iter().find(|attribute| attribute.name() == name)
    }

    fn supported_patterns(&self) -> Vec<PatternName> {
        Vec::new()
    }

    fn invalidate(&self) {}
}

/// The `app` name of a process attribute.
const fn process_attribute_name(attribute: ProcessAttribute) -> &'static str {
    match attribute {
        ProcessAttribute::ProcessName => application::PROCESS_NAME,
        ProcessAttribute::ExecutablePath => application::EXECUTABLE_PATH,
        ProcessAttribute::CommandLine => application::COMMAND_LINE,
        ProcessAttribute::UserName => application::USER_NAME,
        ProcessAttribute::StartTime => application::START_TIME,
        ProcessAttribute::Architecture => application::ARCHITECTURE,
    }
}

/// The process attribute an `app` name stands for.
fn process_attribute(name: &str) -> Option<ProcessAttribute> {
    ProcessAttribute::ALL.into_iter().find(|attribute| process_attribute_name(*attribute) == name)
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
    use super::{AgentAppNode, ProcessFacts, process_attribute_name};
    use crate::agent::session::AgentSession;
    use platynui_core::ui::attribute_names::{application, common};
    use platynui_core::ui::{Namespace, UiNode, UiValue};
    use platynui_process::ProcessIdentity;
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

    /// The node's attributes as (namespace, name, value) triples.
    fn listed(app: &AgentAppNode) -> Vec<(Namespace, String, UiValue)> {
        app.attributes()
            .map(|attribute| (attribute.namespace(), attribute.name().to_owned(), attribute.value()))
            .collect()
    }

    /// The process attributes the reader finds for the own process, as the node
    /// must list them under `app`.
    fn own_process_attributes() -> Vec<(Namespace, String, UiValue)> {
        let identity = ProcessIdentity::capture(std::process::id()).expect("the own process exists");
        identity
            .read_all()
            .iter()
            .map(|(attribute, value)| {
                (Namespace::App, process_attribute_name(attribute).to_owned(), UiValue::from(value))
            })
            .collect()
    }

    const PROCESS_ATTRIBUTE_NAMES: [&str; 6] = [
        application::PROCESS_NAME,
        application::EXECUTABLE_PATH,
        application::COMMAND_LINE,
        application::USER_NAME,
        application::START_TIME,
        application::ARCHITECTURE,
    ];

    /// Spec: *A Java application served by the in-JVM agent carries its process
    /// attributes under app*.
    #[test]
    fn the_process_attributes_are_the_readers_under_app() {
        let app = node(&session());
        let listed = listed(&app);
        let app_attributes: Vec<_> =
            listed.iter().filter(|(namespace, ..)| *namespace == Namespace::App).cloned().collect();
        assert!(!app_attributes.is_empty(), "the own process is readable");
        assert_eq!(app_attributes, own_process_attributes());
        for (_, name, value) in &app_attributes {
            assert_ne!(value, &UiValue::from(""), "app:{name} is empty");
        }
        for name in PROCESS_ATTRIBUTE_NAMES {
            assert!(
                !listed.iter().any(|(namespace, listed, _)| *namespace == Namespace::Control && listed == name),
                "{name} is listed under control as well"
            );
        }
    }

    #[test]
    fn a_lookup_by_name_agrees_with_the_listing() {
        let app = node(&session());
        let listed = listed(&app);
        for name in PROCESS_ATTRIBUTE_NAMES {
            for namespace in [Namespace::App, Namespace::Control] {
                let found = app.attribute(namespace, name).map(|attribute| attribute.value());
                let expected = listed
                    .iter()
                    .find(|(ns, listed, _)| *ns == namespace && listed == name)
                    .map(|(.., value)| value.clone());
                assert_eq!(found, expected, "{namespace:?}:{name}");
            }
        }
        for (namespace, name, value) in &listed {
            assert_eq!(app.attribute(*namespace, name).map(|attribute| attribute.value()).as_ref(), Some(value));
        }
    }

    /// Spec: *An application's self-description does not replace a process
    /// attribute*. Built from JSON, so that it survives the removal of these
    /// fields from `ProcessFacts`, and shows that serde ignores them.
    #[test]
    fn what_the_jvm_says_about_itself_changes_no_process_attribute() {
        let facts: ProcessFacts = serde_json::from_value(serde_json::json!({
            "name": "Main",
            "userName": "someone-else",
            "architecture": "amd64",
            "startTimeMillis": 1,
        }))
        .expect("facts");
        let app = AgentAppNode::new(session(), facts, None, None);
        let listed = listed(&app);
        let app_attributes: Vec<_> =
            listed.iter().filter(|(namespace, ..)| *namespace == Namespace::App).cloned().collect();
        assert_eq!(app_attributes, own_process_attributes());
        assert_eq!(
            app.attribute(Namespace::Control, common::NAME).map(|name| name.value()),
            Some(UiValue::from("Main"))
        );
    }

    /// Spec: *An application node whose process has ended reports no process
    /// attributes*.
    #[test]
    fn a_node_without_a_process_lists_no_process_attribute() {
        let app = node(&Arc::new(AgentSession::unconnected(0x3FFF_FFFC)));
        assert!(app.attributes().all(|attribute| attribute.namespace() != Namespace::App));
        for name in PROCESS_ATTRIBUTE_NAMES {
            assert!(app.attribute(Namespace::App, name).is_none(), "app:{name}");
        }
    }

    /// Every call of this session fails, so a listing that called into the JVM
    /// would count a failure.
    #[test]
    fn listing_the_attributes_does_not_call_into_the_jvm() {
        let session = session();
        let app = node(&session);
        let _ = app.attributes().count();
        let _ = app.attribute(Namespace::App, application::PROCESS_NAME);
        assert_eq!(session.failure_count(), 0, "the attributes called into the agent");
    }
}
