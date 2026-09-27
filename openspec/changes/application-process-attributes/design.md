# Design

## Context

The motivation is in proposal.md (Why), and the contract is in `specs/application-process-attributes/spec.md`. This section records only the current code that shapes the approach. **Verified** marks what was read in the tree at `37b6322` (2026-09-27), after the workspace lint adoption and `logging-concept`; the rest is marked as assumed.

**Four implementations of one question.** Every provider that builds application nodes reads the process table itself.

- **Windows UIA** uses native Win32 calls in `crates/provider-windows-uia/src/map.rs`:
  - `open_process_query` :271-281, which falls back to limited query rights (:279);
  - `query_executable_path` :283-322;
  - the command line through `NtQueryInformationProcess` :326-381, verbatim;
  - `DOMAIN\user` through `LookupAccountSidW` :384-438;
  - the start time with milliseconds :440-461 (`'.{:03}Z'` :456);
  - the architecture from the PE header of the executable path :477-507, whose mapping turns ARM into `arm64` and an unknown machine into `"unknown"` (:500-505); otherwise `GetNativeSystemInfo` :463-475, which ignores the process handle and reports the host.

  All seven attribute objects are listed unconditionally (`node.rs:1509-1553`, indices 4-10 at :1536-1542). Each `value()` answers `""`, a null or `"unknown"` when it cannot read (`node.rs:1387`, `:1403`, `:1425`, `:1447`, `:1469`, `:1505`). **Verified.**

  The display name comes from the same helpers:
  - `AppDisplayNameAttr` (`node.rs:1306-1334`) and `ApplicationNode::name` (`:1873-1890`) use `open_process_query`, `query_executable_path` and `file_stem`;
  - `id()` returns the name (`:1894-1897`), which `AppAttrsIter` emits as `control:Id` (`:1528-1535`).

  **Verified.**
- **JAB** uses `sysinfo` in `crates/provider-java-jab/src/process.rs`:
  - the process name from the executable's stem :27. Under limited rights `sysinfo` answers an empty executable path, and the name then falls back to the Toolhelp image name with `.exe`, for example `javaw.exe` (:34-35);
  - the command line as `sysinfo` arguments joined by spaces :45;
  - the bare user name :57;
  - the start time to the second :64;
  - the architecture from the PE header, falling back to the compile-time `std::env::consts::ARCH` :79-85. Its parser answers `"unknown"` for a machine it does not know (:109).

  A value it cannot read becomes a null in the attribute layer. `JabAppNode`'s display name reads the same process name (`node.rs:1388-1390`), eagerly at `:1430`. **Verified.** That `sysinfo` 0.39.3 answers an empty executable path under limited rights is read from its source, not re-measured.
- **The Java agent** does not read the process table at all. It reports what the JVM says about itself (`agent/process`, `crates/provider-java/src/agent/app.rs:48`), in the `control` namespace (`push_optional` :176), with the start time as epoch milliseconds (:149). Its process name is the main class's simple name or the jar's file name, and its executable path is always `<java.home>/bin/java(.exe)`, also for a JVM started through `javaw.exe` (`java/agent/src/main/java/platynui/agent/ProcessFacts.java:52-64`, `:73`). The module documentation of `app.rs` (:8-12) and the Javadoc of `ProcessFacts.java` (:10-18) give the reason: the JVM's self-description is cheaper and better than a host-side process query. **Verified.**
- **AT-SPI** uses `sysinfo` and `getpwuid_r` in `crates/provider-atspi/src/process.rs`. After `atspi-process-identity` it decides presence at enumeration and substitutes nothing. It still derives the process name from the executable's *stem* (:31-41), which turns `python3.12` into `python3`. It falls back to `sysinfo`'s `name()`, the kernel's `comm`, which is truncated to 15 characters. `sysinfo` and `chrono` are regular dependencies of the crate; `libc` is gated to Linux. **Verified.**

**What users and consumers read.**

- **Users** reach the metadata through:
  - XPath in any keyword that takes a selector; a bare `@X` is the `control` namespace (`crates/runtime/src/xpath.rs:1034-1040`), so the metadata is `@app:ProcessName`;
  - `Get Attribute    <application>    app:ProcessName`, which splits the prefix (`src/PlatynUI/BareMetal/__init__.py:2406-2410`) but whose documentation names only `native:` (`:2391-2399`);
  - `UiNode.attribute(name, namespace)`, which raises `AttributeNotFoundError` for an absent attribute (`packages/native/src/runtime.rs:101-116`);
  - `PlatynUI.ui.Application`;
  - `platynui-cli query` and `snapshot`, which print every attribute and render a null as `null` (`crates/cli/src/commands/query.rs:104-145`, `snapshot.rs:283-299`);
  - the Inspector's attribute view, which renders a null as `<null>` and copies a name in XPath form (`apps/inspector/src/model/tree_data.rs:403-448`).

  A null attribute exists today as an attribute node with an empty value, so `[@app:X]` is true for it (`crates/runtime/src/xpath.rs:962-964`). The user documentation says nothing about these attributes. BareMetal mentions "an application's process details" (`:545`) and documents pinning by `@ProcessId` (`:564`, `:614-615`). **Verified.**
- **`PlatynUI.ui.Application`** reads `app:ProcessId` (`src/PlatynUI/ui/application.py:31-39`), which no provider reports, and `app:ProcessName` (`:42-49`), which the Java agent reports under `control:`. Both raise `TypeError` when the value is not there. **Verified.**
- **The three window managers** read `control:ProcessId` and accept an integer `0`, because `u32::try_from(v).ok()` gives `Some(0)`:
  - `crates/platform-windows/src/window_manager.rs:88-99`, whose module documentation (`:13`) wrongly says it reads `native:ProcessId`;
  - `crates/platform-linux-x11/src/window_manager.rs:411-423`;
  - `crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs:450`.

  A lookup with `0` would match any window whose process the platform does not fill in (`platform-windows` `find_hwnd_for_pid`, `:125-127`; X11 `get_window_pid`, `:125-128`). `sidecar-deployment` already requires the Linux window manager and the compositor backend to resolve nothing for `0` (`openspec/specs/sidecar-deployment/spec.md:259`). **Verified.**
- **Process ID `0` elsewhere:**
  - The UIA point hit-test takes `get_process_id(&elem).ok()` with only a check against its own process (`provider.rs:528-529`) and builds an app-scoped ancestor chain from it (`:534-548`, `ApplicationNode::orphan` at `node.rs:218`).
  - UIA's per-application window filter compares a target process ID of `0` with the `0` that `GetWindowThreadProcessId` leaves for an unknown window (`provider.rs:252-258`).
  - JAB reads a window's process in `top_level_window_at` (`provider.rs:530`) and in the enumeration (`:631`). Since `ab496bc`, the pass without the Access Bridge DLL (`awt_windows_without_bridge`, `:484-510`) turns those process IDs into candidates for automatic agent attachment and into the process list of the missing-DLL warning (`crates/provider-java/src/provider.rs:260-261`, `:335-354`).
  - JAB's hit-test builds `JabAppNode::orphan(pid)` with a non-optional process ID (`node.rs:759-773`).
  - A guarded helper `process_id_of` already exists (`node.rs:931-942`).

  **Verified.**

**Workspace rules (verified).** The workspace lints (`Cargo.toml:31-46`) deny `unsafe_code`, warn on `unused_crate_dependencies` (an error under `clippy -D warnings`), and enable pedantic clippy. Every crate sets `[lints] workspace = true`. FFI modules carry a scoped `#![allow(unsafe_code)]` with a reason (for example `crates/platform-windows/src/window_manager.rs:17-18`). Platform dependencies are target-gated, following `crates/provider-windows-uia/Cargo.toml:14-16`. `crates/provider-java` puts all its dependencies under `cfg(windows)`.

**Windows API availability.** The workspace pins `windows` 0.62.2:

- `GetProcessInformation` and `ProcessMachineTypeInfo` carry no feature gate of their own. They sit in the `Win32_System_Threading` module.
- `IsWow64Process2`, `PROCESS_MACHINE_INFORMATION` and the `IMAGE_FILE_MACHINE_*` constants need `Win32_System_SystemInformation`. The UIA crate enables it (`Cargo.toml:29`); JAB does not (`Cargo.toml:33`, Threading only).

All of this is **verified** in the crate sources. The Windows versions below are **verified** against Microsoft's documentation. What the calls return is documented or measured by others, not measured here:

- `GetProcessInformation(ProcessMachineTypeInfo)` needs build 22000 or later, which means Windows 11 and Windows Server 2025; Server 2022 is build 20348 and does not have it. It reports the process's own machine, including an x64 process emulated on ARM64 (`AMD64`, measured by Microsoft on 22000).
- `IsWow64Process2` needs Windows 10 1709 or later. It reports a guest machine only for a WOW64 process, and `UNKNOWN` otherwise.
- For an ARM64EC process, `ProcessMachineTypeInfo` reports `AMD64` as well, the same answer as for an emulated x64 process. That comes from a developer's comparison of ARM64EC Office with x64 PowerPoint on Microsoft Q&A, and matches Microsoft's grouping of "x64/Arm64EC" as one process class. No documented API tells the two apart.

## Goals / Non-Goals

**Goals:**

- Each platform has one implementation of "read the process table for this process ID". Every provider on that platform calls it, so two providers cannot disagree about the same process (spec: *Each platform reports the process attributes it has a source for*).
- A macOS or any later provider implements the reader for its platform once and gets the contract for free.
- Every provider reports presence the same way: decided at enumeration, and a lookup by name reads only what it names.
- Users read the same attribute, in the same place and form, from every provider, and find it documented.

**Non-Goals:**

- The empty-string placeholders of `control:Name` on UIA, JAB and the Java agent. Where the name comes from the process name, it follows D9; an application without one keeps today's behaviour.
- The PID-namespace rules stay in `sidecar-deployment`: which process ID an AT-SPI application reports, and when the process table may be read at all. The reader assumes it is given a process ID valid in the runtime's namespace, and every caller stays responsible for that.
- The Java agent's wire protocol and JAR do not change. See D4.
- macOS gets no implementation. Its provider builds no application nodes today.

## Decisions

### D1: One process reader per platform, in `crates/process`

The library crate `platynui-process` answers the process attributes for a process ID valid in the runtime's namespace, one function per attribute. Each function returns the value in the specification's format, or nothing:

- **Windows**: native Win32. The UIA code is the starting point because it already reads everything with limited query rights; `sysinfo`'s Windows backend is not used.
- **Linux**: `/proc`, through `sysinfo`, plus `getpwuid_r`. AT-SPI's `process.rs` moves here.
- **Every other target**: every function answers nothing.

It depends on no other PlatynUI crate. The providers depend on it: UIA, JAB, the Java provider (under `cfg(windows)` until the provider runs on Linux) and AT-SPI.

`snapshot-validity` creates the same crate with a process identity: the pid and the exact start time recorded when an application node is created, and a check whether that process still runs. When it has landed, this change adds its reader next to it. Otherwise this change creates the crate, and `snapshot-validity` adds the identity. The crate follows the workspace rules (Context): `[lints] workspace = true`, dependencies gated per target, and a scoped `allow(unsafe_code)` with a reason on the modules that call Win32 or `getpwuid_r`.

On Linux the identity reads field 22 of `/proc/<pid>/stat`, in clock ticks. This reader's `StartTime` may derive from that same reading, with the boot time, truncated to the second. It may also keep `sysinfo`'s start time. Both give the same value to the second, and the choice does not change the specification.

*Why a crate:* process facts are properties of the process, not of the toolkit or the accessibility API, and the same process can reach PlatynUI through two providers on one platform (JAB and the in-JVM agent on Windows). One implementation makes the spec's "same value from every provider" hold by construction instead of by review.

*Alternatives considered:*

- **Fix each provider in place.** Rejected: four implementations, of which three would converge on the same Windows code, and the agreement between providers stays a matter of discipline. macOS would add a fifth.
- **A `ProcessInfo` device registered by the platform crates**, like the window manager. Rejected: the providers do not reach platform devices today, and this answer needs no per-runtime state or connection. It is a pure function of a process ID, and a plain library is the smallest thing that can hold it.
- **Put it into `platynui-core`.** Rejected: core is platform-neutral and has no OS dependencies (`crates/core/Cargo.toml`, **verified**); this reader is nothing but OS dependencies.
- **`sysinfo` on every platform.** Rejected for Windows:
  - it only offers arguments already split by `CommandLineToArgvW`, which cannot give the verbatim command line;
  - it has no domain-qualified user name and no architecture;
  - it answers an empty executable path under limited rights.

### D2: How each attribute is read, and what makes it absent

These are per-platform sources in the reader. Each row produces the format the spec fixes.

| Attribute | Windows | Linux | Absent when |
|---|---|---|---|
| `ProcessName` | the file name of `ExecutablePath`, with a trailing `.exe` removed (case-insensitively) | the file name of `ExecutablePath`, unchanged | the executable path is absent. There is no fallback to `comm` or to the image name, which is truncated or can be changed by the process |
| `ExecutablePath` | `QueryFullProcessImageNameW` (Win32 form) | the target of `/proc/<pid>/exe`, without the ` (deleted)` the kernel appends once the file has been replaced or removed | the process cannot be opened or the link read |
| `CommandLine` | `NtQueryInformationProcess(ProcessCommandLineInformation)`, verbatim, which works with limited rights | `/proc/<pid>/cmdline`, arguments joined by single spaces | unreadable, or empty (a kernel thread has none) |
| `UserName` | the token user through `LookupAccountSidW`, as `DOMAIN\user`. For a local account the returned domain is the computer name (**assumed** from the API documentation) | the effective UID through `getpwuid_r` | no token or UID, or the account has no name |
| `StartTime` | `GetProcessTimes` creation time, in UTC, truncated to the second | the process's start time, to the second (D1) | unreadable, or `0` |
| `Architecture` | D3 | not supplied (spec) | D3 |

A process whose executable was replaced since it started — typically by a package update while the application runs — keeps the path it was started from, and its process name stays correct. On Linux, `/proc/<pid>/exe` then reads `<path> (deleted)`. `sysinfo` already removes that suffix (`sysinfo` 0.39.3, `src/unix/linux/process.rs:503-515`, **verified**; the suffix was **measured** with a copied binary removed while it ran). The reader keeps that behaviour, so neither the path nor the name derived from it ever carries the suffix.

AT-SPI's process name changes with this (Context). `python3.12` stays `python3.12`, and a process whose executable cannot be read no longer reports a truncated `comm` name.

*Alternative considered:* **keep `comm` as a fallback for the process name.** Rejected: `comm` is truncated to 15 characters and can be set by the process itself, so it is a plausible wrong answer. That is exactly what the spec forbids.

### D3: The Windows architecture comes from documented OS calls

The reader asks `GetProcessInformation(ProcessMachineTypeInfo)` first. Where that call does not exist, before build 22000 (Windows 10, Windows Server 2022), it asks `IsWow64Process2`. For a WOW64 process that returns the guest machine, and for any other process the native machine. The machine then maps into the spec's vocabulary:

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

The Java provider's application node takes its process attributes from the reader (D1), using the session's process ID. That ID is the JVM's own view of its PID (`AgentPaths.currentPid()`), which on Windows is valid in the runtime's namespace.

The agent keeps sending its facts. The provider still uses the application name for `control:Name` and the JVM facts for the `native:` attributes, and ignores the rest. The module documentation of `app.rs` is rewritten, because it gives the opposite reason (Context). The Javadoc of `ProcessFacts.java` keeps that reason until the agent moves for another change.

*Why unchanged:* the agent cannot be unloaded from a JVM, and the provider and the agent are compared for an exact version at connect time (AGENTS.md, Java routing). Changing what the agent sends would move the agent, the provider and the delivery package together for no gain. The provider ignoring fields it no longer needs is backward compatible in both directions.

*Alternative considered:* **remove the process fields from `ProcessFacts.java` now.** Deferred: it is harmless clean-up that can ride along with the next change that moves the agent version anyway.

### D5: Presence at enumeration, reads by name

Every provider lists a process attribute only when the reader returned a value, and overrides the named lookup so that `@app:X` reads only `X`. This is the pattern `atspi-process-identity` introduced for AT-SPI (`AtspiNode::attribute`), for the same reason: a listing that decides presence by reading must not pay for six reads when one name is asked for.

UIA today opens the process once per `value()`. After the change, a full listing opens it once per attribute. That is acceptable for a listing and never happens for a lookup by name. Listings happen in the Inspector's attribute view and in `platynui-cli query` and `snapshot`, which list every attribute of every matched node (`query.rs:104-115`, `snapshot.rs:232`, `:390-400`). On JAB, the native reader replaces a `sysinfo` refresh with a system-wide Toolhelp snapshot per read, so its listings get cheaper.

A node reads by its process ID. When `snapshot-validity` has given the node a recorded process identity, a read on a node whose process has ended answers nothing. A node held across a pid reuse therefore never reports the attributes of the process that received its pid.

### D6: Process ID `0` is rejected at every entry and every exit

- The reader answers nothing for the process ID `0`.
- **UIA's hit-test** treats `0` like an unknown process and uses the desktop scope it already has for that case (`provider.rs:534-537`). It builds no orphan application node for `0`. The per-application window filter (`provider.rs:252-258`) answers nothing for a target `0` as well.
- **JAB** reads a window's process through its existing `process_id_of` (`node.rs:931-942`), which answers "no process" for `0`, at `provider.rs:530` and `:631`. A window without a process is therefore no candidate for automatic agent attachment and is not listed in the missing-DLL warning. JAB's hit-test uses the desktop scope for such a window, like UIA, instead of `JabAppNode::orphan(0)`.
- **The three window managers'** `pid_from_attr` accept only a positive number, so a node that carries `0` resolves no window. On Linux this implements what `sidecar-deployment` already requires.

### D7: The mock models process attributes on one application

The mock's "Mock Application" gains `control:ProcessId` and a subset of the `app:` attributes in the specification's formats. Its process ID is 4242, the `native:ProcessId` its window already carries (`crates/provider-mock/assets/mock_tree.xml:7`). The subset deliberately leaves out `CommandLine`, so the scenario *Listing and predicate agree for a missing attribute* has a fixture in the mock tree, tested through the runtime's XPath evaluation. It is also the fixture for the Python `Application` (D10).

"Mock Settings" stays without process attributes, as an application whose process is unknown.

### D8: Documentation follows the contract, and the spec stays the source

- **`dev-docs/architecture.md`**:
  - the Application row of the pattern catalog: `ProcessId` becomes optional, and all six `app:` attributes are listed;
  - the per-platform source table gets the D2 and D3 sources;
  - the Windows UIA checklist names the attributes and their presence rule.
- **`dev-docs/platform-windows.md`** and **`dev-docs/platform-linux.md`**: the application-node attributes and their sources.
- **`dev-docs/planning.md`**: the parity item is resolved, and the source notes that name `comm` or the executable stem go.
- **`dev-docs/python-library-design.md`**: the `Application` sketch.
- **`AGENTS.md`** and the crate tree in `dev-docs/architecture.md` §2: `crates/process`, unless `snapshot-validity` has added it.
- **The core attribute constants**: a one-line pointer to the capability.
- **User documentation**: a short section in BareMetal's library documentation on the `app:` attributes — what each is, that each may be absent, how to test for one with `[@app:X]`, and their formats. The documentation of `Get Attribute` names the `app:` prefix next to `native:`.

The docs point to the spec for formats instead of restating them.

### D9: The display name follows the process name

UIA's and JAB's application nodes are named after their program. UIA also reports that name as `control:Id` (Context). Both now take the reader's `ProcessName`. For a `.exe` program the value stays what it is today. For an image with another extension it keeps that extension, because the reader strips only `.exe`. When the process name is absent, the name keeps today's behaviour (Non-Goals).

The spec states it: where a provider names an application node after its program, the name is the process name. `control:Name` and `control:Id` are not process attributes, so they do not count as a second name for the same fact.

*Alternative considered:* **keep `file_stem` for the name.** Rejected: two derivations of the same name would sit side by side, and a user's `[@Name="..."]` could disagree with `[@app:ProcessName="..."]` for the same application.

### D10: The Python `Application` reads the contract

`Application.process_id` reads `control:ProcessId`, and `Application.process_name` reads `app:ProcessName`. Each answers `None` when the attribute is absent, and raises `TypeError` only for a present value of the wrong type. Their return types become `int | None` and `str | None`. `tests/PlatynUI/test_application.py` changes accordingly, and a pytest against the mock (D7) reads both from "Mock Application".

### D11: The 32-bit scenario runs against a window the repository builds

The scenario *A 32-bit process on 64-bit Windows reports its own architecture* needs a 32-bit process with a window. The repository has none. Both JVMs the Swing fixture is provisioned with are 64-bit (`apps/test-app-swing/build/java-launchers.properties` names two JDKs under `C:/Program Files`), and the only Windows Rust target installed is `x86_64-pc-windows-msvc` (**verified** on the maintainer's machine, 2026-09-27). Applications that ship with Windows are excluded from tests and measurements, because they change with Windows versions and updates (maintainer, 2026-09-27).

So the repository gains `apps/win32-test-window`, a helper for process-level tests:

- It shows one top-level window of the predefined `STATIC` class, the shape of the UIA test window `test_window_child` in `crates/provider-windows-uia/src/node.rs`.
- Its title and a lifetime it imposes on itself come from the command line.
- Its bitness follows the build target, and the Windows lane builds it for `i686-pc-windows-msvc`.
- It is not a fixture of the blueprint (`dev-docs/testing-strategy.md` §5), because it has no control catalog, only a process and a window. Its suite therefore sits in a directory of its own, `tests/acceptance/win32`.

*Alternatives considered:*

- **`%WINDIR%\SysWOW64\charmap.exe`, the earlier plan.** Rejected: it is an application that ships with Windows.
- **A 32-bit JVM for the Swing fixture.** This is the most realistic case for JAB. But the fixture's JDKs come from Gradle toolchains, which provision for the build machine's architecture, so a 32-bit JDK would need provisioning of its own. That is out of proportion for one assertion.
- **Unit tests only.** The machine mapping and the fallback rule are unit-tested anyway (task 2.2). Only a real WOW64 process, though, shows that the provider asks the OS for the process's machine and not the host's.

## Risks / Trade-offs

- **[Shapes change that users see]** Users read this metadata in selectors, with `Get Attribute`, through `PlatynUI.ui.Application`, in the CLI's output and in the Inspector, and the change alters what they see (proposal, *Behavior changes that users see*). → That is its purpose: today the values are wrong or differ by provider. PlatynUI is at 0.x, so the changes are not marked as breaking, and the release notes list each of them. Inside the repository, only `PlatynUI.ui.Application` and `@ProcessId` selectors read the metadata. The first is fixed here (D10), and the second keeps its form.
- **[Windows behaviour is verified only on a Windows machine]** CI has no Windows test job; its Windows jobs only build wheels (`.github/workflows/ci.yml`, **verified**). → The reader's Windows functions carry unit tests on their own process, which run with `just test` on Windows. The Windows acceptance lane (`just test-acceptance-windows`) asserts the formats end to end.
- **[The ARM64EC answer rests on reports, not documentation]** Context marks it. → If Windows ever reports ARM64EC differently, it can only report a machine the table maps or leaves absent (D3).
- **[The Windows lane needs a 32-bit Rust target]** The 32-bit test window builds only with `i686-pc-windows-msvc` and the MSVC x86 libraries installed (D11). → `CONTRIBUTING.md` lists both. A missing target fails `just build-win32-test-window-x86` with rustc's message, which names the target. A missing binary fails the suite with a message naming the recipe; it never skips.
- **[Losing the process name for unreadable executables on Linux]** Without the `comm` fallback, a process whose `/proc/<pid>/exe` cannot be read has no name. → That is the contract: absent rather than possibly truncated. On a desktop, the user's own applications are always readable.
- **[Java agent on Linux]** When the Java provider runs on Linux, the session's process ID may come from a container. → Using the reader there needs the same guard AT-SPI has: only a process ID valid in the runtime's namespace. `java-provider-linux` makes the agent compile on Linux, but its artifacts do not mention this guard yet. Whichever change makes the agent's application node read the process table on Linux adds it. D4 holds on Windows, where there is one namespace.
- **[More process opens for full listings on UIA]** → Only listings pay (D5): the Inspector's attribute view and the CLI's `query` and `snapshot`.
- **[Overlap with the snapshot changes]** `snapshot-validity` adds an identity and `is_valid` to the same three nodes and creates the crate, and `xdm-snapshot-release` changes their children iterators. → Small, local conflicts. Land `snapshot-validity` first and rebase.

## Migration Plan

- **Behavioural, not additive.** It changes shapes, as the proposal's list of behavior changes says, and adds or extends one crate. It is not marked as breaking, because PlatynUI is at 0.x.
- **Needs a native rebuild.** The Java agent JAR does not change (D4), so there is no agent version move and no `just install-provider-java`.
- **Order:**
  1. The 32-bit test window, then the acceptance suites, written first: red on Windows, a regression guard on Linux.
  2. The reader with its unit tests, in the crate `snapshot-validity` created, or in a new one.
  3. AT-SPI onto the reader, which is behaviour-preserving apart from D2's process name.
  4. UIA, JAB and the Java provider, with D9's names.
  5. The `0` guards.
  6. The mock, then the Python `Application`.
  7. The docs.
- **Rollback:** reverting the change's commits restores the previous behaviour. There is no persisted state, configuration or data format to migrate back.
