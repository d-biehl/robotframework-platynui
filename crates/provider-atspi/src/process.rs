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
        return with_process(pid, |p| {
            let uid = p.effective_user_id().or_else(|| p.user_id())?;
            resolve_username(**uid)
        });
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
            libc::getpwuid_r(uid, pwd.as_mut_ptr(), buf.as_mut_ptr().cast::<libc::c_char>(), buf_size, &mut result)
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

/// Return the process architecture by reading the ELF header of the
/// executable.
///
/// Falls back to the system architecture if the ELF header cannot be
/// read (e.g. insufficient permissions or non-ELF binary).
pub fn query_architecture(pid: u32) -> Option<String> {
    // Try reading the ELF header from the exe path obtained via sysinfo.
    if let Some(exe_path) = query_executable_path(pid)
        && let Some(arch) = read_elf_architecture(&exe_path)
    {
        return Some(arch);
    }
    // Fallback: compile-time system architecture.
    Some(normalize_arch(std::env::consts::ARCH).to_string())
}

/// Read the `e_machine` field from an ELF file header.
fn read_elf_architecture(path: &str) -> Option<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = [0u8; 20]; // bytes 0..4 (magic) + bytes 18..20 (e_machine)
    file.read_exact(&mut header).ok()?;

    // Verify ELF magic: 0x7f 'E' 'L' 'F'
    if header[0..4] != [0x7f, b'E', b'L', b'F'] {
        return None;
    }
    // e_machine is at offset 18 in both 32-bit and 64-bit ELF headers.
    let e_machine = u16::from_le_bytes([header[18], header[19]]);
    Some(elf_machine_to_string(e_machine))
}
/// Map ELF `e_machine` to a human-readable architecture string.
fn elf_machine_to_string(machine: u16) -> String {
    match machine {
        0x03 => "x86",
        0x3E => "x64",
        0x28 => "arm",
        0xB7 => "arm64",
        0xF3 => "riscv",
        _ => "unknown",
    }
    .to_string()
}

/// Normalize a CPU architecture name to our canonical format.
fn normalize_arch(arch: &str) -> &str {
    match arch {
        "x86_64" | "amd64" => "x64",
        "x86" | "i386" | "i686" => "x86",
        "aarch64" => "arm64",
        "arm" | "armv7l" => "arm",
        "riscv64" => "riscv",
        other => other,
    }
}
