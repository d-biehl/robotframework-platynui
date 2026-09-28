//! Which process a pid stands for, whether it still does, and its process
//! attributes.
//!
//! A pid alone does not name a process: once a process has ended, the system may
//! hand its pid to the next one. [`ProcessIdentity`] therefore records the
//! process's start time together with its pid, and [`ProcessIdentity::check`]
//! compares both.
//!
//! [`ProcessIdentity::read`] and [`ProcessIdentity::read_all`] read the process
//! attributes of the recorded process — its name, executable path, command line,
//! user, start time and architecture — in one format on every platform. They
//! read only while the process is still the recorded one, so an application node
//! never reports the process that received its pid. An identity recorded
//! without a start time reads nothing. See [`ProcessAttribute`] for the formats.
//!
//! The check has three answers ([`Liveness`]), and "cannot tell" is one of them
//! on purpose. A process the user may not inspect, or a platform without a start
//! time, is no proof that the process ended; a caller that took it for one would
//! give up on an application that still runs.
//!
//! - **Windows:** `OpenProcess` with the limited query right, then
//!   `GetExitCodeProcess` and `GetProcessTimes` (the creation time, in 100 ns
//!   units). A process object can outlive its process while another handle is
//!   open, so only `STILL_ACTIVE` counts as running. A process that exited with
//!   that very code (259) is told apart by waiting on its handle, when the
//!   handle may be waited on.
//! - **Linux:** field 22 of `/proc/<pid>/stat`, the start time in clock ticks
//!   since boot. A zombie counts as ended once no thread of it runs; a leader
//!   that exited alone leaves a zombie entry while its other threads run on.
//!   `kill(pid, 0)` answers when the file cannot be read.
//! - **Other Unix systems:** `kill(pid, 0)` only. There is no start time to
//!   compare, so a process with the pid cannot be told from the recorded one.
//!
//! The attributes come from native Win32 calls on Windows and from `sysinfo`
//! and `getpwuid_r` on Linux; other platforms have no reader yet and answer
//! nothing.

mod attributes;

pub use attributes::{ProcessAttribute, ProcessAttributes};

/// What a check found out about a recorded process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Liveness {
    /// It runs, and it is the process that was recorded.
    Running,
    /// It has ended, or its pid now belongs to another process.
    Ended,
    /// It cannot be told: access to the process is denied, or there is no start
    /// time to compare, because none was recorded or the platform has none.
    Unknown,
}

impl Liveness {
    /// Whether the recorded process is known to be gone. Only [`Liveness::Ended`]
    /// is; "cannot tell" is not.
    #[must_use]
    pub const fn has_ended(self) -> bool {
        matches!(self, Self::Ended)
    }
}

/// A process, recorded by its pid and its start time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessIdentity {
    pid: u32,
    /// `None` when the start time could not be read at capture, for example
    /// because access to the process was denied.
    start: Option<u64>,
}

impl ProcessIdentity {
    /// Records the process that `pid` stands for now.
    ///
    /// `None` when no process has that pid, or its process has already ended;
    /// pid 0 is never a process here. A process that exists but cannot be
    /// inspected is recorded without its start time.
    #[must_use]
    pub fn capture(pid: u32) -> Option<Self> {
        if pid == 0 {
            return None;
        }
        match sys::capture(pid) {
            Captured::NoProcess => None,
            Captured::Process { start } => Some(Self { pid, start }),
        }
    }

    /// The recorded pid.
    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// Whether the recorded process still runs. See [`Liveness`].
    #[must_use]
    pub fn check(&self) -> Liveness {
        sys::check(self.pid, self.start)
    }
}

/// What capturing a pid found.
enum Captured {
    /// No process has the pid, or its process has already ended.
    NoProcess,
    /// A process has the pid; its start time, when it could be read.
    Process { start: Option<u64> },
}

#[cfg(windows)]
#[allow(unsafe_code, reason = "the process queries are Win32 calls")]
mod sys {
    use super::{Captured, Liveness};
    use windows::Win32::Foundation::{
        CloseHandle, ERROR_INVALID_PARAMETER, FILETIME, HANDLE, STILL_ACTIVE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        WaitForSingleObject,
    };
    use windows::core::HRESULT;

    /// An open process handle, closed when dropped. The process reader reads
    /// through it as well, so that a process is opened in one way only.
    pub(crate) struct Process {
        handle: HANDLE,
        /// Whether the handle may be waited on (`SYNCHRONIZE`).
        can_wait: bool,
    }

    impl Drop for Process {
        fn drop(&mut self) {
            // SAFETY: the handle was opened by `open` and is closed only here.
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }

    impl Process {
        /// Opens the process with the limited query right, and with the right to
        /// wait on it where that is granted.
        pub(crate) fn open(pid: u32) -> Result<Self, HRESULT> {
            // SAFETY: opening a process by pid with query and wait rights only;
            // the handle is owned by the returned value and closed when it drops.
            match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE, false, pid) } {
                Ok(handle) => Ok(Self { handle, can_wait: true }),
                Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => Err(error.code()),
                // SAFETY: as above, with the query right alone.
                Err(_) => unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
                    .map(|handle| Self { handle, can_wait: false })
                    .map_err(|error| error.code()),
            }
        }

        /// The handle, for queries that the process reader makes on it. It stays
        /// owned by `self`.
        pub(crate) const fn handle(&self) -> HANDLE {
            self.handle
        }

        /// Whether the process still runs; `None` when that cannot be queried.
        pub(crate) fn is_running(&self) -> Option<bool> {
            let mut exit_code = 0u32;
            // SAFETY: `self.handle` is a live process handle, `exit_code` a valid out-pointer.
            unsafe { GetExitCodeProcess(self.handle, &raw mut exit_code) }.ok()?;
            if exit_code != STILL_ACTIVE.0.cast_unsigned() {
                return Some(false);
            }
            if !self.can_wait {
                return Some(true);
            }
            // `STILL_ACTIVE` is also the code of a process that exited with 259.
            // A process handle is signaled once the process has ended.
            // SAFETY: `self.handle` is a live process handle opened with `SYNCHRONIZE`.
            match unsafe { WaitForSingleObject(self.handle, 0) } {
                WAIT_OBJECT_0 => Some(false),
                WAIT_TIMEOUT => Some(true),
                _ => None,
            }
        }

        /// The creation time in 100 ns units since 1601; `None` when it cannot
        /// be queried.
        pub(crate) fn creation_time(&self) -> Option<u64> {
            let mut creation = FILETIME::default();
            let mut exit = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            // SAFETY: `self.handle` is a live process handle, and all four are valid out-pointers.
            unsafe { GetProcessTimes(self.handle, &raw mut creation, &raw mut exit, &raw mut kernel, &raw mut user) }
                .ok()?;
            Some((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
        }
    }

    /// What a failed `OpenProcess` says: `ERROR_INVALID_PARAMETER` means that no
    /// process has the pid; anything else, `ERROR_ACCESS_DENIED` above all, says
    /// nothing about whether the process runs.
    pub(super) fn liveness_after_open_failure(code: HRESULT) -> Liveness {
        if code == ERROR_INVALID_PARAMETER.to_hresult() { Liveness::Ended } else { Liveness::Unknown }
    }

    pub(super) fn capture(pid: u32) -> Captured {
        match Process::open(pid) {
            Ok(process) if process.is_running() == Some(false) => Captured::NoProcess,
            Ok(process) => Captured::Process { start: process.creation_time() },
            Err(code) if liveness_after_open_failure(code).has_ended() => Captured::NoProcess,
            Err(_) => Captured::Process { start: None },
        }
    }

    pub(super) fn check(pid: u32, start: Option<u64>) -> Liveness {
        let process = match Process::open(pid) {
            Ok(process) => process,
            Err(code) => return liveness_after_open_failure(code),
        };
        match process.is_running() {
            Some(true) => {}
            Some(false) => return Liveness::Ended,
            None => return Liveness::Unknown,
        }
        match (start, process.creation_time()) {
            (Some(recorded), Some(now)) if recorded == now => Liveness::Running,
            (Some(_), Some(_)) => Liveness::Ended,
            _ => Liveness::Unknown,
        }
    }
}

#[cfg(unix)]
mod sys {
    use super::{Captured, Liveness};
    use rustix::io::Errno;
    use rustix::process::{Pid, test_kill_process};

    /// What `kill(pid, 0)` says: `ESRCH` means that no process has the pid.
    /// Success or `EPERM` means that one does, but not which one.
    pub(super) fn liveness_after_signal(result: Result<(), Errno>) -> Liveness {
        match result {
            Err(Errno::SRCH) => Liveness::Ended,
            _ => Liveness::Unknown,
        }
    }

    /// `kill(pid, 0)`. A pid that does not fit a positive `pid_t` has no
    /// process; it must never reach `kill`, where 0 and negative values address
    /// process groups.
    fn signal(pid: u32) -> Result<(), Errno> {
        let raw = i32::try_from(pid).map_err(|_| Errno::SRCH)?;
        let pid = if raw > 0 { Pid::from_raw(raw) } else { None }.ok_or(Errno::SRCH)?;
        test_kill_process(pid)
    }

    /// The fields of `/proc/<pid>/stat` a check needs. The process reader
    /// compares them around a read as well.
    #[cfg(target_os = "linux")]
    #[derive(Debug)]
    pub(crate) struct Stat {
        /// Field 3, the state of the process's leader thread.
        pub(crate) state: char,
        /// Field 20, the threads of the process that have not exited.
        pub(crate) threads: u64,
        /// Field 22, the start time in clock ticks since boot.
        pub(crate) start: u64,
    }

    #[cfg(target_os = "linux")]
    impl Stat {
        /// A zombie (`Z`) or a dead process (`X`) no longer runs, unless it is a
        /// leader that exited alone while the process's other threads run on.
        pub(crate) const fn has_ended(&self) -> bool {
            matches!(self.state, 'Z' | 'X') && self.threads <= 1
        }
    }

    /// Parses a `/proc/<pid>/stat` line. The command name in field 2 may hold
    /// spaces and parentheses, so the fields are counted from after the last `)`.
    #[cfg(target_os = "linux")]
    pub(super) fn parse_stat(line: &str) -> Option<Stat> {
        let rest = &line[line.rfind(')')? + 1..];
        let mut fields = rest.split_whitespace();
        let state = fields.next()?.chars().next()?;
        // Field 3 is the state; field 20 is the 17th field after it, and field
        // 22 the 2nd after that.
        let threads = fields.nth(16)?.parse().ok()?;
        let start = fields.nth(1)?.parse().ok()?;
        Some(Stat { state, threads, start })
    }

    /// Reads `/proc/<pid>/stat`. The command name in field 2 is the kernel's
    /// raw bytes and need not be UTF-8; only the fields after it are read, so a
    /// name that is not UTF-8 must not cost the start time.
    #[cfg(target_os = "linux")]
    pub(crate) fn read_stat(pid: u32) -> Option<Stat> {
        let bytes = std::fs::read(format!("/proc/{pid}/stat")).ok()?;
        parse_stat(&String::from_utf8_lossy(&bytes))
    }

    #[cfg(target_os = "linux")]
    pub(super) fn capture(pid: u32) -> Captured {
        match read_stat(pid) {
            Some(current) if current.has_ended() => Captured::NoProcess,
            Some(current) => Captured::Process { start: Some(current.start) },
            None if liveness_after_signal(signal(pid)).has_ended() => Captured::NoProcess,
            None => Captured::Process { start: None },
        }
    }

    #[cfg(target_os = "linux")]
    pub(super) fn check(pid: u32, recorded: Option<u64>) -> Liveness {
        let Some(current) = read_stat(pid) else {
            return liveness_after_signal(signal(pid));
        };
        if current.has_ended() {
            return Liveness::Ended;
        }
        match recorded {
            Some(start) if start == current.start => Liveness::Running,
            Some(_) => Liveness::Ended,
            None => Liveness::Unknown,
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub(super) fn capture(pid: u32) -> Captured {
        if liveness_after_signal(signal(pid)).has_ended() {
            Captured::NoProcess
        } else {
            Captured::Process { start: None }
        }
    }

    /// Without a start time, a process that has the pid cannot be told from the
    /// recorded one, so only a free pid gives an answer.
    #[cfg(not(target_os = "linux"))]
    pub(super) fn check(pid: u32, _start: Option<u64>) -> Liveness {
        liveness_after_signal(signal(pid))
    }
}

#[cfg(test)]
mod tests {
    use super::{Liveness, ProcessIdentity};
    use std::process::{Child, Command, Stdio};
    use std::time::Duration;

    /// Set in the environment of the child that [`waiting_child`] runs as.
    const CHILD_ENV: &str = "PLATYNUI_PROCESS_TEST_CHILD";

    /// When set, [`waiting_child`] exits with this code after a short wait,
    /// instead of waiting until it is killed.
    const CHILD_EXIT_ENV: &str = "PLATYNUI_PROCESS_TEST_CHILD_EXIT";

    /// A pid that no process can have: above Linux's `PID_MAX_LIMIT` (2^22), and
    /// far above any pid Windows hands out.
    pub(crate) const UNUSED_PID: u32 = 0x3FFF_FFFC;

    /// The start time is compared only on Windows and Linux; elsewhere a running
    /// pid cannot be told from the recorded process.
    const RUNNING: Liveness =
        if cfg!(any(windows, target_os = "linux")) { Liveness::Running } else { Liveness::Unknown };

    /// The child process of these tests: this test binary, re-executed into
    /// this test, which only waits. It never runs by itself, and returns at once
    /// when it is run without [`CHILD_ENV`].
    #[test]
    #[ignore = "the child process of the other tests; started by them"]
    fn waiting_child() {
        if std::env::var_os(CHILD_ENV).is_none() {
            return;
        }
        if let Some(code) = std::env::var(CHILD_EXIT_ENV).ok().and_then(|code| code.parse().ok()) {
            std::thread::sleep(Duration::from_millis(500));
            std::process::exit(code);
        }
        std::thread::sleep(Duration::from_secs(60));
    }

    /// Kills the child when a test ends, however it ends.
    pub(crate) struct WaitingChild(pub(crate) Child);

    impl WaitingChild {
        pub(crate) fn start() -> Self {
            Self::spawn(None)
        }

        /// A child that exits with `code` on its own, after half a second.
        #[cfg(windows)]
        fn exiting_with(code: i32) -> Self {
            Self::spawn(Some(code))
        }

        /// A child whose command line carries `arguments` as well. The test
        /// harness takes them for more name filters, which match nothing.
        #[cfg(any(windows, target_os = "linux"))]
        pub(crate) fn start_with_arguments(arguments: &[&str]) -> Self {
            let binary = std::env::current_exe().expect("test binary");
            Self::try_spawn_from(&binary, None, arguments).expect("start the child")
        }

        fn spawn(exit_code: Option<i32>) -> Self {
            let binary = std::env::current_exe().expect("test binary");
            Self::try_spawn_from(&binary, exit_code, &[]).expect("start the child")
        }

        /// A child run from `binary`, a copy of this test binary.
        pub(crate) fn try_spawn_from(
            binary: &std::path::Path,
            exit_code: Option<i32>,
            arguments: &[&str],
        ) -> std::io::Result<Self> {
            let mut command = Command::new(binary);
            command
                .args(["--exact", "tests::waiting_child", "--ignored", "--nocapture"])
                .args(arguments)
                .env(CHILD_ENV, "1")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if let Some(code) = exit_code {
                command.env(CHILD_EXIT_ENV, code.to_string());
            }
            command.spawn().map(Self)
        }

        pub(crate) fn pid(&self) -> u32 {
            self.0.id()
        }
    }

    impl Drop for WaitingChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn the_own_process_is_running_and_the_same() {
        let identity = ProcessIdentity::capture(std::process::id()).expect("the own process exists");
        assert_eq!(identity.pid(), std::process::id());
        assert_eq!(identity.check(), RUNNING);
    }

    #[test]
    fn a_child_is_running_while_it_lives_and_has_ended_once_killed() {
        let mut child = WaitingChild::start();
        let identity = ProcessIdentity::capture(child.pid()).expect("the child exists");
        assert_eq!(identity.check(), RUNNING);

        child.0.kill().expect("kill the child");
        // Killed but not yet reaped: a zombie on Linux, a process object that a
        // handle keeps on Windows. Either way it no longer runs. Other Unix
        // systems answer `kill(pid, 0)` for a zombie, so there only the reaped
        // child is known to be gone.
        #[cfg(any(windows, target_os = "linux"))]
        {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while identity.check() != Liveness::Ended {
                assert!(
                    std::time::Instant::now() < deadline,
                    "the killed child still checks as {:?}",
                    identity.check()
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        child.0.wait().expect("reap the child");
        assert_eq!(identity.check(), Liveness::Ended, "a reaped child has ended");
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn a_running_process_without_a_recorded_start_cannot_be_told() {
        // What a node holds when the start could not be read at capture, for a
        // process that can be inspected now: no proof that it is another one.
        let identity = ProcessIdentity { pid: std::process::id(), start: None };
        assert_eq!(identity.check(), Liveness::Unknown);
    }

    /// `STILL_ACTIVE` is 259, which is also a code a process can exit with. The
    /// handle this test keeps holds the process object, and its pid, alive.
    #[cfg(windows)]
    #[test]
    fn a_process_that_exited_with_the_code_of_a_running_one_has_ended() {
        let mut child = WaitingChild::exiting_with(259);
        let identity = ProcessIdentity::capture(child.pid()).expect("the child exists");
        assert_eq!(identity.check(), Liveness::Running);

        let status = child.0.wait().expect("the child exits on its own");
        assert_eq!(status.code(), Some(259));
        assert_eq!(identity.check(), Liveness::Ended, "the child exited, although its exit code reads as running");
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn another_start_time_is_another_process() {
        let recorded = ProcessIdentity::capture(std::process::id()).expect("the own process exists");
        let start = recorded.start.expect("the own start time is readable");
        let other = ProcessIdentity { pid: recorded.pid, start: Some(start + 1) };
        assert_eq!(other.check(), Liveness::Ended, "a running process with another start time is another process");
    }

    #[test]
    fn a_pid_that_no_process_has_cannot_be_recorded() {
        assert!(ProcessIdentity::capture(UNUSED_PID).is_none());
        assert!(ProcessIdentity::capture(0).is_none(), "pid 0 is no application's process");
    }

    #[test]
    fn a_free_pid_has_ended_even_without_a_recorded_start_time() {
        // What a node holds when access was denied at creation, for a process
        // that has since gone: the free pid is known although the start was not.
        let identity = ProcessIdentity { pid: UNUSED_PID, start: None };
        assert_eq!(identity.check(), Liveness::Ended);
    }

    #[test]
    fn only_an_ended_process_counts_as_ended() {
        assert!(Liveness::Ended.has_ended());
        assert!(!Liveness::Running.has_ended());
        assert!(!Liveness::Unknown.has_ended(), "cannot tell is no proof that it ended");
    }

    #[cfg(windows)]
    #[test]
    fn a_denied_process_cannot_be_told_and_an_invalid_pid_has_ended() {
        use super::sys::liveness_after_open_failure;
        use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, ERROR_NOT_ENOUGH_MEMORY};
        assert_eq!(liveness_after_open_failure(ERROR_ACCESS_DENIED.to_hresult()), Liveness::Unknown);
        assert_eq!(liveness_after_open_failure(ERROR_INVALID_PARAMETER.to_hresult()), Liveness::Ended);
        assert_eq!(
            liveness_after_open_failure(ERROR_NOT_ENOUGH_MEMORY.to_hresult()),
            Liveness::Unknown,
            "any other failure proves nothing"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_denied_signal_cannot_be_told_and_a_missing_process_has_ended() {
        use super::sys::liveness_after_signal;
        use rustix::io::Errno;
        assert_eq!(liveness_after_signal(Err(Errno::SRCH)), Liveness::Ended);
        assert_eq!(liveness_after_signal(Err(Errno::PERM)), Liveness::Unknown);
        assert_eq!(liveness_after_signal(Ok(())), Liveness::Unknown, "a process exists, but which one is not known");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_stat_line_is_read_after_the_last_parenthesis() {
        use super::sys::parse_stat;
        // The command name may hold spaces and parentheses; fields 3 onwards
        // follow the last `)`. Field 22, the start time, is 4711 here.
        let line =
            "1234 (a b) c) S 1 1234 1234 0 -1 4194560 100 0 0 0 5 3 0 0 20 0 1 0 4711 1000 50 18446744073709551615";
        let stat = parse_stat(line).expect("parsable");
        assert_eq!(stat.state, 'S');
        assert_eq!(stat.start, 4711);
        assert!(!stat.has_ended());
        assert!(parse_stat("1234 (no closing parenthesis S 1").is_none());
        assert!(parse_stat("1234 (short) S 1 2").is_none());
    }

    /// `/proc/<pid>/stat` holds the command name as raw bytes; one cut inside a
    /// multi-byte character is no reason to lose the start time.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_command_name_that_is_not_utf8_does_not_hide_the_start_time() {
        use super::sys::parse_stat;
        let bytes = b"1234 (caf\xC3) S 1 1234 1234 0 -1 4194560 100 0 0 0 5 3 0 0 20 0 1 0 4711 1000 50 0";
        let stat = parse_stat(&String::from_utf8_lossy(bytes)).expect("parsable");
        assert_eq!(stat.start, 4711);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_zombie_has_ended() {
        use super::sys::parse_stat;
        for state in ['Z', 'X'] {
            let line = format!("1234 (app) {state} 1 1234 1234 0 -1 4194560 100 0 0 0 5 3 0 0 20 0 1 0 4711 1000 50 0");
            assert!(parse_stat(&line).expect("parsable").has_ended(), "state {state}");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_zombie_leader_whose_threads_run_on_has_not_ended() {
        use super::sys::parse_stat;
        // The leader thread exited alone; field 20 still counts three threads.
        let line = "1234 (app) Z 1 1234 1234 0 -1 4194560 100 0 0 0 5 3 0 0 20 0 3 0 4711 1000 50 0";
        let stat = parse_stat(line).expect("parsable");
        assert_eq!(stat.threads, 3);
        assert!(!stat.has_ended(), "the process runs on in its other threads");
    }
}
