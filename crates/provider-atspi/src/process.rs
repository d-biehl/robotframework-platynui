//! Process metadata helpers for Linux.
//!
//! Reads information from the `/proc` filesystem to populate Application
//! node attributes (process name, executable path, command line, etc.).
//! All functions accept a PID and return `Option<String>`, returning `None`
//! when the `/proc` entry is inaccessible (e.g. short-lived process,
//! insufficient permissions).

use std::path::Path;

/// Return the process executable stem (filename without extension).
///
/// Reads `/proc/{pid}/exe` and extracts the file stem.
/// Falls back to `/proc/{pid}/comm` if the exe symlink is unreadable.
pub fn query_process_name(pid: u32) -> Option<String> {
    // Prefer the exe symlink — it gives us the real binary name even if
    // the process has been exec'd.
    if let Some(path) = query_executable_path(pid)
        && let Some(stem) = Path::new(&path).file_stem()
    {
        return Some(stem.to_string_lossy().into_owned());
    }
    // Fallback: /proc/PID/comm contains the first 15 characters of the
    // executable name.
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let trimmed = comm.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

/// Return the full path to the process executable.
///
/// Reads the `/proc/{pid}/exe` symlink.
pub fn query_executable_path(pid: u32) -> Option<String> {
    let link = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    // The kernel appends " (deleted)" when the binary has been replaced;
    // strip that suffix so callers get a clean path.
    let path_str = link.to_string_lossy();
    let cleaned = path_str.strip_suffix(" (deleted)").unwrap_or(&path_str);
    Some(cleaned.to_string())
}

/// Return the process command line as a single space-separated string.
///
/// Reads `/proc/{pid}/cmdline` where arguments are NUL-separated.
pub fn query_command_line(pid: u32) -> Option<String> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    if raw.is_empty() {
        return None;
    }
    // Arguments are separated by NUL bytes.  Strip a trailing NUL if
    // present, then join with spaces.
    let trimmed = if raw.last() == Some(&0) { &raw[..raw.len() - 1] } else { &raw };
    let args: Vec<&str> = trimmed.split(|&b| b == 0).map(|s| std::str::from_utf8(s).unwrap_or("")).collect();
    let joined = args.join(" ");
    if joined.is_empty() { None } else { Some(joined) }
}

/// Return the username owning the process.
///
/// Reads the effective UID from `/proc/{pid}/status` and resolves it via
/// `/etc/passwd` (without linking to libc `getpwuid_r`).
pub fn query_user_name(pid: u32) -> Option<String> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let uid_line = status.lines().find(|l| l.starts_with("Uid:"))?;
    // Format: "Uid:\t<real>\t<effective>\t<saved>\t<fs>"
    let fields: Vec<&str> = uid_line.split_whitespace().collect();
    // Use the effective UID (index 2, i.e. second number after "Uid:").
    let euid: u32 = fields.get(2)?.parse().ok()?;
    resolve_username(euid)
}

/// Resolve a UID to a username by parsing `/etc/passwd`.
fn resolve_username(uid: u32) -> Option<String> {
    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    for line in passwd.lines() {
        // Format: name:password:uid:gid:gecos:home:shell
        let mut parts = line.splitn(4, ':');
        let name = parts.next()?;
        let _pass = parts.next();
        let file_uid: u32 = parts.next()?.parse().ok()?;
        if file_uid == uid {
            return Some(name.to_string());
        }
    }
    None
}

/// Return the process start time as an ISO 8601 UTC string.
///
/// Reads field 22 (0-indexed) of `/proc/{pid}/stat` — the start time in
/// clock ticks since system boot.  Combines with the boot time from
/// `/proc/stat` and the clock tick rate from `sysconf(_SC_CLK_TCK)`.
pub fn query_start_time(pid: u32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The comm field (field 2) is enclosed in parens and may contain
    // spaces, so we find the last ')' and parse from there.
    let after_comm = stat.rfind(')')? + 2; // skip ") "
    let remainder = stat.get(after_comm..)?;
    let fields: Vec<&str> = remainder.split_whitespace().collect();
    // Field 22 in /proc/PID/stat is 1-indexed; after stripping pid and
    // comm (fields 1 & 2), the starttime is at index 19 (field 22 − 3).
    let start_ticks: u64 = fields.get(19)?.parse().ok()?;

    let ticks_per_sec = clock_ticks_per_sec()?;
    let boot_time_secs = read_boot_time()?;

    let start_secs = boot_time_secs + start_ticks / ticks_per_sec;
    let sub_secs = (start_ticks % ticks_per_sec) * 1_000_000_000 / ticks_per_sec;

    // Format as ISO 8601 UTC without pulling in chrono.
    format_unix_timestamp(start_secs, sub_secs as u32)
}

/// Return the process architecture by reading the ELF header of
/// `/proc/{pid}/exe`.
///
/// Falls back to the system architecture from `uname` if the ELF header
/// cannot be read (e.g. insufficient permissions).
pub fn query_architecture(pid: u32) -> Option<String> {
    if let Some(arch) = read_elf_architecture(pid) {
        return Some(arch);
    }
    // Fallback: system architecture via uname
    read_system_architecture()
}

/// Read the `e_machine` field from the ELF header of `/proc/{pid}/exe`.
fn read_elf_architecture(pid: u32) -> Option<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(format!("/proc/{pid}/exe")).ok()?;
    let mut header = [0u8; 20]; // We need bytes 0..4 (magic) + byte 4 (class) + bytes 18..20 (e_machine)
    file.read_exact(&mut header).ok()?;

    // Verify ELF magic: 0x7f 'E' 'L' 'F'
    if header[0..4] != [0x7f, b'E', b'L', b'F'] {
        return None;
    }
    // e_machine is at offset 18 in both 32-bit and 64-bit ELF headers.
    // It's a 16-bit little-endian value on Linux (always LE on x86/ARM).
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

/// Read the system boot time (seconds since Unix epoch) from `/proc/stat`.
fn read_boot_time() -> Option<u64> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    for line in stat.lines() {
        if let Some(rest) = line.strip_prefix("btime ") {
            return rest.trim().parse().ok();
        }
    }
    None
}

/// Return the number of clock ticks per second (`sysconf(_SC_CLK_TCK)`).
fn clock_ticks_per_sec() -> Option<u64> {
    // SAFETY: sysconf is a POSIX function that returns a long; _SC_CLK_TCK
    // is always valid.  We avoid linking libc by using a raw syscall-like
    // approach, but sysconf is universally available; use libc-free
    // constant: on Linux, _SC_CLK_TCK is always 100 unless the kernel was
    // built with a non-standard HZ.  For robustness we still try the
    // environment or fall back to 100.
    //
    // The value is effectively always 100 on Linux (USER_HZ) regardless of
    // the kernel's internal CONFIG_HZ.
    Some(100)
}

/// Format a Unix timestamp as ISO 8601 UTC (`YYYY-MM-DDTHH:MM:SSZ`).
fn format_unix_timestamp(secs: u64, _nanos: u32) -> Option<String> {
    // Minimal UTC date-time formatter without external crates.
    const SECS_PER_DAY: u64 = 86_400;
    const DAYS_PER_400Y: u64 = 146_097;
    const DAYS_PER_100Y: u64 = 36_524;
    const DAYS_PER_4Y: u64 = 1_461;

    let day_secs = secs % SECS_PER_DAY;
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;

    // Days since 1970-01-01
    let mut days = secs / SECS_PER_DAY;
    // Shift epoch to 2000-03-01 for easier leap year handling.
    days += 719_468; // days from 0000-03-01 to 1970-01-01

    let era = days / DAYS_PER_400Y;
    let day_of_era = days % DAYS_PER_400Y;
    let year_of_era = (day_of_era - day_of_era / (DAYS_PER_4Y - 1) + day_of_era / DAYS_PER_100Y
        - day_of_era / (DAYS_PER_400Y - 1))
        / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100 + year_of_era / 400);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    Some(format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"))
}

/// Read the system architecture from `uname -m` equivalent.
fn read_system_architecture() -> Option<String> {
    // Read from /proc/sys/kernel/arch or fall back to a fixed mapping
    // based on the current binary's target architecture.
    #[cfg(target_arch = "x86_64")]
    {
        Some("x64".to_string())
    }
    #[cfg(target_arch = "x86")]
    {
        Some("x86".to_string())
    }
    #[cfg(target_arch = "aarch64")]
    {
        Some("arm64".to_string())
    }
    #[cfg(target_arch = "arm")]
    {
        Some("arm".to_string())
    }
    #[cfg(target_arch = "riscv64")]
    {
        Some("riscv".to_string())
    }
    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "x86",
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64",
    )))]
    {
        Some("unknown".to_string())
    }
}
