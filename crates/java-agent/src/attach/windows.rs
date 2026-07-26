//! The Windows leg of the attach protocol.
//!
//! This is the one genuinely `unsafe` part of the transport, and the one the
//! design flagged for a focused review. What it does, and why each step is
//! needed:
//!
//! 1. **Create a named pipe** for the reply. HotSpot writes an attach
//!    operation's result to a pipe the *client* creates and names.
//! 2. **Locate `JVM_EnqueueOperation`** in the target. Its address is found by
//!    loading the target's *own* `jvm.dll` locally (without running its
//!    initialisation) to get the export's offset from the module base, then
//!    applying that offset to the module base in the target. ASLR makes the
//!    two bases differ; the offset does not.
//! 3. **Inject a small stub plus its argument block** and run it with
//!    `CreateRemoteThread`. A stub is unavoidable: a thread start routine
//!    receives exactly one pointer, and `JVM_EnqueueOperation` takes five.
//! 4. **Read the reply** off the pipe, bounded by a deadline.
//!
//! Every wait has a timeout. A remote thread we cannot join, or a JVM that
//! never answers, must surface as an error — never as a hung test run.
//!
//! Only x86-64 is implemented. A 32-bit or ARM64 target needs its own stub
//! encoding, and both fail loudly here rather than corrupting a foreign
//! process.

#![allow(unsafe_code)]

use crate::error::AgentError;
use crate::jvm;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, ERROR_IO_INCOMPLETE, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, FreeLibrary,
    GetLastError, HANDLE, WAIT_OBJECT_0,
};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_OVERLAPPED, PIPE_ACCESS_INBOUND, ReadFile};
use windows::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::LibraryLoader::{DONT_RESOLVE_DLL_REFERENCES, GetProcAddress, LoadLibraryExW};
use windows::Win32::System::Memory::{
    MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READWRITE, VirtualAllocEx, VirtualFreeEx,
};
use windows::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_WAIT};
use windows::Win32::System::Threading::{
    CreateEventW, CreateRemoteThread, GetExitCodeThread, OpenProcess, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION,
    PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, WaitForSingleObject,
};
use windows::core::{PCSTR, PCWSTR};

/// The export that enqueues an attach operation inside a HotSpot JVM.
const ENQUEUE_EXPORT: &[u8] = b"JVM_EnqueueOperation\0";

/// HotSpot's own limit on an attach argument (`AttachOperation::arg_length_max`).
const MAX_ARG_LEN: usize = 1024;

/// Layout of the argument block the injected stub reads. The offsets are baked
/// into the stub's `lea` displacements, so the two must change together — which
/// the stub-encoding test pins.
const OFF_FUNC: usize = 0;
const OFF_CMD: usize = 8;
const LEN_CMD: usize = 32;
const OFF_PIPE: usize = OFF_CMD + LEN_CMD;
const LEN_PIPE: usize = 264;
const OFF_ARG0: usize = OFF_PIPE + LEN_PIPE;
const LEN_ARG: usize = MAX_ARG_LEN + 1;
const OFF_ARG1: usize = OFF_ARG0 + LEN_ARG;
const OFF_ARG2: usize = OFF_ARG1 + LEN_ARG;
const DATA_SIZE: usize = OFF_ARG2 + LEN_ARG;

/// The stub is placed at the start of the allocation, the argument block after
/// it. 64 bytes leaves room for the ~54-byte stub and keeps the block aligned.
const CODE_AREA: usize = 64;

// The slots must not overlap, and the block must end where the last one does —
// a layout bug would make the target read garbage pointers, so it fails the
// build rather than the run.
const _: () = assert!(OFF_CMD + LEN_CMD <= OFF_PIPE);
const _: () = assert!(OFF_PIPE + LEN_PIPE <= OFF_ARG0);
const _: () = assert!(OFF_ARG0 + LEN_ARG <= OFF_ARG1);
const _: () = assert!(OFF_ARG1 + LEN_ARG <= OFF_ARG2);
const _: () = assert!(OFF_ARG2 + LEN_ARG == DATA_SIZE);

pub fn execute(pid: u32, command: &str, args: &[&str], timeout: Duration) -> Result<String, AgentError> {
    if !cfg!(target_arch = "x86_64") {
        return Err(AgentError::AttachFailed {
            pid,
            details: "the native attach transport implements only x86-64 targets; \
                      load the agent with -javaagent instead"
                .to_owned(),
        });
    }
    for arg in args {
        if arg.len() > MAX_ARG_LEN {
            return Err(AgentError::AttachFailed {
                pid,
                details: format!("attach argument of {} bytes exceeds the JVM's limit of {MAX_ARG_LEN}", arg.len()),
            });
        }
    }

    let pipe_name = unique_pipe_name(pid);
    let pipe = Pipe::create(&pipe_name).map_err(|details| AgentError::AttachFailed { pid, details })?;
    let process = Process::open(pid)?;
    let enqueue = remote_enqueue_address(pid)?;

    let deadline = Instant::now() + timeout;
    let exit_code = process.run_stub(pid, enqueue, command, &pipe_name, args, timeout)?;
    if exit_code != 0 {
        return Err(AgentError::AttachFailed {
            pid,
            details: format!("JVM_EnqueueOperation returned {exit_code} — the JVM rejected the attach request"),
        });
    }

    pipe.wait_for_client(pid, remaining(deadline))?;
    pipe.read_reply(pid, deadline)
}

// ------------------------------------------------------------------ process

/// An owned target-process handle.
struct Process {
    handle: HANDLE,
}

impl Process {
    fn open(pid: u32) -> Result<Self, AgentError> {
        let access = PROCESS_QUERY_INFORMATION
            | PROCESS_CREATE_THREAD
            | PROCESS_VM_OPERATION
            | PROCESS_VM_WRITE
            | PROCESS_VM_READ;
        // SAFETY: opening a process by pid; the handle is closed in `Drop`.
        let handle = unsafe { OpenProcess(access, false, pid) }.map_err(|e| AgentError::AttachFailed {
            pid,
            details: format!("cannot open process {pid} for attach: {e} (a different user, or elevated?)"),
        })?;
        Ok(Self { handle })
    }

    /// Writes the stub and its arguments into the target, runs it, and returns
    /// what `JVM_EnqueueOperation` returned.
    fn run_stub(
        &self,
        pid: u32,
        enqueue: usize,
        command: &str,
        pipe_name: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<u32, AgentError> {
        let allocation = RemoteMemory::allocate(self.handle, pid, CODE_AREA + DATA_SIZE)?;
        let mut buffer = vec![0u8; CODE_AREA + DATA_SIZE];
        let stub = build_stub();
        buffer[..stub.len()].copy_from_slice(&stub);

        // The argument block starts right after the code area, and the stub
        // addresses every field relative to its own base — so `data` is that
        // base and the layout offsets apply to it unchanged.
        let data_base = allocation.address + CODE_AREA;
        let data = &mut buffer[CODE_AREA..];
        write_field(data, OFF_FUNC, &(enqueue as u64).to_le_bytes());
        write_string(data, OFF_CMD, LEN_CMD, command)?;
        write_string(data, OFF_PIPE, LEN_PIPE, pipe_name)?;
        for (index, offset) in [OFF_ARG0, OFF_ARG1, OFF_ARG2].into_iter().enumerate() {
            write_string(data, offset, LEN_ARG, args.get(index).copied().unwrap_or(""))?;
        }

        allocation.write(pid, &buffer)?;
        let thread = self.start_thread(pid, allocation.address, data_base)?;
        thread.join(pid, timeout)
    }

    fn start_thread(&self, pid: u32, code: usize, parameter: usize) -> Result<Thread, AgentError> {
        use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;

        // SAFETY: the range was just written by us and is committed as
        // executable; flushing before running injected code is required on
        // architectures with a separate instruction cache.
        unsafe {
            let _ = FlushInstructionCache(self.handle, Some(code as *const core::ffi::c_void), CODE_AREA);
        }
        // SAFETY: `code` points at the stub we just wrote into the target with
        // PAGE_EXECUTE_READWRITE, and its signature matches a thread start
        // routine (one pointer argument, u32 return). `parameter` points at the
        // argument block inside the same allocation, which outlives the thread
        // because the allocation is freed only after the join below.
        let handle = unsafe {
            let start: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32 = std::mem::transmute(code);
            CreateRemoteThread(self.handle, None, 0, Some(start), Some(parameter as *const core::ffi::c_void), 0, None)
        }
        .map_err(|e| AgentError::AttachFailed {
            pid,
            details: format!("cannot start the attach thread in process {pid}: {e}"),
        })?;
        Ok(Thread { handle })
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: closing the handle this type owns.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// An owned remote thread handle.
struct Thread {
    handle: HANDLE,
}

impl Thread {
    /// Waits for the stub to return and reports its exit code.
    fn join(&self, pid: u32, timeout: Duration) -> Result<u32, AgentError> {
        let millis = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        // SAFETY: waiting on the thread handle this type owns.
        let wait = unsafe { WaitForSingleObject(self.handle, millis) };
        if wait != WAIT_OBJECT_0 {
            return Err(AgentError::AttachFailed {
                pid,
                details: format!("the attach thread in process {pid} did not finish within {timeout:?}"),
            });
        }
        let mut exit_code = 0u32;
        // SAFETY: the thread has terminated; `exit_code` is a valid out-pointer.
        unsafe { GetExitCodeThread(self.handle, &raw mut exit_code) }.map_err(|e| AgentError::AttachFailed {
            pid,
            details: format!("cannot read the attach thread's result: {e}"),
        })?;
        Ok(exit_code)
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        // SAFETY: closing the handle this type owns.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// An owned `VirtualAllocEx` region in the target process.
struct RemoteMemory {
    process: HANDLE,
    address: usize,
}

impl RemoteMemory {
    fn allocate(process: HANDLE, pid: u32, size: usize) -> Result<Self, AgentError> {
        // SAFETY: allocating fresh pages in the target; freed in `Drop`.
        let address = unsafe { VirtualAllocEx(process, None, size, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE) };
        if address.is_null() {
            return Err(AgentError::AttachFailed {
                pid,
                details: format!("cannot allocate {size} bytes in process {pid}: {}", last_error()),
            });
        }
        Ok(Self { process, address: address as usize })
    }

    fn write(&self, pid: u32, buffer: &[u8]) -> Result<(), AgentError> {
        use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;

        let mut written = 0usize;
        // SAFETY: `buffer` is a valid local slice and the destination is the
        // allocation this type owns, which is at least `buffer.len()` bytes.
        unsafe {
            WriteProcessMemory(
                self.process,
                self.address as *const core::ffi::c_void,
                buffer.as_ptr().cast(),
                buffer.len(),
                Some(&raw mut written),
            )
        }
        .map_err(|e| AgentError::AttachFailed { pid, details: format!("cannot write into process {pid}: {e}") })?;
        if written != buffer.len() {
            return Err(AgentError::AttachFailed {
                pid,
                details: format!("wrote {written} of {} bytes into process {pid}", buffer.len()),
            });
        }
        Ok(())
    }
}

impl Drop for RemoteMemory {
    fn drop(&mut self) {
        // SAFETY: releasing the region this type allocated. Only reached after
        // the remote thread has been joined, so nothing in the target is still
        // executing out of it.
        unsafe {
            let _ = VirtualFreeEx(self.process, self.address as *mut core::ffi::c_void, 0, MEM_RELEASE);
        }
    }
}

// --------------------------------------------------------------------- stub

/// The injected stub, in machine code.
///
/// A thread start routine gets one pointer (in `rcx`);
/// `JVM_EnqueueOperation(cmd, arg0, arg1, arg2, pipe)` needs five. The stub
/// spreads the argument block across the Microsoft x64 calling convention:
/// four registers plus one stack slot, on top of the mandatory 32-byte shadow
/// space — hence `sub rsp, 0x28`, which also restores 16-byte alignment for the
/// call. `rcx` is loaded last, because everything else is addressed through it.
fn build_stub() -> Vec<u8> {
    let mut code = Vec::with_capacity(64);
    code.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]); // sub  rsp, 0x28
    code.extend_from_slice(&[0x48, 0x8B, 0x01]); // mov  rax, [rcx]        (the function pointer)
    lea(&mut code, [0x4C, 0x8D, 0x91], OFF_PIPE); //      lea  r10, [rcx+pipe]
    code.extend_from_slice(&[0x4C, 0x89, 0x54, 0x24, 0x20]); // mov [rsp+0x20], r10   (5th argument)
    lea(&mut code, [0x48, 0x8D, 0x91], OFF_ARG0); //      lea  rdx, [rcx+arg0]
    lea(&mut code, [0x4C, 0x8D, 0x81], OFF_ARG1); //      lea  r8,  [rcx+arg1]
    lea(&mut code, [0x4C, 0x8D, 0x89], OFF_ARG2); //      lea  r9,  [rcx+arg2]
    lea(&mut code, [0x48, 0x8D, 0x89], OFF_CMD); //       lea  rcx, [rcx+cmd]
    code.extend_from_slice(&[0xFF, 0xD0]); //             call rax
    code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28]); // add  rsp, 0x28
    code.extend_from_slice(&[0xC3]); //                   ret                (eax = return value)
    code
}

/// Emits `lea <reg>, [rcx + displacement]`.
///
/// The single place a layout offset becomes machine code, and therefore the
/// single place the `usize` layout has to fit the encoding's 32 bits — which it
/// does by a wide margin, the whole block being a few kilobytes.
fn lea(code: &mut Vec<u8>, opcode: [u8; 3], displacement: usize) {
    let displacement = u32::try_from(displacement).expect("argument-block offsets are far below 4 GiB");
    code.extend_from_slice(&opcode);
    code.extend_from_slice(&displacement.to_le_bytes());
}

fn write_field(data: &mut [u8], offset: usize, bytes: &[u8]) {
    data[offset..offset + bytes.len()].copy_from_slice(bytes);
}

/// Writes a NUL-terminated ASCII string into the argument block.
///
/// `JVM_EnqueueOperation` takes `char*`, so the bytes go out as-is; a non-ASCII
/// path would be handed to the JVM in an encoding neither side agreed on, which
/// is worth an error rather than a mangled file name.
fn write_string(data: &mut [u8], offset: usize, capacity: usize, value: &str) -> Result<(), AgentError> {
    let bytes = value.as_bytes();
    if bytes.len() + 1 > capacity {
        return Err(AgentError::AttachFailed {
            pid: 0,
            details: format!("attach argument {value:?} does not fit in {capacity} bytes"),
        });
    }
    if !value.is_ascii() {
        return Err(AgentError::AttachFailed {
            pid: 0,
            details: format!("the attach protocol carries bytes, not text; {value:?} is not ASCII"),
        });
    }
    write_field(data, offset, bytes);
    data[offset + bytes.len()] = 0;
    Ok(())
}

// ------------------------------------------------------------------- lookup

/// The address of `JVM_EnqueueOperation` inside the target process.
fn remote_enqueue_address(pid: u32) -> Result<usize, AgentError> {
    let (module_path, remote_base) = jvm::jvm_module(pid).ok_or(AgentError::NotAJvm { pid })?;
    let wide: Vec<u16> = module_path.encode_utf16().chain(std::iter::once(0)).collect();

    // DONT_RESOLVE_DLL_REFERENCES maps the image and its export table without
    // running its initialisation — we want an address, not a JVM in our own
    // process.
    // SAFETY: loading a module by path; freed below on every path.
    let local = unsafe { LoadLibraryExW(PCWSTR(wide.as_ptr()), None, DONT_RESOLVE_DLL_REFERENCES) }.map_err(|e| {
        AgentError::AttachFailed {
            pid,
            details: format!(
                "cannot inspect the target's {module_path}: {e} \
                 (a 32-bit JVM cannot be attached from this 64-bit process — use -javaagent)"
            ),
        }
    })?;
    // SAFETY: `local` is a live module handle and the name is NUL-terminated.
    let procedure = unsafe { GetProcAddress(local, PCSTR(ENQUEUE_EXPORT.as_ptr())) };
    let local_base = local.0 as usize;
    // SAFETY: releasing the module we just loaded; only the offset is kept.
    unsafe {
        let _ = FreeLibrary(local);
    }

    let procedure = procedure.ok_or_else(|| AgentError::AttachFailed {
        pid,
        details: format!("{module_path} exports no JVM_EnqueueOperation — not a HotSpot-compatible JVM"),
    })?;
    let offset = procedure as usize - local_base;
    Ok(remote_base + offset)
}

// --------------------------------------------------------------------- pipe

/// The reply pipe, in overlapped mode so every wait can carry a deadline.
struct Pipe {
    handle: HANDLE,
    event: HANDLE,
}

impl Pipe {
    fn create(name: &str) -> Result<Self, String> {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: creating a named pipe from a NUL-terminated name; the handle
        // is closed in `Drop`.
        let handle = unsafe {
            CreateNamedPipeW(
                PCWSTR(wide.as_ptr()),
                PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_BYTE | PIPE_WAIT,
                1,
                4096,
                8192,
                0,
                None,
            )
        };
        if handle.is_invalid() {
            return Err(format!("cannot create the reply pipe {name}: {}", last_error()));
        }
        // SAFETY: creating a manual-reset, initially unsignalled event; closed in `Drop`.
        let event = match unsafe { CreateEventW(None, true, false, PCWSTR::null()) } {
            Ok(event) => event,
            Err(e) => {
                // SAFETY: closing the pipe handle we just created.
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return Err(format!("cannot create the reply pipe event: {e}"));
            }
        };
        Ok(Self { handle, event })
    }

    /// Waits until the target JVM connects to the reply pipe.
    fn wait_for_client(&self, pid: u32, timeout: Duration) -> Result<(), AgentError> {
        let mut overlapped = self.overlapped();
        // SAFETY: `overlapped` lives until the wait below completes, and the
        // handle was created with FILE_FLAG_OVERLAPPED.
        let connected = unsafe { ConnectNamedPipe(self.handle, Some(&raw mut overlapped)) };
        if connected.is_ok() {
            return Ok(());
        }
        // SAFETY: reading the thread's last error immediately after the call.
        let error = unsafe { GetLastError() };
        if error == ERROR_PIPE_CONNECTED {
            // The JVM was faster than we were; that is a success, not a race.
            return Ok(());
        }
        if error != ERROR_IO_PENDING {
            return Err(AgentError::AttachFailed {
                pid,
                details: format!("cannot listen on the reply pipe: {}", windows::core::Error::from(error)),
            });
        }
        self.wait(pid, timeout, "the JVM did not answer the attach request")
    }

    /// Reads the whole reply, bounded by `deadline`.
    fn read_reply(&self, pid: u32, deadline: Instant) -> Result<String, AgentError> {
        let mut reply = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let mut overlapped = self.overlapped();
            let mut read = 0u32;
            // SAFETY: `chunk` and `overlapped` outlive the wait below; the
            // handle is overlapped, so no synchronous completion is assumed.
            let started =
                unsafe { ReadFile(self.handle, Some(&mut chunk), Some(&raw mut read), Some(&raw mut overlapped)) };
            if started.is_err() {
                // SAFETY: reading the thread's last error immediately after the call.
                let error = unsafe { GetLastError() };
                if error == ERROR_BROKEN_PIPE {
                    break; // The JVM closed its end: the reply is complete.
                }
                if error != ERROR_IO_PENDING {
                    return Err(AgentError::AttachFailed {
                        pid,
                        details: format!("cannot read the attach reply: {}", windows::core::Error::from(error)),
                    });
                }
                self.wait(pid, remaining(deadline), "the JVM stopped writing its attach reply")?;
                let mut transferred = 0u32;
                // SAFETY: the overlapped operation has completed (the event is signalled).
                let completed =
                    unsafe { GetOverlappedResult(self.handle, &raw const overlapped, &raw mut transferred, false) };
                if completed.is_err() {
                    // SAFETY: reading the thread's last error immediately after the call.
                    let error = unsafe { GetLastError() };
                    if error == ERROR_BROKEN_PIPE || error == ERROR_IO_INCOMPLETE {
                        break;
                    }
                    return Err(AgentError::AttachFailed {
                        pid,
                        details: format!("the attach reply was truncated: {}", windows::core::Error::from(error)),
                    });
                }
                read = transferred;
            }
            if read == 0 {
                break;
            }
            reply.extend_from_slice(&chunk[..read as usize]);
        }
        Ok(String::from_utf8_lossy(&reply).into_owned())
    }

    fn overlapped(&self) -> OVERLAPPED {
        OVERLAPPED { hEvent: self.event, ..Default::default() }
    }

    fn wait(&self, pid: u32, timeout: Duration, what: &str) -> Result<(), AgentError> {
        let millis = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        // SAFETY: waiting on the event this type owns.
        let wait = unsafe { WaitForSingleObject(self.event, millis) };
        if wait == WAIT_OBJECT_0 {
            return Ok(());
        }
        Err(AgentError::AttachFailed { pid, details: format!("{what} (waited {timeout:?})") })
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        // SAFETY: closing the handles this type owns.
        unsafe {
            let _ = CloseHandle(self.handle);
            let _ = CloseHandle(self.event);
        }
    }
}

/// A pipe name no other attach in flight can collide with.
fn unique_pipe_name(pid: u32) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| {
        elapsed.as_secs().wrapping_mul(1_000_000_000).wrapping_add(u64::from(elapsed.subsec_nanos()))
    });
    let nonce = now ^ (u64::from(std::process::id()) << 32) ^ COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(r"\\.\pipe\platynui-attach-{pid}-{nonce:016x}")
}

fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

fn last_error() -> windows::core::Error {
    // SAFETY: reading the calling thread's last error code.
    windows::core::Error::from(unsafe { GetLastError() })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The stub's `lea` displacements are the argument-block layout; if the two
    // ever drift, the target reads garbage pointers. This pins the encoding.
    #[test]
    fn the_stub_encodes_the_argument_block_layout() {
        let stub = build_stub();
        // Hand-decoded, so a "harmless" reshuffle of the layout constants
        // cannot silently change what the target executes.
        let expected: Vec<u8> = [
            &[0x48u8, 0x83, 0xEC, 0x28][..], // sub rsp, 0x28
            &[0x48, 0x8B, 0x01],             // mov rax, [rcx]
            &[0x4C, 0x8D, 0x91],
            &40u32.to_le_bytes(),            // lea r10, [rcx+40]   (pipe)
            &[0x4C, 0x89, 0x54, 0x24, 0x20], // mov [rsp+0x20], r10
            &[0x48, 0x8D, 0x91],
            &304u32.to_le_bytes(), // lea rdx, [rcx+304]  (arg0)
            &[0x4C, 0x8D, 0x81],
            &1329u32.to_le_bytes(), // lea r8,  [rcx+1329] (arg1)
            &[0x4C, 0x8D, 0x89],
            &2354u32.to_le_bytes(), // lea r9,  [rcx+2354] (arg2)
            &[0x48, 0x8D, 0x89],
            &8u32.to_le_bytes(),       // lea rcx, [rcx+8]    (cmd)
            &[0xFF, 0xD0],             // call rax
            &[0x48, 0x83, 0xC4, 0x28], // add rsp, 0x28
            &[0xC3],                   // ret
        ]
        .concat();
        assert_eq!(stub, expected);
        assert!(stub.len() <= CODE_AREA, "the stub must fit before the argument block");
    }

    #[test]
    fn strings_are_nul_terminated_and_bounded() {
        let mut data = vec![0xFFu8; DATA_SIZE];
        write_string(&mut data, OFF_CMD, LEN_CMD, "load").expect("fits");
        assert_eq!(&data[OFF_CMD..OFF_CMD + 5], b"load\0");

        let too_long = "x".repeat(LEN_CMD);
        assert!(write_string(&mut data, OFF_CMD, LEN_CMD, &too_long).is_err());
        assert!(write_string(&mut data, OFF_CMD, LEN_CMD, "jär").is_err(), "non-ASCII must not be guessed at");
    }

    #[test]
    fn pipe_names_are_unique_per_call() {
        assert_ne!(unique_pipe_name(1), unique_pipe_name(1));
        assert!(unique_pipe_name(42).starts_with(r"\\.\pipe\platynui-attach-42-"));
    }
}
