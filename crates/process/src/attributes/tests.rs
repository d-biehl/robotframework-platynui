use super::{
    ProcessAttribute, ProcessAttributes, choose_architecture, file_time_to_unix_seconds, format_start_time,
    join_command_line, machine, machine_architecture, strip_exe, wow64_architecture,
};

/// Whether `value` has the one start-time format, `YYYY-MM-DDTHH:MM:SSZ`.
#[cfg(any(windows, target_os = "linux"))]
fn is_start_time(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 20
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        })
}

// ─── Platform-independent ────────────────────────────────────────────────────

#[test]
fn machines_map_to_the_four_architectures() {
    assert_eq!(machine_architecture(machine::I386), Some("x86"));
    assert_eq!(machine_architecture(machine::AMD64), Some("x64"));
    assert_eq!(machine_architecture(machine::ARM), Some("arm"));
    assert_eq!(machine_architecture(machine::ARMNT), Some("arm"));
    assert_eq!(machine_architecture(machine::ARM64), Some("arm64"));
    assert_eq!(machine_architecture(machine::UNKNOWN), None);
    assert_eq!(machine_architecture(0x0200), None, "IA64 is none of them");
}

#[test]
fn is_wow64_process2_reports_the_guest_machine_or_the_native_one() {
    assert_eq!(wow64_architecture(machine::UNKNOWN, machine::AMD64), Some("x64"));
    assert_eq!(wow64_architecture(machine::I386, machine::AMD64), Some("x86"));
    assert_eq!(wow64_architecture(machine::UNKNOWN, machine::ARM64), Some("arm64"));
    assert_eq!(wow64_architecture(machine::I386, machine::ARM64), Some("x86"));
    assert_eq!(wow64_architecture(machine::ARMNT, machine::ARM64), Some("arm"));
    assert_eq!(wow64_architecture(machine::UNKNOWN, 0x0200), None, "an unmapped native machine");
}

/// Spec: *An architecture that cannot be read is absent, not the host's*.
#[test]
fn the_primary_machine_wins_and_the_fallback_answers_only_when_it_failed() {
    let unasked = || -> Option<(u16, u16)> { panic!("the fallback must not be asked when the primary call answered") };
    assert_eq!(choose_architecture(Some(machine::AMD64), unasked), Some("x64"));
    assert_eq!(choose_architecture(Some(0x0200), unasked), None, "an unmapped primary machine is not replaced");
    assert_eq!(choose_architecture(None, || Some((machine::I386, machine::AMD64))), Some("x86"));
    assert_eq!(choose_architecture(None, || None), None, "both paths failed");
}

#[test]
fn only_a_trailing_exe_is_removed_in_any_case() {
    assert_eq!(strip_exe("javaw.exe"), "javaw");
    assert_eq!(strip_exe("LEDGER.EXE"), "LEDGER");
    assert_eq!(strip_exe("tool.Exe"), "tool");
    assert_eq!(strip_exe("python3.12"), "python3.12");
    assert_eq!(strip_exe("setup.exe.bak"), "setup.exe.bak");
    assert_eq!(strip_exe("app.com"), "app.com");
    assert_eq!(strip_exe(".exe"), ".exe", "a name that is only the extension keeps it");
}

/// Spec: a Linux command line is the arguments joined by single spaces, the way
/// `ps` shows them — every argument as it is.
#[test]
fn a_linux_command_line_joins_the_arguments_as_they_are() {
    assert_eq!(join_command_line(b"app\0--title\0 x \0\0").as_deref(), Some("app --title  x  "));
    assert_eq!(join_command_line(b"/usr/bin/python3.12\0-c\0pass\0").as_deref(), Some("/usr/bin/python3.12 -c pass"));
    assert_eq!(join_command_line(b"nginx: worker process").as_deref(), Some("nginx: worker process"), "no terminator");
    assert_eq!(join_command_line(b""), None, "a kernel thread has no command line");
    assert_eq!(join_command_line(b"\0"), None);
}

#[test]
fn the_start_time_is_formatted_in_utc_to_the_second() {
    assert_eq!(format_start_time(0).as_deref(), Some("1970-01-01T00:00:00Z"));
    assert_eq!(format_start_time(951_782_400).as_deref(), Some("2000-02-29T00:00:00Z"), "a leap day");
    assert_eq!(format_start_time(1_790_624_759).as_deref(), Some("2026-09-28T19:45:59Z"));
    assert_eq!(format_start_time(253_402_300_799).as_deref(), Some("9999-12-31T23:59:59Z"));
    assert_eq!(format_start_time(253_402_300_800), None, "beyond four digits of year");
}

#[test]
fn a_file_time_becomes_seconds_since_1970_truncated() {
    const EPOCH: u64 = 116_444_736_000_000_000;
    assert_eq!(file_time_to_unix_seconds(EPOCH), Some(0));
    assert_eq!(file_time_to_unix_seconds(EPOCH + 59_999_999), Some(5), "the fraction is dropped");
    assert_eq!(file_time_to_unix_seconds(EPOCH - 1), None, "before 1970");
    assert_eq!(file_time_to_unix_seconds(0), None);
}

#[test]
fn an_attribute_that_was_not_read_is_absent_and_none_is_empty() {
    let mut attributes = ProcessAttributes::default();
    assert!(attributes.is_empty());
    attributes.set(ProcessAttribute::UserName, Some("HOST\\user".to_owned()));
    attributes.set(ProcessAttribute::ProcessName, Some("ledger".to_owned()));
    attributes.set(ProcessAttribute::CommandLine, Some(String::new()));
    assert_eq!(attributes.get(ProcessAttribute::CommandLine), None, "an empty value counts as not read");
    let listed: Vec<_> = attributes.iter().collect();
    assert_eq!(listed, [(ProcessAttribute::ProcessName, "ledger"), (ProcessAttribute::UserName, "HOST\\user")]);
}

// ─── Reads bound to a recorded identity ──────────────────────────────────────

#[cfg(any(windows, target_os = "linux"))]
mod bound {
    use crate::tests::WaitingChild;
    use crate::{ProcessAttribute, ProcessIdentity};

    fn own() -> ProcessIdentity {
        ProcessIdentity::capture(std::process::id()).expect("the own process exists")
    }

    #[test]
    fn a_lookup_by_name_agrees_with_the_listing() {
        let identity = own();
        let all = identity.read_all();
        assert!(!all.is_empty(), "the own process is readable");
        for attribute in ProcessAttribute::ALL {
            assert_eq!(identity.read(attribute).as_deref(), all.get(attribute), "{attribute:?}");
        }
    }

    /// Every attribute read by name, and the listing, answer nothing: the two
    /// are separate paths, and each must check the process on its own.
    fn assert_reads_nothing(identity: &ProcessIdentity, what: &str) {
        for attribute in ProcessAttribute::ALL {
            assert_eq!(identity.read(attribute), None, "{attribute:?} of {what}");
        }
        assert!(identity.read_all().is_empty(), "the listing of {what}");
    }

    /// Spec: *An application node whose process has ended reports no process
    /// attributes*: once it has ended, and once it has been reaped.
    #[test]
    fn a_child_is_read_while_it_lives_and_not_once_it_has_ended() {
        let mut child = WaitingChild::start();
        let identity = ProcessIdentity::capture(child.pid()).expect("the child exists");
        let living = identity.read_all();
        for attribute in [ProcessAttribute::ProcessName, ProcessAttribute::StartTime, ProcessAttribute::UserName] {
            assert!(living.get(attribute).is_some(), "the living child's {attribute:?} is readable");
        }

        child.0.kill().expect("kill the child");
        // Killed but not yet reaped: a zombie on Linux, whose process table
        // entry still answers, and a process object that a handle keeps on
        // Windows.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while identity.check() != crate::Liveness::Ended {
            assert!(std::time::Instant::now() < deadline, "the killed child does not end");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_reads_nothing(&identity, "a killed child that is not reaped yet");

        child.0.wait().expect("reap the child");
        assert_reads_nothing(&identity, "a reaped child");
    }

    #[test]
    fn a_process_with_another_start_time_is_not_read() {
        let recorded = own();
        assert!(!recorded.read_all().is_empty(), "the recorded process is readable");
        let start = recorded.start.expect("the own start time is readable");
        let other = ProcessIdentity { pid: recorded.pid, start: Some(start + 1) };
        assert_reads_nothing(&other, "the process that has the pid now, which is another one");
    }

    /// Decision M2: without a recorded start no read can tell the recorded
    /// process from one that received its pid.
    #[test]
    fn an_identity_without_a_start_time_reads_nothing() {
        let recorded = own();
        assert!(!recorded.read_all().is_empty(), "the recorded process is readable");
        let unstarted = ProcessIdentity { pid: recorded.pid, start: None };
        assert_reads_nothing(&unstarted, "an identity without a start time");
    }
}

// ─── Windows ─────────────────────────────────────────────────────────────────

#[cfg(windows)]
#[allow(unsafe_code, reason = "the Windows build number is read through RtlGetVersion")]
mod windows_reader {
    use super::super::{machine, machine_architecture, win32, wow64_architecture};
    use super::{is_start_time, strip_exe};
    use crate::sys::Process;
    use crate::tests::WaitingChild;
    use crate::{ProcessAttribute, ProcessIdentity};

    /// The first build whose `GetProcessInformation` knows `ProcessMachineTypeInfo`.
    const MACHINE_TYPE_INFO_BUILD: u32 = 22_000;

    fn own() -> ProcessIdentity {
        // Opens the pid with the limited right, as every provider does; the
        // pseudo-handle of GetCurrentProcess would carry every right.
        ProcessIdentity::capture(std::process::id()).expect("the own process exists")
    }

    fn windows_build() -> u32 {
        use windows::Wdk::System::SystemServices::RtlGetVersion;
        use windows::Win32::System::SystemInformation::OSVERSIONINFOW;
        let mut info = OSVERSIONINFOW {
            dwOSVersionInfoSize: u32::try_from(size_of::<OSVERSIONINFOW>()).expect("small"),
            ..Default::default()
        };
        // SAFETY: `info` is a valid OSVERSIONINFOW with its size set.
        unsafe { RtlGetVersion(&raw mut info) }.ok().expect("RtlGetVersion");
        info.dwBuildNumber
    }

    /// The architecture this test binary was built for, in the spec's vocabulary.
    fn target_architecture() -> &'static str {
        match std::env::consts::ARCH {
            "x86" => "x86",
            "x86_64" => "x64",
            "arm" => "arm",
            "aarch64" => "arm64",
            other => panic!("no architecture for the target {other}"),
        }
    }

    #[test]
    fn the_own_executable_is_the_test_binary_and_its_name_has_no_exe() {
        let identity = own();
        let exe = std::env::current_exe().expect("test binary");
        let path = identity.read(ProcessAttribute::ExecutablePath).expect("executable path");
        assert!(path.eq_ignore_ascii_case(&exe.to_string_lossy()), "{path} is not {}", exe.display());
        let file_name = exe.file_name().expect("file name").to_string_lossy().into_owned();
        assert_eq!(identity.read(ProcessAttribute::ProcessName).as_deref(), Some(strip_exe(&file_name)));
        assert!(!strip_exe(&file_name).to_ascii_lowercase().ends_with(".exe"));
    }

    /// Spec: *A Windows command line keeps its quoting*. An argument with
    /// spaces is passed quoted, and the line keeps the quotes.
    #[test]
    fn a_command_line_keeps_the_quotes_of_an_argument_with_spaces() {
        let child = WaitingChild::start_with_arguments(&[r"C:\My Files\input.txt"]);
        let identity = ProcessIdentity::capture(child.pid()).expect("the child exists");
        let command_line = identity.read(ProcessAttribute::CommandLine).expect("command line");
        assert!(command_line.contains(r#""C:\My Files\input.txt""#), "{command_line}");
    }

    #[test]
    fn the_own_command_line_is_verbatim() {
        let command_line = own().read(ProcessAttribute::CommandLine).expect("command line");
        let exe = std::env::current_exe().expect("test binary");
        let file_name = exe.file_name().expect("file name").to_string_lossy().into_owned();
        assert!(command_line.contains(&file_name), "{command_line} lacks {file_name}");
        for argument in std::env::args().skip(1).filter(|argument| !argument.contains([' ', '"'])) {
            assert!(command_line.contains(&argument), "{command_line} lacks {argument}");
        }
    }

    /// Spec: *A Windows process owned by a local account names the computer as
    /// its domain*.
    #[test]
    fn the_own_user_is_the_domain_and_the_account() {
        let domain = std::env::var("USERDOMAIN").expect("USERDOMAIN");
        let user = std::env::var("USERNAME").expect("USERNAME");
        let user_name = own().read(ProcessAttribute::UserName).expect("user name");
        assert!(user_name.eq_ignore_ascii_case(&format!("{domain}\\{user}")), "{user_name} is not {domain}\\{user}");
        let computer = std::env::var("COMPUTERNAME").expect("COMPUTERNAME");
        if domain.eq_ignore_ascii_case(&computer) {
            let (domain_part, _) = user_name.split_once('\\').expect("DOMAIN\\user");
            assert!(domain_part.eq_ignore_ascii_case(&computer), "a local account's domain is the computer");
        }
    }

    #[test]
    fn the_own_start_time_is_the_recorded_creation_time_to_the_second() {
        let identity = own();
        let start_time = identity.read(ProcessAttribute::StartTime).expect("start time");
        assert!(is_start_time(&start_time), "{start_time}");
        let recorded = identity.start.expect("recorded start");
        let expected = super::file_time_to_unix_seconds(recorded).and_then(super::format_start_time);
        assert_eq!(Some(start_time), expected);
    }

    #[test]
    fn the_own_architecture_is_the_build_targets() {
        let process = Process::open(std::process::id()).expect("open the own process");
        let (_, native) = win32::wow64_machines(&process).expect("IsWow64Process2 answers on Windows 10 and later");
        // On Windows 11 on ARM64 the fallback reports an emulated x64 process
        // as arm64 (design D3), so it is asserted on x64 and x86 hosts only.
        if matches!(native, machine::AMD64 | machine::I386) {
            let fallback = win32::wow64_machines(&process).and_then(|(own, native)| wow64_architecture(own, native));
            assert_eq!(fallback, Some(target_architecture()), "through IsWow64Process2");
        }
        let primary = win32::machine_type_info(&process);
        if windows_build() >= MACHINE_TYPE_INFO_BUILD {
            assert_eq!(
                primary.and_then(machine_architecture),
                Some(target_architecture()),
                "through ProcessMachineTypeInfo"
            );
        } else {
            assert_eq!(primary, None, "ProcessMachineTypeInfo is unknown before build 22000");
        }
        assert_eq!(own().read(ProcessAttribute::Architecture).as_deref(), Some(target_architecture()));
    }

    /// Decision M4: the kernel's System process. Without elevation it cannot be
    /// opened at all, so its identity has no start and nothing is read; only an
    /// elevated run reads it in part. The partly readable process of every run
    /// is the one of the next test.
    #[test]
    fn the_system_process_is_read_attribute_by_attribute() {
        let system = ProcessIdentity::capture(4).expect("the System process always runs");
        let all = system.read_all();
        for attribute in ProcessAttribute::ALL {
            let value = system.read(attribute);
            assert_ne!(value.as_deref(), Some(""), "{attribute:?} is never empty");
            assert_eq!(value.as_deref(), all.get(attribute), "{attribute:?}: the lookup agrees with the listing");
        }
        if system.start.is_some() {
            assert!(all.get(ProcessAttribute::StartTime).is_some(), "a recorded start is always readable");
        } else {
            assert!(all.is_empty(), "without a recorded start nothing is read");
            eprintln!("the System process cannot be opened without elevation; nothing of it is read");
        }
    }

    /// Spec: *The presence of one process attribute SHALL NOT imply the
    /// presence of another*. A child whose token denies this user
    /// `TOKEN_QUERY` has no readable user, while everything else is read, and
    /// the lookups agree with the listing. Needs no elevation: the token's owner
    /// may always change its DACL.
    #[test]
    fn a_partly_readable_process_lists_what_can_be_read() {
        let child = WaitingChild::start();
        deny_token_query(child.pid());
        let identity = ProcessIdentity::capture(child.pid()).expect("the child exists");
        let all = identity.read_all();
        assert_eq!(all.get(ProcessAttribute::UserName), None, "the token cannot be queried");
        for attribute in [
            ProcessAttribute::ProcessName,
            ProcessAttribute::ExecutablePath,
            ProcessAttribute::CommandLine,
            ProcessAttribute::StartTime,
            ProcessAttribute::Architecture,
        ] {
            assert!(all.get(attribute).is_some_and(|value| !value.is_empty()), "{attribute:?} is readable");
        }
        for attribute in ProcessAttribute::ALL {
            assert_eq!(identity.read(attribute).as_deref(), all.get(attribute), "{attribute:?}");
        }
    }

    /// Replaces the DACL of `pid`'s primary token with one that denies this
    /// user `TOKEN_QUERY` and allows it everything else.
    fn deny_token_query(pid: u32) {
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::Security::Authorization::{SE_KERNEL_OBJECT, SetSecurityInfo};
        use windows::Win32::Security::{
            ACL, ACL_REVISION, AddAccessAllowedAce, AddAccessDeniedAce, DACL_SECURITY_INFORMATION, GetLengthSid,
            GetTokenInformation, InitializeAcl, TOKEN_ALL_ACCESS, TOKEN_QUERY, TOKEN_READ_CONTROL, TOKEN_USER,
            TOKEN_WRITE_DAC, TokenUser,
        };
        use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        /// Closes a handle when dropped.
        struct Owned(HANDLE);
        impl Drop for Owned {
            fn drop(&mut self) {
                // SAFETY: the handle was opened below and is closed only here.
                let _ = unsafe { CloseHandle(self.0) };
            }
        }

        // This user's SID, from the own token.
        let mut own_token = HANDLE::default();
        // SAFETY: the pseudo-handle of the own process, and a valid out-pointer.
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut own_token) }.expect("the own token");
        let own_token = Owned(own_token);
        let mut user = vec![0u64; 64];
        let mut length = u32::try_from(user.len() * 8).expect("small");
        // SAFETY: `user` holds `length` writable bytes, aligned for TOKEN_USER.
        unsafe { GetTokenInformation(own_token.0, TokenUser, Some(user.as_mut_ptr().cast()), length, &raw mut length) }
            .expect("the own token user");
        // SAFETY: the call succeeded, so `user` starts with a TOKEN_USER whose
        // SID lies in `user`, which outlives its use.
        let sid = unsafe { &*user.as_ptr().cast::<TOKEN_USER>() }.User.Sid;

        // An ACL with the deny before the allow, so that the deny takes effect.
        // SAFETY: `sid` is a valid SID.
        let sid_length = unsafe { GetLengthSid(sid) };
        let mut acl = vec![0u64; 64];
        let acl_length = u32::try_from(acl.len() * 8).expect("small");
        assert!(sid_length * 2 + 64 < acl_length, "the ACL buffer holds both entries");
        let acl_pointer = acl.as_mut_ptr().cast::<ACL>();
        // SAFETY: `acl` holds `acl_length` writable bytes, aligned for ACL, and
        // `sid` is a valid SID.
        unsafe {
            InitializeAcl(acl_pointer, acl_length, ACL_REVISION).expect("an empty ACL");
            AddAccessDeniedAce(acl_pointer, ACL_REVISION, TOKEN_QUERY.0, sid).expect("the deny entry");
            AddAccessAllowedAce(acl_pointer, ACL_REVISION, TOKEN_ALL_ACCESS.0 & !TOKEN_QUERY.0, sid)
                .expect("the allow entry");
        }

        let process = Process::open(pid).expect("open the child");
        let mut token = HANDLE::default();
        // SAFETY: the child's handle is live, and `token` is a valid out-pointer.
        unsafe { OpenProcessToken(process.handle(), TOKEN_WRITE_DAC | TOKEN_READ_CONTROL, &raw mut token) }
            .expect("the child's token, for its DACL");
        let token = Owned(token);
        // SAFETY: the token handle carries WRITE_DAC, and `acl` is a valid ACL.
        unsafe {
            SetSecurityInfo(token.0, SE_KERNEL_OBJECT, DACL_SECURITY_INFORMATION, None, None, Some(acl_pointer), None)
        }
        .ok()
        .expect("the child's token DACL");
    }

    /// The 32-bit process of the architecture scenario, built by
    /// `just build-win32-test-window-x86`; the Windows lane runs this test.
    #[test]
    #[ignore = "needs the 32-bit Win32 test window (PLATYNUI_WIN32_TEST_WINDOW_X86); the Windows lane runs it"]
    fn a_32_bit_process_reads_as_x86() {
        let Some(binary) = std::env::var_os("PLATYNUI_WIN32_TEST_WINDOW_X86") else {
            eprintln!("PLATYNUI_WIN32_TEST_WINDOW_X86 is not set; run `just build-win32-test-window-x86` first");
            return;
        };
        assert!(std::path::Path::new(&binary).is_file(), "{} is not built", binary.to_string_lossy());
        let mut child = std::process::Command::new(&binary)
            .args(["--title", "PlatynUI process reader x86", "--auto-close", "10"])
            .spawn()
            .expect("start the 32-bit window");
        let identity = ProcessIdentity::capture(child.id()).expect("the window's process exists");
        let process = Process::open(child.id()).expect("open the window's process");

        let fallback = win32::wow64_machines(&process).and_then(|(own, native)| wow64_architecture(own, native));
        assert_eq!(fallback, Some("x86"), "through IsWow64Process2");
        if windows_build() >= MACHINE_TYPE_INFO_BUILD {
            assert_eq!(win32::machine_type_info(&process).and_then(machine_architecture), Some("x86"));
        }
        assert_eq!(identity.read(ProcessAttribute::Architecture).as_deref(), Some("x86"));

        let _ = child.kill();
        let _ = child.wait();
    }
}

// ─── Linux ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
#[allow(unsafe_code, reason = "the expected user name is read through getpwuid")]
mod linux_reader {
    use super::{format_start_time, is_start_time};
    use crate::tests::WaitingChild;
    use crate::{ProcessAttribute, ProcessIdentity};
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn own() -> ProcessIdentity {
        ProcessIdentity::capture(std::process::id()).expect("the own process exists")
    }

    fn now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).expect("after 1970").as_secs()
    }

    #[test]
    fn the_own_process_reads_as_linux_presents_it() {
        let identity = own();
        let exe = std::env::current_exe().expect("test binary");
        assert_eq!(identity.read(ProcessAttribute::ExecutablePath).as_deref(), exe.to_str());
        assert_eq!(
            identity.read(ProcessAttribute::ProcessName).as_deref(),
            exe.file_name().and_then(|name| name.to_str())
        );

        let arguments = std::env::args().collect::<Vec<_>>().join(" ");
        assert_eq!(identity.read(ProcessAttribute::CommandLine), Some(arguments));

        // SAFETY: getpwuid returns a pointer into static storage, read at once
        // on this thread; the entry exists for the user running the test.
        let expected_user = unsafe {
            let entry = libc::getpwuid(libc::geteuid());
            assert!(!entry.is_null(), "the effective user has a name");
            std::ffi::CStr::from_ptr((*entry).pw_name).to_string_lossy().into_owned()
        };
        assert_eq!(identity.read(ProcessAttribute::UserName), Some(expected_user));

        let start_time = identity.read(ProcessAttribute::StartTime).expect("start time");
        assert!(is_start_time(&start_time), "{start_time}");
        let earliest = format_start_time(now() - 60).expect("formattable");
        let latest = format_start_time(now() + 1).expect("formattable");
        // The format sorts like the time it names.
        assert!(earliest <= start_time && start_time <= latest, "{start_time} is not within the last minute");

        assert_eq!(identity.read(ProcessAttribute::Architecture), None, "Linux keeps no architecture per process");
    }

    /// Arguments keep their surrounding whitespace, and an empty one stays: the
    /// command line is the process's, not a tidied version of it.
    #[test]
    fn a_command_line_keeps_every_argument_as_it_is() {
        let child = WaitingChild::start_with_arguments(&[" padded ", ""]);
        let identity = ProcessIdentity::capture(child.pid()).expect("the child exists");
        let command_line = identity.read(ProcessAttribute::CommandLine).expect("command line");
        assert!(command_line.ends_with("--nocapture  padded  "), "{command_line:?}");
        assert_eq!(identity.read_all().get(ProcessAttribute::CommandLine), Some(command_line.as_str()));
    }

    /// The process name is the executable's full file name: no stem cut, and
    /// no ` (deleted)` once the file is gone.
    #[test]
    fn a_process_is_named_after_its_full_file_name_even_once_the_file_is_deleted() {
        let directory = std::env::temp_dir().join(format!("platynui-process-probe-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("probe directory");
        let probe = directory.join("probe.v2");
        std::fs::copy(std::env::current_exe().expect("test binary"), &probe).expect("copy the test binary");

        let child = spawn_probe(&probe);
        let identity = ProcessIdentity::capture(child.pid()).expect("the probe exists");
        assert_eq!(identity.read(ProcessAttribute::ProcessName).as_deref(), Some("probe.v2"));

        std::fs::remove_file(&probe).expect("delete the probe binary");
        let path = identity.read(ProcessAttribute::ExecutablePath).expect("executable path");
        assert!(path.ends_with("/probe.v2"), "{path}");
        assert_eq!(identity.read(ProcessAttribute::ProcessName).as_deref(), Some("probe.v2"));

        drop(child);
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// A freshly written binary can be busy for a moment while another thread
    /// of the test process forks.
    fn spawn_probe(probe: &Path) -> WaitingChild {
        for _ in 0..50 {
            match WaitingChild::try_spawn_from(probe, None, &[]) {
                Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                result => return result.expect("start the probe"),
            }
        }
        panic!("the probe binary stayed busy");
    }

    /// Holds whether or not the test runs as root.
    #[test]
    fn init_is_read_as_far_as_it_is_readable() {
        let init = ProcessIdentity::capture(1).expect("pid 1 exists");
        let all = init.read_all();
        match all.get(ProcessAttribute::ExecutablePath) {
            None => assert_eq!(all.get(ProcessAttribute::ProcessName), None, "no name without a path"),
            Some(path) => assert_eq!(
                all.get(ProcessAttribute::ProcessName),
                Path::new(path).file_name().and_then(|name| name.to_str())
            ),
        }
        assert!(all.get(ProcessAttribute::StartTime).is_some(), "the start time of pid 1 is readable");
    }

    #[test]
    fn a_kernel_thread_has_no_command_line_but_a_start_time() {
        let Some(pid) = kernel_thread() else {
            eprintln!("no kernel thread is visible here");
            return;
        };
        let identity = ProcessIdentity::capture(pid).expect("the kernel thread exists");
        assert_eq!(identity.read(ProcessAttribute::CommandLine), None);
        assert!(identity.read(ProcessAttribute::StartTime).is_some());
    }

    /// A pid whose `stat` flags (field 9) carry `PF_KTHREAD`.
    fn kernel_thread() -> Option<u32> {
        const PF_KTHREAD: u64 = 0x0020_0000;
        std::fs::read_dir("/proc").ok()?.flatten().find_map(|entry| {
            let pid: u32 = entry.file_name().to_str()?.parse().ok()?;
            let stat = std::fs::read_to_string(entry.path().join("stat")).ok()?;
            let flags: u64 = stat[stat.rfind(')')? + 1..].split_whitespace().nth(6)?.parse().ok()?;
            (flags & PF_KTHREAD != 0).then_some(pid)
        })
    }
}
