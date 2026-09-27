//! Provider factory, configuration, backend routing, claims and diagnostics.
//!
//! # Configuration (`providers.java.*`)
//!
//! | Key | Default | Effect |
//! |---|---|---|
//! | `providers.java.enabled` | `true` | Umbrella kill switch: off means no backend is built at all, and Java windows are served by the platform's native provider |
//! | `providers.java.agent.enabled` | `true` | The in-JVM agent backend's kill switch |
//! | `providers.java.agent.auto_attach` | `true` | Inject the agent into a Java window's JVM that carries none. On by default: attaching to an application somebody else launched is the normal path, and the deliberate consent is installing the agent package. `false` limits the backend to `-javaagent`-launched targets |
//! | `providers.java.agent.jar` | discovery | Explicit `platynui-agent.jar` |
//! | `providers.java.agent.call_timeout_ms` | `5000` | Per-call deadline on the agent connection |
//! | `providers.java.jab.enabled` | `true` | The JAB backend's kill switch; it then loads no DLL and contributes nothing, leaving the umbrella and other backends alone |
//! | `providers.java.jab.dll_path` | discovery | Explicit `WindowsAccessBridge-64.dll` |
//! | `providers.java.jab.call_timeout_ms` | `2000` | Per-call deadline on the JAB pump thread |
//!
//! Each backend reads only its own sub-map; the provider resolves the namespace
//! and hands the sub-map down. **These flags are the entire user-facing surface
//! of backend selection** — there is no attach or connect keyword, and no CLI
//! subcommand: which backend serves a window follows from whether that JVM has
//! an agent, which is the router's business and not the user's.
//!
//! The pre-`unify-java-provider` spelling `providers.jab.*` is simply gone —
//! not aliased and not diagnosed. The config layer ignores sections nobody
//! claims, which is the right answer here: there is no released version that
//! ever read those keys.
//!
//! Each part checks the settings it reads when it is built: the provider its
//! own keys, each backend its sub-map. An unknown key and a value of the wrong
//! type are warned about under `providers.java`, a backend's keys dotted
//! (`agent.call_timeout_ms`), and the default applies.

use crate::agent::AgentBackend;
use crate::backend::{BackendOwnership, JavaBackend, UnservedCause, UnservedJavaWindow};
use crate::jab::JabBackend;
use platynui_core::config::{ConfigMap, ConfigTypeMismatch, ENABLED_KEY, RuntimeConfig};
use platynui_core::platform::{WindowManager, window_claims};
use platynui_core::provider::{ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory};
use platynui_core::register_provider;
use platynui_core::types::Point;
use platynui_core::ui::{TechnologyId, UiNode};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use tracing::{debug, info, warn};

pub const PROVIDER_ID: &str = "java";
pub const PROVIDER_NAME: &str = "Java";
/// The provider's registered technology. Individual nodes carry their
/// *backend's* `@Technology` (`"JAB"`, …) — this names the umbrella.
pub const TECHNOLOGY: &str = "Java";

/// The component the provider's settings are reported under.
const COMPONENT: &str = "providers.java";

/// The provider's own settings, besides the umbrella kill switch
/// ([`ENABLED_KEY`], `providers.java.enabled`): the backends' sub-maps.
const KNOWN_SETTINGS: &[&str] = &[crate::agent::BACKEND_ID, crate::jab::BACKEND_ID];

/// Whether the missing Access Bridge DLL has been reported in this process.
static BRIDGE_DLL_MISSING_REPORTED: AtomicBool = AtomicBool::new(false);

static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
    ProviderDescriptor::new(PROVIDER_ID, PROVIDER_NAME, TechnologyId::from(TECHNOLOGY), ProviderKind::Native)
});

pub struct JavaFactory;

impl UiTreeProviderFactory for JavaFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        &DESCRIPTOR
    }

    fn create(&self, config: &RuntimeConfig) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        Ok(Arc::new(Self::build(config)))
    }
}

impl JavaFactory {
    /// Build a concrete provider from `config` — split out from `create` so
    /// the config wiring is unit-testable without a live backend.
    ///
    /// **The order of the backends is their preference order**, strongest first:
    /// the router hands each one the windows the stronger ones did not take, so
    /// "prefer the agent over the Access Bridge" is expressed by pushing the
    /// agent backend first and by nothing else.
    fn build(config: &RuntimeConfig) -> JavaProvider {
        let settings = config.provider(PROVIDER_ID);
        if let Some(settings) = settings {
            check_unknown_settings(settings, None, KNOWN_SETTINGS);
        }
        let enabled = settings.and_then(|java| setting(java.try_bool(ENABLED_KEY), None)).unwrap_or(true);
        let agent_settings = settings.and_then(|java| setting(java.try_map(crate::agent::BACKEND_ID), None));
        let jab_settings = settings.and_then(|java| setting(java.try_map(crate::jab::BACKEND_ID), None));

        let ownership = Arc::new(BackendOwnership::default());
        let mut backends: Vec<Box<dyn JavaBackend>> = Vec::new();
        let mut agent: Option<Arc<AgentBackend>> = None;
        // Umbrella off: build nothing at all, so no backend can load anything
        // and Java windows stay with the platform's native provider.
        if enabled {
            // The agent goes first, and that single fact *is* the routing rule
            // (design 4): the agent only reports windows of JVMs that carry an
            // agent, so "prefer the agent when one is present, else the Access
            // Bridge" needs no condition anywhere — it is the order.
            let backend = Arc::new(AgentBackend::from_config(agent_settings, Some(ownership.view(backends.len()))));
            agent = Some(Arc::clone(&backend));
            backends.push(Box::new(ArcBackend(backend)));
            backends.push(Box::new(JabBackend::from_config(jab_settings, ownership.view(backends.len()))));
        }
        debug!(backends = ?backends.iter().map(|backend| backend.id()).collect::<Vec<_>>(), "Java provider built");
        JavaProvider::new(backends, ownership, agent)
    }
}

/// Adapts a shared backend to the boxed trait object the router holds, so the
/// factory can keep a second handle on the agent backend — the router has to
/// reach it for the one decision a backend cannot make alone (see
/// [`AgentBackend::consider_attaching`]).
struct ArcBackend(Arc<AgentBackend>);

impl JavaBackend for ArcBackend {
    fn id(&self) -> &'static str {
        self.0.id()
    }
    fn enumerate(&self, parent: &Arc<dyn UiNode>) -> crate::backend::Enumeration {
        self.0.enumerate(parent)
    }
    fn element_at_point(&self, point: Point) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
        self.0.element_at_point(point)
    }
    fn set_window_manager(&self, window_manager: Arc<dyn WindowManager>) {
        self.0.set_window_manager(window_manager);
    }
    fn shutdown(&self) {
        self.0.shutdown();
    }
}

pub struct JavaProvider {
    descriptor: &'static ProviderDescriptor,
    /// Preference order, strongest first — see [`JavaFactory::build`].
    backends: Vec<Box<dyn JavaBackend>>,
    /// Which backend currently serves which top-level window.
    ownership: Arc<BackendOwnership>,
    /// The agent backend, when one was built.
    agent: Option<Arc<AgentBackend>>,
    is_shutdown: AtomicBool,
    /// Window claims currently held by this provider instance.
    claimed: Mutex<HashSet<u64>>,
}

impl JavaProvider {
    fn new(
        backends: Vec<Box<dyn JavaBackend>>,
        ownership: Arc<BackendOwnership>,
        agent: Option<Arc<AgentBackend>>,
    ) -> Self {
        Self {
            descriptor: &DESCRIPTOR,
            backends,
            ownership,
            agent,
            is_shutdown: AtomicBool::new(false),
            claimed: Mutex::new(HashSet::new()),
        }
    }

    /// Bring the process-wide claims registry in line with the windows the
    /// backends currently serve: claim newcomers, release windows that are gone
    /// (closed, process died, or the handle got recycled).
    fn sync_window_claims(&self, served: &HashSet<u64>) {
        let mut claimed = self.claimed.lock().expect("claims mutex poisoned");
        for stale in claimed.difference(served) {
            window_claims::release_window(*stale, PROVIDER_ID);
        }
        for new in served.difference(&claimed) {
            window_claims::claim_window(*new, PROVIDER_ID);
        }
        (*claimed).clone_from(served);
    }

    fn release_all_claims(&self) {
        let mut claimed = self.claimed.lock().expect("claims mutex poisoned");
        for raw in claimed.drain() {
            window_claims::release_window(raw, PROVIDER_ID);
        }
    }

    /// One sweep over the backends, in preference order.
    ///
    /// Each backend's outcome is recorded before the next one runs, so a weaker
    /// backend already sees the windows it must leave alone *within this sweep*.
    /// Selection is the backend's **input** rather than the router's post-filter,
    /// because the windows under an `app:Application` node are enumerated lazily —
    /// after this returns — so a filter applied here would miss them.
    fn sweep(&self, parent: &Arc<dyn UiNode>) -> Sweep {
        let mut sweep = Sweep::default();
        for (rank, backend) in self.backends.iter().enumerate() {
            let mut pass = backend.enumerate(parent);
            self.ownership.record(rank, &pass.served_windows);
            sweep.served.extend(pass.served_windows);
            sweep.nodes.append(&mut pass.nodes);
            sweep.unserved.append(&mut pass.unserved);
            for pid in pass.java_processes {
                if !sweep.java_processes.contains(&pid) {
                    sweep.java_processes.push(pid);
                }
            }
        }
        sweep
    }
}

/// What one sweep over the backends produced.
#[derive(Default)]
struct Sweep {
    nodes: Vec<Arc<dyn UiNode>>,
    served: HashSet<u64>,
    unserved: Vec<crate::backend::UnservedJavaWindow>,
    java_processes: Vec<u32>,
}

impl UiTreeProvider for JavaProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
    }

    fn set_window_manager(&self, window_manager: Arc<dyn WindowManager>) {
        for backend in &self.backends {
            backend.set_window_manager(Arc::clone(&window_manager));
        }
    }

    fn shutdown(&self) {
        if self.is_shutdown.swap(true, Ordering::AcqRel) {
            return;
        }
        info!("Java provider shutting down");
        self.release_all_claims();
        for backend in &self.backends {
            backend.shutdown();
        }
    }

    fn get_nodes(
        &self,
        parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
        if self.is_shutdown.load(Ordering::Acquire) {
            return Err(shut_down());
        }

        let mut sweep = self.sweep(&parent);

        // "Offer the agent to this JVM" turns on the process having no agent, not
        // on its window being unserved: a bridge-served window still has a better
        // representation available. The pid list comes from the backends that
        // enumerate windows, which the agent backend cannot — it finds agents.
        // Only the router can ask, because only it sees all the backends.
        if let Some(agent) = self.agent.as_ref() {
            let attached = agent.consider_attaching(&sweep.java_processes);
            if !attached.is_empty() {
                // The sweep above is now stale for those JVMs: it was taken before
                // they had an agent, so it holds Access Bridge nodes for windows the
                // agent is about to serve better. **Redo it rather than edit it** —
                // which node stands for which window is not something a flat node
                // list can answer, and the alternative is the caller seeing the
                // bridge until it enumerates again (in the Inspector: pressing
                // refresh twice). The second sweep has the agent recording those
                // windows at its own rank first, so the Access Bridge skips them on
                // its own, exactly as in the steady state.
                //
                // Paid at most once per successful attachment, and attachment is
                // attempted a bounded number of times per process — not per pass.
                debug!(?attached, "re-enumerating: these JVMs gained an agent during this pass");
                sweep = self.sweep(&parent);
            }
        }

        self.sync_window_claims(&sweep.served);
        // "Tell the user this JVM is unreachable" turns on *no* backend reaching
        // the window, which is the router's own knowledge. Taken from the final
        // sweep, so a window the agent took over after attaching is not reported.
        let unreachable: Vec<UnservedJavaWindow> =
            sweep.unserved.into_iter().filter(|window| !sweep.served.contains(&window.window)).collect();
        emit_enablement_diagnostics(&unreachable, &BRIDGE_DLL_MISSING_REPORTED);

        Ok(Box::new(sweep.nodes.into_iter()))
    }

    /// Route the point to the first backend that answers for it.
    ///
    /// A backend reporting `UnsupportedOperation` is abstaining ("not a window
    /// of mine"), so the next one gets its turn; anything else — a hit, a
    /// deliberate miss inside a Java window, or a genuine failure — is the
    /// answer. With no backend able to answer, the abstention travels on so the
    /// runtime falls through to the platform's native provider.
    fn element_at_point(&self, point: Point) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
        if self.is_shutdown.load(Ordering::Acquire) {
            return Err(unsupported_at_point("provider shut down"));
        }
        let mut abstained = None;
        for backend in &self.backends {
            match backend.element_at_point(point) {
                Err(err @ ProviderError::UnsupportedOperation { .. }) => abstained = Some(err),
                answer => return answer,
            }
        }
        Err(abstained.unwrap_or_else(|| unsupported_at_point("no Java backend is available")))
    }
}

fn shut_down() -> ProviderError {
    ProviderError::CommunicationFailure {
        channel: PROVIDER_ID,
        details: Some("Java provider has been shut down".into()),
    }
}

fn unsupported_at_point(details: &str) -> ProviderError {
    ProviderError::UnsupportedOperation { operation: "element_at_point", details: Some(details.into()) }
}

/// Tell the user why the Java-looking windows no backend serves stay unserved.
/// Never mutates any target-side configuration; it only tells the user how to.
///
/// - The Access Bridge DLL is missing: one warning per process for all such
///   windows (`dll_missing_reported` holds whether it was given), naming where
///   the DLL can come from. Most Windows machines have no JDK, so the missing
///   DLL is worth a warning only now that a window is left without a backend.
/// - The bridge is not enabled in the window's JVM: the shared "JVM window
///   absent from native accessibility" diagnostic (see
///   `platynui_core::platform::java`), with how to enable it, which the shared
///   registry de-duplicates per window, process-wide.
fn emit_enablement_diagnostics(unreachable: &[UnservedJavaWindow], dll_missing_reported: &AtomicBool) {
    use platynui_core::platform::java::{JavaToolkit, jvm_unreachable_diagnostic_once};
    let without_dll: Vec<u32> = unreachable
        .iter()
        .filter(|window| window.cause == UnservedCause::BridgeDllNotFound)
        .map(|window| window.pid)
        .fold(Vec::new(), |mut pids, pid| {
            if !pids.contains(&pid) {
                pids.push(pid);
            }
            pids
        });
    if !without_dll.is_empty() && !dll_missing_reported.swap(true, Ordering::AcqRel) {
        warn!(
            pids = ?without_dll,
            "Java Access Bridge DLL not found; the elements of Swing and AWT windows that the PlatynUI agent \
             does not serve are missing from query results (set providers.java.jab.dll_path or PLATYNUI_JAB_DLL \
             to the WindowsAccessBridge-64.dll of a 64-bit JDK, or put a 64-bit JDK on PATH)"
        );
    }
    for window in unreachable.iter().filter(|window| window.cause == UnservedCause::BridgeNotEnabled) {
        let toolkit = JavaToolkit::from_window_class(&window.class_name).unwrap_or(JavaToolkit::Unknown);
        if let Some(hint) = jvm_unreachable_diagnostic_once(window.window, toolkit) {
            warn!(
                hwnd = format!("0x{:X}", window.window),
                class = %window.class_name,
                pid = window.pid,
                toolkit = toolkit.label(),
                "JVM window is absent from native accessibility. {hint}"
            );
        }
    }
}

/// Warns for each key of `settings` that `known` does not list. `section` is
/// the backend whose sub-map the settings are (`agent` for
/// `providers.java.agent`), so that the key is reported dotted under the
/// provider; `None` for the provider's own keys.
pub(crate) fn check_unknown_settings(settings: &ConfigMap, section: Option<&str>, known: &[&str]) {
    for key in settings.unknown_keys(known) {
        warn!(component = COMPONENT, key = %SettingKey(section, key), "unknown setting; it is ignored");
    }
}

/// The value of a setting, or `None` after a warning when it has the wrong
/// type, so that the caller applies the default. `section` as for
/// [`check_unknown_settings`].
pub(crate) fn setting<T>(read: Result<Option<T>, ConfigTypeMismatch>, section: Option<&str>) -> Option<T> {
    read.unwrap_or_else(|mismatch| {
        warn!(
            component = COMPONENT,
            key = %SettingKey(section, &mismatch.key),
            expected = mismatch.expected,
            found = mismatch.found,
            "setting has the wrong type; the default applies"
        );
        None
    })
}

/// A setting's key relative to `providers.java`: `enabled`, `agent.jar`.
struct SettingKey<'a>(Option<&'a str>, &'a str);

impl std::fmt::Display for SettingKey<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(section) => write!(f, "{section}.{}", self.1),
            None => f.write_str(self.1),
        }
    }
}

pub static JAVA_FACTORY: JavaFactory = JavaFactory;

// Auto-register the Java provider when linked (Windows builds only — the whole
// module is `cfg(windows)`).
register_provider!(&JAVA_FACTORY);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Enumeration, UnservedJavaWindow};
    use platynui_core::config::ConfigMap;
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

    fn desktop() -> Arc<dyn UiNode> {
        Arc::new(DesktopStub(RuntimeId::from("desktop")))
    }

    /// A backend that serves exactly the windows its handle currently holds,
    /// so a test can change what the next enumeration pass finds. Like a real
    /// backend it drops the windows a stronger one already serves, which is what
    /// makes the preference order observable.
    struct StubBackend {
        id: &'static str,
        served: Arc<Mutex<Vec<u64>>>,
        unserved: Vec<u64>,
        unserved_cause: UnservedCause,
        foreign: Option<Arc<crate::backend::ForeignWindows>>,
        shut_down: Arc<AtomicBool>,
    }

    struct StubHandle {
        served: Arc<Mutex<Vec<u64>>>,
        shut_down: Arc<AtomicBool>,
    }

    fn stub(served: &[u64], unserved: &[u64]) -> (Box<dyn JavaBackend>, StubHandle) {
        named_stub("stub", served, unserved, None)
    }

    fn named_stub(
        id: &'static str,
        served: &[u64],
        unserved: &[u64],
        foreign: Option<Arc<crate::backend::ForeignWindows>>,
    ) -> (Box<dyn JavaBackend>, StubHandle) {
        stub_with_cause(id, served, unserved, UnservedCause::BridgeNotEnabled, foreign)
    }

    fn stub_with_cause(
        id: &'static str,
        served: &[u64],
        unserved: &[u64],
        unserved_cause: UnservedCause,
        foreign: Option<Arc<crate::backend::ForeignWindows>>,
    ) -> (Box<dyn JavaBackend>, StubHandle) {
        let backend = StubBackend {
            id,
            served: Arc::new(Mutex::new(served.to_vec())),
            unserved: unserved.to_vec(),
            unserved_cause,
            foreign,
            shut_down: Arc::new(AtomicBool::new(false)),
        };
        let handle = StubHandle { served: Arc::clone(&backend.served), shut_down: Arc::clone(&backend.shut_down) };
        (Box::new(backend), handle)
    }

    impl JavaBackend for StubBackend {
        fn id(&self) -> &'static str {
            self.id
        }
        fn enumerate(&self, _parent: &Arc<dyn UiNode>) -> Enumeration {
            let served_windows: Vec<u64> = self
                .served
                .lock()
                .expect("served set mutex poisoned")
                .iter()
                .copied()
                .filter(|window| !self.foreign.as_ref().is_some_and(|foreign| foreign.is_foreign(*window)))
                .collect();
            Enumeration {
                served_windows,
                nodes: Vec::new(),
                unserved: self
                    .unserved
                    .iter()
                    .map(|window| UnservedJavaWindow {
                        window: *window,
                        pid: 1,
                        class_name: "SunAwtFrame".into(),
                        cause: self.unserved_cause,
                    })
                    .collect(),
                java_processes: Vec::new(),
            }
        }
        fn element_at_point(&self, _point: Point) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
            Err(unsupported_at_point("stub"))
        }
        fn set_window_manager(&self, _window_manager: Arc<dyn WindowManager>) {}
        fn shutdown(&self) {
            self.shut_down.store(true, Ordering::SeqCst);
        }
    }

    /// The preference order *is* the routing rule (design 4), so it is what the
    /// default configuration has to pin: the agent first, the Access Bridge
    /// behind it. Reversing these two would silently make JAB win every window
    /// it can reach, which is exactly the fidelity regression this change exists
    /// to remove.
    #[test]
    fn defaults_prefer_the_agent_over_the_access_bridge() {
        let provider = JavaFactory::build(&RuntimeConfig::default());
        let ids: Vec<_> = provider.backends.iter().map(|backend| backend.id()).collect();
        assert_eq!(ids, vec![crate::agent::BACKEND_ID, crate::jab::BACKEND_ID]);
        assert!(provider.agent.is_some(), "the router keeps a handle for the attach decision");
    }

    #[test]
    fn umbrella_kill_switch_builds_no_backend() {
        let providers = ConfigMap::new().with(PROVIDER_ID, ConfigMap::new().with(ENABLED_KEY, false));
        let provider = JavaFactory::build(&RuntimeConfig::new(ConfigMap::new(), providers));
        assert!(provider.backends.is_empty(), "umbrella off must build no backend at all");
        let nodes: Vec<_> = provider.get_nodes(desktop()).expect("inert ok").collect();
        assert!(nodes.is_empty());
    }

    #[test]
    fn backend_settings_come_from_the_backends_own_sub_map() {
        // The umbrella resolves `providers.java.<backend>.*`; both backends below
        // are built with their kill switch off and must stay inert without
        // failing — a disabled backend is still a backend, just one that
        // contributes nothing.
        let providers = ConfigMap::new().with(
            PROVIDER_ID,
            ConfigMap::new()
                .with(crate::jab::BACKEND_ID, ConfigMap::new().with("enabled", false))
                .with(crate::agent::BACKEND_ID, ConfigMap::new().with("enabled", false)),
        );
        let provider = JavaFactory::build(&RuntimeConfig::new(ConfigMap::new(), providers));
        assert_eq!(provider.backends.len(), 2, "a disabled backend is still built, just inert");
        let nodes: Vec<_> = provider.get_nodes(desktop()).expect("inert ok").collect();
        assert!(nodes.is_empty());
    }

    #[test]
    fn claims_follow_what_the_backends_serve() {
        // The registry is process-global, so every test uses its own windows.
        let (backend, handle) = stub(&[0xC100], &[]);
        let provider = JavaProvider::new(vec![backend], Arc::new(BackendOwnership::default()), None);

        let _ = provider.get_nodes(desktop()).expect("enumerate");
        assert!(window_claims::is_claimed_by_other(0xC100, "windows-uia"), "another provider must see the claim");
        assert!(!window_claims::is_claimed_by_other(0xC100, PROVIDER_ID), "the owner itself is not 'other'");

        // Window disappears on the next pass → claim released.
        handle.served.lock().expect("served set mutex poisoned").clear();
        let _ = provider.get_nodes(desktop()).expect("enumerate");
        assert!(!window_claims::is_claimed(0xC100));
    }

    #[test]
    fn shutdown_releases_claims_and_stops_serving() {
        let (backend, handle) = stub(&[0xC200], &[]);
        let provider = JavaProvider::new(vec![backend], Arc::new(BackendOwnership::default()), None);
        let _ = provider.get_nodes(desktop()).expect("enumerate");
        assert!(window_claims::is_claimed(0xC200));

        provider.shutdown();
        assert!(!window_claims::is_claimed(0xC200));
        assert!(handle.shut_down.load(Ordering::SeqCst), "shutdown reaches the backends");
        assert!(provider.get_nodes(desktop()).is_err(), "a shut-down provider stops answering");
    }

    #[test]
    fn a_window_no_backend_serves_is_not_claimed() {
        use platynui_core::platform::java::jvm_unreachable_diagnostic_emitted;

        // 0xC300 looks like a JVM window but no backend reaches it: unclaimed,
        // so the native provider keeps it — and the user is told why.
        let (backend, _handle) = stub(&[], &[0xC300]);
        let provider = JavaProvider::new(vec![backend], Arc::new(BackendOwnership::default()), None);
        let _ = provider.get_nodes(desktop()).expect("enumerate");
        assert!(!window_claims::is_claimed(0xC300));
        assert!(jvm_unreachable_diagnostic_emitted(0xC300), "an unreachable JVM window must say how to fix it");
    }

    /// The whole point of the preference order: when two backends can reach one
    /// window, the stronger one serves it and the weaker one stays out — and the
    /// window is claimed exactly once, so no consumer sees two Java trees for it.
    #[test]
    fn the_stronger_backend_takes_a_window_both_can_serve() {
        let ownership = Arc::new(BackendOwnership::default());
        let (strong, strong_handle) = named_stub("strong", &[0xC400], &[], Some(ownership.view(0)));
        let (weak, _weak_handle) = named_stub("weak", &[0xC400], &[], Some(ownership.view(1)));
        let provider = JavaProvider::new(vec![strong, weak], Arc::clone(&ownership), None);

        let _ = provider.get_nodes(desktop()).expect("enumerate");
        assert!(window_claims::is_claimed(0xC400));
        assert_eq!(
            provider.claimed.lock().expect("claims").len(),
            1,
            "one window, one claim — the weaker backend must not add a second"
        );

        // The stronger backend loses the window (its channel went away): the
        // weaker one takes over on the next pass, still exactly one claim.
        strong_handle.served.lock().expect("served").clear();
        let _ = provider.get_nodes(desktop()).expect("enumerate");
        assert!(window_claims::is_claimed(0xC400), "the weaker backend keeps the window served");
        provider.shutdown();
        assert!(!window_claims::is_claimed(0xC400));
    }

    /// The mid-session case: a window the weaker backend has been serving must
    /// move to the stronger one as soon as that one can reach it. "Already owned"
    /// therefore cannot be what excludes a window — only "owned by someone
    /// stronger" can.
    #[test]
    fn a_stronger_backend_appearing_later_takes_the_window_over() {
        let ownership = Arc::new(BackendOwnership::default());
        let (strong, strong_handle) = named_stub("strong", &[], &[], Some(ownership.view(0)));
        let (weak, weak_handle) = named_stub("weak", &[0xC500], &[], Some(ownership.view(1)));
        let provider = JavaProvider::new(vec![strong, weak], Arc::clone(&ownership), None);

        let _ = provider.get_nodes(desktop()).expect("enumerate");
        assert!(window_claims::is_claimed(0xC500));

        // An agent appears in that JVM: the strong backend now reaches the window.
        strong_handle.served.lock().expect("served").push(0xC500);
        let _ = provider.get_nodes(desktop()).expect("enumerate");
        assert!(window_claims::is_claimed(0xC500), "still served, just by the other backend");
        assert_eq!(provider.claimed.lock().expect("claims").len(), 1, "one window, still one claim");
        // The takeover itself: the window is now foreign to rank 1, although the
        // weaker backend still wants it and had it a pass ago.
        assert!(!weak_handle.served.lock().expect("served").is_empty(), "the weaker backend still offers it");
        assert!(ownership.view(1).is_foreign(0xC500), "the stronger backend owns it now");
        assert!(!ownership.view(0).is_foreign(0xC500), "and it is not foreign to the owner itself");
        provider.shutdown();
    }

    #[test]
    fn hit_test_falls_through_when_every_backend_abstains() {
        let (backend, _handle) = stub(&[], &[]);
        let provider = JavaProvider::new(vec![backend], Arc::new(BackendOwnership::default()), None);
        let answer = provider.element_at_point(Point::new(10.0, 10.0));
        assert!(matches!(answer, Err(ProviderError::UnsupportedOperation { .. })));
    }

    fn warnings(log: &str) -> Vec<&str> {
        log.lines().filter(|line| line.contains(" WARN ")).collect()
    }

    fn java_settings(settings: ConfigMap) -> RuntimeConfig {
        RuntimeConfig::new(ConfigMap::new(), ConfigMap::new().with(PROVIDER_ID, settings))
    }

    #[test]
    fn an_unknown_setting_is_reported_once() {
        let (_provider, log) = logged(|| JavaFactory::build(&java_settings(ConfigMap::new().with("bogus", 1_i64))));
        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 1, "{log}");
        assert!(warnings[0].contains("providers.java") && warnings[0].contains("key=bogus"), "{log}");
    }

    /// The backends check their own sub-maps and report their keys dotted
    /// under the provider, so the user finds the key where they wrote it.
    #[test]
    fn a_backends_unknown_setting_is_reported_dotted() {
        let settings = ConfigMap::new()
            .with(crate::agent::BACKEND_ID, ConfigMap::new().with("bogus", 1_i64))
            .with(crate::jab::BACKEND_ID, ConfigMap::new().with("bogus", 1_i64));
        let (_provider, log) = logged(|| JavaFactory::build(&java_settings(settings)));
        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 2, "{log}");
        assert!(warnings.iter().any(|line| line.contains("key=agent.bogus")), "{log}");
        assert!(warnings.iter().any(|line| line.contains("key=jab.bogus")), "{log}");
        assert!(warnings.iter().all(|line| line.contains("providers.java")), "{log}");
    }

    /// The diagnostic-logging scenario "a string where a flag was expected, Java".
    #[test]
    fn a_string_where_a_flag_was_expected_is_reported_and_the_default_applies() {
        let settings = ConfigMap::new().with(crate::agent::BACKEND_ID, ConfigMap::new().with("enabled", "False"));
        let (provider, log) = logged(|| JavaFactory::build(&java_settings(settings)));
        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 1, "{log}");
        for expected in ["providers.java", "key=agent.enabled", "expected=\"bool\"", "found=\"string\""] {
            assert!(warnings[0].contains(expected), "the warning names `{expected}`: {log}");
        }
        assert_eq!(provider.backends.len(), 2, "the default applies: the agent backend stays on");
    }

    #[test]
    fn a_mistyped_umbrella_setting_is_reported_and_the_default_applies() {
        let settings = ConfigMap::new().with(ENABLED_KEY, "False").with(crate::jab::BACKEND_ID, true);
        let (provider, log) = logged(|| JavaFactory::build(&java_settings(settings)));
        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 2, "{log}");
        assert!(
            warnings.iter().any(|line| line.contains("key=enabled") && line.contains("expected=\"bool\"")),
            "{log}"
        );
        assert!(warnings.iter().any(|line| line.contains("key=jab") && line.contains("expected=\"map\"")), "{log}");
        assert_eq!(provider.backends.len(), 2, "the umbrella stays on by default");
    }

    #[test]
    fn known_settings_report_nothing() {
        let settings = ConfigMap::new()
            .with(ENABLED_KEY, true)
            .with(
                crate::agent::BACKEND_ID,
                ConfigMap::new().with("enabled", true).with("auto_attach", false).with("call_timeout_ms", 750_i64),
            )
            .with(crate::jab::BACKEND_ID, ConfigMap::new().with("enabled", false).with("call_timeout_ms", 500_i64));
        let (_provider, log) = logged(|| JavaFactory::build(&java_settings(settings)));
        assert!(warnings(&log).is_empty(), "{log}");
    }

    /// A missing Access Bridge DLL is said once per process, for all windows
    /// it leaves unserved, with the DLL remedy instead of the `jabswitch`
    /// hint, and without using up the per-window diagnostic.
    #[test]
    fn a_missing_bridge_dll_is_reported_once_per_process() {
        use platynui_core::platform::java::jvm_unreachable_diagnostic_emitted;

        let reported = AtomicBool::new(false);
        let window = |window: u64, pid: u32| UnservedJavaWindow {
            window,
            pid,
            class_name: "SunAwtFrame".into(),
            cause: UnservedCause::BridgeDllNotFound,
        };
        let ((), log) = logged(|| {
            emit_enablement_diagnostics(&[window(0xC700, 41), window(0xC701, 42), window(0xC702, 41)], &reported);
            emit_enablement_diagnostics(&[window(0xC703, 43)], &reported);
        });

        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 1, "{log}");
        for expected in ["providers.java.jab.dll_path", "PLATYNUI_JAB_DLL", "64-bit JDK", "pids=[41, 42]"] {
            assert!(warnings[0].contains(expected), "the warning names `{expected}`: {log}");
        }
        assert!(!warnings[0].contains("jabswitch"), "enabling the bridge does not help without its DLL: {log}");
        assert!(
            !jvm_unreachable_diagnostic_emitted(0xC700),
            "the per-window diagnostic stays for a bridge that is there"
        );
    }

    /// Without the DLL, a Swing window the agent serves is served: no warning
    /// about the bridge, even though the bridge side reports it as unserved.
    #[test]
    fn an_agent_served_window_is_not_reported_without_the_bridge_dll() {
        let ownership = Arc::new(BackendOwnership::default());
        let (agent, _agent_handle) = named_stub("agent", &[0xC800], &[], Some(ownership.view(0)));
        let (bridge, _bridge_handle) =
            stub_with_cause("jab", &[], &[0xC800], UnservedCause::BridgeDllNotFound, Some(ownership.view(1)));
        let provider = JavaProvider::new(vec![agent, bridge], Arc::clone(&ownership), None);

        let (_, log) = logged(|| provider.get_nodes(desktop()).map(Iterator::count));

        assert!(warnings(&log).is_empty(), "{log}");
        provider.shutdown();
    }

    /// Runs `f` and returns what it logged, one line per record.
    fn logged<R>(f: impl FnOnce() -> R) -> (R, String) {
        #[derive(Clone)]
        struct Captured(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Captured {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().expect("log buffer").extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Captured(Arc::clone(&buffer));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .without_time()
            .with_writer(move || writer.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let log = String::from_utf8(buffer.lock().expect("log buffer").clone()).expect("utf-8 log");
        (result, log)
    }
}
