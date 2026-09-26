//! Backend configuration, top-level discovery, and hit-testing.
//!
//! The JAB code is consumed as a backend of the single Java provider
//! (`platynui-provider-java`). That crate owns registration, the window claims
//! and the enablement diagnostic; what lives here is the per-window surface it
//! routes to.

use crate::client::JabClient;
use crate::dll::{DiscoveryInputs, discover_dll};
use crate::error::JabError;
use crate::ffi::VmId;
use crate::handle::JabObject;
use crate::node::{IdScope, JabAppNode, JabNode};
use crate::pump::DegradedTracker;
use platynui_core::config::{ConfigMap, ConfigTypeMismatch, ENABLED_KEY};
use platynui_core::platform::WindowManager;
use platynui_core::provider::ProviderError;
use platynui_core::types::Point;
use platynui_core::ui::UiNode;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// The backend's id: its settings live in `providers.java.<BACKEND_ID>.*`, the
/// Java provider resolves the namespace and hands the sub-map to
/// [`JabProvider::from_config`].
pub const BACKEND_ID: &str = "jab";

/// Default per-call deadline (`providers.java.jab.call_timeout_ms`).
const DEFAULT_CALL_TIMEOUT_MS: u64 = 2000;
/// `providers.java.jab.dll_path`: an explicit `WindowsAccessBridge-64.dll`.
const DLL_PATH_KEY: &str = "dll_path";
/// `providers.java.jab.call_timeout_ms`: the per-call deadline.
const CALL_TIMEOUT_MS_KEY: &str = "call_timeout_ms";
/// The settings the backend reads; [`ENABLED_KEY`] is always known.
const KNOWN_SETTINGS: &[&str] = &[DLL_PATH_KEY, CALL_TIMEOUT_MS_KEY];
/// The component the backend's settings are reported under. They are a nested
/// map of the Java provider, so their keys are reported as `jab.<key>`.
const COMPONENT: &str = "providers.java";
/// How long the first enumeration after connect waits for the asynchronous
/// bridge rendezvous before reporting "no Java windows". The spike measured
/// 16 ms on a warm machine; the budget is generous because it is paid at most
/// once and only when a Java-looking desktop yields nothing yet.
const FIRST_DISCOVERY_WINDOW: Duration = Duration::from_millis(1500);
const FIRST_DISCOVERY_POLL: Duration = Duration::from_millis(50);

static SELF_PID: LazyLock<u32> = LazyLock::new(std::process::id);

/// One enumeration pass of the backend.
#[derive(Default)]
pub struct JabEnumeration {
    /// Native handles of the Java top-level windows this backend serves — what
    /// the Java provider turns into window claims.
    pub served_windows: Vec<u64>,
    /// Window and `app:Application` nodes to attach under the enumerated parent.
    pub nodes: Vec<Arc<dyn UiNode>>,
    /// Java-looking windows the bridge does not answer for; the Java provider
    /// owns what to say about them.
    pub unserved: Vec<UnservedWindow>,
    /// Processes behind every Java window this pass saw, served or not — what a
    /// stronger backend needs in order to offer itself to those JVMs.
    pub java_processes: Vec<u32>,
}

/// A visible top-level window whose class says AWT (`SunAwt*`) but which this
/// backend cannot serve. Reported outward rather than diagnosed here: only the
/// Java provider knows whether another backend serves the window.
pub struct UnservedWindow {
    /// Native window handle, in the raw form the claims and diagnostic
    /// registries key on.
    pub window: u64,
    pub pid: u32,
    /// Platform window class — the toolkit discriminator.
    pub class_name: String,
    /// Why the backend cannot serve it; the remedies differ.
    pub cause: UnservedCause,
}

/// Why the backend cannot serve a Java-looking window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnservedCause {
    /// The bridge is loaded but does not recognise the window: the signature
    /// of a JVM without the Access Bridge enabled.
    BridgeNotEnabled,
    /// The Access Bridge client DLL was not found, so no window can be served
    /// through the bridge at all.
    DllNotFound,
}

impl std::fmt::Display for UnservedCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::BridgeNotEnabled => "Access Bridge not enabled in the JVM",
            Self::DllNotFound => "Access Bridge DLL not found",
        })
    }
}

/// Native windows this backend must leave alone because something else serves
/// them.
///
/// One Swing window is reachable through more than one Java channel — the
/// bridge sees it, and so does an in-JVM agent — and only the Java provider
/// knows which one is serving it. It therefore installs a hook here instead of
/// filtering this backend's results: an `app:Application` node enumerates its
/// windows *lazily*, long after the pass that produced it, so a set handed to
/// that pass would be both stale and out of reach by the time it mattered.
///
/// Nothing installed means "serve everything you can reach", which is the
/// single-backend case and the historical behaviour.
pub trait WindowExclusions: Send + Sync {
    /// Whether `window` (a raw native handle) is served elsewhere.
    fn excludes(&self, window: u64) -> bool;
}

/// Lazily established bridge connection; `DllNotFound` and `Unavailable`
/// remember that discovery or loading has been tried and recorded already.
enum ClientState {
    Untried,
    /// No client DLL was found. Kept apart from `Unavailable`: without a DLL
    /// the backend still reports the AWT windows it sees, so that the Java
    /// provider can say why nobody serves them and offer their JVMs the agent.
    DllNotFound,
    Unavailable,
    Ready(Arc<JabClient>),
}

pub struct JabProvider {
    enabled: bool,
    dll_path: Option<PathBuf>,
    call_timeout: Duration,
    /// Windows a stronger channel serves; see [`WindowExclusions`].
    exclusions: Option<Arc<dyn WindowExclusions>>,
    client: Mutex<ClientState>,
    connected_at: Mutex<Option<Instant>>,
    first_discovery_done: AtomicBool,
    window_manager: Mutex<Option<Arc<dyn WindowManager>>>,
    is_shutdown: AtomicBool,
    degraded: Arc<DegradedTracker>,
    /// JVMs whose bridge version has been info-logged.
    version_logged: Mutex<HashSet<VmId>>,
}

impl JabProvider {
    /// Build the backend from its own settings sub-map (`providers.java.jab.*`)
    /// — split out so the config wiring is unit-testable without a live bridge.
    ///
    /// `exclusions` is the Java provider's answer to "another backend serves
    /// this window"; `None` means this backend is alone with the Java windows it
    /// can reach.
    ///
    /// Checks the settings first: an unknown key and a value of the wrong type
    /// are warned about as `jab.<key>` of `providers.java`, and the default
    /// applies.
    #[must_use]
    pub fn from_config(settings: Option<&ConfigMap>, exclusions: Option<Arc<dyn WindowExclusions>>) -> Self {
        let Some(settings) = settings else {
            return Self::new(true, None, Duration::from_millis(DEFAULT_CALL_TIMEOUT_MS), exclusions);
        };
        for key in settings.unknown_keys(KNOWN_SETTINGS) {
            warn!(component = COMPONENT, key = %format_args!("{BACKEND_ID}.{key}"), "unknown setting; it is ignored");
        }
        let enabled = setting(settings.try_bool(ENABLED_KEY)).unwrap_or(true);
        let dll_path = setting(settings.try_str(DLL_PATH_KEY)).map(PathBuf::from);
        let call_timeout_ms = setting(settings.try_i64(CALL_TIMEOUT_MS_KEY))
            .and_then(|ms| u64::try_from(ms).ok())
            .filter(|ms| *ms > 0)
            .unwrap_or(DEFAULT_CALL_TIMEOUT_MS);
        Self::new(enabled, dll_path, Duration::from_millis(call_timeout_ms), exclusions)
    }

    fn new(
        enabled: bool,
        dll_path: Option<PathBuf>,
        call_timeout: Duration,
        exclusions: Option<Arc<dyn WindowExclusions>>,
    ) -> Self {
        Self {
            enabled,
            dll_path,
            call_timeout,
            exclusions,
            client: Mutex::new(ClientState::Untried),
            connected_at: Mutex::new(None),
            first_discovery_done: AtomicBool::new(false),
            window_manager: Mutex::new(None),
            is_shutdown: AtomicBool::new(false),
            degraded: Arc::new(DegradedTracker::default()),
            version_logged: Mutex::new(HashSet::new()),
        }
    }

    /// The connected client, establishing the connection on first use. A
    /// missing or unloadable DLL yields `Err`, and the backend serves nothing.
    ///
    /// A missing DLL is recorded at debug only: most Windows machines have no
    /// JDK, and nothing is lost until a Swing or AWT window turns up that no
    /// other backend serves — which the Java provider reports, from the
    /// windows [`Self::enumerate`] lists as unserved.
    fn client(&self) -> Result<Arc<JabClient>, JabError> {
        if self.is_shutdown.load(Ordering::Acquire) {
            return Err(JabError::Shutdown);
        }
        let mut state = self.client.lock().expect("client state mutex poisoned");
        match &*state {
            ClientState::Ready(client) => Ok(Arc::clone(client)),
            ClientState::DllNotFound => Err(JabError::ClientUnavailable(UnservedCause::DllNotFound.to_string())),
            ClientState::Unavailable => Err(JabError::ClientUnavailable("previously failed".into())),
            ClientState::Untried => {
                let inputs = DiscoveryInputs::from_environment(self.dll_path.clone());
                let dll = match discover_dll(&inputs) {
                    Ok(dll) => dll,
                    Err(failure) => {
                        debug!(tried = ?failure.tried, "Access Bridge DLL not found; the JAB backend serves no window");
                        *state = ClientState::DllNotFound;
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
                        warn!(dll = %dll.display(), "JAB backend inactive: {message}");
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
        let mut discovery = discover_java_windows(client, None, self.exclusions.as_deref());
        if !discovery.windows.is_empty() || self.first_discovery_done.swap(true, Ordering::AcqRel) {
            return discovery;
        }
        let connected_at = self.connected_at.lock().expect("connected_at mutex poisoned").unwrap_or_else(Instant::now);
        let deadline = connected_at + FIRST_DISCOVERY_WINDOW;
        while discovery.windows.is_empty() && Instant::now() < deadline {
            std::thread::sleep(FIRST_DISCOVERY_POLL);
            discovery = discover_java_windows(client, None, self.exclusions.as_deref());
        }
        discovery
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

    /// Inject the runtime's window manager, once.
    ///
    /// # Panics
    ///
    /// If another thread panicked while holding the window-manager slot.
    pub fn set_window_manager(&self, window_manager: Arc<dyn WindowManager>) {
        let mut slot = self.window_manager.lock().expect("window manager mutex poisoned");
        if slot.is_none() {
            *slot = Some(window_manager);
        }
    }

    /// Release the bridge connection. Idempotent.
    ///
    /// # Panics
    ///
    /// If another thread panicked while holding the client state.
    pub fn shutdown(&self) {
        if self.is_shutdown.swap(true, Ordering::AcqRel) {
            return;
        }
        info!("JAB backend shutting down");
        // Dropping our client reference lets the pump wind down once the last
        // outstanding node releases its clone.
        *self.client.lock().expect("client state mutex poisoned") = ClientState::Unavailable;
    }

    /// One enumeration pass under `parent`: the Java windows this backend can
    /// serve, the nodes for them, and the Java-looking windows it cannot serve.
    ///
    /// Inert — an empty pass, no failure — when the backend is disabled, shut
    /// down, or the bridge cannot be loaded. Without a client DLL it serves
    /// nothing either, but still reports the AWT windows it sees as unserved,
    /// with [`UnservedCause::DllNotFound`] and their processes.
    #[must_use]
    pub fn enumerate(&self, parent: &Arc<dyn UiNode>) -> JabEnumeration {
        if self.is_shutdown.load(Ordering::Acquire) || !self.enabled {
            return JabEnumeration::default();
        }
        let Ok(client) = self.client() else {
            if self.dll_not_found() {
                return self.unserved_without_bridge();
            }
            return JabEnumeration::default();
        };

        let discovery = self.discover_with_rendezvous_grace(&client);
        self.log_bridge_versions(&client, &discovery.windows);

        let window_manager = self.window_manager();
        let mut served_windows: Vec<u64> = Vec::with_capacity(discovery.windows.len());
        let mut nodes: Vec<Arc<dyn UiNode>> = Vec::with_capacity(discovery.windows.len() * 2);
        // One list, used twice: as the `app:Application` nodes to emit and as the
        // processes reported outward.
        let mut seen_pids: Vec<u32> = Vec::new();
        for window in discovery.windows {
            if !seen_pids.contains(&window.pid) {
                seen_pids.push(window.pid);
            }
            served_windows.push(hwnd_as_claim(window.hwnd));
            nodes.push(JabNode::new_window(
                Arc::clone(&client),
                window_manager.clone(),
                window.vm,
                window.ctx,
                window.hwnd,
                IdScope::Desktop,
                Some(parent),
            ) as Arc<dyn UiNode>);
        }
        for pid in seen_pids.iter().copied() {
            nodes.push(JabAppNode::new(
                pid,
                Arc::clone(&client),
                window_manager.clone(),
                self.exclusions.clone(),
                parent,
            ) as Arc<dyn UiNode>);
        }
        let mut java_processes = seen_pids;
        for suspect in &discovery.sunawt_suspects {
            if !java_processes.contains(&suspect.pid) {
                java_processes.push(suspect.pid);
            }
        }
        JabEnumeration { served_windows, nodes, unserved: discovery.sunawt_suspects, java_processes }
    }

    fn dll_not_found(&self) -> bool {
        matches!(*self.client.lock().expect("client state mutex poisoned"), ClientState::DllNotFound)
    }

    /// The pass of a backend without a client DLL: no nodes and no served
    /// windows, but the visible `SunAwt*` windows that no stronger backend
    /// serves, as unserved with [`UnservedCause::DllNotFound`], and their
    /// processes. Window enumeration and class names only, no bridge call.
    ///
    /// Reporting them is what keeps a machine without a JDK usable: the Java
    /// provider offers those JVMs the agent, and says why a window stays
    /// unserved only when no backend serves it.
    fn unserved_without_bridge(&self) -> JabEnumeration {
        awt_windows_without_bridge(enumerate_visible_top_level_windows(), self.exclusions.as_deref())
    }

    /// Point-based hit-test of Java windows (design decisions 1–3 and 5 of
    /// `add-jab-hit-test`).
    ///
    /// Gates on the top-level window under the point first: only for a Java
    /// window (`isJavaWindow`) that is not the host process's own does the
    /// bridge's native hit-test (`getAccessibleContextAt`) run; every other
    /// point reports `UnsupportedOperation` so the remaining providers handle
    /// it. All bridge calls run on the pump thread under the per-call
    /// deadline, so an unresponsive JVM surfaces as a prompt provider error
    /// for that point, never a hang.
    ///
    /// # Errors
    ///
    /// `UnsupportedOperation` when the point is not this backend's to answer
    /// (no window, own process, not a Java window, bridge unavailable, backend
    /// off), and the bridge's own error when a call against the JVM fails.
    pub fn element_at_point(&self, point: Point) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
        if self.is_shutdown.load(Ordering::Acquire) || !self.enabled {
            return Err(unsupported_at_point("backend disabled or shut down"));
        }
        let Some((hwnd, pid)) = top_level_window_at(point) else {
            return Err(unsupported_at_point("no window at point"));
        };
        // Never resolve the host process's own UI (the Inspector must not
        // pick itself); own-process windows are never Java windows anyway.
        if pid == *SELF_PID {
            return Err(unsupported_at_point("own-process window"));
        }
        // A window a stronger channel serves stays that channel's, hit-test
        // included: answering here would hand the picker a node whose shape does
        // not match the tree the same window shows.
        if self.exclusions.as_ref().is_some_and(|excluded| excluded.excludes(hwnd_as_claim(hwnd))) {
            return Err(unsupported_at_point("window is served by another Java backend"));
        }
        let Ok(client) = self.client() else {
            return Err(unsupported_at_point("JAB client unavailable"));
        };
        if !client.is_java_window(hwnd)? {
            return Err(unsupported_at_point("not a Java window"));
        }
        // From here on the window is ours: `Ok(None)` (not `Err`) when no
        // context resolves, so the runtime does not fall through to another
        // provider's representation of a Java window.
        let Some((vm, window_ctx)) = client.context_from_hwnd(hwnd)? else {
            return Ok(None);
        };
        // Both `WindowFromPoint` and `getAccessibleContextAt` take desktop
        // pixels; the point passes through unchanged (design decision 2).
        #[expect(clippy::cast_possible_truncation, reason = "desktop coordinates fit in i32")]
        let (x, y) = (point.x().round() as i32, point.y().round() as i32);
        let hit = match client.context_at(&window_ctx, x, y)? {
            Some(hit) => Some(hit),
            // The JDK's native hit-test answers null for every point until the
            // target JVM has seen a mouse event (see `geometric_hit`); descend
            // geometrically instead of reporting a miss.
            None => crate::node::geometric_hit(&client, &window_ctx, hwnd, point),
        };
        let hit = match hit {
            Some(hit) => hit,
            // Over the Java window but outside every child (frame/title-bar
            // area): the window itself is the hit. Take a fresh bridge
            // reference so ownership stays one release per handle.
            None => match client.context_from_hwnd(hwnd)? {
                Some((_, window_hit)) => window_hit,
                None => return Ok(None),
            },
        };
        Ok(Some(crate::node::hit_test_node(
            &client,
            self.window_manager(),
            vm,
            window_ctx,
            hwnd,
            pid,
            hit,
            self.exclusions.clone(),
        )))
    }
}

fn unsupported_at_point(details: &str) -> ProviderError {
    ProviderError::UnsupportedOperation { operation: "element_at_point", details: Some(details.into()) }
}

/// The value of a setting, or `None` after a warning when it has the wrong
/// type, so that the caller applies the default.
fn setting<T>(read: Result<Option<T>, ConfigTypeMismatch>) -> Option<T> {
    read.unwrap_or_else(|mismatch| {
        warn!(
            component = COMPONENT,
            key = %format_args!("{BACKEND_ID}.{}", mismatch.key),
            expected = mismatch.expected,
            found = mismatch.found,
            "setting has the wrong type; the default applies"
        );
        None
    })
}

/// The pass of [`JabProvider::unserved_without_bridge`] over the given
/// top-level windows, split out so that it is testable without a desktop.
fn awt_windows_without_bridge(
    candidates: Vec<WindowCandidate>,
    exclusions: Option<&dyn WindowExclusions>,
) -> JabEnumeration {
    let mut pass = JabEnumeration::default();
    for candidate in candidates {
        if candidate.pid == *SELF_PID || !candidate.class_name.starts_with("SunAwt") {
            continue;
        }
        let window = hwnd_as_claim(candidate.hwnd);
        // A window a stronger backend serves is served: not this backend's to
        // report, and its JVM already carries what attaching would bring.
        if exclusions.is_some_and(|excluded| excluded.excludes(window)) {
            continue;
        }
        if !pass.java_processes.contains(&candidate.pid) {
            pass.java_processes.push(candidate.pid);
        }
        pass.unserved.push(UnservedWindow {
            window,
            pid: candidate.pid,
            class_name: candidate.class_name,
            cause: UnservedCause::DllNotFound,
        });
    }
    pass
}

/// Top-level window under `point` (`WindowFromPoint` → `GetAncestor(GA_ROOT)`)
/// and its owning process id.
#[allow(unsafe_code)]
fn top_level_window_at(point: Point) -> Option<(isize, u32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, GetWindowThreadProcessId, WindowFromPoint};

    #[expect(clippy::cast_possible_truncation, reason = "desktop coordinates fit in i32")]
    let pt = POINT { x: point.x().round() as i32, y: point.y().round() as i32 };
    // SAFETY: read-only point/window queries with valid out-parameters.
    unsafe {
        let hwnd = WindowFromPoint(pt);
        if hwnd.is_invalid() {
            return None;
        }
        let root = GetAncestor(hwnd, GA_ROOT);
        let top_level = if root.is_invalid() { hwnd } else { root };
        let mut pid = 0u32;
        GetWindowThreadProcessId(top_level, Some(&raw mut pid));
        Some((top_level.0 as isize, pid))
    }
}

/// One attached Java top-level window.
pub(crate) struct JavaWindow {
    pub hwnd: isize,
    pub pid: u32,
    pub vm: VmId,
    pub ctx: JabObject,
}

pub(crate) struct Discovery {
    pub windows: Vec<JavaWindow>,
    pub sunawt_suspects: Vec<UnservedWindow>,
}

/// Enumerate visible Java top-level windows via `EnumWindows` +
/// `isJavaWindow` + `GetAccessibleContextFromHWND` (used both for the desktop
/// stream and, PID-filtered, for `app:Application` children).
pub(crate) fn java_windows(
    client: &Arc<JabClient>,
    pid_filter: Option<u32>,
    exclusions: Option<&dyn WindowExclusions>,
) -> Vec<JavaWindow> {
    discover_java_windows(client, pid_filter, exclusions).windows
}

fn discover_java_windows(
    client: &Arc<JabClient>,
    pid_filter: Option<u32>,
    exclusions: Option<&dyn WindowExclusions>,
) -> Discovery {
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
        // Before the bridge is asked anything: a window another backend serves
        // is not this backend's to serve, and it is not an unserved-JVM
        // diagnostic either — somebody *is* serving it.
        if exclusions.is_some_and(|excluded| excluded.excludes(hwnd_as_claim(candidate.hwnd))) {
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
                sunawt_suspects.push(UnservedWindow {
                    window: hwnd_as_claim(candidate.hwnd),
                    pid: candidate.pid,
                    class_name: candidate.class_name,
                    cause: UnservedCause::BridgeNotEnabled,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_without_config() {
        let provider = JabProvider::from_config(None, None);
        assert!(provider.enabled);
        assert!(provider.dll_path.is_none());
        assert_eq!(provider.call_timeout, Duration::from_millis(DEFAULT_CALL_TIMEOUT_MS));
        assert!(provider.exclusions.is_none(), "alone with the Java windows unless told otherwise");
    }

    #[test]
    fn backend_reads_its_settings() {
        let settings = ConfigMap::new()
            .with("enabled", false)
            .with("dll_path", "C:\\bridge\\WindowsAccessBridge-64.dll")
            .with("call_timeout_ms", 500_i64);
        let provider = JabProvider::from_config(Some(&settings), None);
        assert!(!provider.enabled);
        assert_eq!(provider.dll_path.as_deref(), Some(std::path::Path::new("C:\\bridge\\WindowsAccessBridge-64.dll")));
        assert_eq!(provider.call_timeout, Duration::from_millis(500));
    }

    #[test]
    fn invalid_timeout_falls_back_to_default() {
        let settings = ConfigMap::new().with("call_timeout_ms", -1_i64);
        let provider = JabProvider::from_config(Some(&settings), None);
        assert_eq!(provider.call_timeout, Duration::from_millis(DEFAULT_CALL_TIMEOUT_MS));
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

    fn warnings(log: &str) -> Vec<&str> {
        log.lines().filter(|line| line.contains(" WARN ")).collect()
    }

    #[test]
    fn an_unknown_setting_is_reported_once_under_the_java_provider() {
        let settings = ConfigMap::new().with("bogus", 1_i64);
        let (_provider, log) = logged(|| JabProvider::from_config(Some(&settings), None));

        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 1, "{log}");
        assert!(warnings[0].contains("providers.java") && warnings[0].contains("key=jab.bogus"), "{log}");
    }

    #[test]
    fn a_mistyped_setting_is_reported_and_the_default_applies() {
        let settings = ConfigMap::new().with("enabled", "False").with("call_timeout_ms", "500");
        let (provider, log) = logged(|| JabProvider::from_config(Some(&settings), None));

        assert!(provider.enabled, "the default applies, not the string's meaning");
        assert_eq!(provider.call_timeout, Duration::from_millis(DEFAULT_CALL_TIMEOUT_MS));
        let warnings = warnings(&log);
        assert_eq!(warnings.len(), 2, "{log}");
        for expected in ["providers.java", "key=jab.enabled", "expected=\"bool\"", "found=\"string\""] {
            assert!(warnings.iter().any(|line| line.contains(expected)), "a warning names `{expected}`: {log}");
        }
        assert!(warnings.iter().any(|line| line.contains("key=jab.call_timeout_ms")), "{log}");
    }

    #[test]
    fn known_settings_report_nothing() {
        let settings = ConfigMap::new()
            .with("enabled", true)
            .with("dll_path", "C:\\bridge\\WindowsAccessBridge-64.dll")
            .with("call_timeout_ms", 500_i64);
        let (_provider, log) = logged(|| JabProvider::from_config(Some(&settings), None));
        assert!(warnings(&log).is_empty(), "{log}");
    }

    /// Without the DLL nothing is served, but the AWT windows are still
    /// reported, so the Java provider can offer their JVMs the agent and say
    /// why a window stays unserved.
    #[test]
    fn without_a_dll_awt_windows_are_reported_unserved_with_their_processes() {
        struct ExcludeOne(u64);
        impl WindowExclusions for ExcludeOne {
            fn excludes(&self, window: u64) -> bool {
                window == self.0
            }
        }
        let candidate =
            |hwnd: isize, pid: u32, class_name: &str| WindowCandidate { hwnd, pid, class_name: class_name.into() };
        let candidates = vec![
            candidate(0x10, 4100, "SunAwtFrame"),
            candidate(0x11, 4100, "SunAwtDialog"),
            candidate(0x20, 4200, "Notepad"),
            candidate(0x30, *SELF_PID, "SunAwtFrame"),
            candidate(0x40, 4300, "SunAwtFrame"),
        ];

        let pass = awt_windows_without_bridge(candidates, Some(&ExcludeOne(0x40)));

        assert!(pass.nodes.is_empty() && pass.served_windows.is_empty(), "without a DLL nothing is served");
        let windows: Vec<u64> = pass.unserved.iter().map(|window| window.window).collect();
        assert_eq!(windows, [0x10, 0x11], "AWT windows only; not the host's own, not one a stronger backend serves");
        assert!(pass.unserved.iter().all(|window| window.cause == UnservedCause::DllNotFound));
        assert_eq!(pass.java_processes, [4100], "each process once, so that the agent can be offered to it");
        assert_eq!(UnservedCause::DllNotFound.to_string(), "Access Bridge DLL not found");
    }

    /// A missing DLL is nothing to warn about by itself: the enumeration serves
    /// nothing and reports what it sees, and warning is the Java provider's call.
    #[test]
    fn a_backend_without_a_dll_serves_nothing_and_warns_about_nothing() {
        let provider = JabProvider::from_config(None, None);
        *provider.client.lock().expect("state") = ClientState::DllNotFound;
        let parent: Arc<dyn UiNode> = Arc::new(desktop_stub());

        let (pass, log) = logged(|| provider.enumerate(&parent));

        assert!(pass.nodes.is_empty() && pass.served_windows.is_empty());
        assert!(pass.unserved.iter().all(|window| window.cause == UnservedCause::DllNotFound));
        assert!(warnings(&log).is_empty(), "{log}");
        assert!(matches!(provider.client(), Err(JabError::ClientUnavailable(_))));
    }

    fn desktop_stub() -> impl UiNode {
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
        DesktopStub(RuntimeId::from("desktop"))
    }

    /// The exclusion hook has to reach every place a window can enter the tree.
    /// Discovery and the lazy `app:Application` children are separate code paths
    /// (the second runs long after the pass that produced the node), and a hook
    /// wired into only one of them would let an agent-served window reappear
    /// under the application node.
    #[test]
    fn the_exclusion_hook_reaches_both_window_paths() {
        struct ExcludeAll;
        impl WindowExclusions for ExcludeAll {
            fn excludes(&self, _window: u64) -> bool {
                true
            }
        }

        let provider = JabProvider::from_config(None, Some(Arc::new(ExcludeAll)));
        assert!(provider.exclusions.is_some());
        // Both window-producing paths take the hook as an argument, so this is a
        // compile-time property; assert on the source rather than on a live
        // bridge, which a unit test has no way to stand up.
        let source = include_str!("node.rs");
        assert!(
            source.contains("java_windows(&self.client, Some(self.pid), self.exclusions.as_deref())"),
            "the app node's lazy children must consult the hook, not a snapshot"
        );
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

        let settings = ConfigMap::new().with("enabled", false);
        let provider = JabProvider::from_config(Some(&settings), None);
        let parent: Arc<dyn UiNode> = Arc::new(DesktopStub(RuntimeId::from("desktop")));
        let pass = provider.enumerate(&parent);
        assert!(pass.nodes.is_empty());
        assert!(pass.served_windows.is_empty());
        assert!(
            matches!(&*provider.client.lock().expect("state"), ClientState::Untried),
            "kill switch must not load the DLL"
        );
    }

    /// Pins the security constraint: the backend reports a disabled bridge but
    /// never mutates target-side configuration. No source file may call
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
}
