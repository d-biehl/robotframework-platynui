# Design

## Context

The motivation is in proposal.md (Why), and the contract is in `specs/application-process-attributes/spec.md`. This section records only the current code that shapes the approach.

**Verified** marks what was read in the tree at `1c3300c` (2026-09-28), after `snapshot-validity` (`8f6fc02`, `94f5f97`) created `crates/process` and gave the three application nodes a recorded process identity. The rest is marked as assumed.

**Four implementations of one question.** Every provider that builds application nodes reads the process table itself.

- **Windows UIA** uses native Win32 calls in `crates/provider-windows-uia/src/map.rs`:
  - `open_process_query` :289-299, which first asks for `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ` (:292) and falls back to limited query rights (:297);
  - `query_executable_path` :301-340, whose buffer growth compares an HRESULT with the raw Win32 code 122 (:327) and never runs, so a path longer than 4095 UTF-16 units answers nothing;
  - the command line through `NtQueryInformationProcess` :344-399, verbatim;
  - `DOMAIN\user` through `LookupAccountSidW` :402-456;
  - the start time with milliseconds :458-479 (`'.{:03}Z'` :474);
  - the architecture from the PE header of the executable path :495-525. Its mapping turns `ARM` (0x1C0) into `arm64` and anything else, `ARMNT` (0x1C4) included, into `"unknown"` (:518-523). Otherwise `GetNativeSystemInfo` :481-493, which ignores the process handle and reports the host.

  All seven attribute objects are listed unconditionally (`node.rs:1513-1557`, indices 4-10 at `:1540-1546`). Each `value()` answers `""`, a null or `"unknown"` when it cannot read:
  - `ProcessName` `:1391`
  - `ExecutablePath` `:1407`/`:1413`
  - `CommandLine` `:1429`/`:1435`
  - `UserName` `:1451`/`:1457`
  - `StartTime` `:1473`/`:1479`
  - `Architecture`: the host at `:1503`, `"unknown"` at `:1509`

  `ProcessId` always answers the node's pid, `0` included (`:1355-1368`). **Verified.**

  The display name comes from the same helpers, in three places that each call `open_process_query`, `query_executable_path` and `Path::file_stem`:
  - `AppDisplayNameAttr` (`node.rs:1310-1338`), read again on every `value()`;
  - `AppProcessNameAttr` (`:1370-1393`);
  - `ApplicationNode::name` (`:1884-1901`), cached in `name_cell` (`:1775`).

  `id()` returns the name (`:1905-1908`), which `AppAttrsIter` emits as `control:Id` (`:1531-1539`) through the owner-backed `IdAttr` (`:1941-1962`). Since `snapshot-validity` the node records a `ProcessIdentity` at creation (`:1771`, `:1794`) and answers `is_valid` from it (`:1934-1936`). **Verified.**
- **JAB** uses `sysinfo` in `crates/provider-java-jab/src/process.rs`:
  - the process name from the executable's stem :27. Under limited rights `sysinfo` answers an empty executable path, and the name then falls back to the Toolhelp image name with `.exe`, for example `javaw.exe` (:34-35);
  - the command line as `sysinfo` arguments joined by spaces :45;
  - the bare user name :57;
  - the start time to the second :64;
  - the architecture from the PE header, falling back to the compile-time `std::env::consts::ARCH` :79-85. Its parser answers `"unknown"` for a machine it does not know (:109).

  A value it cannot read becomes a null in the attribute layer (`node.rs:1513`). `JabAppNode`'s display name reads the same process name (`node.rs:1398-1400`, cached at `:1339`) and is listed eagerly (`:1445`). Since `snapshot-validity` the node records a `ProcessIdentity` of its pid at creation (`:1329`, `:1373`) and answers `is_valid` from it (`:1466-1468`). Pid `0` records none (`crates/process/src/lib.rs:64-66`). **Verified.** That `sysinfo` 0.39.3 answers an empty executable path under limited rights is read from its source, not re-measured.
- **The Java agent** does not read the process table at all. It reports what the JVM says about itself (`agent/process`, `crates/provider-java/src/agent/app.rs:48`). The node lists it in the `control` namespace (`push_optional`, `:191-195`, called at `:163-167`), with the start time as an integer of epoch milliseconds (`:168-170`). The facts are system properties (`java/agent/src/main/java/platynui/agent/ProcessFacts.java:27-42`):
  - the process name is the main class's simple name or the jar's file name (`:52-64`);
  - the executable path is always `<java.home>/bin/java(.exe)` (`:66-75`), also for a JVM started through `javaw.exe`. On Java 8, `java.home` is the JDK's `jre` directory (**assumed** from the JDK 8 layout; `jre\bin\java.exe` exists in the lane's JDK 8);
  - the command line is `sun.java.command` (`:32`);
  - the user name is `user.name` (`:33`), and the architecture is `os.arch` (`:34`);
  - the start time needs `ProcessHandle` and is absent on Java 8 (`:77-96`), which the Windows lane runs the fixture on (`justfile:378-379`).

  The module documentation of `app.rs` (`:8-12`) and the Javadoc of `ProcessFacts.java` (`:10-14`) give the reason: the JVM's self-description is cheaper and better than a host-side process query. Since `snapshot-validity` the node records a `ProcessIdentity` of the session's pid (`app.rs:65`, `:80`) and answers `is_valid` from it and the session (`:134-138`). **Verified.**
- **AT-SPI** uses `sysinfo` and `getpwuid_r` in `crates/provider-atspi/src/process.rs`. After `atspi-process-identity` it decides presence at enumeration and substitutes nothing. It still derives the process name from the executable's *stem* (`query_process_name` :31-44, stem :35-38), which turns `python3.12` into `python3`, and falls back to `sysinfo`'s `name()`, the kernel's `comm`, which is truncated to 15 characters (:40-42). The user name falls back from the effective to the real UID (`:73`). It builds a new `sysinfo::System` and refreshes the process for every single attribute (`:12-25`). `sysinfo` and `chrono` are regular dependencies (`Cargo.toml:26-27`), and `libc` is gated to Linux (`:33-35`); `process.rs` is the only user of all three. **Verified.**

**The process identity (verified).** `crates/process` holds `ProcessIdentity { pid, start }` with `capture` and `check` and a three-way `Liveness` (`crates/process/src/lib.rs:26-84`).

- On Windows, `sys::Process::open` (`:124-137`) opens the pid with `PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE`, then with the limited right alone. The recorded start is the `GetProcessTimes` creation time in 100 ns units (`:160-170`, stored at `:183`).
- On Linux the start is field 22 of `/proc/<pid>/stat`, parsed after the last `)` (`sys::parse_stat`/`read_stat`, `:231-269`).
- `start` is `None` only when capture could not read it (`:51-53`). On other Unix systems no start is ever recorded (`:296-303`).
- The crate depends on `windows` (Foundation, Threading) on Windows and `rustix` on Unix (`Cargo.toml:20-24`), and carries `#[allow(unsafe_code, reason = "...")]` on `mod sys` (`lib.rs:94-96`).

**What users and consumers read.**

- **Users** reach the metadata through:
  - XPath in any keyword that takes a selector; a bare `@X` is the `control` namespace (`crates/runtime/src/xpath.rs:637-646`, the mapping at `:638`), so the metadata is `@app:ProcessName`;
  - `Get Attribute    <application>    app:ProcessName`, which splits the prefix (`src/PlatynUI/BareMetal/__init__.py:2412-2414`) but whose documentation names only `native:` (`:2395-2410`, at `:2398` and `:2405`);
  - `UiNode.attribute(name, namespace)`, which raises `AttributeNotFoundError` for an absent attribute (`packages/native/src/runtime.rs:101-115`);
  - `PlatynUI.ui.Application`;
  - `platynui-cli query` and `snapshot`, which print every attribute and render a null as `null` (`crates/cli/src/commands/query.rs:104-145`, `snapshot.rs:297-299`, `:314-316`);
  - the Inspector's attribute view, which renders a null as `<null>` and copies a name in XPath form (`apps/inspector/src/model/tree_data.rs:403-449`).

  A null attribute exists today as an attribute node (`xpath.rs:649-656`: a present attribute becomes an attribute node whatever its value), whose null value atomizes to an empty sequence (`:1126`). So `[@app:X]` is true for it. The user documentation says nothing about these attributes. BareMetal mentions "an application's process details" (`:551`), documents pinning by `@ProcessId` (`:570`, `:620-621`), and gives a pinning example under *Targeting a specific application* (`:837`, `:858`). **Verified.**
- **`PlatynUI.ui.Application`** reads `app:ProcessId` (`src/PlatynUI/ui/application.py:31-39`), which no provider reports, and `app:ProcessName` (`:42-49`), which the Java agent reports under `control:`. Both fail when the value is not there. `UiNodeAdapter.attribute_value` raises `KeyError` for an absent attribute (`src/PlatynUI/core/adapters/ui_node.py:668-672`). The test stub answers `None` instead (`tests/PlatynUI/_ui_helpers.py:603-604`), which the properties turn into `TypeError`. **Verified.**
- **The three window managers** read `control:ProcessId` and accept `0` as an integer or a string:
  - `crates/platform-windows/src/window_manager.rs:87-101` (`:91`, `:98`), whose module documentation (`:13`) wrongly says `native:ProcessId`;
  - `crates/platform-linux-x11/src/window_manager.rs:411-424` (`:414`, `:421`);
  - the compositor backend also as the number `0.0` (`crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs:447-461`: `:450`, `:452`, `:458`).

  What a lookup with `0` does:
  - on Windows it matches any window whose process `GetWindowThreadProcessId` cannot name (`find_hwnd_for_pid`, `:126-128`);
  - on X11 it matches only a window that declares `_NET_WM_PID` 0 (`get_window_pid` `:125-128`, compared at `:181-182`);
  - the compositor never reports `0`, so there matching falls through to title and size (`platynui_ipc.rs:332-375`), as for a node without a process ID.

  `sidecar-deployment` already requires the Linux window manager and the compositor backend to resolve nothing for `0` (`openspec/specs/sidecar-deployment/spec.md:259`). **Verified.**
- **Process ID `0` elsewhere:**
  - The UIA point hit-test takes `get_process_id(&elem).ok()` with only a check against its own process (`provider.rs:528-531`). It builds an app-scoped ancestor chain from it (`:534-548`), which `UiaNode::attach_ancestor_chain` caps with `ApplicationNode::orphan(pid)` (`node.rs:221`; `orphan` at `:1816-1823`). For pid `0` that node lists `ProcessId = 0` and the runtime id `uia://app/0`. It records no identity and reports itself invalid (`node.rs:1934-1936`), but it still lists the `0`.
  - The root enumeration already skips pids ≤ 0 (`provider.rs:294`, `:316`), so only the hit-test builds an application node for `0`.
  - UIA's per-application window filter compares a target of `0` with the `0` that `GetWindowThreadProcessId` leaves when it fails (`provider.rs:252-258`). It fails only for a window destroyed after `EnumWindows`, which `window_is_ready` and `ElementFromHandle` then drop (`:265-270`). The comparison is latent; the guard is defence in depth (**assumed** from the code path).
  - JAB's enumeration turns a served window's process ID into an application node (`JabAppNode::new`, `provider.rs:349-357`). Its hit-test builds `JabAppNode::orphan(pid)` with a non-optional process ID (`node.rs:764-775`).
  - A guarded helper `process_id_of` exists (`node.rs:936-947`, private), but `WindowCandidate` carries a plain `u32` (`provider.rs:610-614`).
  - Both JAB passes drop the host's own process (`:490`, `:568`, and the hit-test at `:408`), but not `0`. Automatic attachment (`crates/provider-java/src/agent/backend.rs:257-297`) drops neither; a `0` there costs an attempt (`:271-275`), then fails in `load_agent`'s liveness check. Its input is the router's union over every window-enumerating backend (`crates/provider-java/src/provider.rs:204-208`).
  - The agent's own sessions never carry `0`: a handshake file is accepted only for a live process (`crates/java-agent/src/handshake.rs:99-102`).

  **Verified.**

**Workspace rules (verified).** The workspace lints (`Cargo.toml:38-53`) deny `unsafe_code`, warn on `unused_crate_dependencies` (an error under `clippy -D warnings`), and enable pedantic clippy. Every crate sets `[lints] workspace = true`. FFI modules carry a scoped allow with a reason; in `crates/process` it has the form `#[allow(unsafe_code, reason = "the process queries are Win32 calls")]` on `mod sys`. Platform dependencies are target-gated, following `crates/provider-windows-uia/Cargo.toml:15-17`. `crates/provider-java` puts all its dependencies under `cfg(windows)`.

**Windows API availability.** The workspace pins `windows` 0.62.2:

- `GetProcessInformation` and `ProcessMachineTypeInfo` carry no feature gate of their own. They sit in the `Win32_System_Threading` module (`windows-0.62.2/src/Windows/Win32/System/Threading/mod.rs:871`, `:3034`).
- `IsWow64Process2` (`:1130-1132`), `PROCESS_MACHINE_INFORMATION` (`:2771-2773`) and the `IMAGE_FILE_MACHINE_*` constants (`System/SystemInformation/mod.rs:560-591`) need `Win32_System_SystemInformation`. 0.62.2 has no `IMAGE_FILE_MACHINE_ARM64EC`.
- The UIA crate enables that feature (`crates/provider-windows-uia/Cargo.toml:30`). JAB does not: `crates/provider-java-jab/Cargo.toml:31-36` enables Foundation, Security, Threading and UI_WindowsAndMessaging, the middle two for the elevation check in `pump.rs:236-237`. `crates/process` enables only Foundation and Threading (`Cargo.toml:24`).
- Every `windows` function is a static import (`windows-link` 0.2.1, `kind = "raw-dylib"`).

All of this is **verified** in the crate sources. The Windows versions below are **verified** against Microsoft's documentation, except where marked. What the calls return is documented or measured by others, not measured here:

- `GetProcessInformation(ProcessMachineTypeInfo)` needs build 22000 or later, which means Windows 11 and Windows Server 2025; Server 2022 is build 20348 and does not have it. It reports the process's own machine, including an x64 process emulated on ARM64 (`AMD64`, measured by Microsoft on 22000).
- `IsWow64Process2` needs Windows 10 version 1511 (Windows Server 2016) or later (**assumed**; to be re-checked against Microsoft's reference page). Because the import is static, that is also the lowest Windows on which a binary that links the reader loads. It reports a guest machine only for a WOW64 process, and `UNKNOWN` otherwise.
- For an ARM64EC process, `ProcessMachineTypeInfo` reports `AMD64` as well, the same answer as for an emulated x64 process. That comes from a developer's comparison of ARM64EC Office with x64 PowerPoint on Microsoft Q&A, and matches Microsoft's grouping of "x64/Arm64EC" as one process class. No documented API tells the two apart.

## Goals / Non-Goals

**Goals:**

- Each platform has one implementation of "read the process table for this process". Every provider on that platform calls it, so two providers cannot disagree about the same process (spec: *Each platform reports the process attributes it has a source for*).
- A node reads the process it was created for, through the identity it recorded, and nothing else.
- A macOS or any later provider implements the reader for its platform once and gets the contract for free.
- Every provider reports presence the same way: decided at enumeration, and a lookup by name reads only what it names.
- Users read the same attribute, in the same place and form, from every provider, and find it documented.

**Non-Goals:**

- The empty-string placeholders of `control:Name` on UIA, JAB and the Java agent. Where the name comes from the process name, it follows D9; an application without one keeps today's behaviour.
- The PID-namespace rules stay in `sidecar-deployment`: which process ID an AT-SPI application reports, and when the process table may be read at all. The reader assumes it is given a process ID valid in the runtime's namespace, and every caller stays responsible for that.
- The Java agent's wire protocol and JAR do not change. See D4.
- SWT and JavaFX get nothing of their own: Java support is Swing-only for now (maintainer), and the change is toolkit-neutral anyway.
- macOS gets no implementation. Its provider builds no application nodes today.

## Decisions

### D1: One process reader per platform, in `crates/process`, bound to the identity

The library crate `platynui-process` reads the process attributes of a recorded `ProcessIdentity` through two entry points:

- `ProcessIdentity::read(&self, ProcessAttribute) -> Option<String>` reads one attribute;
- `ProcessIdentity::read_all(&self) -> ProcessAttributes` reads all six, for a listing.

Each opens the process once, confirms that it is still the recorded process, reads in the specification's format, and closes. No handle leaves the call (`HANDLE` is not `Send`). The mapping to `app:` names stays in the providers, because the crate depends on no PlatynUI crate. A caller that holds only a number, like AT-SPI, captures an identity first (`ProcessIdentity::capture`, `lib.rs:63-71`).

- **Windows**: native Win32, next to the identity, through its `sys::Process::open` (`:124-137`): `PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE`, then the limited right alone. Every read works with the limited right: `QueryFullProcessImageNameW`, `NtQueryInformationProcess(ProcessCommandLineInformation)`, `OpenProcessToken(TOKEN_QUERY)`, `GetProcessInformation(ProcessMachineTypeInfo)` and `IsWow64Process2`. This is **assumed** from the API documentation, and for the undocumented command-line class from others' reports; task 2.2's own-process test proves it. The reads start from `crates/provider-windows-uia/src/map.rs:289-479`. Its first open with `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ` (`:292`) and its PE-header path (`:495-525`) are not carried over. `sysinfo`'s Windows backend is not used.
- **Linux**: `sysinfo`, as AT-SPI uses it today, plus `getpwuid_r` (decision M1):
  - one targeted refresh per read, of only this pid and only what the read needs (`ProcessesToUpdate::Some(&[pid])` with a `ProcessRefreshKind` of exe, cmd and user), instead of a fresh `System` per attribute as today (`crates/provider-atspi/src/process.rs:11-25`);
  - `exe()` for the executable path, which `sysinfo` already strips of the ` (deleted)` suffix;
  - `cmd()` for the command line;
  - `effective_user_id()` for the account, resolved to a name through `getpwuid_r`, because `sysinfo` has only the UID and its user list can miss accounts from LDAP, SSSD or NIS (the reason recorded at `process.rs:63-68`, `:85-88`);
  - `start_time()` for the start, in seconds since the epoch, which `sysinfo` computes from the boot time and field 22 of `stat`.

  AT-SPI's `process.rs` goes away; its `with_process` and `resolve_username` (`crates/provider-atspi/src/process.rs:11-25`, `:85-122`) move here. `sysinfo` then serves only this reader, since JAB's `process.rs` goes too; `chrono` leaves both providers but stays in `crates/xpath` (`Cargo.toml:20`).
- **Every other target**: every function answers nothing.

It depends on no other PlatynUI crate. UIA, JAB and the Java provider already depend on it under `cfg(windows)` (`crates/provider-windows-uia/Cargo.toml:19`, `crates/provider-java-jab/Cargo.toml:22`, `crates/provider-java/Cargo.toml:23`). AT-SPI gains it as a regular dependency.

`snapshot-validity` created the crate (`8f6fc02`) with `ProcessIdentity` and `Liveness` (`:26-84`). This change adds the reader next to them, in a module of its own, for example `src/attributes.rs` with Windows and Linux submodules.

- It reuses `sys::Process` (`:107-171`) with its `open`, `is_running`, `creation_time` and `handle`, and `sys::parse_stat`/`read_stat` (`:252-269`), all made `pub(crate)`, instead of opening or parsing a process a second way. The crate root keeps its exports.
- It follows the workspace rules (Context): `[lints] workspace = true`.
- Linux-only dependencies go under `cfg(target_os = "linux")`, not the existing `cfg(unix)` section (`Cargo.toml:20-21`), which the macOS cross-check also builds.
- Modules that call Win32 or `getpwuid_r` carry `#[allow(unsafe_code, reason = "...")]`, as `mod sys` does.

`StartTime`:

- On Windows it is the start the identity recorded, the `GetProcessTimes` creation time (`lib.rs:160-170`, stored at `:183`), in 100 ns units since 1601: Unix seconds = `start / 10_000_000 − 11_644_473_600`. The attribute and the identity come from one reading.
- On Linux it is `sysinfo`'s `start_time()`, which rests on the same field 22 the identity recorded. The identity check around the read (D5) makes sure it is the same process.
- One platform-independent helper formats Unix seconds as `YYYY-MM-DDTHH:MM:SSZ`, so neither `FileTimeToSystemTime` (`Win32_System_Time`) nor `chrono` is needed in the crate.

*Why a crate:* process facts are properties of the process, not of the toolkit or the accessibility API, and the same process can reach PlatynUI through two providers on one platform (JAB and the in-JVM agent on Windows). One implementation makes the spec's "same value from every provider" hold by construction instead of by review. Binding it to the identity makes "a node reports only the process it was created for" hold by construction too.

*Alternatives considered:*

- **Fix each provider in place.** Rejected: four implementations, of which three would converge on the same Windows code, and the agreement between providers stays a matter of discipline. macOS would add a fifth.
- **A `ProcessInfo` device registered by the platform crates**, like the window manager. Rejected: the providers do not reach platform devices today, and this answer needs no per-runtime state or connection.
- **Put it into `platynui-core`.** Rejected: core is platform-neutral and has no OS dependencies (`crates/core/Cargo.toml`, **verified**); this reader is nothing but OS dependencies.
- **`sysinfo` on every platform.** Rejected for Windows:
  - it only offers arguments already split by `CommandLineToArgvW`, which cannot give the verbatim command line;
  - it has no domain-qualified user name and no architecture;
  - it answers an empty executable path under limited rights.
- **Reading `/proc/<pid>` directly on Linux** (decision M1). Rejected: `sysinfo` already translates `exe`, `cmdline`, the UIDs and the start time, including the ` (deleted)` suffix, and it is proven in AT-SPI. Its cost today comes from refreshing a whole `System` per attribute, which one targeted refresh per read removes. Reading `/proc` directly would re-implement that translation to save one dependency.
- **A free function `read(pid, …)`.** Rejected: it could read the process that received a reused pid.

### D2: How each attribute is read, and what makes it absent

These are per-platform sources in the reader. Each row produces the format the spec fixes.

| Attribute | Windows | Linux | Absent when |
|---|---|---|---|
| `ProcessName` | the file name of `ExecutablePath`, with a trailing `.exe` removed (case-insensitively) | the file name of `ExecutablePath`, unchanged | the executable path is absent. There is no fallback to `comm` or to the image name, which is truncated or can be changed by the process |
| `ExecutablePath` | `QueryFullProcessImageNameW` (Win32 form), with one 32 768-unit buffer. The UIA loop's growth never runs (`map.rs:327`) and is not carried over | `sysinfo`'s `exe()`, the target of `/proc/<pid>/exe` without the ` (deleted)` the kernel appends once the file has been replaced or removed | the process cannot be opened or the link read |
| `CommandLine` | `NtQueryInformationProcess(ProcessCommandLineInformation)`, verbatim; limited rights **assumed** (the class is undocumented) | `sysinfo`'s `cmd()`, arguments joined by single spaces | unreadable, or empty (a kernel thread has none) |
| `UserName` | the token user through `LookupAccountSidW`, as `DOMAIN\user`. For a local account the returned domain is the computer name (**assumed** from the API documentation) | `sysinfo`'s `effective_user_id()` through `getpwuid_r`, with no fallback to the real UID (today `process.rs:73`) | no token or UID, or the account has no name |
| `StartTime` | the creation time the identity recorded (`GetProcessTimes`), in UTC, truncated to the second | `sysinfo`'s `start_time()`, in UTC, to the second | no start was recorded, or `0` |
| `Architecture` | D3 | not supplied (spec) | D3 |

A process whose executable was replaced since it started — typically by a package update while the application runs — keeps the path it was started from, and its process name stays correct. On Linux, `/proc/<pid>/exe` then reads `<path> (deleted)`. `sysinfo` removes that suffix (`sysinfo` 0.39.3, `src/unix/linux/process.rs:503-515`, **verified**; the suffix was **measured** with a copied binary removed while it ran). The reader keeps that behaviour, so neither the path nor the name derived from it ever carries the suffix.

AT-SPI's process name changes with this (Context). `python3.12` stays `python3.12`, and a process whose executable cannot be read no longer reports a truncated `comm` name.

*Alternative considered:* **keep `comm` as a fallback for the process name.** Rejected: `comm` is truncated to 15 characters and can be set by the process itself, so it is a plausible wrong answer. That is exactly what the spec forbids.

### D3: The Windows architecture comes from documented OS calls

The reader asks `GetProcessInformation(ProcessMachineTypeInfo)` first. Where that call fails — before build 22000 (Windows 10, Windows Server 2022), `GetProcessInformation` exists but does not know `ProcessMachineTypeInfo` — it asks `IsWow64Process2`. Both calls use the one handle the read opened (D5). `IsWow64Process2` returns the guest machine for a WOW64 process, and the native machine for any other process. The machine then maps into the spec's vocabulary:

- `I386` → `x86`
- `AMD64` → `x64`
- `ARM` or `ARMNT` → `arm`
- `ARM64` → `arm64`

Any other machine, or two failed calls, leaves the attribute absent.

This answers the question that matters on Windows on ARM: is the application an ARM application, or an x86 or x64 one running under emulation? An ARM64EC application reports `x64`, because Windows itself classifies it as an x64-compatible process (Context). The maintainer decided that this distinction is not worth any source beyond the documented calls.

*Why:* the OS knows how it runs the process. The PE header only says what the file claims. Reading it through the path also opens whatever file carries that name now, which is the same class of wrong answer `atspi-process-identity` removed on Linux.

The `IsWow64Process2` fallback is exact where it applies:

- on x64 hosts, including Windows Server 2022;
- on released Windows 10 on ARM, which emulates only x86.

There, "not WOW64" means "native". The one exception is Windows 10 ARM64 Insider builds with x64 emulation: they lack `ProcessMachineTypeInfo`, so the fallback reports an emulated x64 process as `arm64`. That is accepted for a pre-release build.

*Alternatives considered:*

- **Keep the PE header and fix the mapping.** Rejected: it can still be a different file than the one the process runs, and it cannot see emulation.
- **`ProcessMachineTypeInfo` only.** Rejected by the maintainer: Windows 10 and Server 2022 would lose the architecture, although a documented call answers it there.
- **`IsWow64Process2` only.** Rejected: on Windows 11 on ARM it reports an x64 process under emulation as the native `arm64`.
- **Tell ARM64EC apart as `arm64`.** Rejected: no documented API does. Task Manager and System Informer read the executable image for it, and an undocumented NT query (`ProcessImageInformation`) may answer differently, but neither is a source this contract accepts.

### D4: The Java agent's application node reads the platform, and the agent stays unchanged

The Java provider's application node takes its process attributes from the reader (D1). The process ID is the session's (`crates/provider-java/src/agent/session.rs:72`), which the agent published as its own PID (`AgentPaths.currentPid()`, `java/agent/src/main/java/platynui/agent/AgentPaths.java:166`). It is accepted only for a live process (`crates/java-agent/src/handshake.rs:99-102`) and is valid in the runtime's namespace on Windows. The reader is asked through the node's recorded identity (`app.rs:65`, `:80`).

The agent keeps sending its facts. The provider still uses the application name for `control:Name` and the JVM facts for the `native:` attributes, and ignores the rest. The provider's `ProcessFacts` keeps only `name`, `vm_name` and `java_version` (`app.rs:24-43`), and `push_optional` (`:191-195`) goes; serde ignores the fields the agent keeps sending, because the struct does not deny unknown fields. The module documentation of `app.rs` is rewritten, because it gives the opposite reason (Context). The Javadoc of `ProcessFacts.java` keeps that reason until the agent moves for another change.

*Why unchanged:* the agent cannot be unloaded from a JVM, and the provider and the agent are compared for an exact version at connect time (AGENTS.md, Java routing). Changing what the agent sends would move the agent, the provider and the delivery package together for no gain. The provider ignoring fields it no longer needs is backward compatible in both directions.

*Alternative considered:* **remove the process fields from `ProcessFacts.java` now.** Deferred: it is harmless clean-up that can ride along with the next change that moves the agent version anyway, such as `java-agent-tree-items`.

### D5: Presence at enumeration, reads by name, through the identity

Every provider lists a process attribute only when the reader returned a value, and overrides the named lookup so that `@app:X` reads only `X`. This is the pattern `atspi-process-identity` introduced for AT-SPI (`AtspiNode::attribute`), for the same reason: a listing that decides presence by reading must not pay for six reads when one name is asked for.

UIA today opens the process once per `value()`, and since `snapshot-validity` once more when it creates an application node (`node.rs:1794`). After the change:

- a listing opens it once: the attribute iterator calls `read_all` when it reaches the first process attribute, and keeps the values;
- a lookup by name opens it once and reads one attribute.

On JAB, every `value()` today builds a fresh `sysinfo::System` and refreshes the process (`crates/provider-java-jab/src/process.rs:11-24`). The reader opens the process once per read with the limited right, so JAB's listings should not get more expensive (not measured). Listings happen in the Inspector's attribute view and in `platynui-cli query` and `snapshot` (`query.rs:104-115`, `snapshot.rs:232`, `:403`).

A node reads through the `ProcessIdentity` it recorded at creation (UIA `node.rs:1771`, JAB `node.rs:1329`, agent `app.rs:65`). AT-SPI records one when it builds a node's process attributes at enumeration (`crates/provider-atspi/src/node.rs:1836-1850`), from the local process number, and reads through it later (`:1897-1905`):

- On Windows the read opens the pid, confirms on that handle that the process runs and that its creation time equals the recorded start, and reads through the same handle. Windows does not reuse a pid while a handle to its process is open (**assumed**).
- On Linux it compares field 22 before and after the read.
- These nodes list no `app:` process attribute:
  - a node without an identity: no process had the pid at creation, or the pid is `0` (`crates/process/src/lib.rs:64-66`);
  - a node whose process has ended or been replaced;
  - a node whose start could not be recorded (decision M2).
- `control:ProcessId` stays for every positive pid (decision M3).

So a node held across a pid reuse never reports the attributes of the process that received its pid.

### D6: Process ID `0` is rejected at every entry and every exit

- The reader answers nothing for the process ID `0`, since `capture(0)` records no identity.
- **UIA's hit-test** maps its pid through one pure function, `Option<i32>` → `UiaIdScope`, which answers the desktop scope for `None`, `0` and negative pids (`provider.rs:534-537`). It builds no orphan application node for them (`node.rs:221`). The per-application window filter (`provider.rs:252-258`) goes through a pure predicate that rejects a target pid ≤ 0 before it compares. The enumeration cannot reach that guard today (Context).
- **JAB** reads a window's process only through `process_id_of` (`node.rs:936-947`, made `pub(crate)`), which answers "no process" when `GetWindowThreadProcessId` yields `0`. For a top-level window that means the window no longer exists (**assumed** from Win32 semantics).
  - In `enumerate_visible_top_level_windows` (`provider.rs:617-645`, call at `:631`), `WindowCandidate`'s pid (`:610-614`) becomes `Option<u32>`.
  - Both passes, `discover_java_windows` (`:559-608`) and `awt_windows_without_bridge` (`:484-510`), skip a candidate without a process next to their own-process check (`:568`, `:490`). Such a window gets no `JabAppNode` (`:349-357`), is no candidate for automatic agent attachment, and is not listed in the missing-DLL warning.
  - `top_level_window_at` (`:515-533`, call at `:530`) answers `None` for such a window. `element_at_point` then abstains as for "no window at point" (`:403-405`) and never builds `JabAppNode::orphan` (`node.rs:775`) without a process. Every node the hit-test returns stays app-scoped, as `jab-hit-test` (*Reveal-ready hit result*, `openspec/specs/jab-hit-test/spec.md:19`) requires, so that capability is unchanged.
- **The Java agent's** application node needs no guard: its pid comes from a handshake file that is accepted only for a live process. **Automatic attachment** drops `0` before its handshake lookup and attempt count (`crates/provider-java/src/agent/backend.rs:257-297`).
- **The three window managers'** `pid_from_attr` accept only a positive number, so `0` counts as no process ID and `extract_pid` goes on to the ancestors (Windows `:75-85`, X11 `:377-392`, Wayland `:415-429`).
  - Windows and X11 then resolve no window (`platform-windows/src/window_manager.rs:170-177`, `platform-linux-x11/src/window_manager.rs:682-683`).
  - The compositor backend matches by title and size, as for any node without a process ID (`platynui_ipc.rs:339-375`).

  No window is found through the process ID `0`.

### D7: The mock models process attributes on one application

The mock's "Mock Application" gains `control:ProcessId` and a subset of the `app:` attributes in the specification's formats. Its process ID is 4242, the `native:ProcessId` its window already carries (`crates/provider-mock/assets/mock_tree.xml:7`). The subset deliberately leaves out `CommandLine`, so the scenario *Listing and predicate agree for a missing attribute* has a fixture in the mock tree, tested through the runtime's XPath evaluation. It is also the fixture for the Python `Application` (D10).

"Mock Settings" stays without process attributes, as an application whose process is unknown.

### D8: Documentation follows the contract, and the spec stays the source

- **`dev-docs/architecture.md`**:
  - the Application row of the pattern catalog: `ProcessId` becomes optional, and all six `app:` attributes are listed;
  - the per-platform source table gets the D2 and D3 sources;
  - the Windows UIA checklist names the attributes and their presence rule;
  - the §2 trees: the `crates/process` entry also names the process reader, and the apps tree gains `win32-test-window`.
- **`dev-docs/platform-windows.md`** and **`dev-docs/platform-linux.md`**: the application-node attributes and their sources.
- **`dev-docs/planning.md`**: the parity item is resolved, and the source notes that name `comm` or the executable stem go.
- **`dev-docs/python-library-design.md`**: the `Application` sketch.
- **`dev-docs/testing-strategy.md`** §5: `apps/win32-test-window` is a helper for process-level tests, not a fixture.
- **`dev-docs/platform-linux-wayland.md`**: the link to AT-SPI's process module points to `crates/process`.
- **`CONTRIBUTING.md`**: the new Windows-lane prerequisite `i686-pc-windows-msvc`.
- **`AGENTS.md`**: the `crates/process` entry that `snapshot-validity` added also names the process reader, and the apps list gains `win32-test-window`.
- **The core attribute constants**: a one-line pointer to the capability.
- **User documentation**: a short section in BareMetal's library documentation on the `app:` attributes — what each is, that each may be absent, how to test for one with `[@app:X]`, and their formats. The documentation of `Get Attribute` names the `app:` prefix next to `native:`.

The docs point to the spec for formats instead of restating them.

### D9: The display name follows the process name

UIA's and JAB's application nodes are named after their program. UIA also reports that name as `control:Id` (Context). Both now take the reader's `ProcessName`. For a `.exe` program the value stays what it is today. For an image with another extension it keeps that extension, because the reader strips only `.exe`. When the process name is absent, the name keeps today's behaviour (Non-Goals).

The spec states it: where a provider names an application node after its program, the name is the process name. `control:Name` and `control:Id` are not process attributes, so they do not count as a second name for the same fact.

*Alternative considered:* **keep `file_stem` for the name.** Rejected: two derivations of the same name would sit side by side, and a user's `[@Name="..."]` could disagree with `[@app:ProcessName="..."]` for the same application.

### D10: The Python `Application` reads the contract

`Application.process_id` reads `control:ProcessId`, and `Application.process_name` reads `app:ProcessName`. Each answers `None` when the attribute is absent — whether the stub answers `None` or the real adapter raises `KeyError` — and raises `TypeError` only for a present value of the wrong type. Their return types become `int | None` and `str | None`. `tests/PlatynUI/test_application.py` changes accordingly, and a pytest against the mock (D7) reads both from "Mock Application".

### D11: The 32-bit scenario runs against a window the repository builds

The scenario *A 32-bit process on 64-bit Windows reports its own architecture* needs a 32-bit process with a window. The repository has none. Both JVMs the Swing fixture is provisioned with are 64-bit (`apps/test-app-swing/build/java-launchers.properties` names two JDKs under `C:/Program Files`), and the only Windows Rust target installed is `x86_64-pc-windows-msvc` (**verified** on the maintainer's machine, 2026-09-27). Applications that ship with Windows are excluded from tests and measurements, because they change with Windows versions and updates (maintainer, 2026-09-27).

So the repository gains `apps/win32-test-window`, a helper for process-level tests:

- It shows one visible top-level window of the predefined `STATIC` class, off screen, without `WS_EX_NOACTIVATE`, without activating it, and it pumps its messages, like the UIA test window `test_window_child` (`crates/provider-windows-uia/src/node.rs:2166-2242`). The UIA root enumeration lists only a visible, uncloaked window with an area and without `WS_EX_NOACTIVATE` (`provider.rs:115-167`) that answers a 300 ms `WM_GETOBJECT` probe (`:38`, `:66-111`).
- Its title and a lifetime it imposes on itself come from the command line.
- Its bitness follows the build target, and the Windows lane builds it for `i686-pc-windows-msvc`.
- It is not a fixture of the blueprint (`dev-docs/testing-strategy.md` §5), because it has no control catalog, only a process and a window. Its suite therefore sits in a directory of its own, `tests/acceptance/win32`.

*Alternatives considered:*

- **`%WINDIR%\SysWOW64\charmap.exe`, the earlier plan.** Rejected: it is an application that ships with Windows.
- **A 32-bit JVM for the Swing fixture.** This is the most realistic case for JAB. But the fixture's JDKs come from Gradle toolchains, which provision for the build machine's architecture, so a 32-bit JDK would need provisioning of its own. That is out of proportion for one assertion.
- **Unit tests only.** The machine mapping and the fallback rule are unit-tested anyway (task 2.2). Only a real WOW64 process, though, shows that the provider asks the OS for the process's machine and not the host's.

### Decisions M1-M5 (maintainer defaults, 2026-09-28)

- **M1: the Linux reader uses `sysinfo`**, with one targeted refresh per read, and `getpwuid_r` for the user name (D1). The maintainer chose it over reading `/proc` directly, since `sysinfo` already translates what the reader needs.
- **M2: an identity recorded without a start time reads nothing.** `start` is `None` only when capture could not read it; a later read with the same rights normally fails too, so this costs nothing in practice. It is the only rule under which "never reports the process that received its pid" holds. A later macOS reader would first need a start time in the identity.
- **M3: `control:ProcessId` stays for every positive pid**, also on a node whose process has ended or that has no identity. It is the pid the platform reported, part of the node's runtime id (`uia://app/<pid>`, `node.rs:1902-1904`) and the selector key. Only the six `app:` attributes depend on the identity.
- **M4: PID 4, the System process, may serve as the partly unreadable process in tests.** It is the kernel's process, not an application that ships with Windows, and no process a test starts can be made unreadable without UAC.
- **M5: `IsWow64Process2` stays a static import** (Risks).

## Risks / Trade-offs

- **[Shapes change that users see]** Users read this metadata in selectors, with `Get Attribute`, through `PlatynUI.ui.Application`, in the CLI's output and in the Inspector, and the change alters what they see (proposal, *Behavior changes that users see*). → That is its purpose: today the values are wrong or differ by provider. PlatynUI is at 0.x, so the changes are not marked as breaking, and the release notes list each of them. Inside the repository, only `PlatynUI.ui.Application` and `@ProcessId` selectors read the metadata. The first is fixed here (D10), and the second keeps its form.
- **[Windows behaviour is verified only on a Windows machine]** CI has no Windows test job; on Windows it only lints (`rust-windows`, `just clippy`, `.github/workflows/ci.yml:140-179`) and builds wheels (**verified**). → The reader's Windows functions carry unit tests on their own process, which run with `just test` on Windows. The Windows acceptance lane (`just test-acceptance-windows`) asserts the formats end to end.
- **[The ARM64EC answer rests on reports, not documentation]** Context marks it. → If Windows ever reports ARM64EC differently, it can only report a machine the table maps or leaves absent (D3).
- **[The Windows lane needs a 32-bit Rust target]** The 32-bit test window builds only with `i686-pc-windows-msvc` and the MSVC x86 libraries installed (D11). → `CONTRIBUTING.md` lists both. A missing target fails `just build-win32-test-window-x86` with rustc's message, which names the target. A missing binary fails the suite with a message naming the recipe; it never skips.
- **[`IsWow64Process2` is a static import]** A binary that links the reader does not load on a Windows older than 10 version 1511 / Server 2016 (Context). → Accepted (M5). If it is not, resolve it through `GetProcAddress`.
- **[Losing the process name for unreadable executables on Linux]** Without the `comm` fallback, a process whose `/proc/<pid>/exe` cannot be read has no name. → That is the contract: absent rather than possibly truncated. On a desktop, the user's own applications are always readable.
- **[Java agent on Linux]** When the Java provider runs on Linux, the session's process ID may come from a container. → Using the reader there needs the same guard AT-SPI has: only a process ID valid in the runtime's namespace. `java-provider-linux` makes the agent compile on Linux, but its artifacts do not mention this guard yet. Whichever change makes the agent's application node read the process table on Linux adds it. D4 holds on Windows, where there is one namespace.
- **[More process opens for full listings on UIA]** → A listing opens the process once and reads all six attributes (D5); a lookup by name opens it once.
- **[Overlap with neighbouring changes]** `gate-uia-window-patterns`, `jab-discovery-containment`, `fix-jab-hit-test-virtual-children` and `xpath-document-order` edit the same nodes and functions (proposal, Coordination). → Small, local conflicts. This change lands first among the Java changes; whichever lands second rebases. `snapshot-validity` has landed, and `xdm-snapshot-release` is archived.

## Migration Plan

- **Behavioural, not additive.** It changes shapes, as the proposal's list of behavior changes says, and extends one crate. It is not marked as breaking, because PlatynUI is at 0.x.
- **Needs a native rebuild.** The Java agent JAR does not change (D4), so there is no agent version move and no `just install-provider-java`.
- **Order:**
  1. The acceptance suites and the 32-bit test window, written first: red on Windows (the win32 suite is a green guard), a regression guard on Linux.
  2. The reader with its unit tests, in `crates/process`, next to the process identity.
  3. The Java provider's agent node, UIA and JAB, with D9's names and their `0` guards. The agent comes first, since it is the main Java path.
  4. AT-SPI onto the reader, which is behaviour-preserving apart from D2's process name.
  5. The window-manager `0` guards.
  6. The mock, then the Python `Application`.
  7. The docs.
- **Rollback:** reverting the change's commits restores the previous behaviour. There is no persisted state, configuration or data format to migrate back.
