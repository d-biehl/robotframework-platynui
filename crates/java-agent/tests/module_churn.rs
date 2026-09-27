//! An attach must never call a process "not a JVM" only because its module
//! list was changing at that moment.
//!
//! A JVM loads and unloads libraries all through its start-up, and Windows
//! fails a module snapshot taken meanwhile. The child process here stands in
//! for such a JVM without being one: it keeps a copy of a system library loaded
//! under the name `jvm.dll`, and loads and unloads other system libraries in a
//! loop. An attach to it has to fail — the stand-in exports no
//! `JVM_EnqueueOperation`, so no thread is ever started in it — but never with
//! `NotAJvm`. No JVM and no window is needed, so this runs in the plain
//! `just test`.

// On non-Windows targets the `cfg` below strips the whole crate body, leaving
// every dependency of this test target unused.
#![cfg_attr(not(windows), allow(unused_crate_dependencies))]
#![cfg(windows)]
#![allow(unsafe_code)]

use platynui_java_agent::{AgentError, attach, jvm};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::FreeLibrary;
use windows::Win32::System::LibraryLoader::LoadLibraryW;
use windows::core::HSTRING;

// Dependencies of the library that this test target does not use directly.
use serde as _;
use serde_json as _;
use thiserror as _;
use tracing as _;

/// Set for the child process: the directory that holds its `jvm.dll`.
const CHILD_ENV: &str = "PLATYNUI_MODULE_CHURN_CHILD";

/// How long the child keeps changing its module list at most, so that it ends
/// on its own if the parent test dies.
const CHILD_LIFETIME: Duration = Duration::from_secs(30);

/// System libraries the test binary does not load by itself, so that loading
/// and unloading them really changes the child's module list.
const CHURN_LIBRARIES: [&str; 4] = ["winhttp.dll", "dbghelp.dll", "wtsapi32.dll", "mpr.dll"];

fn system_library(name: &str) -> PathBuf {
    let root = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    root.join("System32").join(name)
}

/// Not a test of its own: the child process of
/// `an_attach_never_mistakes_a_changing_module_list_for_no_jvm`. Started
/// without that test's environment, it returns at once.
#[test]
#[ignore = "the child process of an_attach_never_mistakes_a_changing_module_list_for_no_jvm, which starts it"]
fn module_churn_child() {
    let Some(dir) = std::env::var_os(CHILD_ENV) else { return };
    let stand_in = HSTRING::from(Path::new(&dir).join("jvm.dll").as_os_str());
    // SAFETY: loading a copy of a system library, kept for the life of the process.
    let jvm = unsafe { LoadLibraryW(&stand_in) }.expect("the stand-in jvm.dll loads");
    let deadline = Instant::now() + CHILD_LIFETIME;
    while Instant::now() < deadline {
        for name in CHURN_LIBRARIES {
            let path = HSTRING::from(system_library(name).as_os_str());
            // SAFETY: loading a system library and freeing it again at once.
            if let Ok(module) = unsafe { LoadLibraryW(&path) } {
                // SAFETY: `module` is the handle just loaded.
                let _ = unsafe { FreeLibrary(module) };
            }
        }
    }
    // SAFETY: the handle loaded above.
    let _ = unsafe { FreeLibrary(jvm) };
}

/// Ends the child process when the test ends, however it ends.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn an_attach_never_mistakes_a_changing_module_list_for_no_jvm() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::copy(system_library("version.dll"), dir.path().join("jvm.dll")).expect("stand-in jvm.dll");
    let jar = dir.path().join("agent.jar");
    std::fs::write(&jar, b"not loaded: the attach fails before").expect("stand-in agent JAR");

    let child = Command::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", "module_churn_child", "--ignored", "--nocapture"])
        .env(CHILD_ENV, dir.path())
        .spawn()
        .expect("child process");
    let child = ChildGuard(child);
    let pid = child.0.id();

    let loaded_by = Instant::now() + Duration::from_secs(10);
    while jvm::process_runs_jvm(pid) != Some(true) {
        assert!(Instant::now() < loaded_by, "the child did not load its stand-in jvm.dll");
        std::thread::sleep(Duration::from_millis(20));
    }

    let mut attempts = 0_u32;
    let mut not_a_jvm = 0_u32;
    let until = Instant::now() + Duration::from_secs(3);
    while Instant::now() < until {
        match attach::load_agent(pid, &jar, None, Duration::from_secs(2)) {
            Err(AgentError::NotAJvm { .. }) => not_a_jvm += 1,
            Err(AgentError::AttachFailed { .. }) => {}
            other => panic!("an attach to a process without JVM_EnqueueOperation must fail: {other:?}"),
        }
        attempts += 1;
    }

    assert!(attempts >= 20, "only {attempts} attaches in the time given");
    assert_eq!(not_a_jvm, 0, "{not_a_jvm} of {attempts} attaches called a process with jvm.dll loaded not a JVM");
}
