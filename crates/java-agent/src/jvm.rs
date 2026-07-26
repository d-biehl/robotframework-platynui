//! Two process-level questions the transport must answer for a bare PID: *is
//! this process alive?* and *does it run a JVM?*
//!
//! Same signal the Java classifier uses (`jvm.dll` in the module list on
//! Windows, `libjvm.so` in `/proc/<pid>/maps` on Linux — see
//! `dev-docs/java-toolkits.md`), deliberately re-implemented here rather than
//! borrowed from `platynui-platform-windows`: that classifier answers for a
//! *window*, this crate must answer for a *pid* with no window in sight, and a
//! transport crate that pulled in a platform crate would stop being usable on
//! its own.
//!
//! **No machine-wide JVM enumeration.** `jps`/`hsperfdata`-style listing
//! answers a question PlatynUI never asks — only JVMs that own windows matter,
//! and those arrive through window enumeration. It is also unreliable in
//! practice: off under `-XX:-UsePerfData`, inconsistent across containers and
//! user boundaries, absent on OpenJ9.

#![allow(unsafe_code)]

/// Whether the process still exists.
///
/// A handshake file whose process is gone is stale: no connection is attempted
/// and the file is eligible for cleanup. `false` also for a process this user
/// may not query — unreachable is as good as gone for our purposes.
#[must_use]
pub fn process_is_alive(pid: u32) -> bool {
    process_is_alive_impl(pid)
}

/// Whether the process has a JVM runtime loaded.
///
/// `None` means "cannot tell" — an elevated target that denies the module
/// scan, or a platform with no free signal — and callers must treat it as
/// *unknown*, never as a no. Guessing here would refuse to attach to a JVM we
/// simply could not inspect.
#[must_use]
pub fn process_runs_jvm(pid: u32) -> Option<bool> {
    process_runs_jvm_impl(pid)
}

// ------------------------------------------------------------------ Windows

#[cfg(windows)]
fn process_is_alive_impl(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    // SAFETY: opening a process handle by pid with a query-only right; the
    // handle is closed below on every path.
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return false;
    };
    let mut exit_code = 0u32;
    // SAFETY: `handle` is a live process handle, `exit_code` a valid out-pointer.
    let queried = unsafe { GetExitCodeProcess(handle, &raw mut exit_code) }.is_ok();
    // SAFETY: closing the handle we just opened.
    unsafe {
        let _ = CloseHandle(handle);
    }
    // A pid can outlive its process while a handle is held elsewhere; the exit
    // code is what actually says whether it is still running.
    queried && exit_code == STILL_ACTIVE.0.cast_unsigned()
}

#[cfg(windows)]
fn process_runs_jvm_impl(pid: u32) -> Option<bool> {
    module_list(pid).map(|modules| modules.iter().any(|name| name.eq_ignore_ascii_case("jvm.dll")))
}

/// The module file names loaded in `pid`, or `None` if no snapshot is possible.
#[cfg(windows)]
fn module_list(pid: u32) -> Option<Vec<String>> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
    };

    // SAFETY: snapshot creation from a flags/pid pair; the handle is closed below.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) }.ok()?;
    let mut entry = MODULEENTRY32W {
        dwSize: u32::try_from(std::mem::size_of::<MODULEENTRY32W>()).unwrap_or(0),
        ..Default::default()
    };
    let mut modules = Vec::new();
    // SAFETY: `entry` is a correctly sized MODULEENTRY32W and the snapshot handle is valid.
    let mut ok = unsafe { Module32FirstW(snapshot, &raw mut entry) }.is_ok();
    while ok {
        let len = entry.szModule.iter().position(|&c| c == 0).unwrap_or(entry.szModule.len());
        modules.push(String::from_utf16_lossy(&entry.szModule[..len]));
        // SAFETY: same handle and entry as above.
        ok = unsafe { Module32NextW(snapshot, &raw mut entry) }.is_ok();
    }
    // SAFETY: closing the snapshot handle we own.
    unsafe {
        let _ = CloseHandle(snapshot);
    }
    Some(modules)
}

/// The full path of the JVM runtime module loaded in `pid`.
///
/// The Windows attach leg needs it: it locates `JVM_EnqueueOperation` by
/// loading *the target's own* `jvm.dll` locally and applying the offset, so it
/// must be the same file, not whichever `jvm.dll` happens to be on our PATH.
#[cfg(windows)]
pub(crate) fn jvm_module(pid: u32) -> Option<(String, usize)> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
    };

    // SAFETY: snapshot creation from a flags/pid pair; the handle is closed below.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) }.ok()?;
    let mut entry = MODULEENTRY32W {
        dwSize: u32::try_from(std::mem::size_of::<MODULEENTRY32W>()).unwrap_or(0),
        ..Default::default()
    };
    let mut found = None;
    // SAFETY: `entry` is a correctly sized MODULEENTRY32W and the snapshot handle is valid.
    let mut ok = unsafe { Module32FirstW(snapshot, &raw mut entry) }.is_ok();
    while ok {
        let len = entry.szModule.iter().position(|&c| c == 0).unwrap_or(entry.szModule.len());
        if String::from_utf16_lossy(&entry.szModule[..len]).eq_ignore_ascii_case("jvm.dll") {
            let path_len = entry.szExePath.iter().position(|&c| c == 0).unwrap_or(entry.szExePath.len());
            found = Some((String::from_utf16_lossy(&entry.szExePath[..path_len]), entry.modBaseAddr as usize));
            break;
        }
        // SAFETY: same handle and entry as above.
        ok = unsafe { Module32NextW(snapshot, &raw mut entry) }.is_ok();
    }
    // SAFETY: closing the snapshot handle we own.
    unsafe {
        let _ = CloseHandle(snapshot);
    }
    found
}

// --------------------------------------------------------------------- Unix

#[cfg(unix)]
fn process_is_alive_impl(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    let Some(pid) = rustix::process::Pid::from_raw(pid) else {
        return false;
    };
    // Signal 0: existence and permission check only, nothing delivered.
    rustix::process::test_kill_process(pid).is_ok()
}

#[cfg(target_os = "linux")]
fn process_runs_jvm_impl(pid: u32) -> Option<bool> {
    let maps = std::fs::read_to_string(format!("/proc/{pid}/maps")).ok()?;
    Some(maps.lines().any(|line| line.contains("libjvm.so")))
}

#[cfg(all(unix, not(target_os = "linux")))]
fn process_runs_jvm_impl(_pid: u32) -> Option<bool> {
    // macOS and the BSDs expose no cheap, unprivileged module list. Unknown is
    // the honest answer: the attach attempt itself becomes the test, and it
    // fails with a clear protocol error against a non-JVM.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_process_is_alive_and_runs_no_jvm() {
        let self_pid = std::process::id();
        assert!(process_is_alive(self_pid));
        // Unknown is acceptable (platforms without a free signal); a positive
        // answer would mean the probe matched something it should not.
        assert_ne!(process_runs_jvm(self_pid), Some(true), "the test binary loads no JVM runtime");
    }

    #[test]
    fn an_impossible_pid_is_not_alive() {
        // 0 is never a normal user process on either platform: on Windows it is
        // the System Idle Process, on Unix it addresses a process group.
        assert!(!process_is_alive(0xFFFF_FFF0));
    }
}
