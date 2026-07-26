//! Windows backend of the Java-app classifier
//! ([`platynui_core::platform::java`]).
//!
//! Signals, per the table in `dev-docs/java-toolkits.md`:
//!
//! - **is-JVM**: `jvm.dll` loaded in the owning process, via
//!   `CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid)` +
//!   `Module32FirstW/NextW` — robust against renamed/jpackage launchers
//!   (HotSpot *and* OpenJ9 ship `jvm.dll`). Results are cached per PID with a
//!   short TTL so tree enumerations do not re-snapshot every process.
//! - **toolkit**: the top-level window class (`GetClassNameW`) — `SunAwt*`,
//!   `SWT_Window*`, `Glass*` prefixes.
//! - **native accessibility**: only the free signal — the process-wide window
//!   claim (the JAB provider claims exactly the windows `isJavaWindow`
//!   acknowledged). No eager UIA probe, by design.
//! - **agent present**: the PlatynUI agent's per-user handshake file for that
//!   pid (`platynui-java-agent`). Probed only for windows already classified as
//!   JVM, and it is one `stat` on one exact path — never a directory scan, and
//!   never a connection. Reading the signal must not instrument anything; that
//!   is the consuming provider's decision, not the classifier's.

#![allow(unsafe_code)]

use platynui_core::platform::java::{JavaClassification, JavaClassifier, classify_from_signals};
use platynui_core::platform::{PlatformError, WindowId, window_claims};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a per-PID `jvm.dll` scan result stays valid. Modules are loaded
/// early in a JVM's life, so a short TTL only guards against PID reuse and
/// keeps repeated tree enumerations cheap.
const JVM_MODULE_CACHE_TTL: Duration = Duration::from_secs(5);

/// Windows [`JavaClassifier`] backend (window class + Toolhelp module scan).
pub struct WindowsJavaClassifier {
    /// PID → (scan time, scan result); `None` result = scan not possible
    /// (access denied / process gone), reported as "unknown" upstream.
    jvm_module_cache: Mutex<HashMap<u32, (Instant, Option<bool>)>>,
}

impl WindowsJavaClassifier {
    pub fn new() -> Self {
        Self { jvm_module_cache: Mutex::new(HashMap::new()) }
    }

    /// Whether `pid` has `jvm.dll` loaded; `None` when the module snapshot is
    /// unavailable (e.g. elevated target, exited process).
    fn jvm_runtime_loaded(&self, pid: u32) -> Option<bool> {
        let now = Instant::now();
        let mut cache = self.jvm_module_cache.lock().expect("jvm module cache mutex poisoned");
        if let Some((scanned_at, result)) = cache.get(&pid)
            && now.duration_since(*scanned_at) < JVM_MODULE_CACHE_TTL
        {
            return *result;
        }
        // Opportunistic pruning keeps the cache from accumulating dead PIDs.
        cache.retain(|_, (scanned_at, _)| now.duration_since(*scanned_at) < JVM_MODULE_CACHE_TTL);
        let result = scan_for_jvm_module(pid);
        cache.insert(pid, (now, result));
        result
    }
}

impl Default for WindowsJavaClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaClassifier for WindowsJavaClassifier {
    fn name(&self) -> &'static str {
        "Win32 window class + Toolhelp"
    }

    fn classify(&self, window: WindowId, pid: u32) -> Result<JavaClassification, PlatformError> {
        let class_name = window_class_name(window)?;
        let jvm_runtime = self.jvm_runtime_loaded(pid);
        let claimed = window_claims::is_claimed(window.raw());
        // Probe only where the answer means something. A non-JVM window is the
        // overwhelmingly common case in a desktop enumeration, and it must not
        // pay for a Java question.
        let looks_like_a_jvm = jvm_runtime == Some(true)
            || class_name.as_deref().and_then(platynui_core::platform::java::JavaToolkit::from_window_class).is_some();
        let agent_present = looks_like_a_jvm.then(|| platynui_java_agent::handshake::agent_present(pid));
        Ok(classify_from_signals(jvm_runtime, class_name.as_deref(), claimed, agent_present))
    }
}

/// The window's class name via `GetClassNameW`; `Ok(None)` when the class is
/// empty, `Err` when the handle is no longer a window.
fn window_class_name(window: WindowId) -> Result<Option<String>, PlatformError> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;

    #[expect(clippy::cast_possible_wrap, reason = "HWND stores the raw bit pattern of the u64 handle")]
    let hwnd = HWND(window.raw() as isize as *mut core::ffi::c_void);
    let mut buffer = [0u16; 256];
    // SAFETY: read-only query; `buffer` is a valid out-buffer of the given length.
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if len <= 0 {
        return Err(PlatformError::OperationFailed {
            operation: "GetClassNameW",
            details: Some(format!("no class name for window 0x{:X} (destroyed?)", window.raw())),
        });
    }
    let class_name = String::from_utf16_lossy(&buffer[..usize::try_from(len).unwrap_or(0)]);
    Ok((!class_name.is_empty()).then_some(class_name))
}

/// One Toolhelp module snapshot of `pid`, looking for `jvm.dll`. `None` when
/// the snapshot cannot be taken (access denied, process exited).
fn scan_for_jvm_module(pid: u32) -> Option<bool> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
    };

    // SAFETY: snapshot creation with a flags/pid pair; the handle is closed by
    // the owned `HANDLE` drop path below.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) }.ok()?;
    let mut entry = MODULEENTRY32W {
        dwSize: u32::try_from(std::mem::size_of::<MODULEENTRY32W>()).unwrap_or(0),
        ..Default::default()
    };
    let mut found = false;
    // SAFETY: `entry` is a properly sized MODULEENTRY32W; the snapshot handle is valid.
    let mut ok = unsafe { Module32FirstW(snapshot, &raw mut entry) }.is_ok();
    while ok {
        let name_len = entry.szModule.iter().position(|&c| c == 0).unwrap_or(entry.szModule.len());
        let module_name = String::from_utf16_lossy(&entry.szModule[..name_len]);
        if module_name.eq_ignore_ascii_case("jvm.dll") {
            found = true;
            break;
        }
        // SAFETY: same handle/entry as above.
        ok = unsafe { Module32NextW(snapshot, &raw mut entry) }.is_ok();
    }
    // SAFETY: closing the snapshot handle we own.
    unsafe {
        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
    }
    Some(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::java::JavaToolkit;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPED,
    };
    use windows::core::PCWSTR;

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        // SAFETY: plain pass-through window procedure.
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    /// A hidden native window registered under `class_name`; destroyed on drop.
    struct TestWindow {
        hwnd: HWND,
    }

    impl TestWindow {
        fn create(class_name: &str) -> Self {
            let wide: Vec<u16> = class_name.encode_utf16().chain(std::iter::once(0)).collect();
            // SAFETY: registering a process-local class and creating a hidden
            // window of it; RegisterClassW failing with "already registered"
            // (another test in this process) is fine — CreateWindowExW below
            // still resolves the class.
            let hwnd = unsafe {
                let instance = GetModuleHandleW(PCWSTR::null()).expect("own module handle");
                let class = WNDCLASSW {
                    lpfnWndProc: Some(wnd_proc),
                    hInstance: instance.into(),
                    lpszClassName: PCWSTR(wide.as_ptr()),
                    ..Default::default()
                };
                let _ = RegisterClassW(&raw const class);
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    PCWSTR(wide.as_ptr()),
                    PCWSTR::null(),
                    WS_OVERLAPPED,
                    0,
                    0,
                    10,
                    10,
                    None,
                    None,
                    Some(instance.into()),
                    None,
                )
                .expect("test window creation")
            };
            Self { hwnd }
        }

        fn id(&self) -> WindowId {
            WindowId::new(self.hwnd.0 as u64)
        }
    }

    impl Drop for TestWindow {
        fn drop(&mut self) {
            // SAFETY: destroying the window this test created.
            let _ = unsafe { DestroyWindow(self.hwnd) };
        }
    }

    fn self_pid() -> u32 {
        std::process::id()
    }

    // Real-provider scenario (java-app-classifier task 4.2, class-signal
    // stand-in): windows carrying the SWT/JavaFX toolkit classes classify by
    // window class even though such windows are served by UIA.
    #[test]
    fn swt_and_javafx_window_classes_classify_by_class() {
        let classifier = WindowsJavaClassifier::new();
        for (class, toolkit) in [("SWT_Window0", JavaToolkit::Swt), ("GlassWindowClass", JavaToolkit::JavaFx)] {
            let window = TestWindow::create(class);
            let c = classifier.classify(window.id(), self_pid()).expect("classify");
            assert!(c.is_jvm, "{class} must classify as JVM");
            assert_eq!(c.toolkit, Some(toolkit), "class {class}");
            assert_eq!(c.native_a11y_visible, None, "no eager UIA probe for {class}");
        }
    }

    // Real-provider scenario (task 4.3): a native window classifies as
    // not-JVM and produces no Java diagnostic.
    #[test]
    fn native_window_is_not_jvm_and_fires_no_diagnostic() {
        use platynui_core::platform::java::jvm_unreachable_diagnostic_emitted;

        let classifier = WindowsJavaClassifier::new();
        let window = TestWindow::create("PlatynUiClassifierPlainWindow");
        let c = classifier.classify(window.id(), self_pid()).expect("classify");
        assert!(!c.is_jvm, "this test process runs no JVM");
        assert_eq!(c.toolkit, None);
        assert_eq!(c.native_a11y_visible, None);
        assert!(!jvm_unreachable_diagnostic_emitted(window.id().raw()), "classification must not warn");
    }

    #[test]
    fn swing_class_reports_reachability_from_window_claims() {
        let classifier = WindowsJavaClassifier::new();
        let window = TestWindow::create("SunAwtFrame");

        let c = classifier.classify(window.id(), self_pid()).expect("classify");
        assert_eq!(c.toolkit, Some(JavaToolkit::SwingAwt));
        assert_eq!(c.native_a11y_visible, Some(false), "unclaimed Swing window is not reachable");

        window_claims::claim_window(window.id().raw(), "java");
        let c = classifier.classify(window.id(), self_pid()).expect("classify");
        assert_eq!(c.native_a11y_visible, Some(true), "the Java provider's claim is the free reachability signal");
        window_claims::release_window(window.id().raw(), "java");
    }

    #[test]
    fn destroyed_window_yields_an_error_not_a_guess() {
        let classifier = WindowsJavaClassifier::new();
        let id = {
            let window = TestWindow::create("PlatynUiClassifierTransientWindow");
            window.id()
        }; // dropped → destroyed
        assert!(classifier.classify(id, self_pid()).is_err());
    }

    #[test]
    fn jvm_module_scan_answers_for_own_process_and_unknown_for_inaccessible_pid() {
        let classifier = WindowsJavaClassifier::new();
        assert_eq!(classifier.jvm_runtime_loaded(self_pid()), Some(false), "the test binary loads no jvm.dll");
        // PID 4 (System) never grants a module snapshot → "unknown", not a guess.
        assert_eq!(classifier.jvm_runtime_loaded(4), None);
    }
}
