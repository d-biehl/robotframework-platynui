//! The dedicated JAB pump thread.
//!
//! One thread owns everything JAB: it loads the client DLL, binds the exports,
//! calls `Windows_run()` — which creates the hidden rendezvous window **on the
//! calling thread** — pumps that thread's Win32 message queue (JVM discovery
//! and every callback arrive as window messages there), and services API
//! requests sent by [`crate::client::JabClient`]. No JAB function is ever
//! called from another thread.
//!
//! Every bridge call is synchronous blocking IPC into the target JVM, so a
//! hung JVM blocks this thread until the OS call returns; callers time out
//! promptly via their reply channels and the degraded-vmID tracker keeps new
//! work away from the stuck JVM (see `client.rs`). Handle releases arrive on a
//! separate queue and are drained between requests; releases aimed at a
//! degraded JVM are deferred so the release itself cannot wedge the pump.

use crate::dll::Bridge;
use crate::ffi::{JObject64, VmId};
use crate::handle::ReleaseSender;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Consecutive per-call timeouts after which a `vmID` is marked degraded.
const DEGRADED_THRESHOLD: u32 = 3;
/// Minimum spacing between health probes against a degraded `vmID`.
const PROBE_INTERVAL: Duration = Duration::from_secs(1);
/// Idle wait per pump iteration; bounds message-pump latency.
const IDLE_TICK: Duration = Duration::from_millis(10);

/// A unit of work executed on the pump thread with exclusive bridge access.
pub(crate) struct Job {
    pub run: Box<dyn FnOnce(&Bridge) + Send>,
}

/// Tracks JVMs that stopped answering within the call deadline. Shared between
/// the client (fail-fast + probe bookkeeping) and the pump (release deferral).
#[derive(Default)]
pub(crate) struct DegradedTracker {
    inner: Mutex<HashMap<VmId, VmHealth>>,
}

#[derive(Default)]
struct VmHealth {
    consecutive_timeouts: u32,
    last_probe: Option<Instant>,
}

impl VmHealth {
    fn is_degraded(&self) -> bool {
        self.consecutive_timeouts >= DEGRADED_THRESHOLD
    }
}

impl DegradedTracker {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<VmId, VmHealth>> {
        self.inner.lock().expect("DegradedTracker mutex poisoned")
    }

    pub(crate) fn record_timeout(&self, vm: VmId) {
        let mut map = self.lock();
        let health = map.entry(vm).or_default();
        health.consecutive_timeouts = health.consecutive_timeouts.saturating_add(1);
        if health.consecutive_timeouts == DEGRADED_THRESHOLD {
            warn!(vm, "JVM marked degraded after {DEGRADED_THRESHOLD} consecutive JAB call timeouts");
        }
    }

    pub(crate) fn record_success(&self, vm: VmId) {
        let mut map = self.lock();
        if map.remove(&vm).is_some_and(|health| health.is_degraded()) {
            info!(vm, "JVM recovered; degraded flag cleared");
        }
    }

    pub(crate) fn is_degraded(&self, vm: VmId) -> bool {
        self.lock().get(&vm).is_some_and(VmHealth::is_degraded)
    }

    /// Whether a health probe against a degraded `vm` is due now; records the
    /// probe attempt when it is (so concurrent callers do not stampede).
    pub(crate) fn probe_due(&self, vm: VmId) -> bool {
        let mut map = self.lock();
        let Some(health) = map.get_mut(&vm) else {
            return false;
        };
        if !health.is_degraded() {
            return false;
        }
        let due = health.last_probe.is_none_or(|at| at.elapsed() >= PROBE_INTERVAL);
        if due {
            health.last_probe = Some(Instant::now());
        }
        due
    }
}

/// Live connection to the pump thread. Dropping every clone of the senders
/// lets the pump wind down on its own; the thread is deliberately not joined
/// because it may be stuck inside a `SendMessage` to a hung JVM (the OS call
/// cannot be cancelled — accepted residual of the design).
pub(crate) struct PumpConnection {
    pub job_tx: mpsc::Sender<Job>,
    pub release_tx: ReleaseSender,
}

/// Spawn the pump thread: discover nothing here — the caller resolved the DLL
/// path — but load, bind, and `Windows_run()` all happen on the new thread.
/// Blocks until the pump reports ready (or failed to initialize).
pub(crate) fn spawn(dll_path: PathBuf, degraded: Arc<DegradedTracker>) -> Result<PumpConnection, String> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let (job_tx, job_rx) = mpsc::channel::<Job>();
    let (release_tx, release_rx) = mpsc::channel::<(VmId, JObject64)>();

    std::thread::Builder::new()
        .name("platynui-jab-pump".into())
        .spawn(move || pump_main(&dll_path, &ready_tx, &job_rx, &release_rx, &degraded))
        .map_err(|e| format!("spawning the JAB pump thread failed: {e}"))?;

    match ready_rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(())) => Ok(PumpConnection { job_tx, release_tx }),
        Ok(Err(message)) => Err(message),
        Err(_) => Err("JAB pump thread did not become ready within 10 s".to_string()),
    }
}

#[allow(unsafe_code)]
fn pump_main(
    dll_path: &std::path::Path,
    ready: &mpsc::Sender<Result<(), String>>,
    jobs: &mpsc::Receiver<Job>,
    releases: &mpsc::Receiver<(VmId, JObject64)>,
    degraded: &DegradedTracker,
) {
    let bridge = match Bridge::load(dll_path) {
        Ok(bridge) => bridge,
        Err(message) => {
            let _ = ready.send(Err(message));
            return;
        }
    };

    allow_bridge_messages_when_elevated();

    // SAFETY: `Windows_run` must be called exactly once on the thread that
    // will pump messages; it creates the hidden rendezvous window here.
    unsafe { (bridge.windows_run)() };

    info!(dll = %dll_path.display(), "JAB pump running (Windows_run issued)");
    let _ = ready.send(Ok(()));

    // Releases aimed at a degraded JVM wait here so that `releaseJavaObject`
    // (blocking IPC like everything else) cannot wedge the pump; retried once
    // the JVM recovers, dropped when the pump winds down (a dead JVM reclaims
    // its references anyway).
    let mut deferred_releases: HashMap<VmId, Vec<JObject64>> = HashMap::new();

    loop {
        pump_pending_messages();
        drain_releases(&bridge, releases, degraded, &mut deferred_releases);

        match jobs.recv_timeout(IDLE_TICK) {
            Ok(job) => (job.run)(&bridge),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Wind-down: best-effort release of everything still queued for healthy
    // JVMs; the library (inside `bridge`) is freed last when it drops here.
    drain_releases(&bridge, releases, degraded, &mut deferred_releases);
    let leftover: usize = deferred_releases.values().map(Vec::len).sum();
    if leftover > 0 {
        debug!(leftover, "JAB pump exiting with unreleased handles for degraded JVMs");
    }
    info!("JAB pump thread exiting");
}

#[allow(unsafe_code)]
fn pump_pending_messages() {
    use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage};

    let mut msg = MSG::default();
    // SAFETY: standard non-blocking message pump on the current thread; `msg`
    // is a valid out-parameter for PeekMessageW.
    while unsafe { PeekMessageW(&raw mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
    }
}

#[allow(unsafe_code)]
fn drain_releases(
    bridge: &Bridge,
    releases: &mpsc::Receiver<(VmId, JObject64)>,
    degraded: &DegradedTracker,
    deferred: &mut HashMap<VmId, Vec<JObject64>>,
) {
    // Retry deferred releases for JVMs that have recovered since.
    deferred.retain(|&vm, handles| {
        if degraded.is_degraded(vm) {
            return true;
        }
        for &handle in handles.iter() {
            // SAFETY: releasing a JVM-side reference previously handed out by
            // the bridge; runs on the pump thread like every bridge call.
            unsafe { (bridge.release_java_object)(vm, handle) };
        }
        false
    });

    while let Ok((vm, handle)) = releases.try_recv() {
        if degraded.is_degraded(vm) {
            deferred.entry(vm).or_default().push(handle);
        } else {
            // SAFETY: as above.
            unsafe { (bridge.release_java_object)(vm, handle) };
        }
    }
}

/// UIPI: when PlatynUI runs elevated, a non-elevated JVM's rendezvous messages
/// (`WM_COPYDATA` and the registered "AccessBridge-FromJava-Hello") are
/// silently filtered before they reach our hidden window, so the bridge never
/// connects. Opening the filter for exactly these messages restores the
/// channel (the NVDA-documented workaround). An elevated *target* app remains
/// out of reach the other way around; that is documented, not worked around.
#[allow(unsafe_code)]
fn allow_bridge_messages_when_elevated() {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows::Win32::UI::WindowsAndMessaging::{
        ChangeWindowMessageFilter, MSGFLT_ADD, RegisterWindowMessageW, WM_COPYDATA,
    };
    use windows::core::w;

    // SAFETY: querying our own process token for the elevation flag; handles
    // are closed on every path.
    let elevated = unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token).is_err() {
            return;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some((&raw mut elevation).cast()),
            u32::try_from(std::mem::size_of::<TOKEN_ELEVATION>()).unwrap_or(4),
            &raw mut returned,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    };
    if !elevated {
        return;
    }

    // SAFETY: process-wide UIPI filter adjustments; both calls are plain Win32
    // with no memory contracts beyond valid message ids.
    unsafe {
        if ChangeWindowMessageFilter(WM_COPYDATA, MSGFLT_ADD).is_err() {
            warn!("ChangeWindowMessageFilter(WM_COPYDATA) failed; JAB may not connect while elevated");
        }
        let hello = RegisterWindowMessageW(w!("AccessBridge-FromJava-Hello"));
        if hello != 0 && ChangeWindowMessageFilter(hello, MSGFLT_ADD).is_err() {
            warn!("ChangeWindowMessageFilter(AccessBridge-FromJava-Hello) failed; JAB may not connect while elevated");
        }
    }
    info!("running elevated: UIPI filter opened for JAB rendezvous messages");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degraded_after_threshold_timeouts() {
        let tracker = DegradedTracker::default();
        assert!(!tracker.is_degraded(1));
        for _ in 0..DEGRADED_THRESHOLD - 1 {
            tracker.record_timeout(1);
        }
        assert!(!tracker.is_degraded(1));
        tracker.record_timeout(1);
        assert!(tracker.is_degraded(1));
        // Other vms unaffected.
        assert!(!tracker.is_degraded(2));
    }

    #[test]
    fn success_resets_timeout_streak() {
        let tracker = DegradedTracker::default();
        tracker.record_timeout(1);
        tracker.record_timeout(1);
        tracker.record_success(1);
        tracker.record_timeout(1);
        assert!(!tracker.is_degraded(1), "streak must restart after a success");
    }

    #[test]
    fn probe_due_only_for_degraded_vms_and_rate_limited() {
        let tracker = DegradedTracker::default();
        assert!(!tracker.probe_due(1), "healthy vm never probes");
        for _ in 0..DEGRADED_THRESHOLD {
            tracker.record_timeout(1);
        }
        assert!(tracker.probe_due(1), "first probe after degradation is due");
        assert!(!tracker.probe_due(1), "second probe within the interval is suppressed");
    }

    #[test]
    fn recovery_clears_degraded_state() {
        let tracker = DegradedTracker::default();
        for _ in 0..DEGRADED_THRESHOLD {
            tracker.record_timeout(9);
        }
        assert!(tracker.is_degraded(9));
        tracker.record_success(9);
        assert!(!tracker.is_degraded(9));
    }
}
