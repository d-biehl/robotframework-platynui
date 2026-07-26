//! Provider factory, configuration, backend routing, claims and diagnostics.
//!
//! # Configuration (`providers.java.*`)
//!
//! | Key | Default | Effect |
//! |---|---|---|
//! | `providers.java.enabled` | `true` | Umbrella kill switch: off means no backend is built at all, and Java windows are served by the platform's native provider |
//! | `providers.java.jab.enabled` | `true` | The JAB backend's kill switch; it then loads no DLL and contributes nothing, leaving the umbrella and other backends alone |
//! | `providers.java.jab.dll_path` | discovery | Explicit `WindowsAccessBridge-64.dll` |
//! | `providers.java.jab.call_timeout_ms` | `2000` | Per-call deadline on the JAB pump thread |
//!
//! `providers.java.agent.*` is reserved for the in-JVM agent backend and lands
//! with the `provider-java-swing` change. Each backend reads only its own
//! sub-map; the provider resolves the namespace and hands the sub-map down.
//!
//! The pre-`unify-java-provider` spelling `providers.jab.*` is simply gone —
//! not aliased and not diagnosed. The config layer ignores sections nobody
//! claims, which is the right answer here: there is no released version that
//! ever read those keys.

use crate::backend::JavaBackend;
use crate::jab::JabBackend;
use platynui_core::config::RuntimeConfig;
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

/// Umbrella kill switch (`providers.java.enabled`).
const ENABLED_KEY: &str = "enabled";

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
    fn build(config: &RuntimeConfig) -> JavaProvider {
        let settings = config.provider(PROVIDER_ID);
        let mut backends: Vec<Box<dyn JavaBackend>> = Vec::new();
        // Umbrella off: build nothing at all, so no backend can load anything
        // and Java windows stay with the platform's native provider.
        if settings.and_then(|java| java.get_bool(ENABLED_KEY)).unwrap_or(true) {
            backends.push(Box::new(JabBackend::from_config(
                settings.and_then(|java| java.get_map(crate::jab::BACKEND_ID)),
            )));
        }
        debug!(backends = ?backends.iter().map(|backend| backend.id()).collect::<Vec<_>>(), "Java provider built");
        JavaProvider::new(backends)
    }
}

pub struct JavaProvider {
    descriptor: &'static ProviderDescriptor,
    backends: Vec<Box<dyn JavaBackend>>,
    is_shutdown: AtomicBool,
    /// Window claims currently held by this provider instance.
    claimed: Mutex<HashSet<u64>>,
}

impl JavaProvider {
    fn new(backends: Vec<Box<dyn JavaBackend>>) -> Self {
        Self {
            descriptor: &DESCRIPTOR,
            backends,
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

        let mut nodes: Vec<Arc<dyn UiNode>> = Vec::new();
        let mut served: HashSet<u64> = HashSet::new();
        let mut unserved = Vec::new();
        // Every backend's nodes go through as-is: with a single backend there
        // is nothing to arbitrate. Selecting between two backends over the same
        // window is `provider-java-swing` task 3.1, and it needs a richer
        // `Enumeration` than a flat node list — the constraint is written down
        // there, next to the agent-presence signal that drives the selection.
        for backend in &self.backends {
            let mut pass = backend.enumerate(&parent);
            served.extend(pass.served_windows);
            nodes.append(&mut pass.nodes);
            unserved.append(&mut pass.unserved);
        }

        self.sync_window_claims(&served);
        // A Java-looking window is only worth a diagnostic once it is clear
        // that *no* backend reaches it — with one backend that is every window
        // it reported, but the rule is the router's, not the backend's.
        emit_enablement_diagnostics(&unserved, &served);

        Ok(Box::new(nodes.into_iter()))
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

/// Emit the shared "JVM window absent from native accessibility" diagnostic
/// (see `platynui_core::platform::java`) for the Java-looking windows no
/// backend serves — on Windows the "bridge not enabled" case. The shared
/// registry de-duplicates per window, process-wide. Never mutates any
/// target-side configuration; it only tells the user how to.
fn emit_enablement_diagnostics(unserved: &[crate::backend::UnservedJavaWindow], served: &HashSet<u64>) {
    use platynui_core::platform::java::{JavaToolkit, jvm_unreachable_diagnostic_once};
    for window in unserved {
        if served.contains(&window.window) {
            continue;
        }
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
    /// so a test can change what the next enumeration pass finds.
    struct StubBackend {
        served: Arc<Mutex<Vec<u64>>>,
        unserved: Vec<u64>,
        shut_down: Arc<AtomicBool>,
    }

    struct StubHandle {
        served: Arc<Mutex<Vec<u64>>>,
        shut_down: Arc<AtomicBool>,
    }

    fn stub(served: &[u64], unserved: &[u64]) -> (Box<dyn JavaBackend>, StubHandle) {
        let backend = StubBackend {
            served: Arc::new(Mutex::new(served.to_vec())),
            unserved: unserved.to_vec(),
            shut_down: Arc::new(AtomicBool::new(false)),
        };
        let handle = StubHandle { served: Arc::clone(&backend.served), shut_down: Arc::clone(&backend.shut_down) };
        (Box::new(backend), handle)
    }

    impl JavaBackend for StubBackend {
        fn id(&self) -> &'static str {
            "stub"
        }
        fn enumerate(&self, _parent: &Arc<dyn UiNode>) -> Enumeration {
            Enumeration {
                served_windows: self.served.lock().expect("served set mutex poisoned").clone(),
                nodes: Vec::new(),
                unserved: self
                    .unserved
                    .iter()
                    .map(|window| UnservedJavaWindow { window: *window, pid: 1, class_name: "SunAwtFrame".into() })
                    .collect(),
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

    #[test]
    fn defaults_register_the_jab_backend() {
        let provider = JavaFactory::build(&RuntimeConfig::default());
        let ids: Vec<_> = provider.backends.iter().map(|backend| backend.id()).collect();
        assert_eq!(ids, vec![crate::jab::BACKEND_ID]);
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
        // The umbrella resolves `providers.java.jab.*`; the backend below is
        // built with its kill switch off and must stay inert without failing.
        let providers = ConfigMap::new()
            .with(PROVIDER_ID, ConfigMap::new().with(crate::jab::BACKEND_ID, ConfigMap::new().with("enabled", false)));
        let provider = JavaFactory::build(&RuntimeConfig::new(ConfigMap::new(), providers));
        assert_eq!(provider.backends.len(), 1, "a disabled backend is still built, just inert");
        let nodes: Vec<_> = provider.get_nodes(desktop()).expect("inert ok").collect();
        assert!(nodes.is_empty());
    }

    #[test]
    fn claims_follow_what_the_backends_serve() {
        // The registry is process-global, so every test uses its own windows.
        let (backend, handle) = stub(&[0xC100], &[]);
        let provider = JavaProvider::new(vec![backend]);

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
        let provider = JavaProvider::new(vec![backend]);
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
        let provider = JavaProvider::new(vec![backend]);
        let _ = provider.get_nodes(desktop()).expect("enumerate");
        assert!(!window_claims::is_claimed(0xC300));
        assert!(jvm_unreachable_diagnostic_emitted(0xC300), "an unreachable JVM window must say how to fix it");
    }

    #[test]
    fn hit_test_falls_through_when_every_backend_abstains() {
        let (backend, _handle) = stub(&[], &[]);
        let provider = JavaProvider::new(vec![backend]);
        let answer = provider.element_at_point(Point::new(10.0, 10.0));
        assert!(matches!(answer, Err(ProviderError::UnsupportedOperation { .. })));
    }
}
