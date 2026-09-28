//! The Linux reader: `sysinfo` for the process table, `getpwuid_r` for the
//! user name, and `/proc/<pid>/cmdline` for the command line.
//!
//! Each read refreshes only this pid, and only what it needs. The start time in
//! `/proc/<pid>/stat` must equal the recorded one before and after the read;
//! otherwise the pid may have passed to another process in between, and the
//! read answers nothing.
//!
//! The command line is read from `/proc` itself: `sysinfo`'s `cmd()` trims every
//! argument and drops empty ones, which would report another command line than
//! the one the process has.

use super::{ProcessAttribute, ProcessAttributes, format_start_time, join_command_line};
use crate::sys::read_stat;
use std::path::Path;
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub(super) fn read(pid: u32, start: u64, attribute: ProcessAttribute) -> Option<String> {
    let refresh = match attribute {
        ProcessAttribute::ProcessName | ProcessAttribute::ExecutablePath => {
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always)
        }
        ProcessAttribute::CommandLine => return recorded(pid, start, || command_line(pid)).flatten(),
        ProcessAttribute::UserName => ProcessRefreshKind::nothing().with_user(UpdateKind::Always),
        ProcessAttribute::StartTime => ProcessRefreshKind::nothing(),
        ProcessAttribute::Architecture => return None,
    };
    with_recorded(pid, start, refresh, |process| match attribute {
        ProcessAttribute::ProcessName => process_name(process),
        ProcessAttribute::ExecutablePath => executable_path(process),
        ProcessAttribute::UserName => user_name(process),
        ProcessAttribute::StartTime => start_time(process),
        ProcessAttribute::CommandLine | ProcessAttribute::Architecture => None,
    })
    .flatten()
}

pub(super) fn read_all(pid: u32, start: u64) -> ProcessAttributes {
    let refresh = ProcessRefreshKind::nothing().with_exe(UpdateKind::Always).with_user(UpdateKind::Always);
    with_recorded(pid, start, refresh, |process| {
        let mut attributes = ProcessAttributes::default();
        attributes.set(ProcessAttribute::ProcessName, process_name(process));
        attributes.set(ProcessAttribute::ExecutablePath, executable_path(process));
        attributes.set(ProcessAttribute::CommandLine, command_line(pid));
        attributes.set(ProcessAttribute::UserName, user_name(process));
        attributes.set(ProcessAttribute::StartTime, start_time(process));
        attributes
    })
    .unwrap_or_default()
}

/// Applies `read` while the pid stands for the recorded process before and
/// after it.
fn recorded<R>(pid: u32, start: u64, read: impl FnOnce() -> R) -> Option<R> {
    let is_recorded = || read_stat(pid).is_some_and(|stat| stat.start == start && !stat.has_ended());
    if !is_recorded() {
        return None;
    }
    let value = read();
    is_recorded().then_some(value)
}

/// Refreshes `pid` with `refresh` and applies `read` to it, while the pid still
/// stands for the recorded process before and after the refresh.
fn with_recorded<R>(pid: u32, start: u64, refresh: ProcessRefreshKind, read: impl FnOnce(&Process) -> R) -> Option<R> {
    recorded(pid, start, || {
        let mut system = System::new();
        let sysinfo_pid = Pid::from_u32(pid);
        // Tasks are left out: a process's threads are not what is read here.
        system.refresh_processes_specifics(ProcessesToUpdate::Some(&[sysinfo_pid]), true, refresh.without_tasks());
        system.process(sysinfo_pid).map(read)
    })
    .flatten()
}

/// The file name of the executable, whole: `python3.12` stays `python3.12`.
/// There is no fallback to `comm`, which is truncated and can be set by the
/// process itself.
fn process_name(process: &Process) -> Option<String> {
    process.exe()?.file_name()?.to_str().map(str::to_owned)
}

/// The target of `/proc/<pid>/exe`, without the ` (deleted)` the kernel appends
/// once the file has been replaced or removed; `sysinfo` strips it.
fn executable_path(process: &Process) -> Option<String> {
    process.exe().and_then(Path::to_str).map(str::to_owned)
}

/// `/proc/<pid>/cmdline`: the arguments joined by single spaces, the way `ps`
/// shows them. A kernel thread has none.
fn command_line(pid: u32) -> Option<String> {
    join_command_line(&std::fs::read(format!("/proc/{pid}/cmdline")).ok()?)
}

/// The login name of the effective user. There is no fallback to the real
/// user, which is another account for a set-uid program.
fn user_name(process: &Process) -> Option<String> {
    resolve_username(**process.effective_user_id()?)
}

/// The start time, which `sysinfo` computes from the boot time and field 22 of
/// `stat`; absent when it answers `0`.
fn start_time(process: &Process) -> Option<String> {
    match process.start_time() {
        0 => None,
        seconds => format_start_time(seconds),
    }
}

/// Resolves a UID to a login name through `getpwuid_r(3)`.
///
/// It goes through the Name Service Switch, so it also finds accounts from LDAP,
/// SSSD, NIS or systemd-homed, which a list of users may miss when enumeration
/// is disabled.
fn resolve_username(uid: u32) -> Option<String> {
    // The initial buffer; grown when `getpwuid_r` answers ERANGE.
    let mut buffer_size = 1024_usize;
    loop {
        let mut buffer = vec![0u8; buffer_size];
        let mut entry: std::mem::MaybeUninit<libc::passwd> = std::mem::MaybeUninit::uninit();
        let mut result: *mut libc::passwd = std::ptr::null_mut();
        // SAFETY: `entry`, `buffer` of `buffer_size` bytes and `result` are valid
        // for the duration of the call.
        let code = unsafe {
            libc::getpwuid_r(
                uid,
                entry.as_mut_ptr(),
                buffer.as_mut_ptr().cast::<libc::c_char>(),
                buffer_size,
                &raw mut result,
            )
        };
        if code == libc::ERANGE {
            buffer_size = buffer_size.checked_mul(2)?;
            continue;
        }
        if code != 0 || result.is_null() {
            return None;
        }
        // SAFETY: `result` is non-null and points at the initialized `entry`,
        // whose `pw_name` is a NUL-terminated string inside `buffer`.
        let name = unsafe { std::ffi::CStr::from_ptr((*result).pw_name) };
        return Some(name.to_string_lossy().into_owned());
    }
}
