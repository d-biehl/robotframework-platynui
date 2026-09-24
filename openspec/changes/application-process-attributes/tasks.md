# Tasks

Windows-only work is verified on a Windows machine, because CI has no Windows test job. There, `just test-acceptance-windows` takes a full robotcode command, for example `just test-acceptance-windows --profile real-windows run --suite '*.Egui.ProcessAttributes'`. Its agent-served Swing suites need the agent JAR installed once with `just install-provider-java`; that is an existing lane prerequisite, not something this change adds.

## 1. Acceptance suites first

- [ ] 1.1 Add `tests/acceptance/egui/process_attributes.robot` against the default egui instance, following the `robot-test-style` skill. It runs unchanged on every real lane, because the expected values come from the platform the suite runs on and not from tags. It asserts:
  - `/app:Application[@ProcessId=<launched pid>]` selects exactly one node (spec: *The process ID stays addressable in the control namespace*).
  - `@app:ProcessName` is the test binary's file name without `.exe`.
  - `@app:ExecutablePath` names the launched binary. Compare `os.path.normcase(os.path.realpath(...))` of both sides: the lane hands the path over with `/`, Windows reports `\`, and Linux resolves symlinks.
  - `@app:CommandLine` is present and contains the binary's file name. On Windows it contains `"PlatynUI Test App"` with its quotes, which the launch passes as one argument containing spaces (*A Windows command line keeps its quoting*).
  - `@app:StartTime` matches `YYYY-MM-DDTHH:MM:SSZ` and lies within a minute of the launch (*The start time has one format on every provider*).
  - `@app:UserName` is the current account in the platform's form: `%USERDOMAIN%\%USERNAME%` on Windows, the login name on Linux (*A Windows process owned by a local account names the computer as its domain*).
  - `@app:Architecture` is `x64` on the x64 Windows lane and absent on Linux, while the five other attributes are present there (*A Linux application carries no architecture*).

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
    - Every attribute present on both nodes is equal (*Two providers report the same process identically*).

  Verify: on a Windows machine the suite fails today on the agent's namespace, process name, start time and architecture, and on JAB's user name and command line. Record that in the run notes.
- [ ] 1.3 In `tests/acceptance/egui/process_attributes.robot`, add a test tagged `platform:windows`, so that it inherits `acceptance real` from the directory. It starts `%WINDIR%\SysWOW64\charmap.exe` and asserts `@app:Architecture = x86` on its application node (*A 32-bit process on 64-bit Windows reports its own architecture*). If the binary is missing, the test fails with a message naming it; it never skips. Verify: on a Windows machine it passes today through the PE header and must stay green after 3.2 replaces that path with the OS call. It is a regression guard, not a red test.

## 2. The process reader

- [ ] 2.1 Create `crates/process` (`platynui-process`) as a workspace member with no dependency on other PlatynUI crates (design D1).
  - Linux dependencies: `sysinfo`, `libc`, `chrono`.
  - Windows dependencies: the `windows` features UIA uses today for these calls, including `Wdk_System_Threading` and `Win32_System_SystemInformation`.
  - Give every public function an answer-nothing body for now.
  - Add `--package platynui-process` to `windows_rust_packages` and `macos_rust_packages` in the `justfile`, so the cross-checks build and lint it with `--all-targets`.

  Verify: `cargo build -p platynui-process` succeeds here, and `just check-windows` and `just check-macos-arm` list and compile the crate.
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
- [ ] 2.3 Implement the platform-independent helpers (machine mapping, fallback combination rule, `.exe` stripping). Then implement the Linux reader by moving the logic of `crates/provider-atspi/src/process.rs`, with design D2's changes: the process name is the file name of the executable path, with no stem cut and no `comm` fallback, and the ` (deleted)` suffix never reaches the path. Verify: the platform-independent and Linux tests of 2.2 pass with `just test-crate platynui-process`.
- [ ] 2.4 Implement the Windows reader from the UIA code in `crates/provider-windows-uia/src/map.rs`, with design D2 and D3:
  - limited query rights;
  - the verbatim command line;
  - `DOMAIN\user`;
  - the start time truncated to the second;
  - `GetProcessInformation(ProcessMachineTypeInfo)` and the `IsWow64Process2` fallback as two separately callable paths, and no PE header.

  Verify:
  - `just check-windows`, `just clippy-windows` and `just check-macos-arm` are clean here.
  - On a Windows machine, `just test-crate platynui-process` passes.

## 3. Providers onto the reader

- [ ] 3.1 Move AT-SPI onto the reader.
  - `crates/provider-atspi/src/process.rs` goes away, `pidns_harness.rs` calls the reader for the command line, and `sysinfo`, `libc` and `chrono` leave `crates/provider-atspi/Cargo.toml` once grep finds no use.
  - The attribute path calls the reader only with the local process number, keeping the `sidecar-deployment` gate.

  Verify:
  - `just test-crate platynui-provider-atspi` passes.
  - `just test-atspi-pidns dbus-daemon` and `just test-atspi-pidns dbus-broker` pass.
  - The egui suite of 1.1 stays green on both Linux lanes.
- [ ] 3.2 Windows UIA.
  - **Tests first**, in `cfg(windows)` unit tests:
    - An application node for a process that does not exist lists no `app:` process attribute.
    - A lookup by name agrees with the listing for every attribute.
    - The hit-test's choice of scope, extracted into a pure function (`Option<i32>` → `UiaIdScope`), maps `Some(0)`, `Some(-1)` and `None` to the desktop scope.
  - **Then implement design D5 and D6**:
    - Presence is decided at enumeration.
    - Named lookup overrides `attribute()`, so it reads only the named attribute.
    - The reader replaces the `map.rs` process helpers.
    - The `""`, null and `"unknown"` answers go away.
    - The hit-test at `provider.rs:497` uses the extracted scope function.

  Verify:
  - `just check-windows` and `just clippy-windows` are clean here.
  - On a Windows machine, `just test-crate platynui-provider-windows-uia` passes, and the egui suite of 1.1 with the test of 1.3 passes.
- [ ] 3.3 JAB.
  - **Tests first**:
    - An application node for a process that does not exist lists no `app:` process attribute.
    - A lookup by name agrees with the listing.
    - The window-to-process helper answers "no process" when `GetWindowThreadProcessId` yields `0`, for example for a null window.
  - **Then implement design D5 and D6**:
    - Presence is decided at enumeration.
    - Named lookup overrides `attribute()`.
    - The reader replaces `crates/provider-java-jab/src/process.rs`.
    - The two `GetWindowThreadProcessId` calls (`provider.rs:417`, `:517`) go through the helper.
    - `sysinfo` and `chrono` leave the crate's dependencies once grep finds no use.

  Verify:
  - `just check-windows` and `just clippy-windows` are clean here.
  - On a Windows machine, `just test-crate platynui-provider-java-jab` passes, and the JAB half of the Swing suite of 1.2 passes.
- [ ] 3.4 The Java provider's agent application node (design D4, D5).
  - **Tests first**: the node's `app:` process attributes are those of the reader for the session's process ID, with none under `control:`. A lookup by name agrees with the listing, and no attribute has an empty value.
  - **Then implement**:
    - The process attributes come from the reader, under `app:`, decided at enumeration, with `attribute()` overridden.
    - `control:Name` and the `native:` JVM facts stay as they are.
    - The agent's own process fields are no longer read. The agent and its version stay untouched.

  Verify:
  - `just check-windows` and `just clippy-windows` are clean here.
  - On a Windows machine, `just test-crate platynui-provider-java` passes and the agent half of the Swing suite of 1.2 passes.

## 4. Process ID 0 in the window managers

- [ ] 4.1 Write tests first: each window manager's process-ID reader answers "no process" for a node carrying `ProcessId = 0` as `Integer(0)`, `Number(0.0)` and `String("0")` (*A window is never looked up by process ID 0*). Then make `pid_from_attr` accept only a positive number in:
  - `crates/platform-windows/src/window_manager.rs`
  - `crates/platform-linux-x11/src/window_manager.rs`
  - `crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs`

  Verify:
  - Here: `just test-crate platynui-platform-linux-x11` and `just test-crate platynui-platform-linux-wayland` pass, and `just check-windows` is clean.
  - On a Windows machine, `just test-crate platynui-platform-windows` passes.

## 5. The mock

- [ ] 5.1 Write a test first in `crates/runtime/src/runtime/evaluation.rs`, next to the existing mock-runtime evaluations (design D7, spec *Listing and predicate agree for a missing attribute*):
  - `/app:Application[@ProcessId][@app:ProcessName][not(@app:CommandLine)]` selects exactly "Mock Application".
  - Its attribute listing contains `ProcessName` but not `CommandLine`.
  - "Mock Settings" carries no `ProcessId`.

  Confirm it fails with `just test-crate platynui-runtime`. Then give "Mock Application" `control:ProcessId` and the `app:` attributes `ProcessName`, `ExecutablePath`, `UserName`, `StartTime` in the spec's formats, deliberately without `CommandLine`.

  Verify: `just test-crate platynui-runtime` passes, and `just test`, `just test-python` and `just test-baremetal` stay green. The mock tree feeds many tests.

## 6. Documentation

- [ ] 6.1 Update the docs as design D8 describes. They point to the spec for formats and add no status content.
  - `dev-docs/architecture.md`:
    - The §2 crate tree gains `process`.
    - The Application row of the pattern catalog: `ProcessId` optional, all six `app:` attributes listed, and no "executable stem" note.
    - The per-platform source table: D2 and D3 sources, and no `comm`, `cmdline[0]`, PE header or ELF.
  - `dev-docs/platform-windows.md`: the UIA application-node attributes and the JAB section's process metadata (`:126`).
  - `dev-docs/planning.md`: the parity item resolved, and `:122` and `:461` no longer describe a `comm` or stem source.
  - `dev-docs/platform-linux-wayland.md:654`: the link to `crates/provider-atspi/src/process.rs` points to `crates/process`.
  - `AGENTS.md`: the crate list gains `crates/process`.
  - `crates/core/src/ui/attributes.rs`: a one-line pointer to the capability at the `application` constants.

  Verify: `grep -rn -e 'stem' -e 'comm' -e 'GetNativeSystemInfo' -e 'PE header' -e 'ELF' -e 'provider-atspi/src/process.rs' dev-docs AGENTS.md` shows no stale source claim, and no doc restates a format that the spec does not fix.

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
- [ ] 7.2 On a Windows machine, run `just test` and `just test-acceptance-windows`, which cover the egui suite, the Swing suite and the 32-bit test. CI has no Windows test job, so this is the only check of the Windows half. Verify: green, with the results recorded in the change notes.
- [ ] 7.3 When the maintainer asks for it, commit as `fix(provider)!: report process attributes by one contract` (Conventional Commits, the repo's existing singular scope, subject ≤ 72 characters). The body names the new reader and the four aligned providers. A `BREAKING CHANGE:` footer lists the proposal's shape changes, including AT-SPI's process name. Verify: `git log -1 --format=%B` shows the subject and the footer, and `just pre-commit` passed before the commit.
