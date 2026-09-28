//! The process attributes of a recorded process, read in one format on every
//! platform.
//!
//! Every read goes through a [`ProcessIdentity`]: it opens the process once,
//! confirms that it is still the recorded one, reads, and closes. An identity
//! recorded without a start time reads nothing, because no read through it could
//! tell the recorded process from one that received its pid.

use crate::ProcessIdentity;

/// A process attribute, and the one format a present value has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessAttribute {
    /// The file name of the executable the process runs, without its directory
    /// and, on Windows, without `.exe`: `javaw`, `python3.12`.
    ProcessName,
    /// The absolute path of that executable, as the platform presents it.
    ExecutablePath,
    /// On Windows the command line, verbatim, quoting included; on Linux the
    /// arguments joined by single spaces.
    CommandLine,
    /// The account the process runs as: `DOMAIN\user` on Windows, with the
    /// computer name as the domain of a local account; the login name of the
    /// effective user on Linux.
    UserName,
    /// The moment the process was created, in UTC to the second:
    /// `YYYY-MM-DDTHH:MM:SSZ`.
    StartTime,
    /// The architecture the process's code runs as: `x86`, `x64`, `arm` or
    /// `arm64`. Windows only; Linux keeps no architecture per process.
    Architecture,
}

impl ProcessAttribute {
    /// Every attribute, in the order a listing uses.
    pub const ALL: [Self; 6] = [
        Self::ProcessName,
        Self::ExecutablePath,
        Self::CommandLine,
        Self::UserName,
        Self::StartTime,
        Self::Architecture,
    ];

    const fn index(self) -> usize {
        self as usize
    }
}

/// The process attributes one read found. An attribute that could not be read
/// is absent; none is ever empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessAttributes {
    values: [Option<String>; 6],
}

impl ProcessAttributes {
    /// The value of `attribute`, when it was read.
    #[must_use]
    pub fn get(&self, attribute: ProcessAttribute) -> Option<&str> {
        self.values[attribute.index()].as_deref()
    }

    /// The attributes that were read, in the order of [`ProcessAttribute::ALL`].
    pub fn iter(&self) -> impl Iterator<Item = (ProcessAttribute, &str)> + '_ {
        ProcessAttribute::ALL.into_iter().filter_map(|attribute| self.get(attribute).map(|value| (attribute, value)))
    }

    /// Whether no attribute was read.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.iter().all(Option::is_none)
    }

    /// Records `value` for `attribute`; an empty value counts as not read.
    #[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code, reason = "no reader on this platform"))]
    fn set(&mut self, attribute: ProcessAttribute, value: Option<String>) {
        self.values[attribute.index()] = value.filter(|value| !value.is_empty());
    }
}

impl ProcessIdentity {
    /// Reads one attribute of the recorded process.
    ///
    /// `None` when it cannot be read, when the process has ended or its pid
    /// belongs to another process now, and when no start time was recorded.
    #[must_use]
    pub fn read(&self, attribute: ProcessAttribute) -> Option<String> {
        let start = self.start?;
        platform::read(self.pid, start, attribute).filter(|value| !value.is_empty())
    }

    /// Reads every attribute of the recorded process, opening it once.
    ///
    /// Empty when the process has ended or its pid belongs to another process
    /// now, and when no start time was recorded.
    #[must_use]
    pub fn read_all(&self) -> ProcessAttributes {
        match self.start {
            Some(start) => platform::read_all(self.pid, start),
            None => ProcessAttributes::default(),
        }
    }
}

#[cfg(windows)]
#[allow(unsafe_code, reason = "the Windows process attributes are read through Win32 calls")]
mod win32;
#[cfg(windows)]
use win32 as platform;

#[cfg(target_os = "linux")]
#[allow(unsafe_code, reason = "the user name is resolved through getpwuid_r")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

/// No reader on this platform yet: every read answers nothing.
#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    use super::{ProcessAttribute, ProcessAttributes};

    pub(super) fn read(_pid: u32, _start: u64, _attribute: ProcessAttribute) -> Option<String> {
        None
    }

    pub(super) fn read_all(_pid: u32, _start: u64) -> ProcessAttributes {
        ProcessAttributes::default()
    }
}

/// The `IMAGE_FILE_MACHINE_*` values the architecture is read from.
#[cfg(any(windows, test))]
mod machine {
    pub(crate) const UNKNOWN: u16 = 0;
    pub(crate) const I386: u16 = 0x014C;
    pub(crate) const ARM: u16 = 0x01C0;
    pub(crate) const ARMNT: u16 = 0x01C4;
    pub(crate) const AMD64: u16 = 0x8664;
    pub(crate) const ARM64: u16 = 0xAA64;
}

/// The architecture a machine value names; nothing for any other machine.
#[cfg(any(windows, test))]
const fn machine_architecture(machine: u16) -> Option<&'static str> {
    match machine {
        machine::I386 => Some("x86"),
        machine::AMD64 => Some("x64"),
        machine::ARM | machine::ARMNT => Some("arm"),
        machine::ARM64 => Some("arm64"),
        _ => None,
    }
}

/// The architecture `IsWow64Process2` reports: the process's machine for a
/// WOW64 process, and the native machine for any other process, whose own
/// machine it answers as `UNKNOWN`.
#[cfg(any(windows, test))]
const fn wow64_architecture(process: u16, native: u16) -> Option<&'static str> {
    machine_architecture(if process == machine::UNKNOWN { native } else { process })
}

/// The architecture from the two paths: the primary call's machine wins, even
/// one that maps to nothing; the fallback is asked only when the primary call
/// failed.
#[cfg(any(windows, test))]
fn choose_architecture(primary: Option<u16>, fallback: impl FnOnce() -> Option<(u16, u16)>) -> Option<&'static str> {
    match primary {
        Some(machine) => machine_architecture(machine),
        None => fallback().and_then(|(process, native)| wow64_architecture(process, native)),
    }
}

/// A file name with a trailing `.exe` removed, in any case. A name that is
/// nothing but the extension keeps it.
#[cfg(any(windows, test))]
fn strip_exe(name: &str) -> &str {
    const EXE: &str = ".exe";
    match name.len().checked_sub(EXE.len()) {
        Some(stem) if stem > 0 && name.is_char_boundary(stem) && name[stem..].eq_ignore_ascii_case(EXE) => {
            &name[..stem]
        }
        _ => name,
    }
}

/// Seconds since 1970 for a `FILETIME` (100 ns units since 1601), truncated to
/// the second; nothing before 1970.
#[cfg(any(windows, test))]
const fn file_time_to_unix_seconds(file_time: u64) -> Option<u64> {
    /// Seconds from 1601-01-01 to 1970-01-01.
    const EPOCH_DIFFERENCE: u64 = 11_644_473_600;
    (file_time / 10_000_000).checked_sub(EPOCH_DIFFERENCE)
}

/// The command line in a `/proc/<pid>/cmdline`: the arguments joined by single
/// spaces, the way `ps` shows them. Every argument stays as it is, surrounding
/// whitespace and empty arguments included; only the NUL the kernel ends the
/// last one with goes. Nothing for an empty file, which a kernel thread has.
#[cfg(any(target_os = "linux", test))]
fn join_command_line(bytes: &[u8]) -> Option<String> {
    let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    if bytes.is_empty() {
        return None;
    }
    let joined: Vec<u8> = bytes.iter().map(|&byte| if byte == 0 { b' ' } else { byte }).collect();
    Some(String::from_utf8_lossy(&joined).into_owned())
}

/// Seconds since 1970 as `YYYY-MM-DDTHH:MM:SSZ`; nothing beyond the year 9999.
#[cfg_attr(not(any(windows, target_os = "linux", test)), allow(dead_code, reason = "no reader on this platform"))]
fn format_start_time(unix_seconds: u64) -> Option<String> {
    let days = unix_seconds / 86_400;
    let seconds_of_day = unix_seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    if year > 9999 {
        return None;
    }
    let (hour, minute, second) = (seconds_of_day / 3600, seconds_of_day % 3600 / 60, seconds_of_day % 60);
    Some(format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"))
}

/// The proleptic Gregorian date `days` after 1970-01-01: Howard Hinnant's
/// `civil_from_days`, for days on or after the epoch.
#[cfg_attr(not(any(windows, target_os = "linux", test)), allow(dead_code, reason = "no reader on this platform"))]
const fn civil_from_days(days: u64) -> (u64, u64, u64) {
    // Days since 0000-03-01, so that a leap day ends its year.
    let days = days + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 };
    let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

#[cfg(test)]
mod tests;
