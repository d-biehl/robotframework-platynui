//! Process metadata for `app:Application` nodes.
//!
//! Mirrors the AT-SPI provider's `process.rs` (sysinfo-backed, PID in →
//! `Option<String>` out); the architecture sniff reads the PE header instead
//! of an ELF header. Kept crate-local for now — sharing a helper crate with
//! the other providers is a later cleanup (see the `add-jab-provider` design).

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

/// Executable stem (filename without extension), preferring the exe path.
pub(crate) fn query_process_name(pid: u32) -> Option<String> {
    with_process(pid, |p| {
        if let Some(exe) = p.exe()
            && let Some(stem) = exe.file_stem()
        {
            return Some(stem.to_string_lossy().into_owned());
        }
        let name = p.name().to_string_lossy();
        if name.is_empty() { None } else { Some(name.into_owned()) }
    })
}

/// Full path to the process executable.
pub(crate) fn query_executable_path(pid: u32) -> Option<String> {
    with_process(pid, |p| p.exe().map(|path| path.to_string_lossy().into_owned()))
}

/// Command line as a single space-separated string.
pub(crate) fn query_command_line(pid: u32) -> Option<String> {
    with_process(pid, |p| {
        let cmd = p.cmd();
        if cmd.is_empty() {
            return None;
        }
        let joined: String = cmd.iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" ");
        if joined.is_empty() { None } else { Some(joined) }
    })
}

/// Account name owning the process (resolved via the sysinfo users table).
pub(crate) fn query_user_name(pid: u32) -> Option<String> {
    let uid = with_process(pid, |p| p.user_id().cloned())?;
    let users = sysinfo::Users::new_with_refreshed_list();
    users.iter().find(|user| *user.id() == uid).map(|user| user.name().to_string())
}

/// Process start time as an ISO 8601 UTC string.
pub(crate) fn query_start_time(pid: u32) -> Option<String> {
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

/// Process architecture from the executable's PE header, falling back to the
/// compile-time host architecture.
#[allow(clippy::unnecessary_wraps)] // keeps the process-metadata helpers uniform
pub(crate) fn query_architecture(pid: u32) -> Option<String> {
    if let Some(exe_path) = query_executable_path(pid)
        && let Some(arch) = read_pe_architecture(&exe_path)
    {
        return Some(arch);
    }
    Some(normalize_arch(std::env::consts::ARCH).to_string())
}

/// Read the COFF `Machine` field from a PE file header.
fn read_pe_architecture(path: &str) -> Option<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = vec![0u8; 4096];
    let n = file.read(&mut header).ok()?;
    let data = &header[..n];
    if data.len() < 0x40 || &data[0..2] != b"MZ" {
        return None;
    }
    let e_lfanew = u32::from_le_bytes([data[0x3C], data[0x3D], data[0x3E], data[0x3F]]) as usize;
    if data.len() < e_lfanew + 4 + 20 || &data[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return None;
    }
    let machine = u16::from_le_bytes([data[e_lfanew + 4], data[e_lfanew + 5]]);
    Some(
        match machine {
            0x8664 => "x64",
            0x014c => "x86",
            0xAA64 => "arm64",
            0x01C0 | 0x01C4 => "arm",
            _ => "unknown",
        }
        .to_string(),
    )
}

/// Normalize a CPU architecture name to the canonical format.
fn normalize_arch(arch: &str) -> &str {
    match arch {
        "x86_64" | "amd64" => "x64",
        "x86" | "i386" | "i686" => "x86",
        "aarch64" => "arm64",
        "arm" | "armv7l" => "arm",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_process_is_queryable() {
        let pid = std::process::id();
        let name = query_process_name(pid).expect("own process name");
        assert!(!name.is_empty());
        let exe = query_executable_path(pid).expect("own exe path");
        assert!(exe.to_ascii_lowercase().ends_with(".exe"));
    }

    #[test]
    fn own_exe_pe_header_yields_known_arch() {
        let pid = std::process::id();
        let arch = query_architecture(pid).expect("architecture");
        assert!(["x64", "x86", "arm64", "arm"].contains(&arch.as_str()), "unexpected arch {arch}");
    }

    #[test]
    fn normalize_arch_maps_rust_names() {
        assert_eq!(normalize_arch("x86_64"), "x64");
        assert_eq!(normalize_arch("aarch64"), "arm64");
        assert_eq!(normalize_arch("riscv64"), "riscv64");
    }
}
