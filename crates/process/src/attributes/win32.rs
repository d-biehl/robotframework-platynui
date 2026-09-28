//! The Windows reader: native Win32 calls on one handle of the recorded process.
//!
//! The handle carries the limited query right only (`crate::sys::Process::open`),
//! and every call here works with it. It is opened once per read, and read
//! through only after the process is confirmed to run with the recorded creation
//! time. Windows does not hand a pid to another process while a handle to its
//! process is open, so everything read through the handle describes the
//! recorded process.

use super::{
    ProcessAttribute, ProcessAttributes, choose_architecture, file_time_to_unix_seconds, format_start_time, strip_exe,
};
use crate::sys::Process;
use std::path::Path;
use windows::Wdk::System::Threading::{NtQueryInformationProcess, ProcessCommandLineInformation};
use windows::Win32::Foundation::{CloseHandle, HANDLE, UNICODE_STRING};
use windows::Win32::Security::{
    GetTokenInformation, LookupAccountSidW, SID_NAME_USE, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::System::SystemInformation::IMAGE_FILE_MACHINE;
use windows::Win32::System::Threading::{
    GetProcessInformation, IsWow64Process2, OpenProcessToken, PROCESS_MACHINE_INFORMATION, PROCESS_NAME_WIN32,
    ProcessMachineTypeInfo, QueryFullProcessImageNameW,
};
use windows::core::PWSTR;

pub(super) fn read(pid: u32, start: u64, attribute: ProcessAttribute) -> Option<String> {
    let process = open_recorded(pid, start)?;
    match attribute {
        ProcessAttribute::ProcessName => executable_path(&process).as_deref().and_then(process_name),
        ProcessAttribute::ExecutablePath => executable_path(&process),
        ProcessAttribute::CommandLine => command_line(&process),
        ProcessAttribute::UserName => user_name(&process),
        ProcessAttribute::StartTime => start_time(start),
        ProcessAttribute::Architecture => architecture(&process).map(str::to_owned),
    }
}

pub(super) fn read_all(pid: u32, start: u64) -> ProcessAttributes {
    let mut attributes = ProcessAttributes::default();
    let Some(process) = open_recorded(pid, start) else {
        return attributes;
    };
    let path = executable_path(&process);
    attributes.set(ProcessAttribute::ProcessName, path.as_deref().and_then(process_name));
    attributes.set(ProcessAttribute::ExecutablePath, path);
    attributes.set(ProcessAttribute::CommandLine, command_line(&process));
    attributes.set(ProcessAttribute::UserName, user_name(&process));
    attributes.set(ProcessAttribute::StartTime, start_time(start));
    attributes.set(ProcessAttribute::Architecture, architecture(&process).map(str::to_owned));
    attributes
}

/// The recorded process, open, while it runs and is still the recorded one.
fn open_recorded(pid: u32, start: u64) -> Option<Process> {
    let process = Process::open(pid).ok()?;
    (process.is_running() == Some(true) && process.creation_time() == Some(start)).then_some(process)
}

/// The file name of the executable, without a trailing `.exe`.
fn process_name(path: &str) -> Option<String> {
    Path::new(path).file_name()?.to_str().map(|name| strip_exe(name).to_owned())
}

/// The recorded creation time, which the identity read from this process.
fn start_time(start: u64) -> Option<String> {
    file_time_to_unix_seconds(start).and_then(format_start_time)
}

/// `QueryFullProcessImageNameW` in the Win32 form, into one buffer that holds
/// the longest path Windows supports.
fn executable_path(process: &Process) -> Option<String> {
    const CAPACITY: u32 = 32_768;
    let mut buffer = vec![0u16; CAPACITY as usize];
    let mut size = CAPACITY;
    // SAFETY: `buffer` holds `size` UTF-16 units, and `size` is a valid in/out
    // pointer; the handle is live for the call.
    unsafe {
        QueryFullProcessImageNameW(process.handle(), PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &raw mut size)
    }
    .ok()?;
    String::from_utf16(buffer.get(..size as usize)?).ok()
}

/// The command line the process was started with, verbatim, from
/// `NtQueryInformationProcess(ProcessCommandLineInformation)`. The answer is a
/// `UNICODE_STRING` whose text follows it in the same buffer; one buffer holds
/// the longest one a `UNICODE_STRING` can describe.
fn command_line(process: &Process) -> Option<String> {
    // Whole `u64`s, so that the `UNICODE_STRING` at its start is aligned.
    const CAPACITY_BYTES: usize = size_of::<UNICODE_STRING>() + u16::MAX as usize + 1;
    let mut buffer = vec![0u64; CAPACITY_BYTES.div_ceil(size_of::<u64>())];
    let capacity = u32::try_from(buffer.len() * size_of::<u64>()).ok()?;
    let mut returned = 0u32;
    // SAFETY: `buffer` holds `capacity` writable bytes and `returned` is a valid
    // out-pointer; the handle is live for the call.
    unsafe {
        NtQueryInformationProcess(
            process.handle(),
            ProcessCommandLineInformation,
            buffer.as_mut_ptr().cast(),
            capacity,
            &raw mut returned,
        )
    }
    .ok()
    .ok()?;
    // SAFETY: the call succeeded, so the buffer starts with a UNICODE_STRING,
    // and the buffer is aligned for it.
    let header = unsafe { &*buffer.as_ptr().cast::<UNICODE_STRING>() };
    let length = usize::from(header.Length) / 2;
    let text = header.Buffer.0.cast_const();
    // The text must lie within the buffer the call filled.
    let start = buffer.as_ptr().addr();
    let end = start + buffer.len() * size_of::<u64>();
    let (first, past) = (text.addr(), text.addr() + length * 2);
    if length == 0 || text.is_null() || first < start || past > end {
        return None;
    }
    // SAFETY: `text` points at `length` UTF-16 units inside `buffer`, checked above.
    String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) }).ok()
}

/// A token handle, closed when dropped.
struct Token(HANDLE);

impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by OpenProcessToken and is closed only here.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// The token user, as `DOMAIN\user`. `LookupAccountSidW` answers the computer
/// name as the domain of a local account.
fn user_name(process: &Process) -> Option<String> {
    let mut token = HANDLE::default();
    // SAFETY: the process handle is live, and `token` is a valid out-pointer.
    unsafe { OpenProcessToken(process.handle(), TOKEN_QUERY, &raw mut token) }.ok()?;
    let token = Token(token);

    let mut needed = 0u32;
    // SAFETY: a size query without a buffer; `needed` is a valid out-pointer.
    let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &raw mut needed) };
    if needed == 0 {
        return None;
    }
    // Whole `u64`s, so that the `TOKEN_USER` at its start is aligned.
    let mut buffer = vec![0u64; (needed as usize).div_ceil(size_of::<u64>())];
    // SAFETY: `buffer` holds at least `needed` writable bytes.
    unsafe { GetTokenInformation(token.0, TokenUser, Some(buffer.as_mut_ptr().cast()), needed, &raw mut needed) }
        .ok()?;
    // SAFETY: the call succeeded, so the buffer starts with a TOKEN_USER whose
    // SID points into the same buffer, which outlives its use below.
    let sid = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() }.User.Sid;

    let (mut name_length, mut domain_length) = (0u32, 0u32);
    let mut kind = SID_NAME_USE::default();
    // SAFETY: a size query without buffers; all out-pointers are valid.
    let _ = unsafe {
        LookupAccountSidW(None, sid, None, &raw mut name_length, None, &raw mut domain_length, &raw mut kind)
    };
    if name_length == 0 {
        return None;
    }
    let mut name = vec![0u16; name_length as usize];
    let mut domain = vec![0u16; domain_length.max(1) as usize];
    domain_length = u32::try_from(domain.len()).ok()?;
    // SAFETY: `name` and `domain` hold the lengths passed with them; all
    // out-pointers are valid.
    unsafe {
        LookupAccountSidW(
            None,
            sid,
            Some(PWSTR(name.as_mut_ptr())),
            &raw mut name_length,
            Some(PWSTR(domain.as_mut_ptr())),
            &raw mut domain_length,
            &raw mut kind,
        )
    }
    .ok()?;
    let name = String::from_utf16(name.get(..name_length as usize)?).ok()?;
    let domain = String::from_utf16(domain.get(..domain_length as usize)?).ok()?;
    match (domain.is_empty(), name.is_empty()) {
        (_, true) => None,
        (true, false) => Some(name),
        (false, false) => Some(format!("{domain}\\{name}")),
    }
}

/// The architecture the process's code runs as (design D3).
fn architecture(process: &Process) -> Option<&'static str> {
    choose_architecture(machine_type_info(process), || wow64_machines(process))
}

/// The primary architecture path: the process's own machine from
/// `GetProcessInformation(ProcessMachineTypeInfo)`, which Windows knows from
/// build 22000 on. It reports an emulated x64 process as `AMD64`.
pub(super) fn machine_type_info(process: &Process) -> Option<u16> {
    let mut info = PROCESS_MACHINE_INFORMATION::default();
    let size = u32::try_from(size_of::<PROCESS_MACHINE_INFORMATION>()).ok()?;
    // SAFETY: `info` is a writable PROCESS_MACHINE_INFORMATION of `size` bytes.
    unsafe { GetProcessInformation(process.handle(), ProcessMachineTypeInfo, (&raw mut info).cast(), size) }.ok()?;
    Some(info.ProcessMachine.0)
}

/// The fallback architecture path: the process machine and the native machine
/// from `IsWow64Process2`. The process machine is `UNKNOWN` for a process that
/// does not run under WOW64.
pub(super) fn wow64_machines(process: &Process) -> Option<(u16, u16)> {
    let (mut own, mut native) = (IMAGE_FILE_MACHINE::default(), IMAGE_FILE_MACHINE::default());
    // SAFETY: both are valid out-pointers; the handle is live for the call.
    unsafe { IsWow64Process2(process.handle(), &raw mut own, Some(&raw mut native)) }.ok()?;
    Some((own.0, native.0))
}
