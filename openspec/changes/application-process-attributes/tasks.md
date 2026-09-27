# Tasks

Windows-only work is verified on a Windows machine, because CI has no Windows test job. There, `just test-acceptance-windows` takes a full robotcode command, for example `just test-acceptance-windows --profile real-windows run --suite '*.Egui.ProcessAttributes'`. Its agent-served Swing suites need the agent JAR installed once with `just install-provider-java`; that is an existing lane prerequisite, not something this change adds. This change adds one: the Rust target `i686-pc-windows-msvc`, for the 32-bit test window of 1.3.

`snapshot-validity` should land first. It creates `crates/process` with a process identity and gives the same three application nodes an `is_valid`. The tasks below say where they then extend instead of create.

## 1. Acceptance suites first

- [ ] 1.1 Add `tests/acceptance/egui/process_attributes.robot` against the default egui instance, following the `robot-test-style` skill. It runs unchanged on every real lane, because the expected values come from the platform the suite runs on and not from tags. It asserts:
  - `/app:Application[@ProcessId=<launched pid>]` selects exactly one node (spec: *The process ID stays addressable in the control namespace*).
  - `@app:ProcessName` is the test binary's file name without `.exe`. On Windows, `@Name` is the same value (*An application named after its program carries its process name*).
  - `@app:ExecutablePath` names the launched binary. Compare `os.path.normcase(os.path.realpath(...))` of both sides: the lane hands the path over with `/`, Windows reports `\`, and Linux resolves symlinks.
  - `@app:CommandLine` is present and contains the binary's file name. On Windows it contains `"PlatynUI Test App"` with its quotes, which the launch passes as one argument containing spaces (*A Windows command line keeps its quoting*).
  - `@app:StartTime` matches `YYYY-MM-DDTHH:MM:SSZ` and lies within a minute of the launch (*The start time has one format on every provider*).
  - `@app:UserName` is the current account in the platform's form: `%USERDOMAIN%\%USERNAME%` on Windows, the login name on Linux (*A Windows process owned by a local account names the computer as its domain*).
  - `@app:Architecture` is `x64` on the x64 Windows lane and absent on Linux, while the five other attributes are present there (*A Linux application carries no architecture*).
  - `BM.Get Attribute    <application>    app:ProcessName` returns the same value as the XPath read.

  Verify:
  - On both Linux lanes the suite passes before any code change, as a regression guard: `just headless=true test-acceptance-x11 --suite '*.Egui.ProcessAttributes'` and the same with `test-acceptance-compositor`.
  - On a Windows machine it fails today on the start time's milliseconds. Record that in the run notes.
- [ ] 1.2 Add `tests/acceptance/swing/process_attributes.robot` for the Windows lane, following the `robot-test-style` skill. First extend `Start Swing Fixture Process` (`tests/acceptance/swing/resources/swing_env.resource`) so that it forwards `Start Process` options such as `env:JAVA_TOOL_OPTIONS=...`. The suite launches under a title with spaces, e.g. `Swing Process Attributes`, and asserts:
  - **The agent-served node, which the default import sees**:
    - `@app:ProcessName` is the launcher's file name without `.exe`.
    - `@app:ExecutablePath` names the launcher, compared normalized as in 1.1.
    - `@ProcessName` is absent.
    - `@app:StartTime` has the fixed format, `@app:UserName` is `%USERDOMAIN%\%USERNAME%`, and `@app:Architecture` is `x64`.

    This covers *A Java application served by the in-JVM agent carries its process attributes under app* and *A Java application reports its process, not its main class*.
  - **A second instance launched under `JAVA_TOOL_OPTIONS=-Duser.name=someone-else`**: `@app:UserName` still names the current account (*An application's self-description does not replace a process attribute*).
  - **The JAB-served node of the same process**, seen through a second BareMetal import with `providers.java.agent.enabled: False`, following `tests/acceptance/swing/dedup.robot`:
    - `@app:UserName` is `%USERDOMAIN%\%USERNAME%`, `@app:CommandLine` contains the quoted title, and `@app:StartTime` has the fixed format.
    - `@Name` equals `@app:ProcessName`.
    - Every attribute present on both nodes is equal (*Two providers report the same process identically*).

  Verify: on a Windows machine the suite fails today on the agent's namespace, process name, start time and architecture, and on JAB's user name and command line. Record that in the run notes.
- [ ] 1.3 Add `apps/win32-test-window` (package `platynui-win32-test-window`), the 32-bit process for the architecture scenario (design D11). It is a helper for process-level tests, not a fixture of the blueprint (`dev-docs/testing-strategy.md` §5).
  - It shows one top-level window of the predefined `STATIC` class, titled by `--title`, without activating it (`SW_SHOWNOACTIVATE`). It exits by itself after `--auto-close <seconds>` (default 60), so that a failed teardown leaves no process behind. The model is the UIA test window `test_window_child` in `crates/provider-windows-uia/src/node.rs`.
  - Its bitness follows the build target. On other platforms `main` says that it runs only on Windows and exits non-zero, as `apps/eis-test-client` does off Linux, so the workspace still builds everywhere.
  - `[lints] workspace = true`. The Win32 calls carry a scoped `#![allow(unsafe_code)]` with a reason and SAFETY comments.
  - A Windows-only `justfile` recipe `build-win32-test-window-x86` builds it with `--target i686-pc-windows-msvc`. `test-acceptance-windows` runs that recipe as a hard prerequisite and hands the binary's path over in `PLATYNUI_WIN32_TEST_WINDOW_X86`. The package joins `windows_rust_packages`.
  - `CONTRIBUTING.md` names the new Windows-lane prerequisite: `rustup target add i686-pc-windows-msvc`, and the MSVC x86 libraries, which the x64/x86 build tools of Visual Studio's C++ workload bring.

  Verify:
  - On a Windows machine, `just build-win32-test-window-x86` builds, and the binary's PE header names the machine `I386` (`dumpbin /headers` shows `14C machine (x86)`).
  - Started with `--title "Win32 Test Window" --auto-close 5`, it shows the window and exits by itself.
  - `just check` is clean on Windows and on Linux, and on Linux `just check-windows` compiles the package.
- [ ] 1.4 Add `tests/acceptance/win32/__init__.robot`, tagged `acceptance`, `real` and `platform:windows`, and `tests/acceptance/win32/process_attributes.robot`, following the `robot-test-style` skill.
  - The suite checks its prerequisite first. When `PLATYNUI_WIN32_TEST_WINDOW_X86` is unset or names no file, it fails with a message naming `just build-win32-test-window-x86`. It never skips.
  - It starts the window under a suite-unique title, pins `/app:Application[@ProcessId=<pid>]` as its root, and asserts `@app:Architecture = x86` (*A 32-bit process on 64-bit Windows reports its own architecture*).
  - Its teardown terminates the process.

  Verify: on a Windows machine it passes today through the PE header and must stay green after 3.2 replaces that path with the OS call. It is a regression guard, not a red test.

## 2. The process reader

- [ ] 2.1 Create `crates/process` (`platynui-process`) as a workspace member with no dependency on other PlatynUI crates, or extend it if `snapshot-validity` has created it (design D1).
  - `[lints] workspace = true`. The Win32 and `getpwuid_r` modules carry a scoped `#![allow(unsafe_code)]` with a reason and SAFETY comments.
  - Dependencies gated per target: on Linux `sysinfo`, `libc`, `chrono`; on Windows the `windows` features UIA uses today for these calls, including `Win32_System_Threading`, `Wdk_System_Threading` and `Win32_System_SystemInformation`.
  - Give every public function of the reader an answer-nothing body for now.
  - If the crate is new, add `--package platynui-process` to `windows_rust_packages` and `macos_rust_packages` in the `justfile`, so the cross-checks build and lint it with `--all-targets`.

  Verify: `cargo build -p platynui-process` succeeds, `just check` is clean, and on Linux `just check-windows` and `just check-macos-arm` list and compile the crate.
- [ ] 2.2 Write the reader's unit tests first.
  - **Platform-independent**:
    - The machine mapping: `I386`→`x86`, `AMD64`→`x64`, `ARM` and `ARMNT`→`arm`, `ARM64`→`arm64`, anything else → nothing.
    - The `IsWow64Process2` combination rule as a pure function over the raw values (process machine, native machine): (`UNKNOWN`, `AMD64`)→`x64`, (`I386`, `AMD64`)→`x86`, (`UNKNOWN`, `ARM64`)→`arm64`, (`I386`, `ARM64`)→`x86`, (`ARMNT`, `ARM64`)→`arm`.
    - The `.exe` stripping: case-insensitive, only a trailing `.exe`.
  - **Linux, own process**:
    - The process name is the executable's file name, and the executable path is `current_exe`.
    - The command line contains the test binary's arguments joined by spaces.
    - The user name is the login of the effective UID.
    - The start time has the fixed format and lies within the last minute.
    - The architecture is absent.
  - **Linux, a child started from a copied binary named `probe.v2`**: the process name is `probe.v2`. After the binary is deleted, the path and the name carry no ` (deleted)`.
  - **Linux, partially readable processes**, holding whether or not the test runs as root:
    - For PID 1, `ProcessName` is present exactly when `ExecutablePath` is, it equals that path's file name, and it is never PID 1's `comm`. `StartTime` is present.
    - For a kernel thread, where one is visible, `CommandLine` is absent.
  - **Linux, no such process and process ID `0`**: every function answers nothing.
  - **`cfg(windows)`, own process**:
    - The process name has no `.exe`, and the executable path is `current_exe`.
    - The command line contains the test binary's path verbatim.
    - The user name is `%USERDOMAIN%\%USERNAME%`.
    - The start time has the fixed format.
    - The architecture is the build target's, through the primary call and also through the fallback path called directly.
  - **`cfg(windows)`, the System process (PID 4)**: no attribute has an empty value, and each is present or absent on its own.
  - **`cfg(windows)`, no such process and process ID `0`**: every function answers nothing.

  Verify: `just test-crate platynui-process` fails against the stubs.
- [ ] 2.3 Implement the platform-independent helpers (machine mapping, fallback combination rule, `.exe` stripping). Then implement the Linux reader by moving the logic of `crates/provider-atspi/src/process.rs`, with design D2's changes:
  - the process name is the file name of the executable path, with no stem cut and no `comm` fallback;
  - the ` (deleted)` suffix never reaches the path.

  The start time may reuse the identity's reading of `/proc/<pid>/stat` if the crate has it (design D1). Verify: the platform-independent and Linux tests of 2.2 pass with `just test-crate platynui-process`.
- [ ] 2.4 Implement the Windows reader from the UIA code in `crates/provider-windows-uia/src/map.rs:271-507`, with design D2 and D3:
  - limited query rights;
  - the verbatim command line;
  - `DOMAIN\user`;
  - the start time truncated to the second;
  - `GetProcessInformation(ProcessMachineTypeInfo)` and the `IsWow64Process2` fallback as two separately callable paths, and no PE header.

  Leave behind the comments that no longer hold (`map.rs:273` on module queries, `:324-325` "not implemented", `:328` on full rights). Verify:
  - on a Windows machine, `just check` is clean and `just test-crate platynui-process` passes;
  - on Linux, `just check-windows`, `just clippy-windows` and `just check-macos-arm` are clean.

## 3. Providers onto the reader

On each of the three application nodes of 3.2 to 3.4: when `snapshot-validity` has given the node a recorded process identity, a read on a node whose process has ended answers nothing (design D5).

- [ ] 3.1 Move AT-SPI onto the reader.
  - `crates/provider-atspi/src/process.rs` goes away, and `pidns_harness.rs` calls the reader for the command line. `sysinfo` and `chrono` leave the regular dependencies of `crates/provider-atspi/Cargo.toml`, and `libc` its Linux-gated dependencies, once grep finds no other use (`unused_crate_dependencies` reports what is left over).
  - The attribute path calls the reader only with the local process number, keeping the `sidecar-deployment` gate.

  Verify:
  - `just test-crate platynui-provider-atspi` passes.
  - `just test-atspi-pidns dbus-daemon` and `just test-atspi-pidns dbus-broker` pass.
  - The egui suite of 1.1 stays green on both Linux lanes.
- [ ] 3.2 Windows UIA.
  - **Tests first**, in `cfg(windows)` unit tests next to `application_node_carries_the_common_attributes` (`crates/provider-windows-uia/src/node.rs:2112-2128`):
    - An application node for a process that does not exist lists no `app:` process attribute.
    - A lookup by name agrees with the listing for every attribute, following `gated_attributes_agree_in_both_directions` (`:2066-2076`).
    - An application node for the own process has `control:Name` and `control:Id` equal to `app:ProcessName` (design D9).
    - The hit-test's choice of scope, extracted into a pure function (`Option<i32>` → `UiaIdScope`), maps `Some(0)`, `Some(-1)` and `None` to the desktop scope.
    - The per-application window filter answers nothing for a target process ID of `0`.
  - **Then implement design D5, D6 and D9**:
    - Presence is decided at enumeration.
    - Named lookup overrides `attribute()`, so it reads only the named attribute.
    - The reader replaces the `map.rs` process helpers.
    - `control:Name` and `control:Id` come from the reader's `ProcessName`.
    - The `""`, null and `"unknown"` answers go away.
    - The hit-test (`provider.rs:528-537`) uses the extracted scope function.
    - The window filter (`provider.rs:252-258`) rejects `0`.
    - The `windows` features only the moved helpers used (`Win32_Security`, `Win32_System_Time`, `Win32_System_SystemInformation`, `Wdk_System_Threading`, and the unused `Win32_System_ProcessStatus`) leave the crate once grep finds no use. `Win32_System_Threading` stays for `node.rs`.

  Verify: on a Windows machine, `just check` is clean, `just test-crate platynui-provider-windows-uia` passes, and the egui suite of 1.1 and the win32 suite of 1.4 pass.
- [ ] 3.3 JAB.
  - **Tests first**:
    - An application node for a process that does not exist lists no `app:` process attribute.
    - A lookup by name agrees with the listing.
    - The application node's name equals `app:ProcessName` (design D9).
    - `process_id_of` (`crates/provider-java-jab/src/node.rs:931-942`) answers "no process" when `GetWindowThreadProcessId` yields `0`, for example for a null window.
    - The pass without the Access Bridge DLL lists no window whose process is `0` among the Java processes or the unserved windows.
    - The hit-test for a window without a process uses the desktop scope instead of building `JabAppNode::orphan(0)` (`node.rs:759-773`).
  - **Then implement design D5, D6 and D9**:
    - Presence is decided at enumeration.
    - Named lookup overrides `attribute()`.
    - The reader replaces `crates/provider-java-jab/src/process.rs`, including its `"unknown"` and its image-name fallback.
    - The name (`node.rs:1388-1390`) comes from the reader's `ProcessName`.
    - The two direct `GetWindowThreadProcessId` calls (`provider.rs:530`, `:631`) go through `process_id_of`.
    - `sysinfo` and `chrono` leave the crate's dependencies once grep finds no use.

  Verify: on a Windows machine, `just check` is clean, `just test-crate platynui-provider-java-jab` passes, and the JAB half of the Swing suite of 1.2 passes.
- [ ] 3.4 The Java provider's agent application node (design D4, D5).
  - **Tests first**: the node's `app:` process attributes are those of the reader for the session's process ID, with none under `control:`. A lookup by name agrees with the listing, and no attribute has an empty value.
  - **Then implement**:
    - The process attributes come from the reader, under `app:`, decided at enumeration, with `attribute()` overridden.
    - `control:Name` and the `native:` JVM facts stay as they are.
    - The agent's own process fields are no longer read. The agent and its version stay untouched.
    - The dependency on `platynui-process` sits under `cfg(windows)`, like the crate's other dependencies.
    - The module documentation of `app.rs` (`:8-12`) no longer says that the JVM's self-description replaces a process query.

  Verify: on a Windows machine, `just check` is clean, `just test-crate platynui-provider-java` passes, and the agent half of the Swing suite of 1.2 passes.

## 4. Process ID 0 in the window managers

- [ ] 4.1 Write tests first: each window manager's process-ID reader answers "no process" for a node carrying `ProcessId = 0` as `Integer(0)`, `Number(0.0)` and `String("0")` (*A window is never looked up by process ID 0*). Then make `pid_from_attr` accept only a positive number in:
  - `crates/platform-windows/src/window_manager.rs:88-99`, whose module documentation (`:13`) is corrected to `control:ProcessId`;
  - `crates/platform-linux-x11/src/window_manager.rs:411-423`;
  - `crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs:450`.

  On Linux this implements what `sidecar-deployment` already requires (`openspec/specs/sidecar-deployment/spec.md:259`). Verify:
  - On Linux, `just test-crate platynui-platform-linux-x11` and `just test-crate platynui-platform-linux-wayland` pass.
  - On a Windows machine, `just test-crate platynui-platform-windows` passes.

## 5. The mock and the Python `Application`

- [ ] 5.1 Write a test first in `crates/runtime/src/runtime/evaluation.rs`, next to the existing mock-runtime evaluations (design D7, spec *Listing and predicate agree for a missing attribute*):
  - `/app:Application[@ProcessId][@app:ProcessName][not(@app:CommandLine)]` selects exactly "Mock Application".
  - Its attribute listing contains `ProcessName` but not `CommandLine`.
  - "Mock Settings" carries no `ProcessId`.

  Confirm it fails with `just test-crate platynui-runtime`. Then give "Mock Application" `control:ProcessId = 4242`, the process ID its window already carries (`crates/provider-mock/assets/mock_tree.xml:7`), and the `app:` attributes `ProcessName`, `ExecutablePath`, `UserName`, `StartTime` in the spec's formats, deliberately without `CommandLine`.

  Verify: `just test-crate platynui-runtime` passes, and `just test`, `just test-python` and `just test-baremetal` stay green. The mock tree feeds many tests.
- [ ] 5.2 The Python `Application` (design D10, spec *The Python Application object reads the process attributes*).
  - **Tests first**:
    - Update `tests/PlatynUI/test_application.py`: `process_id` reads `control:ProcessId`, and `process_name` reads `app:ProcessName`. Both return `None` when the attribute is absent, and raise `TypeError` only for a present value of the wrong type.
    - Add a pytest against the mock runtime: "Mock Application" gives `process_id == 4242` and its process name, and "Mock Settings" gives `None` for both.
  - **Then implement** in `src/PlatynUI/ui/application.py:31-49`, with the return types `int | None` and `str | None`.

  Verify: `just test-python` passes and `just mypy` is clean.

## 6. Documentation

- [ ] 6.1 Update the docs as design D8 describes. They point to the spec for formats and add no status content.
  - `dev-docs/architecture.md`:
    - The §2 crate tree gains `process`, unless `snapshot-validity` has added it.
    - The Application row of the pattern catalog (`:361`): `ProcessId` optional, all six `app:` attributes listed, and no "executable stem" note.
    - The per-platform source table (`:504-509`): D2 and D3 sources, and no `comm`, `cmdline[0]`, PE header or ELF.
    - The Windows UIA checklist (`:573`): `ProcessName` instead of `Name`, and the presence rule.
  - `dev-docs/platform-windows.md`: the UIA application-node attributes (`:77`) and the JAB section's process metadata (`:126`).
  - `dev-docs/platform-linux.md:130-159`: the AT-SPI process name follows D2, and the presence rule points to the capability.
  - `dev-docs/planning.md`: the parity item is resolved, and `:122`, `:445` and `:461` no longer describe a `comm` or stem source. The checked history items stay as they are.
  - `dev-docs/platform-linux-wayland.md:654`: the link to `crates/provider-atspi/src/process.rs` points to `crates/process`.
  - `dev-docs/python-library-design.md:4394-4399`: the `Application` sketch reads `control:ProcessId` and may answer `None`.
  - `AGENTS.md`: the crate list gains `crates/process`, unless `snapshot-validity` has added it, and the apps list gains `apps/win32-test-window`.
  - `dev-docs/testing-strategy.md` §5: one line saying that `apps/win32-test-window` is a helper for process-level tests, not a fixture of the blueprint.
  - `crates/core/src/ui/attributes.rs`: a one-line pointer to the capability at the `application` constants.
  - **User documentation**, in the user-facing voice:
    - BareMetal's library documentation gains a short section on the `app:` attributes: what each is, that each may be absent, how to test for one with `[@app:X]`, and their formats.
    - The documentation of `Get Attribute` (`src/PlatynUI/BareMetal/__init__.py:2391-2399`) names the `app:` prefix next to `native:`.

  Verify:
  - `grep -rn -e '\bcomm\b' -e 'cmdline\[0\]' -e 'executable stem' -e 'GetNativeSystemInfo' -e 'PE header' -e '\bELF\b' -e 'provider-atspi/src/process.rs' dev-docs AGENTS.md` shows no stale source claim, apart from historical entries in `dev-docs/python-migration-status.md`.
  - No doc restates a format that the spec does not fix.
  - The rendered libdoc of `PlatynUI.BareMetal` shows the new section.

## 7. Verification and commit

- [ ] 7.1 Run the full gate on Linux:
  1. `just check` and `just test`.
  2. `just test-python` and `just test-baremetal`, which build the mock native module.
  3. `just build-native`, to rebuild the real native module before the real lanes.
  4. `just headless=true test-acceptance-x11`, then `uv run --no-sync robotcode results summary --failed` at once, because the next lane overwrites `results/output.xml`.
  5. The same for `just headless=true test-acceptance-compositor`.
  6. `just test-atspi-pidns dbus-daemon` and `just test-atspi-pidns dbus-broker`.
  7. `just check-windows`, `just clippy-windows` and `just check-macos-arm`.

  Verify: all green.
- [ ] 7.2 On a Windows machine, run `just check`, `just test` and `just test-acceptance-windows`, which cover the egui suite, the Swing suite and the win32 suite with its 32-bit process. Verify: green, with the results recorded in the change notes.
- [ ] 7.3 When the maintainer asks for it, commit as `fix(provider): report process attributes by one contract` (Conventional Commits, the repo's existing singular scope, subject ≤ 72 characters).
  - The body names the reader and the four aligned providers, and lists the proposal's behavior changes for the changelog, including AT-SPI's process name and the Python `Application`.
  - It carries no `!` and no `BREAKING CHANGE:` footer: PlatynUI is at 0.x (maintainer decision).

  Verify: `git log -1 --format=%B` shows the subject and the list, and `just pre-commit` passed before the commit.
