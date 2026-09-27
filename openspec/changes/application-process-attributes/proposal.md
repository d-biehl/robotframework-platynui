# Proposal

## Why

The process attributes of an application node are part of what users of PlatynUI work with every day:

- They select an application by them in XPath, in any keyword that takes a selector (`/app:Application[@app:ProcessName='notepad']`).
- They read them with `Get Attribute    <application>    app:ProcessName`, through `PlatynUI.ui.Application`, and in the output of `platynui-cli query` and `snapshot`.
- They look them up in the Inspector's attribute view, which offers them for copying as XPath.

A wrong value misleads such a test without any sign, and a shape that differs by provider makes the same selector work on one application and fail on another.

Today every provider that builds `app:Application` nodes decides on its own what these attributes mean, when they are present, and what they say when a value cannot be read. The results disagree, and some of them are wrong:

- **Windows UIA** always lists all seven attributes. A value it cannot read comes back as `""`, a null or `"unknown"`. An architecture it cannot read from the PE header becomes the architecture of the machine PlatynUI runs on (`GetNativeSystemInfo`). That value is wrong for every 32-bit process on a 64-bit Windows and for emulated x64 on ARM64.
- **JAB**:
  - It answers an architecture it cannot read with the one PlatynUI was compiled for, which is wrong for exactly the 32-bit JVMs that JAB exists to serve. Its PE parser answers `"unknown"` for a machine it does not know.
  - Under limited rights it reports the image name with `.exe` as the process name.
  - Every other attribute it cannot read comes back as a null.
- **The Java agent** puts the same attributes in `control:` instead of `app:`, reports the start time as epoch milliseconds instead of an ISO timestamp, and passes the JVM's raw `os.arch` (`amd64`, `aarch64`) through. Its "process name" is the main class and its "executable path" is always `<java.home>/bin/java`, even for a JVM started through `javaw.exe`.
- **`PlatynUI.ui.Application`**:
  - `process_id` reads `app:ProcessId`, which no provider reports, so it always fails (`src/PlatynUI/ui/application.py:31-39`).
  - `process_name` fails on every application that the Java agent serves, because the agent reports the name under `control:` (`:42-49`).

`atspi-process-identity` established the rule for AT-SPI on Linux: an attribute is reported only when it was actually read, never substituted, and the architecture is not reported at all, because Linux keeps none per process. The other providers still work the old way. macOS is next, and further platforms and providers may follow. Without a shared contract each of them invents a fifth variant, so the contract is settled now, while there are only four to align.

## What Changes

- **A cross-provider contract for application process attributes** — `ProcessId`, `ProcessName`, `ExecutablePath`, `CommandLine`, `UserName`, `StartTime`, `Architecture` — that every provider exposing `app:Application` nodes follows:
  - **Every attribute is optional**, `ProcessId` included. An attribute is present only when its value was actually determined for that process. Presence is decided when the attribute is enumerated, so an XPath predicate like `[@app:CommandLine]` means what it says.
  - **No substitutes.** Never an empty string, a null, `"unknown"` or `0`. Never a value that describes the automation host instead of the application: the host's or the build's architecture, the host's user, and the like.
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
  - JAB:
    - remove the null placeholders, the PE parser's `"unknown"`, the image-name fallback and the compile-time fallback;
    - add process-ID-`0` guards, including in the pass that runs without the Access Bridge DLL, whose process IDs become candidates for automatic agent attachment.
  - Java agent: report the process attributes of the JVM process from the platform, under `app:`; the main class stays the display name.
  - Mock: its application nodes follow the contract where they expose process attributes.
  - Window managers on Windows, X11 and Wayland reject process ID `0`.
- **The display name follows the process name.** The application nodes of UIA and JAB are named after their program, and UIA's `control:Id` carries the same name. Both are now derived from the new `ProcessName`. For a `.exe` program the name stays what it is today (`notepad.exe` → `notepad`); for an image with another extension it follows the reader, which strips only `.exe`.
- **`PlatynUI.ui.Application` reads the contract.** `process_id` reads `control:ProcessId`, and `process_name` reads `app:ProcessName`. Both answer `None` when the attribute is absent.
- **Users can look the attributes up.** BareMetal's library documentation describes the `app:` attributes, that each of them may be absent, how to test for one with `[@app:X]`, their formats, and that `Get Attribute` accepts the `app:` prefix.
- **Behavior changes that users see.** PlatynUI is at 0.x, so they are not marked as breaking, and the release notes name each of them:
  - On UIA and JAB, an attribute that cannot be read is now absent instead of `""` or null. `Get Attribute` then raises `AttributeNotFoundError` instead of returning `""` or `None`, `[@app:X]` is false where a null made it true, and the `null` rows disappear from `platynui-cli query` and `snapshot` and the `<null>` rows from the Inspector.
  - Java-agent application nodes carry their metadata under `app:`, not `control:`.
  - Java-agent `StartTime` becomes an ISO timestamp.
  - Architecture values from the Java agent are normalized, for example `amd64` becomes `x64`.
  - The Java agent's `ProcessName`, `ExecutablePath`, `CommandLine` and `UserName` describe the JVM process, for example `javaw` instead of the main class.
  - Windows UIA's `StartTime` loses its milliseconds.
  - JAB's `UserName` gains the domain, its `CommandLine` becomes the verbatim line, and its `ProcessName` loses the `.exe` it carried under limited rights.
  - AT-SPI's `ProcessName` becomes the executable's full file name, for example `python3.12` instead of `python3`, and it is absent instead of the kernel's truncated name when the executable cannot be read.
  - The name of a UIA or JAB application node whose image does not end in `.exe` keeps its extension.
  - `Application.process_id` returns the process ID where it always failed before, and both properties return `None` instead of raising when the attribute is absent.

## Capabilities

### New Capabilities

- `application-process-attributes`: the process attributes of an application node across all providers:
  - which attributes exist, their namespaces and value formats;
  - the rule that each one is reported only when determined and never substituted;
  - which platform provides which attribute from which kind of source;
  - that `0` is never a process ID;
  - that a node named after its program carries the process name;
  - how the Python `Application` reads them.

### Modified Capabilities

None. `sidecar-deployment` keeps its rules for AT-SPI and needs no requirement changed:

- a process-table attribute is read only through a process ID valid in the runtime's namespace, and is absent rather than substituted;
- the window manager and the compositor backend resolve nothing for a process ID of `0` (`openspec/specs/sidecar-deployment/spec.md:259`).

Both are special cases of this contract. The new capability points to that spec for the PID-namespace part, and its window-manager requirement agrees with it on Linux.

## Impact

- **Rust:**
  - `crates/process` (`platynui-process`): one process reader per platform, which every provider calls (design D1). `snapshot-validity` creates this crate with a process identity (pid and exact start time). If it lands first, this change adds its reader to the crate; otherwise this change creates it. It follows the workspace lints.
  - `crates/provider-windows-uia`:
    - the application attributes, `control:Name` and `control:Id` in `node.rs`;
    - its process helpers in `map.rs` move into the reader, and the `windows` features that only they used can go;
    - the hit-test's scope for an unknown process in `provider.rs`.
  - `crates/provider-java-jab`:
    - `process.rs` goes away in favour of the reader;
    - the application metadata and name in `node.rs`;
    - process-ID guards in `provider.rs`, including the pass without the DLL, and in the hit-test.
  - `crates/provider-java`: the agent application node, `src/agent/app.rs`.
  - `crates/provider-mock` and its tree asset.
  - The window-manager process-ID readers in `platform-windows`, `platform-linux-x11` and `platform-linux-wayland`.
  - The attribute documentation in `crates/core`.
  - `crates/provider-atspi`: its process reading moves into the new crate. Its process name becomes the executable's file name, with no stem and no fallback to the truncated kernel name (design D2).
- **Java:** no change. The agent keeps reporting its facts, and the provider stops using them for process attributes (design D4). So there is no JAR rebuild and no agent version move.
- **Python / Robot Framework:**
  - `src/PlatynUI/ui/application.py` (`process_id`, `process_name`) and its tests.
  - BareMetal's library documentation, and the documentation of `Get Attribute`.
  - No keyword or argument changes.
- **Tests:**
  - Unit tests per provider for presence, format and name.
  - pytest for `Application`, with stubs and against the mock.
  - On a Windows machine: `just test` and the Windows acceptance lane, for UIA, JAB and the Java agent.
  - A runtime test over the mock tree, whose "Mock Application" models process attributes.
- **Docs:**
  - `dev-docs/architecture.md`: the Application row of the pattern catalog, where `ProcessId` becomes optional; the per-platform source table, which today claims `IsWow64Process2` on Windows although the code parses the PE header; and the Windows UIA checklist.
  - `dev-docs/platform-windows.md` and `dev-docs/platform-linux.md`.
  - The attribute-parity item and the source notes in `dev-docs/planning.md`, which this change resolves.
  - The `Application` sketch in `dev-docs/python-library-design.md`.
  - `AGENTS.md`, whose crate list gains `crates/process` unless `snapshot-validity` has added it.
  - The user documentation of BareMetal.
- **Build:** native rebuild only.
- **Platforms:**
  - Windows (UIA, JAB, Java agent) changes behaviour.
  - Linux AT-SPI changes only its process name (design D2).
  - The Java agent on Linux follows once the Java provider runs there (design D4, Risks).
  - macOS has no process attributes today (the provider is a stub) and implements against this contract when it gets them.
- **Coordination:**
  - `snapshot-validity` creates `crates/process` and adds a process identity and an `is_valid` to the same three application nodes. It should land first; this change then extends the crate and rebases onto the nodes.
  - `xdm-snapshot-release` changes the children iterators of the same nodes.
  - `fix-jab-hit-test-virtual-children` touches JAB's hit-test, where this change guards process ID `0`.
  - `window-activation-state` touches JAB node attributes.
  - `java-provider-linux` makes the agent compile on Linux, but does not yet add the PID-namespace guard that the agent's application node needs there before it reads the process table (design, Risks).
