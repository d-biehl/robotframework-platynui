//! Provider factory, configuration, top-level discovery, and diagnostics.

use crate::client::JabClient;
use crate::dll::{DiscoveryInputs, discover_dll};
use crate::error::JabError;
use crate::ffi::VmId;
use crate::handle::JabObject;
use crate::node::{IdScope, JabAppNode, JabNode};
use crate::pump::DegradedTracker;
use platynui_core::config::RuntimeConfig;
use platynui_core::platform::{WindowManager, window_claims};
use platynui_core::provider::{ProviderDescriptor, ProviderError, ProviderKind, UiTreeProvider, UiTreeProviderFactory};
use platynui_core::register_provider;
use platynui_core::ui::{TechnologyId, UiNode};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

pub const PROVIDER_ID: &str = "jab";
pub const PROVIDER_NAME: &str = "Java Access Bridge";

/// Default per-call deadline (`providers.jab.call_timeout_ms`).
const DEFAULT_CALL_TIMEOUT_MS: u64 = 2000;
/// How long the first enumeration after connect waits for the asynchronous
/// bridge rendezvous before reporting "no Java windows". The spike measured
/// 16 ms on a warm machine; the budget is generous because it is paid at most
/// once and only when a Java-looking desktop yields nothing yet.
const FIRST_DISCOVERY_WINDOW: Duration = Duration::from_millis(1500);
const FIRST_DISCOVERY_POLL: Duration = Duration::from_millis(50);

static SELF_PID: LazyLock<u32> = LazyLock::new(std::process::id);

static DESCRIPTOR: LazyLock<ProviderDescriptor> = LazyLock::new(|| {
    ProviderDescriptor::new(
        PROVIDER_ID,
        PROVIDER_NAME,
        TechnologyId::from(crate::node::TECHNOLOGY),
        ProviderKind::Native,
    )
});

pub struct JabFactory;

impl UiTreeProviderFactory for JabFactory {
    fn descriptor(&self) -> &ProviderDescriptor {
        &DESCRIPTOR
    }

    fn create(&self, config: &RuntimeConfig) -> Result<Arc<dyn UiTreeProvider>, ProviderError> {
        Ok(Arc::new(Self::build(config)))
    }
}

impl JabFactory {
    /// Build a concrete provider from `config` — split out from `create` so
    /// the config wiring is unit-testable without a live bridge.
    fn build(config: &RuntimeConfig) -> JabProvider {
        let jab_config = config.provider(PROVIDER_ID);
        let enabled = jab_config.and_then(|jab| jab.get_bool("enabled")).unwrap_or(true);
        let dll_path = jab_config.and_then(|jab| jab.get_str("dll_path")).map(PathBuf::from);
        let call_timeout_ms = jab_config
            .and_then(|jab| jab.get_i64("call_timeout_ms"))
            .and_then(|ms| u64::try_from(ms).ok())
            .filter(|ms| *ms > 0)
            .unwrap_or(DEFAULT_CALL_TIMEOUT_MS);
        JabProvider::new(enabled, dll_path, Duration::from_millis(call_timeout_ms))
    }
}

/// Lazily established bridge connection; `Unavailable` remembers that the one
/// actionable discovery/load diagnostic has already been logged.
enum ClientState {
    Untried,
    Unavailable,
    Ready(Arc<JabClient>),
}

pub struct JabProvider {
    descriptor: &'static ProviderDescriptor,
    enabled: bool,
    dll_path: Option<PathBuf>,
    call_timeout: Duration,
    client: Mutex<ClientState>,
    connected_at: Mutex<Option<Instant>>,
    first_discovery_done: AtomicBool,
    window_manager: Mutex<Option<Arc<dyn WindowManager>>>,
    is_shutdown: AtomicBool,
    degraded: Arc<DegradedTracker>,
    /// HWNDs whose "bridge not enabled" diagnostic has been emitted.
    sunawt_warned: Mutex<HashSet<isize>>,
    /// JVMs whose bridge version has been info-logged.
    version_logged: Mutex<HashSet<VmId>>,
    /// Window claims currently held by this provider instance.
    claimed: Mutex<HashSet<u64>>,
}

impl JabProvider {
    fn new(enabled: bool, dll_path: Option<PathBuf>, call_timeout: Duration) -> Self {
        Self {
            descriptor: &DESCRIPTOR,
            enabled,
            dll_path,
            call_timeout,
            client: Mutex::new(ClientState::Untried),
            connected_at: Mutex::new(None),
            first_discovery_done: AtomicBool::new(false),
            window_manager: Mutex::new(None),
            is_shutdown: AtomicBool::new(false),
            degraded: Arc::new(DegradedTracker::default()),
            sunawt_warned: Mutex::new(HashSet::new()),
            version_logged: Mutex::new(HashSet::new()),
            claimed: Mutex::new(HashSet::new()),
        }
    }

    /// The connected client, establishing the connection on first use. The
    /// kill switch and a missing/unloadable DLL yield `Err` — after logging
    /// exactly one actionable diagnostic — and the provider stays inert.
    fn client(&self) -> Result<Arc<JabClient>, JabError> {
        if self.is_shutdown.load(Ordering::Acquire) {
            return Err(JabError::Shutdown);
        }
        let mut state = self.client.lock().expect("client state mutex poisoned");
        match &*state {
            ClientState::Ready(client) => Ok(Arc::clone(client)),
            ClientState::Unavailable => Err(JabError::ClientUnavailable("previously failed".into())),
            ClientState::Untried => {
                let inputs = DiscoveryInputs::from_environment(self.dll_path.clone());
                let dll = match discover_dll(&inputs) {
                    Ok(dll) => dll,
                    Err(failure) => {
                        warn!("JAB provider inactive: {failure}");
                        *state = ClientState::Unavailable;
                        return Err(JabError::ClientUnavailable(failure.to_string()));
                    }
                };
                match crate::pump::spawn(dll.clone(), Arc::clone(&self.degraded)) {
                    Ok(connection) => {
                        info!(dll = %dll.display(), "JAB client connected");
                        let client =
                            Arc::new(JabClient::new(connection, self.call_timeout, Arc::clone(&self.degraded)));
                        *state = ClientState::Ready(Arc::clone(&client));
                        *self.connected_at.lock().expect("connected_at mutex poisoned") = Some(Instant::now());
                        Ok(client)
                    }
                    Err(message) => {
                        warn!(dll = %dll.display(), "JAB provider inactive: {message}");
                        *state = ClientState::Unavailable;
                        Err(JabError::ClientUnavailable(message))
                    }
                }
            }
        }
    }

    fn window_manager(&self) -> Option<Arc<dyn WindowManager>> {
        self.window_manager.lock().expect("window manager mutex poisoned").clone()
    }

    /// Discover Java top-level windows, waiting briefly for the asynchronous
    /// bridge rendezvous on the very first enumeration after connect.
    fn discover_with_rendezvous_grace(&self, client: &Arc<JabClient>) -> Discovery {
        let mut discovery = discover_java_windows(client, None);
        if !discovery.windows.is_empty() || self.first_discovery_done.swap(true, Ordering::AcqRel) {
            return discovery;
        }
        let connected_at = self.connected_at.lock().expect("connected_at mutex poisoned").unwrap_or_else(Instant::now);
        let deadline = connected_at + FIRST_DISCOVERY_WINDOW;
        while discovery.windows.is_empty() && Instant::now() < deadline {
            std::thread::sleep(FIRST_DISCOVERY_POLL);
            discovery = discover_java_windows(client, None);
        }
        discovery
    }

    /// Warn once per HWND about a Java-looking window the bridge does not
    /// answer for — the "bridge not enabled" case. Never mutates any
    /// target-side configuration; it only tells the user how to.
    fn emit_enablement_diagnostics(&self, suspects: &[SunAwtSuspect]) {
        if suspects.is_empty() {
            return;
        }
        let mut warned = self.sunawt_warned.lock().expect("sunawt mutex poisoned");
        for suspect in suspects {
            if warned.insert(suspect.hwnd) {
                warn!(
                    hwnd = format!("0x{:X}", suspect.hwnd),
                    class = %suspect.class_name,
                    pid = suspect.pid,
                    "Java window found but the Access Bridge does not answer for it. Enable the bridge in the \
                     target JVM: launch it with \
                     -Djavax.accessibility.assistive_technologies=com.sun.java.accessibility.AccessBridge \
                     (per-process), or run 'jabswitch -enable' once for the user (persistent)."
                );
            }
        }
    }

    fn log_bridge_versions(&self, client: &Arc<JabClient>, windows: &[JavaWindow]) {
        let mut logged = self.version_logged.lock().expect("version log mutex poisoned");
        for window in windows {
            if logged.insert(window.vm)
                && let Ok(version) = client.version_info(window.vm)
            {
                info!(
                    vm = window.vm,
                    vm_version = %version.vm_version,
                    win_dll = %version.bridge_win_dll_version,
                    java_dll = %version.bridge_java_dll_version,
                    java_class = %version.bridge_java_class_version,
                    "connected to JVM through the Access Bridge"
                );
            }
        }
    }

    /// Bring the process-wide claims registry in line with the current set of
    /// attached Java windows: claim newcomers, release windows that are gone
    /// (closed, JVM died, or the HWND got recycled).
    fn sync_window_claims(&self, windows: &[JavaWindow]) {
        let current: HashSet<u64> = windows.iter().map(|w| hwnd_as_claim(w.hwnd)).collect();
        let mut claimed = self.claimed.lock().expect("claims mutex poisoned");
        for stale in claimed.difference(&current) {
            window_claims::release_window(*stale, PROVIDER_ID);
        }
        for new in current.difference(&claimed) {
            window_claims::claim_window(*new, PROVIDER_ID);
        }
        *claimed = current;
    }

    fn release_all_claims(&self) {
        let mut claimed = self.claimed.lock().expect("claims mutex poisoned");
        for raw in claimed.drain() {
            window_claims::release_window(raw, PROVIDER_ID);
        }
    }
}

impl UiTreeProvider for JabProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        self.descriptor
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
        info!("JAB provider shutting down");
        self.release_all_claims();
        // Dropping our client reference lets the pump wind down once the last
        // outstanding node releases its clone.
        *self.client.lock().expect("client state mutex poisoned") = ClientState::Unavailable;
    }

    fn get_nodes(
        &self,
        parent: Arc<dyn UiNode>,
    ) -> Result<Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send>, ProviderError> {
        if self.is_shutdown.load(Ordering::Acquire) {
            return Err(JabError::Shutdown.into());
        }
        if !self.enabled {
            return Ok(Box::new(std::iter::empty()));
        }
        // Inert when the DLL is absent: the runtime keeps working, the
        // diagnostic has been logged once by `client()`.
        let Ok(client) = self.client() else {
            return Ok(Box::new(std::iter::empty()));
        };

        let discovery = self.discover_with_rendezvous_grace(&client);
        self.emit_enablement_diagnostics(&discovery.sunawt_suspects);
        self.log_bridge_versions(&client, &discovery.windows);
        self.sync_window_claims(&discovery.windows);

        let window_manager = self.window_manager();
        let mut nodes: Vec<Arc<dyn UiNode>> = Vec::with_capacity(discovery.windows.len() * 2);
        let mut seen_pids: Vec<u32> = Vec::new();
        let mut app_pids: Vec<u32> = Vec::new();
        for window in discovery.windows {
            if !seen_pids.contains(&window.pid) {
                seen_pids.push(window.pid);
                app_pids.push(window.pid);
            }
            nodes.push(JabNode::new_window(
                Arc::clone(&client),
                window_manager.clone(),
                window.vm,
                window.ctx,
                window.hwnd,
                IdScope::Desktop,
                Some(&parent),
            ) as Arc<dyn UiNode>);
        }
        for pid in app_pids {
            nodes.push(JabAppNode::new(pid, Arc::clone(&client), window_manager.clone(), &parent) as Arc<dyn UiNode>);
        }
        Ok(Box::new(nodes.into_iter()))
    }
}

/// One attached Java top-level window.
pub(crate) struct JavaWindow {
    pub hwnd: isize,
    pub pid: u32,
    pub vm: VmId,
    pub ctx: JabObject,
}

/// A top-level window whose class says AWT (`SunAwt*`) but which the bridge
/// does not recognise — the signature of a JVM without the bridge enabled.
pub(crate) struct SunAwtSuspect {
    pub hwnd: isize,
    pub pid: u32,
    pub class_name: String,
}

pub(crate) struct Discovery {
    pub windows: Vec<JavaWindow>,
    pub sunawt_suspects: Vec<SunAwtSuspect>,
}

/// Enumerate visible Java top-level windows via `EnumWindows` +
/// `isJavaWindow` + `GetAccessibleContextFromHWND` (used both for the desktop
/// stream and, PID-filtered, for `app:Application` children).
pub(crate) fn java_windows(client: &Arc<JabClient>, pid_filter: Option<u32>) -> Vec<JavaWindow> {
    discover_java_windows(client, pid_filter).windows
}

fn discover_java_windows(client: &Arc<JabClient>, pid_filter: Option<u32>) -> Discovery {
    let mut windows = Vec::new();
    let mut sunawt_suspects = Vec::new();

    for candidate in enumerate_visible_top_level_windows() {
        if candidate.pid == *SELF_PID {
            continue;
        }
        if let Some(pid) = pid_filter
            && candidate.pid != pid
        {
            continue;
        }
        let is_java = match client.is_java_window(candidate.hwnd) {
            Ok(is_java) => is_java,
            Err(err) => {
                debug!(%err, "isJavaWindow failed; aborting this discovery pass");
                break;
            }
        };
        if !is_java {
            if candidate.class_name.starts_with("SunAwt") {
                sunawt_suspects.push(SunAwtSuspect {
                    hwnd: candidate.hwnd,
                    pid: candidate.pid,
                    class_name: candidate.class_name,
                });
            }
            continue;
        }
        match client.context_from_hwnd(candidate.hwnd) {
            Ok(Some((vm, ctx))) => windows.push(JavaWindow { hwnd: candidate.hwnd, pid: candidate.pid, vm, ctx }),
            Ok(None) => debug!(hwnd = format!("0x{:X}", candidate.hwnd), "Java window without accessible context"),
            Err(err) => debug!(%err, "getAccessibleContextFromHWND failed"),
        }
    }

    Discovery { windows, sunawt_suspects }
}

struct WindowCandidate {
    hwnd: isize,
    pid: u32,
    class_name: String,
}

#[allow(unsafe_code)]
fn enumerate_visible_top_level_windows() -> Vec<WindowCandidate> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible,
    };
    use windows::core::BOOL;

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` carries the `&mut Vec<WindowCandidate>` passed to
        // `EnumWindows` below; all per-window queries are read-only.
        unsafe {
            if IsWindowVisible(hwnd).as_bool() {
                let out = &mut *(lparam.0 as *mut Vec<WindowCandidate>);
                let mut pid = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&raw mut pid));
                let mut class_buffer = [0u16; 256];
                let class_len = GetClassNameW(hwnd, &mut class_buffer);
                let class_name = String::from_utf16_lossy(&class_buffer[..usize::try_from(class_len).unwrap_or(0)]);
                out.push(WindowCandidate { hwnd: hwnd.0 as isize, pid, class_name });
            }
        }
        BOOL(1)
    }

    let mut candidates: Vec<WindowCandidate> = Vec::new();
    // SAFETY: the callback only pushes into the Vec passed through `lparam`.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(std::ptr::addr_of_mut!(candidates) as isize)) };
    candidates
}

fn hwnd_as_claim(hwnd: isize) -> u64 {
    // HWND values are 64-bit handles; the sign-preserving cast keeps the raw
    // bit pattern, matching `WindowId::raw()` semantics.
    #[allow(clippy::cast_sign_loss)]
    {
        hwnd as u64
    }
}

pub static JAB_FACTORY: JabFactory = JabFactory;

// Auto-register the JAB provider when linked (Windows builds only — the whole
// module is `cfg(windows)`).
register_provider!(&JAB_FACTORY);

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::config::ConfigMap;

    #[test]
    fn defaults_without_config() {
        let provider = JabFactory::build(&RuntimeConfig::default());
        assert!(provider.enabled);
        assert!(provider.dll_path.is_none());
        assert_eq!(provider.call_timeout, Duration::from_millis(DEFAULT_CALL_TIMEOUT_MS));
    }

    #[test]
    fn factory_reads_provider_config() {
        let providers = ConfigMap::new().with(
            "jab",
            ConfigMap::new()
                .with("enabled", false)
                .with("dll_path", "C:\\bridge\\WindowsAccessBridge-64.dll")
                .with("call_timeout_ms", 500_i64),
        );
        let config = RuntimeConfig::new(ConfigMap::new(), providers);
        let provider = JabFactory::build(&config);
        assert!(!provider.enabled);
        assert_eq!(provider.dll_path.as_deref(), Some(std::path::Path::new("C:\\bridge\\WindowsAccessBridge-64.dll")));
        assert_eq!(provider.call_timeout, Duration::from_millis(500));
    }

    #[test]
    fn invalid_timeout_falls_back_to_default() {
        let providers = ConfigMap::new().with("jab", ConfigMap::new().with("call_timeout_ms", -1_i64));
        let config = RuntimeConfig::new(ConfigMap::new(), providers);
        let provider = JabFactory::build(&config);
        assert_eq!(provider.call_timeout, Duration::from_millis(DEFAULT_CALL_TIMEOUT_MS));
    }

    #[test]
    fn disabled_provider_yields_no_nodes_without_touching_the_dll() {
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

        let providers = ConfigMap::new().with("jab", ConfigMap::new().with("enabled", false));
        let config = RuntimeConfig::new(ConfigMap::new(), providers);
        let provider = JabFactory::build(&config);
        let parent: Arc<dyn UiNode> = Arc::new(DesktopStub(RuntimeId::from("desktop")));
        let nodes: Vec<_> = provider.get_nodes(parent).expect("inert ok").collect();
        assert!(nodes.is_empty());
        assert!(
            matches!(&*provider.client.lock().expect("state"), ClientState::Untried),
            "kill switch must not load the DLL"
        );
    }

    /// Pins the security constraint: the provider diagnoses a disabled bridge
    /// but never mutates target-side configuration. No source file may call
    /// registry-write APIs, spawn helper processes (`jabswitch`), or touch the
    /// user's accessibility properties file.
    #[test]
    fn no_configuration_mutation_code_paths_exist() {
        // Patterns are assembled at runtime so this test file cannot match itself.
        let forbidden: Vec<String> = ["RegSet", "RegCreate", "Command::", "accessibility", "jabswitch -"]
            .iter()
            .zip(["Value", "Key", "new", ".properties", "enable\""])
            .map(|(a, b)| format!("{a}{b}"))
            .collect();
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(&src_dir).expect("src dir readable") {
            let path = entry.expect("dir entry").path();
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("source readable");
            for pattern in &forbidden {
                assert!(
                    !source.contains(pattern.as_str()),
                    "{} contains the forbidden mutation marker {pattern:?} — the provider must never \
                     write target-side configuration",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn claims_sync_registers_and_releases() {
        let provider = JabFactory::build(&RuntimeConfig::default());
        let (tx, _rx) = std::sync::mpsc::channel();
        let windows = vec![JavaWindow { hwnd: 0xB100, pid: 1, vm: 1, ctx: JabObject::new(1, 1, tx.clone()) }];
        provider.sync_window_claims(&windows);
        assert!(window_claims::is_claimed_by_other(0xB100, "windows-uia"));

        // Window disappears on the next pass → claim released.
        provider.sync_window_claims(&[]);
        assert!(!window_claims::is_claimed(0xB100));

        // Shutdown releases whatever is still held.
        let windows = vec![JavaWindow { hwnd: 0xB101, pid: 1, vm: 1, ctx: JabObject::new(1, 2, tx) }];
        provider.sync_window_claims(&windows);
        assert!(window_claims::is_claimed(0xB101));
        provider.shutdown();
        assert!(!window_claims::is_claimed(0xB101));
    }
}
