# Proposal

## Why

Every provider that builds `app:Application` nodes decides on its own what the process attributes mean, when they are present, and what they say when a value cannot be read. The results disagree, and some of them lie:

- **Windows UIA** always lists all seven attributes. A value it cannot read comes back as `""`, a null or `"unknown"`, and an architecture it cannot read from the PE header becomes the architecture of the machine PlatynUI runs on (`GetNativeSystemInfo`). That value is wrong for every 32-bit process on a 64-bit Windows and for emulated x64 on ARM64.
- **JAB** answers the architecture it cannot read with the one PlatynUI was compiled for, which is wrong for exactly the 32-bit JVMs that JAB exists to serve. It also returns a null for every other attribute it cannot read.
- **The Java agent** puts the same attributes in `control:` instead of `app:`, reports the start time as epoch milliseconds instead of an ISO timestamp, and passes the JVM's raw `os.arch` (`amd64`, `aarch64`) through.

`atspi-process-identity` established the rule for AT-SPI on Linux: an attribute is reported only when it was actually read, never substituted, and the architecture is not reported at all, because Linux keeps none per process. The other providers still work the old way. macOS is next, and further platforms and providers may follow. Without a shared contract each of them invents a fifth variant, so the contract is settled now, while there are only four to align.

## What Changes

- **A cross-provider contract for application process attributes** — `ProcessId`, `ProcessName`, `ExecutablePath`, `CommandLine`, `UserName`, `StartTime`, `Architecture` — that every provider exposing `app:Application` nodes follows:
  - **Every attribute is optional**, `ProcessId` included. An attribute is present only when its value was actually determined for that process. Presence is decided when the attribute is enumerated, so an XPath predicate like `[@app:CommandLine]` means what it says.
  - **No substitutes.** Never an empty string, a null, `"unknown"` or `0`, and never a value that describes the automation host instead of the application: the host's or the build's architecture, the host's user, and the like.
  - **One namespace per attribute.** `ProcessId` stays in `control:`, where every provider puts it and every consumer (window managers, Robot suites) reads it. The process metadata lives in `app:`, where UIA, JAB and AT-SPI already put it.
  - **One value format per attribute**, fixed in the spec and chosen by the maintainer:
    - `StartTime`: ISO-8601 UTC to the second.
    - `UserName`: in the platform's form, `DOMAIN\user` on Windows.
    - `CommandLine`: as the platform shows it, the verbatim line on Windows and the arguments joined like `ps` on Linux and macOS.
    - `Architecture`: from the closed vocabulary `x86`/`x64`/`arm`/`arm64`.
  - **Process attributes describe the process, not the toolkit.** They come from the platform, by process ID, for every provider. The Java agent's self-reported values — main class, a path derived from `java.home`, the overridable `user.name` — are no source for them.
  - **Availability per platform.** A provider reports an attribute only where its platform has a real source for it, and the spec says which. For example, there is no architecture on Linux, while Windows reads it through `GetProcessInformation(ProcessMachineTypeInfo)`, with `IsWow64Process2` as the fallback on older versions, instead of parsing a PE header.
  - **`ProcessId` is never `0`**, and no consumer correlates a window with an application by `0`.
- **Align the providers with it:**
  - Windows UIA: remove the placeholders and the host fallback, fix the ARM mapping, use the OS architecture API, and stop building an orphan application node with process ID `0`.
  - JAB: remove the null placeholders and the compile-time fallback, and add process-ID-`0` guards.
  - Java agent: report the process attributes of the JVM process from the platform, under `app:`; the main class stays the display name.
  - Mock: its application nodes follow the contract where they expose process attributes.
  - Window managers on Windows, X11 and Wayland reject process ID `0`.
- **BREAKING**, for consumers that relied on today's shapes:
  - On UIA and JAB, an attribute that cannot be read is now absent instead of `""` or null.
  - Java-agent application nodes carry their metadata under `app:`, not `control:`.
  - Java-agent `StartTime` becomes an ISO timestamp.
  - Architecture values from the Java agent are normalized, for example `amd64` becomes `x64`.
  - The Java agent's `ProcessName`, `ExecutablePath`, `CommandLine` and `UserName` describe the JVM process, for example `javaw` instead of the main class.
  - Windows UIA's `StartTime` loses its milliseconds.
  - JAB's `UserName` gains the domain, and its `CommandLine` becomes the verbatim line.
  - AT-SPI's `ProcessName` becomes the executable's full file name, for example `python3.12` instead of `python3`, and it is absent instead of the kernel's truncated name when the executable cannot be read.

## Capabilities

### New Capabilities

- `application-process-attributes`: the process attributes of an application node across all providers — which attributes exist, their namespaces and value formats, the rule that each one is reported only when determined and never substituted, which platform provides which attribute from which kind of source, and that `0` is never a process ID.

### Modified Capabilities

None. `sidecar-deployment` keeps its rule for AT-SPI, that a process-table attribute is read only through a process ID valid in the runtime's namespace and is absent rather than substituted. That rule is a special case of this contract and stays consistent with it, so no requirement there changes. The new capability points to it for the PID-namespace part.

## Impact

- **Rust**:
  - A new crate, `crates/process` (`platynui-process`): one process reader per platform, which every provider calls (design D1).
  - `crates/provider-windows-uia`: application attributes in `node.rs`; its process helpers in `map.rs` move into the reader. The `windows` features the reader needs, including `Win32_System_SystemInformation` for `IsWow64Process2` and `PROCESS_MACHINE_INFORMATION`, are those UIA enables today.
  - `crates/provider-java-jab`: `process.rs` goes away in favour of the reader, application metadata in `node.rs`, process-ID guards in `provider.rs`.
  - `crates/provider-java`: agent application node, `src/agent/app.rs`.
  - `crates/provider-mock` and its tree asset.
  - The window-manager process-ID readers in `platform-windows`, `platform-linux-x11` and `platform-linux-wayland`.
  - The attribute documentation in `crates/core`.
  - `crates/provider-atspi`: its process reading moves into the new crate. Its process name becomes the executable's file name, with no stem and no fallback to the truncated kernel name (design D2).
- **Java**: no change. The agent keeps reporting its facts, and the provider stops using them for process attributes (design D4). So there is no JAR rebuild and no agent version move.
- **Python / Robot Framework**: no API change. Suites address applications only through `@ProcessId`, which stays in `control:`. The Python `Application.process_id`/`process_name` reading `app:ProcessId` is a separate existing defect, fixed on its own and not in this change.
- **Tests**:
  - Unit tests per provider for presence and format.
  - On a Windows machine: `just test` and the Windows acceptance lane, for UIA, JAB and the Java agent. CI has no Windows test job, and this Linux machine can only cross-check the Windows crates (`just check-windows`, `just clippy-windows`).
  - A runtime test over the mock tree, whose "Mock Application" models process attributes.
- **Docs**:
  - `dev-docs/architecture.md`: the Application row of the pattern catalog, where `ProcessId` becomes optional, and the per-platform source table, which today claims `IsWow64Process2` on Windows although the code parses the PE header.
  - `dev-docs/platform-windows.md`.
  - The attribute-parity item in `dev-docs/planning.md`, which this change resolves.
  - `AGENTS.md`, whose crate list gains `crates/process`.
- **Build**: native rebuild only.
- **Platforms**:
  - Windows (UIA, JAB, Java agent) changes behaviour.
  - Linux AT-SPI changes only its process name (design D2).
  - The Java agent on Linux follows once `java-provider-linux` makes the provider portable.
  - macOS has no process attributes today (the provider is a stub) and implements against this contract when it gets them.
- **Coordination**: `java-provider-linux` and the `provider-java-*` changes touch the agent application node too; whichever lands second rebases onto the other.
