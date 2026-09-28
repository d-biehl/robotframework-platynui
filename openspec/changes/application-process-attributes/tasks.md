# Tasks

Windows-only work is verified on a Windows machine, because CI has no Windows test job. There, `just test-acceptance-windows` takes a full robotcode command, for example `just test-acceptance-windows --profile real-windows run --suite '*.Egui.ProcessAttributes'`. Its agent-served Swing suites need the agent JAR installed once with `just install-provider-java`; that is an existing lane prerequisite, not something this change adds. This change adds one: the Rust target `i686-pc-windows-msvc`, for the 32-bit test window of 1.3.

`snapshot-validity` has landed (`8f6fc02`, `94f5f97`; recorded in `1f09f50`). Its Linux-host run, tasks 2.1 and 9.3, is open and does not block this change.

- `crates/process` holds `ProcessIdentity` and `Liveness` (`crates/process/src/lib.rs:26-84`) and is in `windows_rust_packages` and `macos_rust_packages` (`justfile:12`, `:14`).
- The application nodes of UIA, JAB and the Java agent record an identity (`crates/provider-windows-uia/src/node.rs:1771`, `crates/provider-java-jab/src/node.rs:1329`, `crates/provider-java/src/agent/app.rs:65`) and answer `is_valid` from it.

The tasks below add the reader to that crate and read each node's attributes through its identity. The Windows parts come first; the Linux reader (2.4) and AT-SPI (3.4) are verified on a Linux host.

## 1. Acceptance suites first

- [x] 1.1 Add `tests/acceptance/egui/process_attributes.robot`, following the `robot-test-style` skill.
  - Its suite setup records the UTC time and launches an instance of its own with `Launch Test App    PlatynUI Process Attributes    com.platynui.test.processattributes` (`tests/acceptance/egui/resources/testapp.resource:32-41`), whose title has spaces. `Launch Default Instance` pins the window as root and records no launch time (`:51-59`), so it is not used.
  - Its teardown ends the instance with `Terminate App`.
  - Locators are absolute, `/app:Application[@ProcessId=${pid}]`, as in `app_root_after_exit.robot`.

  It runs unchanged on every real lane, because the expected values come from the platform the suite runs on and not from tags. It asserts:
  - `/app:Application[@ProcessId=<launched pid>]` selects exactly one node (spec: *The process ID stays addressable in the control namespace*).
  - `@app:ProcessName` is the test binary's file name without `.exe`. On Windows, `@Name` is the same value (*An application named after its program carries its process name*).
  - `@app:ExecutablePath` names the launched binary. Compare `os.path.normcase(os.path.realpath(...))` of both sides: the lane hands the path over with `/`, Windows reports `\`, and Linux resolves symlinks.
  - `@app:CommandLine` is present and contains the binary's file name. On Windows it contains `"PlatynUI Process Attributes"` with its quotes, which the launch passes as one argument containing spaces (*A Windows command line keeps its quoting*).
  - `@app:StartTime` matches `YYYY-MM-DDTHH:MM:SSZ` and lies within a minute of the launch (*The start time has one format on every provider*).
  - `@app:UserName` is the current account in the platform's form: `%USERDOMAIN%\%USERNAME%` on Windows; on Linux the name of the effective UID, `${{ pwd.getpwuid(os.geteuid()).pw_name }}` (*A Windows process owned by a local account names the computer as its domain*).
  - `@app:Architecture` is `x64` on the x64 Windows lane and absent on Linux, while the five other attributes are present there (*A Linux application carries no architecture*).
  - `BM.Get Attribute    <application>    app:ProcessName` returns the same value as the XPath read.

  Verify:
  - On a Windows machine it fails today on the start time's milliseconds. Record that in the run notes.
  - On a Linux host, both Linux lanes pass it before any code change, as a regression guard: `just headless=true test-acceptance-x11 --suite '*.Egui.ProcessAttributes'` and the same with `test-acceptance-compositor`.

  Outcome (2026-09-28), Windows:
  - Against `3d41929`, 7 of 8 pass; only *The Start Time Is UTC To The Second* fails, on `2026-09-28T20:11:55.054Z`.
  - With the change, 8 of 8 pass, with no WARN or ERROR message.

  Outcome (2026-09-29), Linux host: on a worktree of `3d41929` with only this suite added, it passes 8 of 8 on both lanes (`just headless=true test-acceptance-x11 --suite '*.Egui.ProcessAttributes'`, and the same with `test-acceptance-compositor`). With the change it passes 8 of 8 on both lanes as part of 7.2.
- [x] 1.2 Add `tests/acceptance/swing/process_attributes.robot` for the Windows lane, following the `robot-test-style` skill.
  - First, `Start Swing Fixture Process` (`tests/acceptance/swing/resources/swing_env.resource:34-42`) gains a launcher argument `${java}=${SWING_JAVA}` and `&{process_options}`, both passed on to `Start Process`. `Launch Swing Agent Test App` (`resources/testapp_agent.resource:27-38`) forwards `${java}`, `@{extra_args}` and `&{process_options}`.
  - The suite imports `resources/testapp_agent.resource` (agent on, as `BM`). It adds a second `PlatynUI.BareMetal` with `config={'providers': {'java': {'agent': {'enabled': False}}}}` (as `resources/testapp.resource:31-32`) under the alias `BMJAB`, the two-import pattern of `dedup.robot:8-12`.
  - Every locator names its backend: `/app:Application[@ProcessId=${pid}][@Technology="JavaAgent"]` on `BM`, and `[@Technology="JAB"]` on `BMJAB`.
  - It launches through `javaw.exe` next to `${SWING_JAVA}` (`${{ os.path.join(os.path.dirname($SWING_JAVA), 'javaw.exe') }}`), under a title with spaces, e.g. `Swing Process Attributes`.

  It asserts:
  - **The agent-served node, which the default import sees**:
    - `@app:ProcessName` is `javaw`.
    - `@app:ExecutablePath` names the launcher, compared normalized as in 1.1.
    - `@ProcessName` is absent.
    - `@app:StartTime` has the fixed format, `@app:UserName` is `%USERDOMAIN%\%USERNAME%`, and `@app:Architecture` is `x64`.

    This covers *A Java application served by the in-JVM agent carries its process attributes under app* and *A Java application reports its process, not its main class*.
  - **A second instance launched with `env:JAVA_TOOL_OPTIONS=-Duser.name=someone-else`** (forwarded through `Launch Swing Agent Test App`): `@app:UserName` on its agent-served node still names the current account (*An application's self-description does not replace a process attribute*). On Windows the handshake directory comes from `%LOCALAPPDATA%`, not from `user.name` (`java/agent/src/main/java/platynui/agent/AgentPaths.java:65-70`), so the override does not hide the agent.
  - **On the JAB-served node of the first instance**, through `BMJAB`:
    - `@app:UserName` is `%USERDOMAIN%\%USERNAME%`, `@app:CommandLine` contains the quoted title, and `@app:StartTime` has the fixed format.
    - `@Name` equals `@app:ProcessName`.
    - Every attribute present on both nodes is equal (*Two providers report the same process identically*).

  Verify: on a Windows machine the suite fails today:
  - the agent-served node carries none of the `app:` process attributes: it reports them under `control:` (`crates/provider-java/src/agent/app.rs:163-170`);
  - on the lane's Java 8 it reports no start time at all (`ProcessFacts.java:77-96`);
  - JAB's user name lacks the domain, and its command line the quotes.

  Record that in the run notes.

  Outcome (2026-09-28): against `3d41929` all four fail, as expected:
  - the agent-served nodes have no `app:ProcessName` and no `app:UserName` (`AttributeNotFoundError`);
  - JAB's user name is `daniel` instead of `VULCAN\daniel`;
  - no attribute is present on both nodes.

  The log check of the second instance holds: `Picked up JAVA_TOOL_OPTIONS: -Duser.name=someone-else`. With the change all four pass.
- [x] 1.3 Add `apps/win32-test-window` (package `platynui-win32-test-window`), the 32-bit process for the architecture scenario (design D11). It is a helper for process-level tests, not a fixture of the blueprint (`dev-docs/testing-strategy.md` §5).
  - It shows one visible top-level window of the predefined `STATIC` class, titled by `--title`, off screen, without `WS_EX_NOACTIVATE` and without activating it (`SW_SHOWNOACTIVATE`).
  - It pumps its messages until it exits, so that the UIA root enumeration lists it (`crates/provider-windows-uia/src/provider.rs:115-167`) and its `WM_GETOBJECT` probe is answered within 300 ms (`:38`, `:66-111`).
  - It exits by itself after `--auto-close <seconds>` (default 60), so that a failed teardown leaves no process behind.
  - The model is `test_window_child` (`crates/provider-windows-uia/src/node.rs:2166-2242`), without its buttons.
  - Its `windows` dependency sits under `[target.'cfg(windows)'.dependencies]`. Its bitness follows the build target. On other platforms `main` says that it runs only on Windows and exits non-zero, as `apps/eis-test-client` does off Linux, so the workspace still builds everywhere.
  - `[lints] workspace = true`. The Win32 calls carry a scoped `#[allow(unsafe_code, reason = "...")]` and SAFETY comments.
  - A Windows-only `justfile` recipe `build-win32-test-window-x86` runs `cargo build -p platynui-win32-test-window --target i686-pc-windows-msvc`.
    - A variable `win32_test_window_x86 := justfile_directory() / "target" / "i686-pc-windows-msvc" / "debug" / "platynui-win32-test-window.exe"` sits next to `egui_test_app` (`justfile:16`).
    - `test-acceptance-windows` (`:380-387`) runs the recipe after `just build-test-app-swing` (`:383`), as a hard prerequisite. Both its live-test step (`:386`) and its Robot step (`:387`) set `$env:PLATYNUI_WIN32_TEST_WINDOW_X86`, since each recipe line is its own PowerShell.
    - The package joins `windows_rust_packages` (`:12`).
  - `CONTRIBUTING.md` names the new Windows-lane prerequisite in its "End-to-end / acceptance" section, next to the Windows lane: `rustup target add i686-pc-windows-msvc`, and the MSVC x86 libraries, which the x64/x86 build tools of Visual Studio's C++ workload bring. Linux gains no prerequisite: the window never runs there, and the Linux cross checks compile the package for `x86_64-pc-windows-gnu` like every other entry of `windows_rust_packages`.
  - CI builds the window for 32-bit, so an i686-only break (pointer-width-dependent bindings or types, which the x64 builds cannot see) shows up on the push, not first in a local lane run. The `rust-windows` job (`.github/workflows/ci.yml:140-179`) installs the target through `dtolnay/rust-toolchain` (`targets: i686-pc-windows-msvc`) and runs `just build-win32-test-window-x86` after `just clippy`. The job's comment says why.

  Verify:
  - On a Windows machine, `just build-win32-test-window-x86` builds, and the binary's PE header names the machine `I386` (`dumpbin /headers` shows `14C machine (x86)`).
  - Started with `--title "Win32 Test Window" --auto-close 5`, it shows the window and exits by itself.
  - `just check` is clean on Windows, and `just cross-target-checks` is clean on a Linux host.
  - The `rust-windows` job passes with the new step on the push.

  Outcome (2026-09-28), Windows:
  - `just build-win32-test-window-x86` builds, and the PE header's machine is `0x014C` (I386; read from the file, since `dumpbin` is not on this machine).
  - Started with `--title "Win32 Test Window" --auto-close 5`, the window shows under that title and the process exits with 0 after about 5 s. An unknown argument exits with 2 and the usage line.
  - clippy is clean for x64 and i686, and `just check` is clean.

  Outcome (2026-09-29):
  - `just cross-target-checks` is clean on a Linux host, against `009595b`.
  - The `rust-windows` job passed on the push of `3315d9a` (CI run 36489572332), including its step *Build the 32-bit Win32 test window*.
- [x] 1.4 Add `tests/acceptance/win32/__init__.robot`, tagged `acceptance`, `real` and `platform:windows`, and `tests/acceptance/win32/process_attributes.robot`, following the `robot-test-style` skill.
  - The suite checks its prerequisite first. When `PLATYNUI_WIN32_TEST_WINDOW_X86` is unset or names no file, it fails with a message naming `just build-win32-test-window-x86`. It never skips.
  - It starts the window under a suite-unique title, waits with `BM.Wait Until Exists    /app:Application[@ProcessId=${pid}]/*[@Name="${title}"]` (no role: UIA may report a top-level `STATIC` window as `Text` rather than `Window`), then pins `/app:Application[@ProcessId=${pid}]` as its root, and asserts `@app:Architecture = x86` (*A 32-bit process on 64-bit Windows reports its own architecture*).
  - Its teardown terminates the process.

  Verify: on a Windows machine it passes today through the PE header (`map.rs:495-525`, `0x014c` → `x86` at `:520`) and must stay green after 3.2 replaces that path with the OS call. It is a regression guard, not a red test.

  Outcome (2026-09-28): it passes against `3d41929`, and with the change through `ProcessMachineTypeInfo` (build 26340).

## 2. The process reader

- [x] 2.1 Extend `crates/process` (`platynui-process`), which `snapshot-validity` created (design D1).
  - Add the reader's API next to the identity: `ProcessAttribute`, `ProcessAttributes`, `ProcessIdentity::read` and `read_all`. Put them in `src/attributes.rs` with Windows and Linux submodules. Stub every function 2.2 calls with an answer-nothing body for now: the API, the machine mapping, the combination rule, the `.exe` stripping, the start-time formatter, the FILETIME conversion, and both architecture paths.
  - Make `sys::Process` (`src/lib.rs:107-171`) with its `open`, `is_running`, `creation_time` and `handle` (`:108-111`, `:126`, `:140`, `:161`), and `sys::read_stat`/`Stat` (`:231-269`), `pub(crate)`. Update the crate description (`Cargo.toml:3`) and module documentation (`lib.rs:1-24`).
  - Keep `[lints] workspace = true`. Win32 and `getpwuid_r` modules carry `#[allow(unsafe_code, reason = "...")]`, as `mod sys` does, and SAFETY comments.
  - Windows: add `Win32_Security`, `Wdk_System_Threading` and `Win32_System_SystemInformation` to `Win32_Foundation` and `Win32_System_Threading` (`Cargo.toml:23-24`). `Win32_System_Time` is not needed (design D1).
  - Linux: `sysinfo` and `libc` go under `[target.'cfg(target_os = "linux")'.dependencies]`, and so does a Linux-only dev-dependency. No `chrono`: the crate formats the start time itself (decision M1).

  Verify: `just check` is clean, and `just test-crate platynui-process` still passes the identity tests. On a Linux host, `just cross-target-checks` is clean.

  Outcome (2026-09-28): done, with `sysinfo` 0.39 (feature `system`) and `libc` under `cfg(target_os = "linux")`. clippy is clean for Windows, `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin` (checked from Windows); `just cross-target-checks` on a Linux host belongs to 7.2.
- [x] 2.2 Write the reader's unit tests first.
  - **Platform-independent**:
    - The machine mapping: `I386`→`x86`, `AMD64`→`x64`, `ARM` and `ARMNT`→`arm`, `ARM64`→`arm64`, anything else → nothing.
    - The `IsWow64Process2` combination rule as a pure function over the raw values (process machine, native machine): (`UNKNOWN`, `AMD64`)→`x64`, (`I386`, `AMD64`)→`x86`, (`UNKNOWN`, `ARM64`)→`arm64`, (`I386`, `ARM64`)→`x86`, (`ARMNT`, `ARM64`)→`arm`; (`UNKNOWN`, an unmapped native machine) → nothing.
    - The architecture choice as a pure function over the two paths' results: the primary's machine wins; the fallback applies only when the primary failed; both failed → nothing (*An architecture that cannot be read is absent, not the host's*).
    - The `.exe` stripping: case-insensitive, only a trailing `.exe`.
    - The start-time formatter over Unix seconds, and the FILETIME conversion.
  - **Linux, own process**:
    - The process name is the executable's file name, and the executable path is `current_exe`.
    - The command line contains the test binary's arguments joined by spaces.
    - The user name is the login of the effective UID.
    - The start time has the fixed format and lies within the last minute.
    - The architecture is absent.
  - **Linux, a copy of the test binary named `probe.v2`**, re-executed into `waiting_child` (`src/lib.rs:338-349`) with the crate's child environment (`:320`): the process name is `probe.v2`. After the binary is deleted, the path and the name carry no ` (deleted)`.
  - **Linux, partially readable processes**, holding whether or not the test runs as root:
    - For PID 1, `ProcessName` is absent when `ExecutablePath` is absent, and otherwise equals that path's file name. `StartTime` is present.
    - For a kernel thread, where one is visible, `CommandLine` is absent while `StartTime` is present.
  - **`cfg(windows)`, own process**, read through `ProcessIdentity::capture(std::process::id())`, which opens the pid with the limited right (`src/lib.rs:124-137`). Never read through `GetCurrentProcess()`, whose pseudo-handle carries every right:
    - The process name has no `.exe`, and the executable path is `current_exe`.
    - The command line contains the test binary's file name and its arguments, verbatim.
    - The user name is `%USERDOMAIN%\%USERNAME%`. When the account is local (`%USERDOMAIN%` equals `%COMPUTERNAME%`), its domain part is the computer name (*A Windows process owned by a local account names the computer as its domain*).
    - The start time has the fixed format and equals the identity's recorded creation time, truncated to the second.
    - The architecture is the build target's through the fallback path called directly, on an x64 or x86 host only (on Windows 11 on ARM64 the fallback reports an emulated x64 process as `arm64`, design D3). Through the primary call it is the build target's on build 22000 or later; before that build the primary call answers nothing.
  - **`cfg(windows)`, the System process (PID 4)** (decision M4): no attribute has an empty value, each is present or absent on its own, and `StartTime` is present when the identity has a start.
  - **Reads bound to a recorded identity** (design D5), on Windows and Linux. Each of these answers nothing for every attribute, after a positive control in the same test:
    - a read through the identity of a `WaitingChild` (`src/lib.rs:351-389`), whose `ProcessName` is present while it lives, and absent once it has been killed and reaped;
    - a read through an identity with the right pid and another start, built as in `another_start_time_is_another_process` (`:449-456`), after the same read with the real start returned a value;
    - a read through an identity without a start (decision M2).
  - `capture(0)` and `capture(UNUSED_PID)` are already `None` (`:458-462`); the identity-without-start case above covers the reader.
  - Optional, `cfg(windows)` and ignored: when `PLATYNUI_WIN32_TEST_WINDOW_X86` names a file, start it with `--auto-close 10`; the fallback answers `x86`, and on build 22000 or later so does the primary call. This needs `-p platynui-process` in the lane's ignored-only step (`justfile:386`), which 1.3 gives the variable.

  Verify: `just test-crate platynui-process` fails on the reader tests that expect a value against the stubs, while the identity tests (`src/lib.rs:391-538`) still pass. The absence-only assertions are green against the stubs, which is why each has its positive control.

  Outcome (2026-09-28), Windows: against the stubs, 15 reader tests fail and the 11 identity tests pass.

  A review of the reader tightened the tests:
  - The ended-process tests read every attribute by name as well as the listing, and do so once the child is killed but not yet reaped (a zombie on Linux), and again once it is reaped.
  - A child started with `C:\My Files\input.txt` as an argument proves the quoting.
  - Without elevation PID 4 cannot be opened at all (measured), so its test checks only that nothing is empty (decision M4, amended in design.md). The partly readable process is a child whose token DACL denies the test's user `TOKEN_QUERY`: its `UserName` is absent, the five other attributes are read, and the lookups agree with the listing.
  - A command name in `/proc/<pid>/stat` that is not UTF-8 no longer hides the start time (`read_stat` reads bytes; a new identity test).

  The Linux tests are written and lint-clean for Linux; they run in 7.2.
- [x] 2.3 Implement the platform-independent helpers (machine mapping, fallback combination rule, `.exe` stripping, the start-time formatter and the FILETIME conversion). Then implement the Windows reader from `crates/provider-windows-uia/src/map.rs:289-479`, with design D1, D2, D3 and D5:
  - one handle per read, opened through `sys::Process::open` (`crates/process/src/lib.rs:124-137`), with the process confirmed running and its creation time equal to the recorded start;
  - `QueryFullProcessImageNameW` with one 32 768-unit buffer. `map.rs:327` compares an HRESULT with the raw code, so its growth never runs;
  - the verbatim command line; `DOMAIN\user`;
  - `StartTime` from the recorded creation time, truncated to the second;
  - `GetProcessInformation(ProcessMachineTypeInfo)` and the `IsWow64Process2` fallback as two separately callable paths on that handle, and no PE header.

  Leave behind the comments that no longer hold: `map.rs:291` on module queries, `:342-343` "not implemented", `:345-346` on full rights. Verify: on a Windows machine, `just check` is clean and the platform-independent and Windows tests of 2.2 pass with `just test-crate platynui-process`; on a Linux host, `just cross-target-checks` is clean.

  Outcome (2026-09-28), Windows:
  - `just test-crate platynui-process` passes 28 of 28, and `just check` is clean.
  - The ignored test with `PLATYNUI_WIN32_TEST_WINDOW_X86` passes: the 32-bit window reads `x86` through `IsWow64Process2` and through `ProcessMachineTypeInfo`.
  - The lane's ignored-only step now runs `-p platynui-process` as well (`justfile`).
- [x] 2.4 Implement the Linux reader on `sysinfo`, moving `with_process` and `resolve_username` (`crates/provider-atspi/src/process.rs:11-25`, `:85-122`, the latter with a reason on its `allow`), with design D1, D2 and D5:
  - one targeted refresh per read, of only this pid and only exe and user; the command line comes from `/proc/<pid>/cmdline`, read in the same start-time window, since `sysinfo`'s `cmd()` trims every argument and drops empty ones;
  - the process name is the file name of `exe()`, with no stem cut and no `comm` fallback; `sysinfo` already removes the ` (deleted)` suffix;
  - the user name resolves `effective_user_id()` through `getpwuid_r`, without the real-UID fallback of `process.rs:73`;
  - the start time is `start_time()`, formatted by the crate's helper; it is absent when it is `0`;
  - field 22 from `sys::read_stat` (`crates/process/src/lib.rs:266-269`) equals the recorded start before and after the refresh; otherwise the read answers nothing.

  Verify on a Linux host: the platform-independent and Linux tests of 2.2 pass with `just test-crate platynui-process`. The same run can be recorded as the crate half of `snapshot-validity` task 9.3.

  Outcome (2026-09-28): implemented and lint-clean for `x86_64-unknown-linux-gnu`. Not run: that needs the Linux host.

  Outcome (2026-09-29), Linux host, against `009595b`, run as an unprivileged user: `just test-crate platynui-process` passes 29 of 29, the Linux reader tests included (own process, the deleted `probe.v2` copy, PID 1, a kernel thread, a command line with ` padded ` and an empty argument, and the reads bound to a recorded identity).

  A stress run found *a command line keeps every argument as it is* flaky on Linux: 14 of 1000 runs failed, and one failure also came up in a full `just test`. `spawn` returns while the child's `execve` is still under way. In that window `/proc/<pid>/cmdline` gives the parent's arguments, or none. The fix is in the test child (`crates/process/src/lib.rs`, `CHILD_READY`): the waiting child prints a line once it runs, and `try_spawn_from` returns only after reading it. The test then passes 2000 of 2000, so does *a process is named after its full file name*, and the whole crate passes 200 stress rounds. The child's output is now piped on Windows too, so 7.1 runs that change there first.

  Decided (maintainer, 2026-09-28): `sysinfo`'s `cmd()` trims every argument and drops empty ones (`split_content`, `sysinfo` 0.39.3 `src/unix/linux/process.rs:988-1004`), so `CommandLine` comes from `/proc/<pid>/cmdline` itself, in the same start-time window; everything else stays on `sysinfo`. The joining is a platform-independent function (`join_command_line`), tested on every platform. The Linux test *a command line keeps every argument as it is* starts a child with ` padded ` and an empty argument.

## 3. Providers onto the reader

The application nodes of 3.1 to 3.3 read their `app:` attributes through the `ProcessIdentity` they recorded at creation (`crates/provider-java/src/agent/app.rs:65`, `crates/provider-windows-uia/src/node.rs:1771`, `crates/provider-java-jab/src/node.rs:1329`). A node without an identity, a node whose process has ended or been replaced, and a node without a recorded start list no `app:` process attribute (design D5, decision M2). `control:ProcessId` stays for every positive pid (decision M3). The agent comes first, since it is the main Java path.

- [x] 3.1 The Java provider's agent application node (design D4, D5, D6).
  - **Tests first**, in `crates/provider-java/src/agent/app.rs` next to the validity tests (`:225-284`), with `session()` (`:234-236`) over `AgentSession::unconnected` (`session.rs:282-283`):
    - For the own process: the `app:` process attributes are the reader's, and none of them is under `control:`. A lookup by name agrees with the listing, and no `app:` value is empty.
    - Facts built with `serde_json::from_value(json!({"name": "Main", "userName": "someone-else", "architecture": "amd64", "startTimeMillis": 1}))` change no `app:` attribute, while `control:Name` stays `Main`. Built from JSON, the fixture survives the removal of those fields from `ProcessFacts`, and it proves that serde ignores them (design D4).
    - A node for `0x3FFF_FFFC` (`:264-270`) lists no `app:` process attribute.
    - Listing calls nothing in the JVM: `failure_count` stays `0` (`session.rs:307-308`), as in `:272-283`.
    - `backend.rs`: `consider_attaching(&[0])` returns nothing and records no attempt (`attach_attempts`, `:99`).
    - Optional live test: one fixture launched with the agent (`launch_with_agent`, `live_fixture.rs:90`). The agent node from the default provider and the JAB node from `build_provider(&jab_only())` (`:226`, `:232`) agree on every `app:` attribute present on both.
  - **Then implement**:
    - Attributes from the reader through the node's identity (`app.rs:65`), under `app:`, decided at enumeration, with `attribute()` overridden.
    - `control:Name` and the `native:` facts (`:173-180`) stay.
    - `ProcessFacts` (`:24-43`) keeps `name`, `vm_name` and `java_version`; `push_optional` (`:191-195`) goes. The agent and its version stay untouched.
    - `attach_to_agentless` (`backend.rs:257-297`) drops pid `0` before its handshake lookup.
    - The module documentation of `app.rs` (`:8-12`) no longer says that the JVM's self-description replaces a process query.

  Verify: on a Windows machine, `just check` is clean, `just test-crate platynui-provider-java` passes, and the agent half of the Swing suite of 1.2 passes.

  Outcome (2026-09-28):
  - Against the old node, three new tests failed: the `app:` attributes, the JVM's self-description, and pid `0` on attach. The listing/lookup agreement and "no call into the JVM" were already green, as guards.
  - With the change, the 53 unit tests pass and `just check` is clean.
  - The Swing suite of 1.2 passes in full, and so does the live test *The agent and the bridge report one process identically*.
- [ ] 3.2 Windows UIA.
  - **Tests first** (`cfg(windows)`, `attribute_surface_tests`, `node.rs:2024-2384`, next to `application_node_carries_the_common_attributes` `:2124-2139` and the validity tests `:2344-2361`):
    - an application node for a pid without a process (`ApplicationNode::orphan(0x3FFF_FFFC)`, as at `:2357-2361`) lists no `app:` process attribute;
    - a `WindowlessChild` (`:2313-2338`) that is killed and reaped after its node was created, as at `:2344-2352`: the node lists no `app:` process attribute, and every `app:` lookup by name answers nothing;
    - for the own process and for a `WindowlessChild`, a lookup by name agrees with the listing in both directions, for every `control:` and `app:` name. This follows `gated_attributes_agree_in_both_directions` (`:2078-2087`), which covers only `control:` on the desktop root;
    - an application node for PID 4 lists no `app:` attribute, and no `control:ProcessId`, that is empty, null or `unknown` (decision M4). `control:Name` may stay `""` there (design Non-Goals, D9);
    - `control:Name` and `control:Id` of the own process equal `app:ProcessName` (design D9);
    - the scope function (`Option<i32>` → `UiaIdScope`) maps `Some(0)`, `Some(-1)` and `None` to the desktop scope, and `Some(p > 0)` to `App { pid: p }`;
    - the window-filter predicate rejects target `0` against window pid `0`. Today's filter cannot fail through the ready-window enumeration (design, Context), hence the predicate.
  - **Then implement design D5, D6 and D9**:
    - `AppAttrsIter` (`:1513-1557`) takes the node's identity in addition to its pid, and calls `read_all` when it reaches the first process attribute.
    - `ApplicationNode` overrides `attribute()`. It answers `app:` names with one read each, and `control:` names other than `Name` and `Id` without a process read; those two use the cached name. Otherwise `@Technology` and `@SupportedPatterns` (`:1549-1550`) pay the full listing through the default `attributes().find()` (`crates/core/src/ui/node.rs:59-61`).
    - `control:ProcessId` (`:1355-1368`) is listed only for a positive pid.
    - `ApplicationNode::name` (`:1884-1901`) takes `read(ProcessName)`.
    - `control:Name` reads the cached name through its owner, like `IdAttr` (`:1941-1962`). This replaces `AppDisplayNameAttr`'s own read and comment (`:1310-1338`).
    - The placeholders go (`:1391`, `:1407`, `:1413`, `:1429`, `:1435`, `:1451`, `:1457`, `:1473`, `:1479`, `:1503`, `:1509`).
    - The hit-test (`provider.rs:528-548`) and the filter (`:252-258`) use the extracted functions.
    - Features that only the moved helpers used leave `Cargo.toml`: `Win32_System_ProcessStatus` (`:28`), `Win32_System_Time` (`:29`), `Win32_System_SystemInformation` (`:30`), `Win32_Security` (`:31`) and `Wdk_System_Threading` (`:36`). Grep finds them only in `map.rs:13-27` and `:348`. `Win32_System_Threading` and `Win32_Foundation` stay for `node.rs:22-23`.
  - `gate-uia-window-patterns` adds a window-manager argument to `ApplicationNode::orphan` and `new`; whichever lands second updates these tests.

  Verify: on a Windows machine, `just check` is clean, `just test-crate platynui-provider-windows-uia` passes, the egui suite of 1.1 and the win32 suite of 1.4 pass, and `tests/acceptance/egui/hit_test.robot` and `inspector_picker.robot` stay green.

  Outcome (2026-09-28):
  - Against `3d41929` (worktree run), *…whose process has ended reports no process attribute* fails: the old node lists `ProcessName ""`, `CommandLine Null` and a start time with milliseconds.
  - Two other new tests pass there only because the old `AppAttrsIter` stopped the whole listing when `Id` was absent, which it is when the name cannot be read. `ProcessId`, `Technology`, `SupportedPatterns` and the `app:` attributes were then never listed. The new iterator skips an absent attribute instead of ending the listing: a behavior change for the release notes.
  - With the change, the 32 unit tests pass, `just check` is clean, and the egui suite and the win32 suite pass.
  - After review, a lookup by a `control` name other than `Name` and `Id` reads no process. A listing opens the process once: the read that lists the `app:` attributes also seeds the node's shared name cell, which `Id`'s presence depends on.
  - Positive controls followed: a nameless node keeps `ProcessId`, `Technology` and `SupportedPatterns`; pid `0` and `-1` list no `ProcessId`; PID 4 keeps `ProcessId = 4`. The UIA tests pass 34 of 34.

  Open: `hit_test.robot` and `inspector_picker.robot` move the pointer and run with the lane (7.1).
- [ ] 3.3 JAB.
  - **Tests first**:
    - `JabAppNode` needs a live bridge client (`node.rs:1364-1385`, `provider.rs:222-226`). Its process attributes and its name therefore come from one function of pid and identity that needs no client. Unit tests:
      - it lists the reader's values for this process, and the name equals `ProcessName`;
      - without an identity it lists no `app:` process attribute;
      - a lookup by name agrees with the listing.
    - `without_a_dll_awt_windows_are_reported_unserved_with_their_processes` (`provider.rs:758-783`) gains a `SunAwtFrame` candidate with `pid: None`. It appears in neither `unserved` nor `java_processes`. This fails to compile first.
    - `process_id_of` (`node.rs:936-947`) answers `None` for a null window. This is green today (`:946`); keep it as a regression guard.
    - Live, in `crates/provider-java/tests/live_fixture.rs`, through `fixture_application(…, "JAB")` (`:1350`), next to `live_a_killed_jvm_leaves_no_valid_application_node_on_the_bridge` (`:1331`): `control:Name` equals `app:ProcessName`, `app:CommandLine` contains the fixture's title with its quotes, and no attribute has an empty value.
  - **Then implement design D5, D6 and D9**:
    - Presence at enumeration; `attribute()` overridden.
    - The reader replaces `process.rs` and `mod process;` (`lib.rs:47`), including its `"unknown"` and its image-name fallback.
    - The name (`node.rs:1398-1400`, listed at `:1445`) comes from `ProcessName`.
    - `process_id_of` becomes `pub(crate)`, and `provider.rs:530` and `:631` go through it.
    - `WindowCandidate.pid` becomes `Option<u32>`, and both passes skip `None`.
    - `top_level_window_at` answers `None` without a process, so the hit-test abstains (design D6; `hit_test_node` keeps its `pid: u32`).
    - `sysinfo` and `chrono` leave `Cargo.toml` (`:29-30`). `Win32_Security` and `Win32_System_Threading` stay for `pump.rs:236-237`.

  Verify: on a Windows machine, `just check`, `just test-crate platynui-provider-java-jab`, and `just test-acceptance-windows`, which runs the live tests (`justfile:386`) and the JAB half of 1.2. `tests/acceptance/swing/picker.robot` stays green.

  Outcome (2026-09-28):
  - The 61 unit tests pass, and `just check` is clean.
  - The JAB half of 1.2 passes, and so do the live tests *The bridge application node carries its process attributes* and the two killed-JVM tests.

  Open: the full lane with `picker.robot` (7.1).
- [x] 3.4 Move AT-SPI onto the reader (on a Linux host).
  - `crates/provider-atspi/src/process.rs` and `mod process;` (`lib.rs:20`) go away. The node records `ProcessIdentity::capture(local_number)` once. The desktop enumeration records it when it creates the node, and a node built elsewhere records it on its first definitive local number. `AppAttr` reads through that identity, so a pid reused after the node was created is never read (design D5). `pidns_harness.rs:136` captures an identity for its number and reads through it. `AppAttr`, `app_attribute` and the named lookup (`node.rs:386-397`, `:1864-1936`) stay.
  - `platynui-process` joins `[dependencies]`. `sysinfo` and `chrono` (`Cargo.toml:26-27`) and `libc` (`:33-35`) leave AT-SPI's manifest; `process.rs` is their only user there, and `sysinfo` and `libc` now sit in `crates/process`.
  - The reader is called only with `process_table` (`node.rs:1840-1849`), which is set only under local numbering, so the `sidecar-deployment` gate is unchanged (`node.rs:391-395`, `:1300-1303`).

  Verify on a Linux host:
  - `just test-crate platynui-provider-atspi` passes.
  - `just test-atspi-pidns dbus-daemon` and `just test-atspi-pidns dbus-broker` pass.
  - The egui suite of 1.1 stays green on both Linux lanes.

  Outcome (2026-09-28):
  - Implemented, after review, with one identity per node rather than per listing. The desktop enumeration records it from the local number when it creates the node, inside the new `AtspiNode::seed_application`, which also keeps that function within clippy's 100 lines. Other nodes record it on their first definitive local number.
  - A listing reads the five attributes once through the identity. A named lookup reads only the one it names, and every other lookup reads none. `invalidate` keeps the record.
  - The new test *a node reads only the process recorded for it* kills a recorded child and then offers a readable number: nothing is read.
  - clippy is clean for Windows and `x86_64-unknown-linux-gnu`, and the crate's 108 unit tests pass on Windows, where the D-Bus-free ones run.

  Outcome (2026-09-29), Linux host, against `009595b`:
  - `just test-crate platynui-provider-atspi` passes 108 of 108, *a node reads only the process recorded for it* included.
  - `just test-atspi-pidns dbus-daemon` and `just test-atspi-pidns dbus-broker` each pass 2 of 2.
  - The egui suite of 1.1 passes 8 of 8 on both Linux lanes (7.2).

## 4. Process ID 0 in the window managers

- [x] 4.1 Write tests first: each window manager's process-ID reader answers "no process" for a node carrying `ProcessId = 0` as `Integer(0)`, `Number(0.0)` and `String("0")` (*A window is never looked up by process ID 0*). `Integer(0)` and `String("0")` fail today on all three. `Number(0.0)` fails only on the Wayland backend (`platynui_ipc.rs:452`) and is a guard on Windows and X11 (`:92-97`; X11 `:415-420`).

  Then make `pid_from_attr` accept only a positive number in:
  - `crates/platform-windows/src/window_manager.rs:87-101`, whose module documentation (`:13`) is corrected to `control:ProcessId`; tests into the existing module;
  - `crates/platform-linux-x11/src/window_manager.rs:411-424`;
  - `crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs:447-461`.

  A node with `0` then counts as having no process ID, and `extract_pid` continues to its ancestors. On Linux this implements what `sidecar-deployment` already requires (`openspec/specs/sidecar-deployment/spec.md:259`). Verify:
  - On a Windows machine, `just test-crate platynui-platform-windows` passes.
  - On a Linux host, `just test-crate platynui-platform-linux-x11` and `just test-crate platynui-platform-linux-wayland` pass.

  Outcome (2026-09-28):
  - Windows: both new tests failed first (`Some(0)`), and now 19 of 19 pass.
  - X11 and Wayland: the same tests and the same fix are in place, and clippy is clean for Linux.

  Outcome (2026-09-29), Linux host, against `009595b`: `just test-crate platynui-platform-linux-x11` passes 27 of 27 and `just test-crate platynui-platform-linux-wayland` 43 of 43 (both within `just test`), the two process-ID tests of each included.

## 5. The mock and the Python `Application`

- [x] 5.1 Write a test first in `crates/runtime/src/runtime/evaluation.rs:223-315`, next to the existing mock-runtime evaluations (`rt_runtime_mock`, `:226`) (design D7, spec *Listing and predicate agree for a missing attribute*):
  - `/app:Application[@ProcessId][@app:ProcessName][not(@app:CommandLine)]` selects exactly "Mock Application".
  - Its attribute listing contains `ProcessName` but not `CommandLine`.
  - "Mock Settings" carries no `ProcessId`.

  Confirm it fails with `just test-crate platynui-runtime`. Then give "Mock Application" (after `mock_tree.xml:4`) `control:ProcessId = 4242`, the process ID its window already carries (`crates/provider-mock/assets/mock_tree.xml:7`), which parses to an `Integer`, and the `app:` attributes `ProcessName`, `ExecutablePath`, `UserName`, `StartTime` in the spec's formats, deliberately without `CommandLine`.

  Verify: `just test-crate platynui-runtime` passes, and `just test`, `just test-python` and `just test-baremetal` stay green. The mock tree feeds many tests.

  Outcome (2026-09-28):
  - The test failed first, selecting nothing.
  - With the attributes it passes, and so do `just test` (2440 of 2440), `just test-python` (891 of 891) and `just test-baremetal` (122 of 122).
  - Not obvious: on the mock an application's `Name` lives in the `app` namespace (`@app:Name`), so the test finds the node through its listing.
- [x] 5.2 The Python `Application` (design D10, spec *The Python Application object reads the process attributes*).
  - **Tests first**:
    - In `tests/PlatynUI/test_application.py`:
      - `process_id` reads `control:ProcessId` and `process_name` reads `app:ProcessName`. Both are `None` when the attribute is absent, and raise `TypeError` only for a present value of the wrong type.
      - A node that carries `ProcessName` only under `control` gives `None`.
      - Every `('ProcessId', 'app')` key is re-keyed to `('ProcessId', 'control')`: `:82`, `:93`, `:205`, `:216`, `:227`, `:242`, `:265`, `:279`.
      - `_force_exit` returns at once for `None`.
    - A mock-runtime pytest next to `tests/PlatynUI/test_ui_node_adapter.py`, using its fixture: "Mock Application" gives `4242` and its name, and "Mock Settings" gives `None` for both. This covers the `KeyError` path (`src/PlatynUI/core/adapters/ui_node.py:668-672`).
  - **Then implement** in `src/PlatynUI/ui/application.py:31-49` and `:78-97`:
    - `int | None` and `str | None`; an absent attribute arrives as `None` from the stub or as `KeyError` from the real adapter;
    - `:84` becomes `if pid is None or pid <= 0`. Without that, `None <= 0` raises outside the `try`, and `just mypy` fails.

  Verify: `just test-python` passes and `just mypy` is clean.

  Outcome (2026-09-28):
  - Ten tests failed against the old properties.
  - Now `just test-python` passes 891 of 891, and `just mypy` and ruff are clean.
  - A `bool` does not count as an `int` for `process_id`.

## 6. Documentation

- [x] 6.1 Update the docs as design D8 describes. They point to the spec for formats and add no status content.
  - `dev-docs/architecture.md`:
    - The §2 crate line `:31` becomes `# Process identity (pid + start time) and process attributes — platynui-process`, and the apps tree (`:48-53`) gains `win32-test-window`.
    - The Application row of the pattern catalog (`:362`): `ProcessId` optional, all six `app:` attributes listed, and no "executable stem" note.
    - The per-platform source table (`:502-510`): D2 and D3 sources, and no `comm`, `cmdline[0]`, PE header or ELF. Its macOS cells are emptied, since `localizedName` is a display name.
    - The Windows UIA checklist (`:574`): `ProcessName` instead of `Name`, and the presence rule.
  - `dev-docs/platform-windows.md`: the UIA application-node attributes (`:77`) and the JAB section's process metadata (`:126`).
  - `dev-docs/platform-linux.md`: `:135` and `:159` follow D2, and the presence rule points to the capability; `:161` stays.
  - `dev-docs/planning.md`: the parity item `:448` is resolved, and `:465` loses its `comm`/stem source and its "process executable stem" rationale. The checked items `:126` and `:449` keep their text, each with "(superseded by application-process-attributes)".
  - `dev-docs/platform-linux-wayland.md:654`: the link to `crates/provider-atspi/src/process.rs` points to `crates/process`.
  - `dev-docs/python-library-design.md` (`:4397`, `:4399`, `:4417`): the `Application` sketch reads `control:ProcessId` and may answer `None`. The text is German; add a short English summary per AGENTS.md.
  - `AGENTS.md`: the `crates/process` entry (`:15`) becomes "— which process a pid stands for (pid plus start time), whether it still runs, and its process attributes, one reader per platform; used by the providers' application nodes, depends on no other PlatynUI crate", and the apps list (`:17`) gains `apps/win32-test-window`.
  - `dev-docs/testing-strategy.md` §5: extend the existing helper bullet (`:272-275`, "Measurement helpers are not fixtures") with `apps/win32-test-window`, and say that it is not the planned native-Win32 row (`:265`).
  - `crates/core/src/ui/attributes.rs`: a one-line pointer to the capability before `:126`.
  - **User documentation**, in the user-facing voice, in `src/PlatynUI/BareMetal/__init__.py`:
    - the `Get Attribute` documentation (`:2395-2410`): `:2397-2398` names `app:` next to `native:`, `:2405` gains "or with an ``app:`` or ``native:`` prefix", and an example is added;
    - the namespaces text (`:569-571`);
    - a new `== Process attributes ==` subsection after `:858`: what each attribute is, that each may be absent, how to test for one with `[@app:X]`, and their formats. `DOMAIN\\user` is escaped, because the docstring is not raw.

  Verify:
  - `grep -rn -e '\bcomm\b' -e 'cmdline\[0\]' -e 'executable stem' -e 'GetNativeSystemInfo' -e 'PE[- ]header' -e '\bELF\b' -e 'provider-atspi/src/process.rs' dev-docs AGENTS.md` shows only the expected hits: `dev-docs/platform-linux.md:161` and `dev-docs/planning.md:131` (why Linux has no architecture); `planning.md:126` and `:449` with the superseded note; `dev-docs/python-migration-status.md:364` (history).
  - No doc restates a format that the spec does not fix.
  - The rendered libdoc of `PlatynUI.BareMetal` shows the new section.

## 7. Verification and commit

- [ ] 7.1 On a Windows machine, run `just check` and `just test`, then `just install-provider-java`, `just test-acceptance-windows` (with the maintainer's go-ahead: it takes over pointer and keyboard), and `uv run --no-sync robotcode results log --level WARN --execution-messages`. Verify:
  - the lane is green, the live step included (`justfile:386`), with the egui, Swing and win32 suites;
  - there is no WARN or ERROR from PlatynUI;
  - compared with `snapshot-validity` 9.2 (125 of 125, `openspec/changes/snapshot-validity/tasks.md:89`), only the new tests are added;
  - the notes record whether the lane account is local, since only then does the `UserName` check prove *A Windows process owned by a local account names the computer as its domain*.

  Record the results in the change notes.
- [x] 7.2 On a Linux host, run the full gate:
  1. `just check` and `just test`.
  2. `just test-python` and `just test-baremetal`, which build the mock native module.
  3. `just build-native`, to rebuild the real native module before the real lanes.
  4. `just headless=true test-acceptance-x11`, then `uv run --no-sync robotcode results summary --failed` at once, because the next lane overwrites `results/output.xml`.
  5. The same for `just headless=true test-acceptance-compositor`.
  6. `just test-atspi-pidns dbus-daemon` and `just test-atspi-pidns dbus-broker`.
  7. `just cross-target-checks` (`justfile:576`, which runs all four cross checks).
  8. `just test-crate platynui-process` (with `snapshot-validity` 9.3).

  Verify: all green.

  Outcome (2026-09-29), on a clean worktree of `009595b`, so that no uncommitted work of other changes was part of the run:
  1. `just check` is clean, and `cargo fmt` changed nothing. `just test` passes 2457 of 2457 (33 skipped).
  2. `just test-python` passes 891 of 891, and `just test-baremetal` 122 of 122.
  3. `just build-native` ran through the lane recipes.
  4. The X11 lane passes 93 of 93.
  5. The compositor lane passes 94 of 94.
  6. Both pidns runs pass 2 of 2.
  7. `just cross-target-checks` is clean.
  8. `just test-crate platynui-process` passes 29 of 29.

  Neither lane logged a WARN or ERROR. Their FAIL messages all come from expected failures inside passing tests, and one from a resize poll that succeeded on its next try.

  A later `just test` on another checkout hit the flaky process test recorded under 2.4. After its fix, `just test` passes 2460 of 2460 there.
- [x] 7.3 When the maintainer asks, commit in reviewable steps, each lint-clean with its tests green:
  1. the reader;
  2. the agent, UIA and JAB, with the suites, the 32-bit window and the lane wiring;
  3. AT-SPI;
  4. the window-manager guards;
  5. the mock and the Python `Application`;
  6. the docs.

  The provider step is `fix(providers): report process attributes by one contract` (the history uses `(providers)`: 94f5f97, 04d3aed, 21f9429). Subjects are at most 72 characters.
  - The provider body names the reader and the four aligned providers, and lists the proposal's behavior changes for the changelog, including AT-SPI's process name and the Python `Application`.
  - No `!` and no `BREAKING CHANGE:` footer: PlatynUI is at 0.x (maintainer decision).

  Verify: `git log -1 --format=%B` shows the subject and the list, and `just pre-commit` passed before each commit.

  Outcome (2026-09-28), asked for by the maintainer:
  - The commits are `3c151f1` (reader), `bfd9297` (agent, UIA, JAB, suites, 32-bit window, lane wiring, CI), `6c85cb8` (AT-SPI), `ca4a765` (window-manager guards), `ca499f1` (mock and Python `Application`) and `4a07527` (docs), plus this record.
  - `just pre-commit` passed on the final state. Each step was checked on its own, with the rest stashed: `just check` and `just test` each time, and `just test-python` and `just test-baremetal` for the mock and Python step. Each step carries the `Cargo.lock` its manifests produce.
  - Each commit body lists its own behavior changes rather than the provider commit listing all of them, so that the changelog attributes each change to its commit.
