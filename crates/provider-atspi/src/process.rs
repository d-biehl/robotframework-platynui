//! Process metadata helpers.
//!
//! Uses the `sysinfo` crate to portably query process attributes across
//! Unix-like systems (Linux, FreeBSD, etc.) that support AT-SPI via D-Bus.
//! All functions accept a PID and return `Option<String>`, returning `None`
//! when the process is inaccessible (e.g. short-lived process, insufficient
//! permissions, or unsupported platform).

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// Refresh a single process and apply `f` to it.
fn with_process<F, R>(pid: u32, f: F) -> Option<R>
where
    F: FnOnce(&sysinfo::Process) -> Option<R>,
{
    let mut sys = System::new();
    let sysinfo_pid = Pid::from_u32(pid);
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[sysinfo_pid]),
        true,
        ProcessRefreshKind::everything().without_cpu().without_memory().without_disk_usage().without_tasks(),
    );
    let process = sys.process(sysinfo_pid)?;
    f(process)
}

/// Return the process executable stem (filename without extension).
///
/// Prefers the exe path for the full binary name; falls back to the
/// process name (limited to 15 characters on Linux).
pub fn query_process_name(pid: u32) -> Option<String> {
    with_process(pid, |p| {
        // Prefer the exe path — it gives us the real binary name even if
        // the process has been exec'd.
        if let Some(exe) = p.exe()
            && let Some(stem) = exe.file_stem()
        {
            return Some(stem.to_string_lossy().into_owned());
        }
        // Fallback: process name.
        let name = p.name().to_string_lossy();
        if name.is_empty() { None } else { Some(name.into_owned()) }
    })
}

/// Return the full path to the process executable.
pub fn query_executable_path(pid: u32) -> Option<String> {
    with_process(pid, |p| p.exe().map(|path| path.to_string_lossy().into_owned()))
}

/// Return the process command line as a single space-separated string.
pub fn query_command_line(pid: u32) -> Option<String> {
    with_process(pid, |p| {
        let cmd = p.cmd();
        if cmd.is_empty() {
            return None;
        }
        let joined: String = cmd.iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" ");
        if joined.is_empty() { None } else { Some(joined) }
    })
}

/// Return the username owning the process.
///
/// Gets the effective UID via `sysinfo` (portable across Unix systems),
/// then resolves it to a username via `getpwuid_r(3)` (POSIX, handles
/// NSS sources like LDAP, SSSD, NIS correctly — unlike enumeration-based
/// approaches that may miss users when NSS enumeration is disabled).
pub fn query_user_name(pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        with_process(pid, |p| {
            let uid = p.effective_user_id().or_else(|| p.user_id())?;
            resolve_username(**uid)
        })
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

/// Resolve a UID to a username via `getpwuid_r(3)`.
///
/// Uses the POSIX Name Service Switch (NSS), which correctly handles
/// `/etc/passwd`, LDAP, SSSD, NIS, and systemd-homed.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
fn resolve_username(uid: u32) -> Option<String> {
    // Initial buffer size; grown if ERANGE is returned.
    let mut buf_size = 1024_usize;
    loop {
        let mut buf = vec![0u8; buf_size];
        let mut pwd: std::mem::MaybeUninit<libc::passwd> = std::mem::MaybeUninit::uninit();
        let mut result: *mut libc::passwd = std::ptr::null_mut();

        // SAFETY: `getpwuid_r` is a POSIX-specified reentrant function.
        // We pass a valid `passwd` struct, a correctly-sized buffer, and
        // a pointer to receive the result. All pointers are valid for the
        // duration of the call.
        let rc = unsafe {
            libc::getpwuid_r(uid, pwd.as_mut_ptr(), buf.as_mut_ptr().cast::<libc::c_char>(), buf_size, &raw mut result)
        };

        if rc == libc::ERANGE {
            // Buffer too small — double and retry.
            buf_size = buf_size.checked_mul(2)?;
            continue;
        }

        if rc != 0 || result.is_null() {
            return None;
        }

        // SAFETY: `result` is non-null and points to the initialized `pwd`.
        // `pw_name` is a valid NUL-terminated C string owned by `buf`.
        let name = unsafe { std::ffi::CStr::from_ptr((*result).pw_name) };
        return Some(name.to_string_lossy().into_owned());
    }
}

/// Return the process start time as an ISO 8601 UTC string.
pub fn query_start_time(pid: u32) -> Option<String> {
    with_process(pid, |p| {
        let start_secs = p.start_time();
        if start_secs == 0 {
            return None;
        }
        let secs = i64::try_from(start_secs).ok()?;
        let dt = chrono::DateTime::from_timestamp(secs, 0)?;
        Some(dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No process has this number: Linux caps `pid_max` at 2^22.
    const NO_SUCH_PROCESS: u32 = u32::MAX;

    /// Spec: *An attribute that cannot be determined is absent, not guessed*.
    #[test]
    fn every_process_table_value_of_an_unreadable_process_is_absent() {
        assert_eq!(query_process_name(NO_SUCH_PROCESS), None);
        assert_eq!(query_executable_path(NO_SUCH_PROCESS), None);
        assert_eq!(query_command_line(NO_SUCH_PROCESS), None);
        assert_eq!(query_user_name(NO_SUCH_PROCESS), None);
        assert_eq!(query_start_time(NO_SUCH_PROCESS), None);
    }
}
