# Proposal

## Why

The process attributes of an application node are part of what users of PlatynUI work with every day:

- They select an application by them in XPath, in any keyword that takes a selector (`/app:Application[@app:ProcessName='ledger']`).
- They read them with `Get Attribute    <application>    app:ProcessName`, through `PlatynUI.ui.Application`, and in the output of `platynui-cli query` and `snapshot`.
- They look them up in the Inspector's attribute view, which offers them for copying as XPath.

A wrong value misleads such a test without any sign, and a shape that differs by provider makes the same selector work on one application and fail on another.

Today every provider that builds `app:Application` nodes decides on its own what these attributes mean, when they are present, and what they say when a value cannot be read. The results disagree, and some of them are wrong:

- **Windows UIA**:
  - It always lists all seven attributes. A value it cannot read comes back as `""`, a null or `"unknown"`.
  - When it can open the process but cannot read the PE header of its executable, the architecture becomes that of the machine PlatynUI runs on (`GetNativeSystemInfo`). That value is wrong for every 32-bit process on a 64-bit Windows and for emulated x64 on ARM64.
  - Its PE mapping also reports 32-bit ARM as `arm64`.
- **JAB**:
  - It answers an architecture it cannot read with the one PlatynUI was compiled for. That is wrong for exactly the 32-bit JVMs, which automatic agent attachment cannot reach, so JAB serves them unless they were started with `-javaagent`. Its PE parser answers `"unknown"` for a machine it does not know.
  - Under limited rights `sysinfo` answers an empty executable path. JAB then reports the image name with `.exe` as the process name.
  - Every other attribute it cannot read comes back as a null.
- **The Java agent** puts the same attributes in `control:` instead of `app:`, and takes them from the JVM's system properties instead of the process:
  - its "process name" is the main class's simple name or the jar's file name;
  - its "executable path" is always `<java.home>\bin\java.exe`. For a JVM started through `javaw.exe` that names the wrong launcher. On Java 8, whose `java.home` is the JDK's `jre` directory, it names `<jdk>\jre\bin\java.exe` although `<jdk>\bin\java.exe` was started;
  - its "command line" is `sun.java.command`, without the launcher, the JVM options or any quoting;
  - its "user name" is the overridable `user.name`, and its architecture is the raw `os.arch` (`amd64`, `aarch64`);
  - its start time is epoch milliseconds, and only on Java 9 or later. On Java 8, which the Windows lane runs the Swing fixture on, it reports none.
- **`PlatynUI.ui.Application`**:
  - `process_id` reads `app:ProcessId`, which no provider reports, so it always fails (`src/PlatynUI/ui/application.py:31-39`).
  - `process_name` fails on every application that the Java agent serves, because the agent reports the name under `control:` (`:42-49`).

`atspi-process-identity` established the rule for AT-SPI on Linux: an attribute is reported only when it was actually read, never substituted, and the architecture is not reported at all, because Linux keeps none per process. The other providers still work the old way. macOS is next, and further platforms and providers may follow. Without a shared contract each of them invents a fifth variant, so the contract is settled now, while there are only four to align.

This change comes first among the Java changes: the Java agent is the main Java path, and today it reports the wrong process for every application it serves.

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
  - **An application node reports the process it was created for.** Its `app:` attributes are read through the process identity recorded for it; once that process has ended or its pid belongs to another process, it reports none of them. `control:ProcessId` stays. UIA, JAB and the Java agent record the identity when the node is created, AT-SPI when it builds the node's process attributes.
  - **Availability per platform.** A provider reports an attribute only where its platform has a real source for it, and the spec says which. For example, there is no architecture on Linux, while Windows reads it through `GetProcessInformation(ProcessMachineTypeInfo)`, with `IsWow64Process2` as the fallback on older versions, instead of parsing a PE header.
  - **`ProcessId` is never `0`**, and no consumer correlates a window with an application by `0`.
- **Align the providers with it:**
  - Windows UIA: read through the shared reader, which drops the placeholders, the host fallback and the PE header with its wrong ARM mapping; stop building an orphan application node with process ID `0`.
  - JAB:
    - read through the shared reader, which drops the null placeholders, the PE parser's `"unknown"`, the image-name fallback and the compile-time fallback;
    - skip a window without a process in both discovery passes, including the pass without the Access Bridge DLL, whose process IDs become candidates for automatic agent attachment. The hit-test abstains for such a window. `GetWindowThreadProcessId` answers `0` for a top-level window only once it no longer exists.
  - Java agent: report the process attributes of the JVM process from the platform, under `app:`; the main class stays the display name. Automatic attachment ignores process ID `0`.
  - AT-SPI: read through the shared reader; its process name becomes the executable's full file name.
  - Mock: its application nodes follow the contract where they expose process attributes.
  - Window managers on Windows, X11 and Wayland reject process ID `0`.
- **The display name follows the process name.** The application nodes of UIA and JAB are named after their program, and UIA's `control:Id` carries the same name. Both are now derived from the new `ProcessName`. For a `.exe` program the name stays what it is today (`ledger.exe` → `ledger`); for an image with another extension it follows the reader, which strips only `.exe`.
- **`PlatynUI.ui.Application` reads the contract.** `process_id` reads `control:ProcessId`, and `process_name` reads `app:ProcessName`. Both answer `None` when the attribute is absent.
- **Users can look the attributes up.** BareMetal's library documentation describes the `app:` attributes, that each of them may be absent, how to test for one with `[@app:X]`, their formats, and that `Get Attribute` accepts the `app:` prefix.
- **Behavior changes that users see.** PlatynUI is at 0.x, so they are not marked as breaking, and the release notes name each of them:
  - UIA and JAB no longer answer `Architecture` with `"unknown"`, the host's architecture or the build's: it is the process's real one, or absent. UIA reports 32-bit ARM as `arm` instead of `arm64` or `"unknown"`.
  - On UIA and JAB, an attribute that cannot be read is now absent instead of `""` or null. `Get Attribute` then raises `AttributeNotFoundError` instead of returning `""` or `None`, `[@app:X]` is false where a null made it true, and the `null` rows disappear from `platynui-cli query` and `snapshot` and the `<null>` rows from the Inspector.
  - An application node whose process has ended, or whose pid now belongs to another process, lists no `app:` process attribute.
  - Java-agent application nodes carry their metadata under `app:`, not `control:`.
  - Java-agent `StartTime` becomes an ISO timestamp, and is present on Java 8 as well, where the agent reported none.
  - Architecture values from the Java agent are normalized, for example `amd64` becomes `x64`.
  - The Java agent's `ProcessName`, `ExecutablePath`, `CommandLine` and `UserName` describe the JVM process:
    - `java` or `javaw` instead of the main class;
    - the launcher that was started instead of `<java.home>\bin\java.exe`;
    - the verbatim command line, with launcher and JVM options, instead of `sun.java.command`;
    - `DOMAIN\user` instead of `user.name`.
  - Windows UIA's `StartTime` loses its milliseconds.
  - JAB's `UserName` gains the domain, and its `CommandLine` becomes the verbatim line. Its `ProcessName`, and with it the name of its application node, loses the `.exe` it carried under limited rights.
  - AT-SPI's `ProcessName` becomes the executable's full file name, for example `python3.12` instead of `python3`, and it is absent instead of the kernel's truncated name when the executable cannot be read.
  - The name of a UIA or JAB application node whose image does not end in `.exe` keeps its extension.
  - `Application.process_id` returns the process ID where it always failed before, and both properties return `None` instead of raising when the attribute is absent.

## Capabilities

### New Capabilities

- `application-process-attributes`: the process attributes of an application node across all providers:
  - which attributes exist, their namespaces and value formats;
  - the rule that each one is reported only when determined and never substituted;
  - that a node reports only the process it was created for;
  - which platform provides which attribute from which kind of source;
  - that `0` is never a process ID;
  - that a node named after its program carries the process name;
  - how the Python `Application` reads them.

### Modified Capabilities

None. `sidecar-deployment` keeps its rules for AT-SPI and needs no requirement changed:

- a process-table attribute is read only through a process ID valid in the runtime's namespace, and is absent rather than substituted;
- the window manager and the compositor backend correlate nothing through a process ID of `0` (`openspec/specs/sidecar-deployment/spec.md:259`). The compositor backend still matches a node without a usable process ID by title and size (`crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs:332-375`); this change leaves that as it is.

Both are special cases of this contract. The new capability points to that spec for the PID-namespace part, and its window-manager requirement agrees with it on Linux.

## Impact

- **Rust:**
  - `crates/process` (`platynui-process`): `snapshot-validity` created it with `ProcessIdentity`, the pid and the exact start time (`8f6fc02`). UIA, JAB and the Java provider already depend on it.
    - This change adds one process reader per platform, which every provider calls through the node's identity (design D1).
    - AT-SPI gains the dependency.
    - The `windows` features grow by `Win32_Security`, `Wdk_System_Threading` and `Win32_System_SystemInformation`; on Linux the crate gains `sysinfo`, with one targeted refresh per read, and `libc` for `getpwuid_r`.
    - It follows the workspace lints.
  - `crates/provider-windows-uia`:
    - the application attributes, `control:Name` and `control:Id` in `node.rs`;
    - its process helpers in `map.rs` move into the reader, and the `windows` features only they used can go;
    - the hit-test's scope for an unknown process, and the pid guard of the per-application window filter, in `provider.rs`.
  - `crates/provider-java-jab`:
    - `process.rs` goes away in favour of the reader;
    - the application metadata and name in `node.rs`;
    - `provider.rs` and `node.rs`: `WindowCandidate`'s process ID becomes optional through `process_id_of`. Both discovery passes, including the one without the DLL, skip a window without a process, and the hit-test abstains for it.
  - `crates/provider-java`: the agent application node, `src/agent/app.rs`, whose `ProcessFacts` drops the fields it no longer reads; and `src/agent/backend.rs`, where `attach_to_agentless` drops process ID `0`.
  - `crates/provider-atspi`: its process reading moves into `crates/process`. Its process name becomes the executable's file name, with no stem and no fallback to the truncated kernel name (design D2). `sysinfo`, `chrono` and `libc` leave its manifest; `sysinfo` and `libc` move with the reader into `crates/process`.
  - `crates/provider-mock` and its tree asset.
  - The window-manager process-ID readers in `platform-windows`, `platform-linux-x11` and `platform-linux-wayland`.
  - The attribute documentation in `crates/core`.
  - `apps/win32-test-window` (new): a minimal Win32 window whose bitness follows the build target. The Windows lane builds it for 32-bit x86, so the 32-bit scenario needs no application that ships with Windows (design D11).
- **Java:** no change. The agent keeps reporting its facts, and the provider stops using them for process attributes (design D4). The provider's `ProcessFacts` drops the fields it no longer reads; serde ignores those the agent keeps sending. So there is no JAR rebuild and no agent version move.
- **Python / Robot Framework:**
  - `src/PlatynUI/ui/application.py` (`process_id`, `process_name`) and its tests.
  - BareMetal's library documentation, and the documentation of `Get Attribute`.
  - No keyword or argument changes.
- **Tests:**
  - Unit tests per provider for presence, format and name; for the ended process, the reader and UIA.
  - pytest for `Application`, with stubs and against the mock.
  - On a Windows machine: `just test`; the lane's live Java tests (`crates/provider-java/tests/live_fixture.rs`: the JAB application node's name, and the agreement of the JAB and agent nodes); and the Windows acceptance lane, for UIA, JAB and the Java agent, and for a 32-bit process from `apps/win32-test-window`.
  - A runtime test over the mock tree, whose "Mock Application" models process attributes.
- **Docs:**
  - `dev-docs/architecture.md`: the Application row of the pattern catalog, where `ProcessId` becomes optional; the per-platform source table. For Windows it names `IsWow64Process2 / PE header`, although no code calls `IsWow64Process2` and the fallback is `GetNativeSystemInfo`. For the Linux process name it names `/proc/PID/comm or cmdline[0]`. Also the Windows UIA checklist.
  - `dev-docs/platform-windows.md` and `dev-docs/platform-linux.md`.
  - The attribute-parity item and the source notes in `dev-docs/planning.md`, which this change resolves.
  - `dev-docs/platform-linux-wayland.md`, whose link to AT-SPI's process module moves to `crates/process`.
  - The `Application` sketch in `dev-docs/python-library-design.md`.
  - `AGENTS.md`, whose `crates/process` entry also names the process reader, and whose apps list gains `apps/win32-test-window`. The crate and apps trees of `dev-docs/architecture.md` §2 change the same way.
  - `dev-docs/testing-strategy.md` §5, which says that the new window is a helper, not a fixture of the blueprint.
  - `CONTRIBUTING.md`, for the new Windows-lane prerequisite `i686-pc-windows-msvc`.
  - The user documentation of BareMetal.
- **Build:** a native rebuild. The Windows lane additionally needs the Rust target `i686-pc-windows-msvc` for the 32-bit test window.
- **Platforms:**
  - Windows (UIA, JAB, Java agent) changes behaviour.
  - Linux AT-SPI changes only its process name (design D2).
  - The Java agent on Linux follows once the Java provider runs there (design D4, Risks).
  - macOS has no process attributes today (the provider is a stub) and implements against this contract when it gets them.
- **Coordination.** This change comes first among the Java changes (maintainer, 2026-09-28).
  - `snapshot-validity` is implemented (`8f6fc02`, `94f5f97`, `0c2d805`, `23ea907`, `aebf956`; recorded in `1f09f50`; not archived). It created `crates/process` and gave the application nodes of UIA, JAB and the Java agent a recorded `ProcessIdentity` and an `is_valid`. This change reads through those identities. Its Linux-host run (its tasks 2.1 and 9.3) is open and does not block this change.
  - `jab-discovery-containment` edits the same JAB code, and whichever lands second rebases:
    - `enumerate_visible_top_level_windows` (`WindowCandidate`'s pid);
    - `awt_windows_without_bridge`;
    - the hit-test's `top_level_window_at` and orphan application node.

    If this change lands first, the containment's pure discovery pass takes candidates whose process ID is already optional.
  - `fix-jab-hit-test-virtual-children` rewrites `hit_test_node`'s fallback. This change edits `top_level_window_at`, which decides whether `hit_test_node` runs; `hit_test_node` itself is unchanged.
  - `gate-uia-window-patterns` passes a window manager to UIA's `ApplicationNode`, `AppWindowIter` and `attach_ancestor_chain`, which this change edits too. Both also edit `crates/platform-windows/src/window_manager.rs` and the `test-acceptance-windows` recipe.
  - `xpath-document-order` changes `AgentAppNode::children`; this change edits its attributes and module documentation. They are separate hunks, but the changes land one after the other.
  - `java-agent-tree-items` changes the agent (`java/agent`). It can carry the deferred removal of the process fields from `ProcessFacts.java` (design D4).
  - `verify-display-scaling` also adds lane wiring and live tests to `live_fixture.rs`; this needs a merge only.
  - `java-provider-linux` makes the agent compile on Linux. It does not yet add the PID-namespace guard that the agent's application node needs there before it reads the process table (design, Risks).
