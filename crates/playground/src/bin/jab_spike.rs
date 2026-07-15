//! THROWAWAY Java Access Bridge spike — OpenSpec change `add-swing-test-app`,
//! task group 4. **Not production code**; the `add-jab-provider` change gets a
//! proper implementation and this file is deleted afterwards.
//!
//! Validates, against the Swing fixture app (`just run-test-app-swing`):
//! - DLL discovery (`PLATYNUI_JAB_DLL` → `JAVA_HOME` → `PATH`, incl. the JDK 8
//!   `jre\bin` layout),
//! - the dedicated pump-thread model (`Windows_run()` + Win32 message pump on
//!   one thread, requests marshaled in via a channel),
//! - `#[repr(C)]` struct layouts transcribed from the JDK headers,
//! - handle-release discipline (`releaseJavaObject` for every context),
//! - the real role vocabulary (`role` / `role_en_US`) of every fixture control,
//! - DPI behavior of JAB-reported bounds vs. `GetWindowRect` under
//!   Per-Monitor-V2 awareness.
//!
//! Findings are recorded in `openspec/changes/add-jab-provider/design.md`.

#[cfg(windows)]
fn main() {
    if let Err(message) = spike::run() {
        eprintln!("jab_spike failed: {message}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("jab_spike is Windows-only.");
}

#[cfg(windows)]
mod spike {
    use std::env;
    use std::fmt::Write as _;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
    };
    use windows::core::BOOL;

    const DLL_NAME: &str = "WindowsAccessBridge-64.dll";
    /// How long a dump waits for the first Java window to rendezvous.
    const DISCOVERY_DEADLINE: Duration = Duration::from_secs(15);
    const MAX_DEPTH: usize = 32;

    /// Minimal FFI surface transcribed from the JDK headers
    /// (`%JAVA_HOME%\include\win32\bridge`): `AccessBridgePackages.h` defines the
    /// structs (natural MSVC alignment, no packing pragmas), `AccessBridgeCalls.c`
    /// the real export names (lowercase camelCase plus `Windows_run`, undecorated
    /// cdecl — the capitalized names in `AccessBridgeCalls.h` are wrapper shims).
    mod ffi {
        use windows::Win32::Foundation::HWND;
        use windows::core::BOOL;

        /// `JOBJECT64` — a `jlong` in both the -32 and -64 APIs (bitness-neutral
        /// wire format; only the legacy DLL uses pointer-sized `jobject`).
        pub type JObject64 = i64;
        /// `vmID` is a C `long`, i.e. 32-bit on Windows (LLP64).
        pub type VmId = i32;

        pub const MAX_STRING_SIZE: usize = 1024;
        pub const SHORT_STRING_SIZE: usize = 256;

        /// `AccessibleContextInfo` from `AccessBridgePackages.h`. The trailing
        /// field was historically `BOOL accessibleValue` and is today the
        /// `accessibleInterfaces` bitfield (same offset).
        #[repr(C)]
        pub struct AccessibleContextInfo {
            pub name: [u16; MAX_STRING_SIZE],
            pub description: [u16; MAX_STRING_SIZE],
            pub role: [u16; SHORT_STRING_SIZE],
            pub role_en_us: [u16; SHORT_STRING_SIZE],
            pub states: [u16; SHORT_STRING_SIZE],
            pub states_en_us: [u16; SHORT_STRING_SIZE],
            pub index_in_parent: i32,
            pub children_count: i32,
            pub x: i32,
            pub y: i32,
            pub width: i32,
            pub height: i32,
            pub accessible_component: i32,
            pub accessible_action: i32,
            pub accessible_selection: i32,
            pub accessible_text: i32,
            pub accessible_interfaces: i32,
        }

        // Layout guard: header-derived expected size (natural alignment, x64).
        const _: () = assert!(std::mem::size_of::<AccessibleContextInfo>() == 6188);

        #[repr(C)]
        pub struct AccessBridgeVersionInfo {
            pub vm_version: [u16; SHORT_STRING_SIZE],
            pub bridge_java_class_version: [u16; SHORT_STRING_SIZE],
            pub bridge_java_dll_version: [u16; SHORT_STRING_SIZE],
            pub bridge_win_dll_version: [u16; SHORT_STRING_SIZE],
        }

        const _: () = assert!(std::mem::size_of::<AccessBridgeVersionInfo>() == 2048);

        pub type WindowsRunFn = unsafe extern "C" fn();
        pub type IsJavaWindowFn = unsafe extern "C" fn(HWND) -> BOOL;
        pub type GetAccessibleContextFromHwndFn = unsafe extern "C" fn(HWND, *mut VmId, *mut JObject64) -> BOOL;
        pub type GetAccessibleContextInfoFn = unsafe extern "C" fn(VmId, JObject64, *mut AccessibleContextInfo) -> BOOL;
        pub type GetAccessibleChildFromContextFn = unsafe extern "C" fn(VmId, JObject64, i32) -> JObject64;
        pub type GetAccessibleParentFromContextFn = unsafe extern "C" fn(VmId, JObject64) -> JObject64;
        pub type ReleaseJavaObjectFn = unsafe extern "C" fn(VmId, JObject64);
        pub type IsSameObjectFn = unsafe extern "C" fn(VmId, JObject64, JObject64) -> BOOL;
        pub type GetVersionInfoFn = unsafe extern "C" fn(VmId, *mut AccessBridgeVersionInfo) -> BOOL;
    }

    /// The bound client DLL. Function pointers stay valid as long as `_library`
    /// lives, so it is kept in the struct.
    struct Bridge {
        windows_run: ffi::WindowsRunFn,
        is_java_window: ffi::IsJavaWindowFn,
        get_accessible_context_from_hwnd: ffi::GetAccessibleContextFromHwndFn,
        get_accessible_context_info: ffi::GetAccessibleContextInfoFn,
        get_accessible_child_from_context: ffi::GetAccessibleChildFromContextFn,
        #[allow(dead_code)]
        get_accessible_parent_from_context: ffi::GetAccessibleParentFromContextFn,
        release_java_object: ffi::ReleaseJavaObjectFn,
        is_same_object: ffi::IsSameObjectFn,
        get_version_info: ffi::GetVersionInfoFn,
        _library: libloading::Library,
    }

    impl Bridge {
        fn load(path: &Path) -> Result<Self, String> {
            unsafe {
                let library =
                    libloading::Library::new(path).map_err(|e| format!("loading {} failed: {e}", path.display()))?;
                macro_rules! bind {
                    ($name:literal) => {
                        *library
                            .get($name)
                            .map_err(|e| format!("export {} not found: {e}", String::from_utf8_lossy($name)))?
                    };
                }
                Ok(Self {
                    windows_run: bind!(b"Windows_run\0"),
                    is_java_window: bind!(b"isJavaWindow\0"),
                    get_accessible_context_from_hwnd: bind!(b"getAccessibleContextFromHWND\0"),
                    get_accessible_context_info: bind!(b"getAccessibleContextInfo\0"),
                    get_accessible_child_from_context: bind!(b"getAccessibleChildFromContext\0"),
                    get_accessible_parent_from_context: bind!(b"getAccessibleParentFromContext\0"),
                    release_java_object: bind!(b"releaseJavaObject\0"),
                    is_same_object: bind!(b"isSameObject\0"),
                    get_version_info: bind!(b"getVersionInfo\0"),
                    _library: library,
                })
            }
        }
    }

    /// Discovery order the provider will use too: explicit env override, then
    /// `JAVA_HOME` (JDK 8 keeps the client DLL in `jre\bin`, JDK 9+ in `bin`),
    /// then `PATH` — including the JDK 8 quirk that `PATH` usually holds
    /// `<jdk>\bin` while the DLL lives in `<jdk>\jre\bin` next to it.
    fn discover_dll() -> Result<PathBuf, String> {
        let mut tried: Vec<String> = Vec::new();

        if let Ok(explicit) = env::var("PLATYNUI_JAB_DLL") {
            let candidate = PathBuf::from(&explicit);
            if candidate.is_file() {
                return Ok(candidate);
            }
            tried.push(explicit);
        }

        if let Ok(java_home) = env::var("JAVA_HOME") {
            for sub in ["jre\\bin", "bin"] {
                let candidate = Path::new(&java_home).join(sub).join(DLL_NAME);
                if candidate.is_file() {
                    return Ok(candidate);
                }
                tried.push(candidate.display().to_string());
            }
        }

        if let Some(path_var) = env::var_os("PATH") {
            for dir in env::split_paths(&path_var) {
                let direct = dir.join(DLL_NAME);
                if direct.is_file() {
                    return Ok(direct);
                }
                if dir.join("java.exe").is_file()
                    && let Some(jdk_root) = dir.parent()
                {
                    let jre = jdk_root.join("jre").join("bin").join(DLL_NAME);
                    if jre.is_file() {
                        return Ok(jre);
                    }
                    tried.push(jre.display().to_string());
                }
            }
        }

        Err(format!(
            "{DLL_NAME} not found. Set PLATYNUI_JAB_DLL or JAVA_HOME, or put a JDK on PATH. Tried: {}",
            if tried.is_empty() { "<nothing>".to_string() } else { tried.join(", ") }
        ))
    }

    enum Request {
        Dump { reply: mpsc::Sender<Result<String, String>> },
    }

    pub fn run() -> Result<(), String> {
        // Mirror the PlatynUI platform init so bounds comparisons are meaningful.
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
                .map_err(|e| format!("SetProcessDpiAwarenessContext failed: {e}"))?;
        }
        println!("jab_spike: Per-Monitor-V2 DPI awareness declared (mirrors PlatynUI platform init).");

        let dll = discover_dll()?;
        println!("jab_spike: client DLL: {}", dll.display());

        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let (request_tx, request_rx) = mpsc::channel::<Request>();
        let pump_thread = std::thread::Builder::new()
            .name("jab-pump".into())
            .spawn(move || jab_thread_main(&dll, &ready_tx, &request_rx))
            .map_err(|e| format!("spawning the JAB pump thread failed: {e}"))?;

        ready_rx.recv().map_err(|_| "JAB pump thread died during initialization".to_string())??;
        println!("jab_spike: Windows_run() called, pump running.\n");

        for pass in 1..=2 {
            let (reply_tx, reply_rx) = mpsc::channel();
            request_tx.send(Request::Dump { reply: reply_tx }).map_err(|_| "JAB pump thread is gone".to_string())?;
            let report = reply_rx
                .recv_timeout(Duration::from_secs(60))
                .map_err(|e| format!("dump pass {pass} timed out: {e}"))??;
            println!("===== dump pass {pass} =====");
            println!("{report}");
        }

        drop(request_tx);
        let _ = pump_thread.join();
        Ok(())
    }

    /// Everything JAB happens on this thread: bind, `Windows_run()`, the message
    /// pump, and every API call (the provider will use the same model).
    fn jab_thread_main(dll: &Path, ready: &mpsc::Sender<Result<(), String>>, requests: &mpsc::Receiver<Request>) {
        let bridge = match Bridge::load(dll) {
            Ok(bridge) => bridge,
            Err(message) => {
                let _ = ready.send(Err(message));
                return;
            }
        };

        unsafe { (bridge.windows_run)() };
        let started = Instant::now();
        let _ = ready.send(Ok(()));

        loop {
            pump_pending_messages();
            match requests.recv_timeout(Duration::from_millis(10)) {
                Ok(Request::Dump { reply }) => {
                    let _ = reply.send(dump_all(&bridge, started));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn pump_pending_messages() {
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    fn enumerate_top_level_windows() -> Vec<HWND> {
        let mut handles: Vec<HWND> = Vec::new();
        unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let handles = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
            handles.push(hwnd);
            BOOL(1)
        }
        let _ = unsafe { EnumWindows(Some(callback), LPARAM(&raw mut handles as isize)) };
        handles
    }

    #[derive(Default)]
    struct Stats {
        nodes: usize,
        released: usize,
        info_failures: usize,
        null_children: usize,
    }

    fn dump_all(bridge: &Bridge, started: Instant) -> Result<String, String> {
        let mut out = String::new();

        // Rendezvous is PostMessage-based and asynchronous: pump-and-retry until
        // the first Java window answers isJavaWindow (checklist item a).
        let discovery_start = Instant::now();
        let java_windows = loop {
            pump_pending_messages();
            let java: Vec<HWND> = enumerate_top_level_windows()
                .into_iter()
                .filter(|&hwnd| unsafe { (bridge.is_java_window)(hwnd) }.as_bool())
                .collect();
            if !java.is_empty() {
                break java;
            }
            if discovery_start.elapsed() > DISCOVERY_DEADLINE {
                return Err(format!(
                    "no Java window answered isJavaWindow within {DISCOVERY_DEADLINE:?} — is the \
                     fixture app running with the bridge enabled? (just run-test-app-swing)"
                ));
            }
            std::thread::sleep(Duration::from_millis(25));
        };

        let _ = writeln!(
            out,
            "found {} Java window(s); {}ms since Windows_run, {}ms into this dump",
            java_windows.len(),
            started.elapsed().as_millis(),
            discovery_start.elapsed().as_millis(),
        );

        for hwnd in java_windows {
            let mut stats = Stats::default();
            dump_window(bridge, hwnd, &mut out, &mut stats)?;
            let _ = writeln!(
                out,
                "-- stats: {} nodes, {} handles released, {} info failures, {} null children\n",
                stats.nodes, stats.released, stats.info_failures, stats.null_children,
            );
        }
        Ok(out)
    }

    fn dump_window(bridge: &Bridge, hwnd: HWND, out: &mut String, stats: &mut Stats) -> Result<(), String> {
        let mut class_buffer = [0u16; 256];
        let class_len = unsafe { GetClassNameW(hwnd, &mut class_buffer) };
        let class_name = String::from_utf16_lossy(&class_buffer[..class_len.max(0) as usize]);

        let mut title_buffer = [0u16; 256];
        let title_len = unsafe { GetWindowTextW(hwnd, &mut title_buffer) };
        let title = String::from_utf16_lossy(&title_buffer[..title_len.max(0) as usize]);

        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
        let visible = unsafe { IsWindowVisible(hwnd) }.as_bool();

        let mut window_rect = RECT::default();
        let _ = unsafe { GetWindowRect(hwnd, &raw mut window_rect) };
        let dpi = unsafe { GetDpiForWindow(hwnd) };

        let _ = writeln!(
            out,
            "window {hwnd:?} class={class_name:?} title={title:?} pid={pid} visible={visible} \
             GetWindowRect=({},{},{}x{}) dpi={dpi}",
            window_rect.left,
            window_rect.top,
            window_rect.right - window_rect.left,
            window_rect.bottom - window_rect.top,
        );

        let mut vm_id: ffi::VmId = 0;
        let mut root: ffi::JObject64 = 0;
        let ok = unsafe { (bridge.get_accessible_context_from_hwnd)(hwnd, &raw mut vm_id, &raw mut root) };
        if !ok.as_bool() {
            let _ = writeln!(out, "  getAccessibleContextFromHWND FAILED");
            return Ok(());
        }

        let mut version = Box::new(ffi::AccessBridgeVersionInfo {
            vm_version: [0; ffi::SHORT_STRING_SIZE],
            bridge_java_class_version: [0; ffi::SHORT_STRING_SIZE],
            bridge_java_dll_version: [0; ffi::SHORT_STRING_SIZE],
            bridge_win_dll_version: [0; ffi::SHORT_STRING_SIZE],
        });
        if unsafe { (bridge.get_version_info)(vm_id, &mut *version) }.as_bool() {
            let _ = writeln!(
                out,
                "  vmID={vm_id} vm={} javaClass={} javaDll={} winDll={}",
                wide_str(&version.vm_version),
                wide_str(&version.bridge_java_class_version),
                wide_str(&version.bridge_java_dll_version),
                wide_str(&version.bridge_win_dll_version),
            );
        }

        // isSameObject sanity: a second lookup of the same HWND must yield a
        // handle that compares equal even though the raw values may differ.
        let mut vm_id2: ffi::VmId = 0;
        let mut root2: ffi::JObject64 = 0;
        if unsafe { (bridge.get_accessible_context_from_hwnd)(hwnd, &raw mut vm_id2, &raw mut root2) }.as_bool() {
            let same = unsafe { (bridge.is_same_object)(vm_id, root, root2) }.as_bool();
            let _ = writeln!(out, "  isSameObject(second lookup)={same} (raw handles {} vs {})", root, root2);
            unsafe { (bridge.release_java_object)(vm_id2, root2) };
            stats.released += 1;
        }

        walk(bridge, vm_id, root, 1, out, stats);
        unsafe { (bridge.release_java_object)(vm_id, root) };
        stats.released += 1;
        Ok(())
    }

    fn walk(
        bridge: &Bridge,
        vm_id: ffi::VmId,
        context: ffi::JObject64,
        depth: usize,
        out: &mut String,
        stats: &mut Stats,
    ) {
        let indent = "  ".repeat(depth);
        if depth > MAX_DEPTH {
            let _ = writeln!(out, "{indent}<max depth {MAX_DEPTH} reached, truncated>");
            return;
        }

        let mut info = Box::new(unsafe { std::mem::zeroed::<ffi::AccessibleContextInfo>() });
        if !unsafe { (bridge.get_accessible_context_info)(vm_id, context, &mut *info) }.as_bool() {
            stats.info_failures += 1;
            let _ = writeln!(out, "{indent}<getAccessibleContextInfo FAILED>");
            return;
        }
        stats.nodes += 1;

        let _ = writeln!(
            out,
            "{indent}[{}] role_en_US={:?} role={:?} name={:?} states_en_US={:?} bounds=({},{},{}x{}) \
             children={} ifaces={}",
            info.index_in_parent,
            wide_str(&info.role_en_us),
            wide_str(&info.role),
            wide_str(&info.name),
            wide_str(&info.states_en_us),
            info.x,
            info.y,
            info.width,
            info.height,
            info.children_count,
            interfaces_to_string(info.accessible_interfaces),
        );

        for index in 0..info.children_count {
            let child = unsafe { (bridge.get_accessible_child_from_context)(vm_id, context, index) };
            if child == 0 {
                stats.null_children += 1;
                let _ = writeln!(out, "{indent}  <child {index} came back null>");
                continue;
            }
            walk(bridge, vm_id, child, depth + 1, out, stats);
            unsafe { (bridge.release_java_object)(vm_id, child) };
            stats.released += 1;
        }
    }

    /// Decode the `accessibleInterfaces` bitfield (constants from
    /// AccessBridgePackages.h; note the header's `// 1 << n` comments are off by
    /// one — these are the actual values).
    fn interfaces_to_string(bits: i32) -> String {
        const NAMES: [(i32, &str); 7] = [
            (1, "value"),
            (2, "action"),
            (4, "component"),
            (8, "selection"),
            (16, "table"),
            (32, "text"),
            (64, "hypertext"),
        ];
        let names: Vec<&str> = NAMES.iter().filter(|(bit, _)| bits & bit != 0).map(|(_, name)| *name).collect();
        if names.is_empty() { format!("none({bits})") } else { names.join("+") }
    }

    fn wide_str(buffer: &[u16]) -> String {
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..end])
    }
}
