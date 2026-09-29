# Tasks

No native rebuild is needed. The change touches the Inspector only, and the recipes below rebuild its binaries (design, Migration Plan). The Robot Framework library is not touched, so there is no mock suite. The Windows checks need a release build on real Windows: the Windows 11 VM counts, Wine does not. A debug build is a console program and cannot take the release path.

## 1. Before the change

- [ ] 1.1 On real Windows, with a release build of the base commit, record the baseline. Build it with `just release=true build-inspector`, which installs `.venv\Scripts\platynui-inspector.exe`:
  - `platynui-inspector --help` in Command Prompt prints nothing.
  - Set `WGPU_BACKEND=metal`, then run a `.cmd` script that starts `platynui-inspector` and echoes `exit=%ERRORLEVEL%`. Command Prompt waits for a GUI program inside a script. The script shows `exit=1` and nothing else, and neither a window nor a dialog appears.

  This confirms the silent loss the review describes, and that `WGPU_BACKEND=metal` makes the start fail (design, Context, *Assumed*). If the start does not fail, find a failure that does (design, Risks), use it in 7.3 and 7.4, and write it down here. This task can run whenever a Windows machine is at hand, as long as it uses a build of the base commit. Record the results here.

## 2. Tests first: unit tests on every platform

- [ ] 2.1 Add the report tests to the new report module (design decisions 2, 4 and 5):
  - A `Runtime` error made from `ProviderError::InitializationFailed { provider: "runtime", details: Some(…) }` reads `Error: cannot create the PlatynUI runtime: provider initialization failed for runtime: …`.
  - A `Ui` error made from `eframe::Error::AppCreation(…)` reads `Error: ` followed by eframe's text.
  - A `Usage` error from `InspectorArgs::try_parse_from([…, "--log-level", "verbose"])` renders clap's message. That message names `verbose` and does not contain a second `error:` prefix.
  - The exit codes are 1 for `Runtime`, 1 for `Ui` and 2 for `Usage`.

  Verify with `just test-crate platynui-inspector` that the tests fail to compile before 4.1 and 4.2.
- [ ] 2.2 Add the decision and dialog tests (design decisions 6 and 7; spec scenario *The dialog is chosen only when nothing else can carry the report*):
  - The attach decision keeps a standard error that is a file or a pipe. It attaches for a character device, for a missing handle and for an invalid one.
  - The route sends the report to the line for a given standard error, for the process's own console, and for an attached parent console. It sends the report to the dialog after the process has detached, and when there is no console.
  - The state a Linux or macOS process reports is "standard error as given", and its route is the line.
  - The dialog's title is `PlatynUI Inspector`. Its body is the reason without the `Error: ` prefix, for a usage error clap's plain message, followed by a paragraph on how to see the diagnostics.

  These are pure functions, so they run on every platform. Verify with `just test-crate platynui-inspector` that the tests fail to compile before 5.2 and 5.3.

## 3. Tests first: Linux integration tests

- [ ] 3.1 Add `apps/inspector/tests/fatal_errors.rs`. It runs the built binary, `env!("CARGO_BIN_EXE_platynui-inspector-rs")`, in a controlled environment:
  - `RUST_LOG` and `PLATYNUI_LOG_LEVEL` are removed;
  - `PLATYNUI_INSPECTOR_SETTINGS_PATH` points into a temporary directory;
  - the display variables are set for each test.

  Mark the tests Linux-only, and allow `unused_crate_dependencies` as the bin target does (`apps/inspector/src/main.rs:5-6`). The spec scenarios become these tests:
  - **Runtime failure** (*A Linux session whose X server does not answer*). With `XDG_SESSION_TYPE=x11`, a `DISPLAY` that names no X server such as `:987`, and no `WAYLAND_DISPLAY`:
    - the exit code is 1;
    - exactly one line starts with `Error: cannot create the PlatynUI runtime: `;
    - the reason after that prefix appears on no other line;
    - no line contains `panicked`.
  - **Window failure** (*A window that cannot be opened is reported in one line*). With `DISPLAY`, `WAYLAND_DISPLAY` and `XDG_SESSION_TYPE` removed:
    - the exit code is 1;
    - the last line starts with `Error: ` and continues with text;
    - no line starts with `Inspector exited with error:`.
  - **Usage error** (*A command line the Inspector cannot use*). With `--log-level verbose`:
    - the exit code is 2;
    - standard error names `verbose` and lists the level names, starting with `off, error`.

  Verify with `just test-crate platynui-inspector` against today's code:
  - the runtime failure test fails, with exit code 101 and a panic;
  - the window failure test fails, on the old prefix;
  - the usage error test passes already and guards the route from now on.

## 4. The returned failure and the report

- [ ] 4.1 Make the failures values in `apps/inspector/src/lib.rs` (design decisions 2, 3 and 5):
  - Add `InspectorError` with the variants `Usage`, `Runtime` and `Ui`, its `Display`, its `source()` and its exit code.
  - Add `create_runtime() -> Result<Arc<Runtime>, ProviderError>`. Call it where `Runtime::new().expect(…)` stands today (`:1150`), before `run_native` as now. It prints nothing and never exits.
  - Move today's body of `run()` into `try_run() -> Result<(), InspectorError>`:
    - read the arguments with `InspectorArgs::try_parse()`;
    - print help and version output as clap does today, and end with success;
    - map a failure of `run_native` to `Ui`.

  Verify with `just test-crate platynui-inspector` that the crate builds and that the tests of 2.1 for the text and exit code of `InspectorError` pass. `just clippy` follows in 4.2, once the report uses everything.
- [ ] 4.2 Add the report (design decisions 1 and 4):
  - Make `pub fn run()` return `std::process::ExitCode`. It calls `try_run`, reports a failure itself and returns the exit code.
  - A usage error is printed by clap. Every other failure is written as `writeln!(stderr, "Error: {error}")`, and a failed write is ignored.
  - Replace `# Errors` and `# Panics` in `run()`'s documentation with what it now does.
  - Reduce both `main` functions, `apps/inspector/src/main.rs` and `packages/inspector/src/main.rs`, to `platynui_inspector::run()`, and keep their `windows_subsystem` attribute.
  - Add the new module(s) to the module tree in `lib.rs`'s crate documentation.

  Verify that 2.1 and 3.1 pass with `just test-crate platynui-inspector`, and that `just clippy` is clean.

## 5. Windows: the console and the dialog

- [ ] 5.1 Add the features `Win32_System_Console`, `Win32_Storage_FileSystem` and `Win32_UI_WindowsAndMessaging` to the Windows-only `windows` dependency in `apps/inspector/Cargo.toml`. Verify with `just check-windows`.
- [ ] 5.2 Implement the console handling (design decision 6):
  - **Placement.** The attach decision and the console state go into a module that is compiled on every platform. The Win32 calls go into a Windows-only part. Put `#[allow(unsafe_code)]` on the smallest possible functions, each with a comment on why the call is sound, as `apps/inspector/src/modifiers.rs:166-171` does. The Windows part is compiled on every Windows build, not only when `debug_assertions` is off.
  - **The first step of `run()`**, before `try_parse` and before `init_stderr`:
    - classify the standard error with `GetFileType`, and keep a file or a pipe;
    - otherwise remember the three standard handles and call `AttachConsole(ATTACH_PARENT_PROCESS)`;
    - record the resulting state: attached, own console, or no console.
  - **The end of the app creator** (`lib.rs:1185-1194`), after `InspectorApp::new` has returned: a process that attached first restores the remembered standard handles with `SetStdHandle`, and then calls `FreeConsole`.
  - **Linux and macOS**: the state is always "standard error as given", and nothing is called.

  Verify that the attach and route tests of 2.2 pass on Linux, and that `just clippy`, `just check-windows` and `just clippy-windows` are clean.
- [ ] 5.3 Implement the dialog (design decision 7). When the route is the dialog, show `MessageBoxW`:
  - without an owner window, with an OK button and the error icon, in the foreground;
  - with the title and body that 2.2 describes, converted to UTF-16.

  After the dialog closes, return the report's exit code. The dispatch between the line and the dialog compiles on every platform; outside Windows the dialog route cannot occur. Verify that the dialog tests of 2.2 pass, and that `just clippy` and `just clippy-windows` are clean. The behavior is checked in 7.4.

## 6. Documentation

- [ ] 6.1 Update `dev-docs/inspector.md`:
  - list the new module(s) in the architecture overview;
  - extend the *Tracing* bullet (`:77`) with where a Windows release build's diagnostics go;
  - add a troubleshooting section, *Windows: the Inspector does not appear*. It covers:
    - the dialog;
    - a start from a terminal, which shows the output of the start only;
    - output that appears after the shell's prompt;
    - a `.cmd` script or `start /wait` to see the exit code;
    - `2> inspector.log` to keep the whole run;
    - the exit codes 1 and 2.

  Verify by reading it against the spec.
- [ ] 6.2 Update the logging concept:
  - in `dev-docs/logging.md` §2, under *Native diagnostics*, add a short paragraph on where a Windows release build of the Inspector writes: to the terminal it was started from while it starts, to a standard error it was given, or else nowhere, with a failure that ends it shown in a dialog;
  - in `.github/instructions/logging.instructions.md` §6, add one line: an entry point reports the failure that ends it once, as `Error: <reason>`, and does not log it as well.

  Verify by reading both against `dev-docs/logging.md` §4.
- [ ] 6.3 In `packages/inspector/README.md`, add a note to *Notes*: on Windows, a start failure is shown in a dialog. Starting the Inspector from a terminal shows its startup diagnostics, and `platynui-inspector --log-level debug 2> inspector.log` keeps all of them.

  Verify with `rg -n "Inspector exited with error|Failed to create PlatynUI runtime"` over `apps`, `packages`, `dev-docs`, `docs`, `.github` and `README.md`: the only match is the negative assertion in `apps/inspector/tests/fatal_errors.rs`.

## 7. Verification

- [ ] 7.1 On Linux, run `just check`, then `just test-crate platynui-inspector`, then `just check-windows` and `just clippy-windows`, and verify that all are clean. Then check the spec scenario *Both binaries report alike*:
  - `just build-inspector` installs a debug build of the wheel's binary as `.venv/bin/platynui-inspector`. It runs a plain `uv run`, which can replace the native build in the environment; the lane recipes of 7.2 rebuild it through `build-native`;
  - run it and `target/debug/platynui-inspector-rs` with `DISPLAY`, `WAYLAND_DISPLAY` and `XDG_SESSION_TYPE` removed;
  - compare their exit codes and last lines.

  Record the result here.
- [ ] 7.2 Run the Inspector suites on the Linux lanes, where the debug build keeps the standard error the suite redirects:
  - `just headless=true test-acceptance-x11 --suite '*.Egui.Inspector*'`;
  - `just headless=true test-acceptance-compositor --suite '*.Egui.Inspector*'`.

  Judge each run with `uv run --no-sync robotcode results summary --failed` and `uv run --no-sync robotcode results log --level WARN --execution-messages`. Verify that both lanes are green and have no WARN or ERROR from PlatynUI. Record the result here.
- [ ] 7.3 On real Windows, from a terminal, with a release build from `just release=true build-inspector`. Run each case in Command Prompt, and in PowerShell under Windows Terminal. Get exit codes from a `.cmd` script that echoes `%ERRORLEVEL%` after the command:
  - `platynui-inspector --help`: the help appears in the terminal, with no window and no dialog.
  - `platynui-inspector --log-level verbose`: the usage message naming `verbose` appears, `exit=2`, no dialog.
  - With `WGPU_BACKEND=metal`, or the failure from 1.1: a line starting with `Error: WGPU error:` appears, `exit=1`, no dialog.
  - With `PLATYNUI_INSPECTOR_RENDERER=vulkan`: the warning that the value is ignored appears, and the window opens.
  - With the window up, press Ctrl+C in the terminal, then close the terminal: the Inspector keeps running and responds.
  - `platynui-inspector 2> inspector.log`:
    - with `PLATYNUI_INSPECTOR_RENDERER=vulkan`, the warning is in the file and not in the terminal;
    - with `WGPU_BACKEND=metal`, the file's last line starts with `Error: WGPU error:`, and no dialog appears.
  - `uv run --no-sync platynui-inspector --help` behaves like a start from the terminal.

  Record each result here. Keep this task open until it has run on real Windows.
- [ ] 7.4 On real Windows, without a terminal, with the same build:
  - From the Run dialog (Win+R), start the full path of `platynui-inspector.exe` with `--log-level verbose`, and then from a shortcut with the same argument. A dialog titled `PlatynUI Inspector` shows the usage message naming `verbose`. After OK, no `platynui-inspector` process is left (`Get-Process`).
  - Set the user environment variable `WGPU_BACKEND=metal`, for example with `setx`, and remove it afterwards. Then start from the Run dialog. The dialog shows the WGPU error's text and the hint on how to see the diagnostics.
  - Without either, start from the Run dialog. The window appears, with no console window and no dialog.
  - Build `platynui-inspector-rs` once with `cargo build --release -p platynui-inspector`, because no recipe builds that binary in release. Started from a terminal with `WGPU_BACKEND=metal`, it prints the same `Error:` line as the wheel's binary.

  Record each result here. Keep this task open until it has run on real Windows.
- [ ] 7.5 On Windows, run the Inspector suites in the acceptance lane: `just test-acceptance-windows --profile real-windows run --suite '*.Egui.Inspector*'`. There the debug build is a console program whose standard error the suite redirects. Judge the run with `uv run --no-sync robotcode results log --level WARN --execution-messages`. Verify that it is green and has no WARN or ERROR from PlatynUI. Record the result here.

## 8. Commit (only when the user asks)

- [ ] 8.1 Commit in reviewable steps. Each step builds, passes `just check` (the hooks lint the whole project) and passes its own tests on its own:
  - the returned runtime failure, the report and both `main` functions, with 2.1 and 3.1, for example `fix(inspector): report a failed start instead of panicking`;
  - the Windows console and dialog, with 2.2 and the `windows` features, for example `fix(inspector): show Windows start failures in terminal or dialog`;
  - the documentation, for example `docs: describe how the Inspector reports a failure that ends it`.

  Use Conventional Commits with subjects of at most 72 characters and no `!`. The commit bodies list the behavior changes from the proposal. Pushing needs a separate instruction.
