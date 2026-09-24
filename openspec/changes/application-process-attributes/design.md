# Design

## Context

The motivation is in proposal.md (Why), and the contract is in `specs/application-process-attributes/spec.md`. This section records only the current code that shapes the approach. Everything marked **verified** was read in the tree at the time of writing; the rest is marked as assumed.

**Four implementations of one question.** Every provider that builds application nodes reads the process table itself:

- **Windows UIA** uses native Win32 calls in `crates/provider-windows-uia/src/map.rs`:
  - `open_process_query` :205, which falls back to limited query rights.
  - `query_executable_path` :215.
  - The command line through `NtQueryInformationProcess`.
  - `DOMAIN\user` through `LookupAccountSidW` :369.
  - The start time with milliseconds :389.
  - The architecture from the PE header of the executable path :410, and otherwise `GetNativeSystemInfo` :396-408, which ignores the process handle and reports the host.

  All seven attribute objects are listed unconditionally, and each `value()` answers `""`, a null or `"unknown"` when it cannot read (`node.rs`, the application attribute types and `AppAttrsIter` indices 4-10). **Verified.**
- **JAB** uses `sysinfo` in `crates/provider-java-jab/src/process.rs`:
  - The process name from the executable's stem :27.
  - The command line as `sysinfo` arguments joined by spaces :45.
  - The bare user name :57.
  - The start time to the second :64.
  - The architecture from the PE header, falling back to the compile-time `std::env::consts::ARCH` :79-85.

  A value it cannot read becomes a null in the attribute layer. **Verified.** The survey also reports that `sysinfo` 0.39.3 answers an empty executable path on Windows when only limited rights are available. That is **not re-measured here**.
- **The Java agent** does not read the process table at all. It reports what the JVM says about itself (`agent/process`, `crates/provider-java/src/agent/app.rs:48`), in the `control` namespace (`push_optional` :176), with the start time as epoch milliseconds (:149). **Verified.**
- **AT-SPI** uses `sysinfo` and `getpwuid_r` in `crates/provider-atspi/src/process.rs`. After `atspi-process-identity` it decides presence at enumeration and substitutes nothing. It still derives the process name from the executable's *stem* (:31-41), which turns `python3.12` into `python3`, and it falls back to `sysinfo`'s `name()`, the kernel's `comm`, which is truncated to 15 characters. **Verified.**

**What consumers read.** The three window managers read `control:ProcessId` and accept an integer `0`, because `u32::try_from(v).ok()` gives `Some(0)`:

- `crates/platform-windows/src/window_manager.rs:85`
- `crates/platform-linux-x11/src/window_manager.rs:407`
- `crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs:450`

The UIA point hit-test takes `get_process_id(&elem).ok()` without a `> 0` check and builds an app-scoped ancestor chain from it (`provider.rs:497-514`, `node.rs:219`). **Verified.**

**Windows API availability.** The workspace pins `windows` 0.62.2:

- `GetProcessInformation` and `ProcessMachineTypeInfo` are not feature-gated.
- `IsWow64Process2`, `PROCESS_MACHINE_INFORMATION` and the `IMAGE_FILE_MACHINE_*` constants need `Win32_System_SystemInformation`. The UIA crate already enables it; JAB does not.

All of this is **verified** in the crate sources. The Windows versions below are **verified** against Microsoft's documentation. What the calls return is documented or measured by others, not measured here:

- `GetProcessInformation(ProcessMachineTypeInfo)` needs build 22000 or later, which means Windows 11 and Windows Server 2025; Server 2022 is build 20348 and does not have it. It reports the process's own machine, including an x64 process emulated on ARM64 (`AMD64`, measured by Microsoft on 22000).
- `IsWow64Process2` needs Windows 10 1709 or later. It reports a guest machine only for a WOW64 process, and `UNKNOWN` otherwise.
- For an ARM64EC process, `ProcessMachineTypeInfo` reports `AMD64` as well, the same answer as for an emulated x64 process. That comes from a developer's comparison of ARM64EC Office with x64 PowerPoint on Microsoft Q&A, and matches Microsoft's grouping of "x64/Arm64EC" as one process class. No documented API tells the two apart.

## Goals / Non-Goals

**Goals:**
- Each platform has one implementation of "read the process table for this process ID". Every provider on that platform calls it, so two providers cannot disagree about the same process (spec: *Each platform reports the process attributes it has a source for*).
- A macOS or any later provider implements the reader for its platform once and gets the contract for free.
- Every provider reports presence the same way: decided at enumeration, and a lookup by name reads only what it names.

**Non-Goals:**
- `control:Name`, the display name, is not a process attribute. Its empty-string placeholders on UIA, JAB and the Java agent stay out of this change.
- The PID-namespace rules stay in `sidecar-deployment`: which process ID an AT-SPI application reports, and when the process table may be read at all. The reader assumes it is given a process ID valid in the runtime's namespace, and every caller stays responsible for that.
- The Java agent's wire protocol and JAR do not change. See D4.
- The Python `Application.process_id` defect is fixed separately (proposal, Impact).
- macOS gets no implementation. Its provider builds no application nodes today.

## Decisions

### D1: One process reader per platform, in a new crate `crates/process`

A new library crate, `platynui-process`, answers the process attributes for a process ID valid in the runtime's namespace, one function per attribute. Each function returns the value in the specification's format, or nothing:

- **Windows**: native Win32. The UIA code is the starting point because it already reads everything with limited query rights; `sysinfo`'s Windows backend is not used.
- **Linux**: `/proc`, through `sysinfo`, plus `getpwuid_r`. AT-SPI's `process.rs` moves here.
- **Every other target**: every function answers nothing.

It depends on no other PlatynUI crate. The providers depend on it: UIA, JAB, the Java provider and AT-SPI.

*Why a crate:* process facts are properties of the process, not of the toolkit or the accessibility API, and the same process can reach PlatynUI through two providers on one platform (JAB and the in-JVM agent on Windows). One implementation makes the spec's "same value from every provider" hold by construction instead of by review.

*Alternatives considered:*
- **Fix each provider in place.** Rejected: four implementations, of which three would converge on the same Windows code, and the agreement between providers stays a matter of discipline. macOS would add a fifth.
- **A `ProcessInfo` device registered by the platform crates**, like the window manager. Rejected: the providers do not reach platform devices today, and this answer needs no per-runtime state or connection. It is a pure function of a process ID, and a plain library is the smallest thing that can hold it.
- **Put it into `platynui-core`.** Rejected: core is platform-neutral and has no OS dependencies (`crates/core/Cargo.toml`, **verified**); this reader is nothing but OS dependencies.
- **`sysinfo` on every platform.** Rejected for Windows: it only offers arguments already split by `CommandLineToArgvW`, which cannot give the verbatim command line. It has no domain-qualified user name and no architecture. And according to the survey, it answers an empty executable path under limited rights.

### D2: How each attribute is read, and what makes it absent

These are per-platform sources in the reader. Each row produces the format the spec fixes.

| Attribute | Windows | Linux | Absent when |
|---|---|---|---|
| `ProcessName` | the file name of `ExecutablePath`, with a trailing `.exe` removed (case-insensitively) | the file name of `ExecutablePath`, unchanged | the executable path is absent. There is no fallback to `comm` or to the image name, which is truncated or can be changed by the process |
| `ExecutablePath` | `QueryFullProcessImageNameW` (Win32 form) | the target of `/proc/<pid>/exe`, without the ` (deleted)` the kernel appends once the file has been replaced or removed | the process cannot be opened or the link read |
| `CommandLine` | `NtQueryInformationProcess(ProcessCommandLineInformation)`, verbatim | `/proc/<pid>/cmdline`, arguments joined by single spaces | unreadable, or empty (a kernel thread has none) |
| `UserName` | the token user through `LookupAccountSidW`, as `DOMAIN\user`. For a local account the returned domain is the computer name (**assumed** from the API documentation) | the effective UID through `getpwuid_r` | no token or UID, or the account has no name |
| `StartTime` | `GetProcessTimes` creation time, in UTC, truncated to the second | `sysinfo`'s start time, which has one-second resolution | unreadable, or `0` |
| `Architecture` | D3 | not supplied (spec) | D3 |

A process whose executable was replaced since it started — typically by a package update while the application runs — keeps the path it was started from, and its process name stays correct. On Linux, `/proc/<pid>/exe` then reads `<path> (deleted)`. `sysinfo` already removes that suffix (`sysinfo` 0.39.3, `src/unix/linux/process.rs:503-515`, **verified**, and the suffix was **measured** with a copied binary removed while it ran). The reader keeps that behaviour, so neither the path nor the name derived from it ever carries the suffix.

AT-SPI's process name changes with this (Context). `python3.12` stays `python3.12`, and a process whose executable cannot be read no longer reports a truncated `comm` name.

*Alternative considered:* **keep `comm` as a fallback for the process name.** Rejected: `comm` is truncated to 15 characters and can be set by the process itself, so it is a plausible wrong answer. That is exactly what the spec forbids.

### D3: The Windows architecture comes from documented OS calls

The reader asks `GetProcessInformation(ProcessMachineTypeInfo)` first. Where that call does not exist, before build 22000 (Windows 10, Windows Server 2022), it asks `IsWow64Process2`. For a WOW64 process that returns the guest machine, and for any other process the native machine. The machine then maps into the spec's vocabulary:

- `I386` → `x86`
- `AMD64` → `x64`
- `ARM` or `ARMNT` → `arm`
- `ARM64` → `arm64`

Any other machine, or two failed calls, leaves the attribute absent.

This answers the question that matters on Windows on ARM: is the application an ARM application or an x86/x64 one running under emulation? An ARM64EC application reports `x64`, because Windows itself classifies it as an x64-compatible process (Context). The maintainer decided that this distinction is not worth any source beyond the documented calls.

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

The agent keeps sending its facts. The provider still uses the application name for `control:Name` and the JVM facts for the `native:` attributes, and ignores the rest.

*Why unchanged:* the agent cannot be unloaded from a JVM, and the provider and the agent are compared for an exact version at connect time (AGENTS.md, Java routing). Changing what the agent sends would move the agent, the provider and the delivery package together for no gain. The provider ignoring fields it no longer needs is backward compatible in both directions.

*Alternative considered:* **remove the process fields from `ProcessFacts.java` now.** Deferred: it is harmless clean-up that can ride along with the next change that moves the agent version anyway.

### D5: Presence at enumeration, reads by name

Every provider lists a process attribute only when the reader returned a value, and overrides the named lookup so that `@app:X` reads only `X`. This is the pattern `atspi-process-identity` introduced for AT-SPI (`AtspiNode::attribute`), for the same reason: a listing that decides presence by reading must not pay for six reads when one name is asked for.

UIA today opens the process once per `value()`. After the change, a full listing opens it once per attribute. That is acceptable for a listing and never happens for a lookup by name.

### D6: Process ID `0` is rejected at every entry and every exit

- The reader answers nothing for the process ID `0`.
- UIA's hit-test and JAB's `GetWindowThreadProcessId` calls (`provider.rs:417`, `:517`) treat `0` as "no process". An orphan application node then gets no `ProcessId`, and UIA's hit-test falls back to the desktop scope it already has for an unknown process.
- The three window managers' `pid_from_attr` accept only a positive number, so a node that carries `0` resolves no window.

### D7: The mock models process attributes on one application

The mock's "Mock Application" gains `control:ProcessId` and a subset of the `app:` attributes in the specification's formats. The subset deliberately leaves out `CommandLine`, so the scenario *Listing and predicate agree for a missing attribute* has a fixture in the mock tree, tested through the runtime's XPath evaluation. This also gives the separate Python fix a fixture to test against.

"Mock Settings" stays without process attributes, as an application whose process is unknown.

### D8: Documentation follows the contract, and the spec stays the source

- **`dev-docs/architecture.md`**:
  - The Application row of the pattern catalog: `ProcessId` becomes optional, and all six `app:` attributes are listed.
  - The per-platform source table gets the D2/D3 sources.
- **`dev-docs/platform-windows.md`**: the application-node attributes.
- **`dev-docs/planning.md`**: the parity item is resolved.
- **`AGENTS.md`** and the crate tree in `dev-docs/architecture.md` §2: both gain `crates/process`.
- **The core attribute constants**: a one-line pointer to the capability.

The docs point to the spec for formats instead of restating them.

## Risks / Trade-offs

- **[BREAKING shapes for existing consumers]** Consumers relied on today's shapes: on UIA and JAB an attribute is always present, the Java agent's attributes sit in `control:`, and the start time or user-name form differs by provider. → The proposal lists every change. Nothing in the repository consumes the metadata beyond `ProcessId` (survey: no suite reads `app:*`, no Rust consumer), so the break reaches only external XPath and Python users. The release notes name it.
- **[Windows behaviour is verified only on a Windows machine]** CI has no Windows test job; its Windows jobs only build wheels (`.github/workflows/ci.yml`, **verified**). → The reader's Windows functions carry unit tests on their own process, which run with `just test` on Windows. The Windows acceptance lane (`just test-acceptance-windows`) asserts the formats end to end. This Linux machine can only cross-check the Windows crates with `just check-windows` and `just clippy-windows`.
- **[The ARM64EC answer rests on reports, not documentation]** Context marks it. → If Windows ever reports ARM64EC differently, it can only report a machine the table maps or leaves absent (D3). The 32-bit scenario runs only where the Windows lane has a 32-bit application, and the task says so rather than skipping silently.
- **[Losing the process name for unreadable executables on Linux]** Without the `comm` fallback, a process whose `/proc/<pid>/exe` cannot be read has no name. → That is the contract: absent rather than possibly truncated. On a desktop, the user's own applications are always readable.
- **[Java agent on Linux]** When `java-provider-linux` makes the provider portable, the session's process ID may come from a container. → Using the reader there needs the same guard AT-SPI has: only a process ID valid in the runtime's namespace. That change owns it. D4 holds on Windows, where there is one namespace.
- **[More process opens for full listings on UIA]** → Only listings pay (D5). The Inspector's attribute view is the main one, and it is interactive.

## Migration Plan

- **Behavioural, not additive.** It changes shapes, as the proposal's BREAKING list says, and adds one crate.
- **Needs a native rebuild.** The Java agent JAR does not change (D4), so there is no agent version move and no `just install-provider-java`.
- **Order:**
  1. The acceptance suites, written first: red on Windows, a regression guard on Linux.
  2. The reader with its unit tests.
  3. AT-SPI onto the reader, which is behaviour-preserving apart from D2's process name.
  4. UIA, JAB and the Java provider.
  5. The `0` guards.
  6. The mock.
  7. The docs.
- **Rollback:** reverting the change's commits restores the previous behaviour. There is no persisted state, configuration or data format to migrate back.
