# Design

## Context

See proposal.md for the motivation, and `specs/inspector-fatal-errors/spec.md` for the behavior. The facts below were read at `7180d20b`. The Linux observations come from the debug binary `target/debug/platynui-inspector-rs`, built from the sources of that commit. *Assumed* marks what was not run, which is everything that needs real Windows.

**The entry points today.**

- `apps/inspector/src/main.rs:4` sets `windows_subsystem = "windows"` for release builds.
  - Its `main` (`:8-12`) prints `Inspector exited with error: {error}` and calls `std::process::exit(1)`.
  - `packages/inspector/src/main.rs:1-8`, the wheel's binary, does the same but prints `Error: {error}`.
- Both call `platynui_inspector::run() -> eframe::Result` (`apps/inspector/src/lib.rs:1130`). Nothing else in the repository calls it.
- `run()` works in this order:
  - it parses the arguments with clap's `parse()`, which prints and exits by itself (`:1131`);
  - it installs the subscriber (`:1133`);
  - it resolves the settings and writes the info record `starting inspector renderer` (`:1134-1148`);
  - it calls `Runtime::new().expect(…)` (`:1150`);
  - it calls `eframe::run_native` (`:1182-1195`), whose app creator builds `InspectorApp::new` (`:628-695`). That reads the persisted settings (`:653`) and builds the view model (`:670`).
- `run()`'s documentation has `# Errors` (`:1123-1125`) and `# Panics` (`:1127-1129`).
- The command-line tool reports a failure as `Error: {error}`: colored in `crates/cli/src/main.rs:8-9`, plain in `packages/cli/src/main.rs:3`.

**A write without a console fails silently.** The Rust standard library (1.98.1) looks the standard handle up on every write (`library/std/src/sys/stdio/windows.rs:68-76`, `:101-106`). A missing handle becomes `ERROR_INVALID_HANDLE`, and `library/std/src/io/stdio.rs:203-208` turns that into a successful write (`:141`, `:167`). So in a release build without a console, `eprintln!` and the tracing subscriber write nowhere, and nothing fails. The same lookup means that a console attached later is used from the next write on.

**Failures observed on Linux.**

- With `XDG_SESSION_TYPE=x11`, `DISPLAY=:987` and no `WAYLAND_DISPLAY`, the Inspector panics:
  - the panic is at `apps/inspector/src/lib.rs:1150:34`, with `Failed to create PlatynUI runtime: InitializationFailed { provider: "runtime", details: Some("platform initialization failed for x11 connection: x11 connect: Connection refused (os error 111)") }` and a backtrace hint;
  - the exit code is 101;
  - the X11 bundle cannot connect (`crates/platform-linux-x11/src/lib.rs:81-82`), and `select_platform` returns `InitializationFailed` (`crates/runtime/src/runtime/mod.rs:422-428`).
- With none of the display variables set:
  - the runtime comes up without a platform and warns about that;
  - eframe then fails to create its event loop, and the Inspector prints `Inspector exited with error: winit EventLoopError: … neither WAYLAND_DISPLAY nor WAYLAND_SOCKET nor DISPLAY is set.` and exits with code 1.
- `--log-level verbose` prints clap's usage message and exits with code 2.

**eframe 0.36.2** (`Cargo.lock:1198-1199`):

- `eframe::Error` (`src/lib.rs:504-531`) has `AppCreation(Box<dyn Error + Send + Sync>)` (`:506`), displayed as `app creation error: {err}` (`:578`).
- `impl Error for Error {}` (`:533`) defines no `source()`, so the reason lives only in the displayed text.
- The app creator runs after the renderer has its window: with wgpu at `src/native/wgpu_integration.rs:258-260`, then `:313-334`; with glow at `src/native/glow_integration.rs:342-367`. A renderer failure therefore returns from `run_native` before the creator runs. A creator that runs means that the window and the renderer exist.
- On Windows, the Inspector's adapter selector (`apps/inspector/src/lib.rs:351-393`) fails with "no wgpu adapters are available …" or "no surface-compatible wgpu adapter found …" (`:386-392`). egui-wgpu 0.36.2 turns that into `Adapter selection failed: {0}` (`src/lib.rs:50-51`, `:238-241`), and eframe then displays it as `WGPU error: …`.

**The runtime and its error.** `Runtime::new()` returns `Result<Self, ProviderError>` (`crates/runtime/src/runtime/mod.rs:106-109`). `ProviderError` displays a fixed base text followed by its details (`crates/core/src/provider/error.rs:17-47`), and it has no `source()`.

**Logging.**

- `init_stderr` writes through `std::io::stderr` (`crates/log-filter/src/lib.rs:169-171`). Its only other writer hook, `init_with_writer`, is `#[doc(hidden)]` and meant for tests (`:173-179`).
- `RUST_LOG` wins over a requested level without a word (`:138-159`):
  - `from_rust_log` is taken first (`:145`);
  - the requested level is only consulted in `or_else` (`:146`);
  - the once-per-process `reject` channel (`:130-135`) is not used for that case.

**Constraints.**

- The workspace denies `unsafe_code` (`Cargo.toml`, `[workspace.lints.rust]`). The Inspector already calls Win32 through a narrowly scoped `#[allow(unsafe_code)]` (`apps/inspector/src/modifiers.rs:166-171`).
- The Windows-only `windows` 0.62 dependency (`apps/inspector/Cargo.toml:60`) enables two features today. windows 0.62.2 provides:
  - `AttachConsole`, `FreeConsole`, `GetStdHandle`, `SetStdHandle` and `ATTACH_PARENT_PROCESS` behind `Win32_System_Console`;
  - `GetFileType` behind `Win32_Storage_FileSystem`;
  - `MessageBoxW` and the `MB_*` styles behind `Win32_UI_WindowsAndMessaging`.
- `unused_crate_dependencies` is a workspace warning. The bin target allows it (`apps/inspector/src/main.rs:5-6`), and so does the library under test (`apps/inspector/src/lib.rs:24`).
- The acceptance suites start the debug build with its output redirected to a file (`tests/acceptance/egui/resources/inspector.resource:30-34`). On Windows, a debug build is a console program.
- CI runs `just clippy` and `just test` on Linux, and a Windows lint job (`.github/workflows/ci.yml:123-130` and the job that follows).

**Assumed (Windows; tasks 1.1, 7.3 and 7.4 check each on real Windows):**

- A GUI-subsystem program has no parent console and no standard handles when Explorer starts it: by a double-click, from the Start menu, the Run dialog or a shortcut. Started from Command Prompt or PowerShell without a redirect, it has no usable standard error either. The review describes exactly that (its entry `apps/inspector/src/main.rs:4`), and task 1.1 confirms it.
- `AttachConsole(ATTACH_PARENT_PROCESS)` fails when the parent has no console and when the process already has one. After it succeeds, the standard handles that the process did not inherit refer to that console (Microsoft's documentation of `AttachConsole`), and inherited handles such as a redirect are not replaced.
- Closing a console sends `CTRL_CLOSE_EVENT` to every process attached to it, and the system ends each of them after its handler returns, whatever the handler returns. Ctrl+C typed into a console reaches every attached process too (Microsoft's documentation of `HandlerRoutine`).
- Command Prompt and PowerShell do not wait for a GUI program started at the prompt, so its output appears after the next prompt. Inside a `.cmd` script, Command Prompt does wait (`start /?`).
- `WGPU_BACKEND=metal` leaves wgpu without an adapter on Windows, so the start fails with a WGPU error.

## Goals / Non-Goals

**Goals:**

- Every failure that ends the Inspector reaches the person who started it, however they started it, and in the same form on every platform.
- A runtime failure is a returned error. It comes from one creation function that the running Inspector can call again.
- A Windows release build started from a terminal behaves like a terminal program while it starts, and like a GUI program once its window is up.
- Debug builds, a redirected standard error, and Linux and macOS behave as before, apart from the wording of the line and the runtime failure.

**Non-Goals:**

- A log file (decision 8), and with it the warning that `RUST_LOG` overrides a requested level.
- Panics. They still write to a standard error that may be missing (Open Questions).
- Showing the non-fatal warnings of a start without a terminal. Only a redirect keeps them.
- A dialog on Linux or macOS. The Inspector ships as a wheel binary that is started from a terminal, and a desktop session keeps a launched program's standard error, in the journal or in `~/.xsession-errors`. That last point is *assumed*, not checked.
- Help output without a terminal. `--help` never opens a dialog.

## Decisions

### 1. `run()` reports its own failure and returns the exit code

`pub fn run() -> std::process::ExitCode` works in three steps:

1. it handles the console (decision 6) before anything reads an argument or writes a diagnostic;
2. it calls a private `try_run() -> Result<(), InspectorError>`, which holds today's body;
3. it reports a failure (decision 4) and returns the exit code.

Both binaries' `main` become `fn main() -> ExitCode { platynui_inspector::run() }`, and each keeps its `windows_subsystem` attribute, which only a binary's crate root can carry. `main` returns the code instead of calling `process::exit`. The report runs after `try_run` has returned, so the runtime has shut down and the window has closed before a dialog appears.

*Alternatives considered:*

- **Keep `run() -> eframe::Result`, and let both `main` functions call a shared report helper.** The console has to be attached before the arguments are parsed, and that happens inside `run()`. Each `main` would then need two calls in the right order, which is easy to get wrong in one of the two binaries. Rejected.
- **Keep `eframe::Result`, and wrap the runtime failure in `eframe::Error::AppCreation`.** eframe 0.36.2 allows that without a signature change. But the line would read `app creation error: …` for a failure that happens before eframe runs, and usage errors would still need a path of their own. Rejected in favor of decision 2.

### 2. A private error type names the step that failed

`InspectorError` is private to the crate and has three variants:

- `Usage(clap::Error)`;
- `Runtime(ProviderError)`, displayed as `cannot create the PlatynUI runtime: {reason}`;
- `Ui(eframe::Error)`, displayed as eframe's own text (`WGPU error: …`, `winit EventLoopError: …`), as the wheel's binary prints it today.

`source()` returns the wrapped error. The exit code is clap's for `Usage`, which is 2, and 1 for the other variants.

This follows `dev-docs/error-handling.md`: a fixed English base text that says which step failed, with the lower layer's text appended as the detail. `Ui` gets no base text of its own. eframe's text already names the failing part, and an event loop that fails at exit is not a start failure, so a base text such as "cannot open the window" would be wrong there.

### 3. The runtime comes from one fallible function, before the window

A function `create_runtime` returns `Result<Arc<Runtime>, ProviderError>`. `try_run` calls it where `lib.rs:1150` stands today and maps a failure to `InspectorError::Runtime`. The `# Panics` section of `run()`'s documentation goes away. The order stays: the runtime is created before `run_native`, so a runtime failure never shows a window first.

The function takes no configuration yet. `runtime-provider-selection` task 3.1 will call it inside the running Inspector with a selection, keep the old runtime when the call fails, and show the reason in the UI. The new runtime path is therefore free of process concerns: no printing, no exit, no dialog.

*Alternative considered:*

- **Create the runtime inside the app creator, so that eframe wraps its error.** A window would appear and vanish, and task 3.1 needs a function outside the creator anyway. Rejected.

### 4. The report is one line, written once, and never logged

- A failure other than a usage error is reported as `Error: {InspectorError}`, without color, like the wheel's command-line tool.
- A usage error is reported in clap's own rendering (decision 5).
- The report is written with `writeln!` to standard error, and a failed write is ignored. A closed pipe must not turn the report into a panic, which `eprintln!` would.
- The failure is not also written as a tracing record, as the review entry for `lib.rs:1182` asks. By `dev-docs/logging.md` §2 and §4, a returned failure is reported by the layer that decides its consequence. Here that is the entry point, whose report is the line itself, and a record would show the failure twice in a terminal.
- The dropped review entry proposed `Error: {error:#}`. `{:#}` is anyhow's format for a whole error chain; `InspectorError`'s own text already carries the reason, so plain `{}` is enough.

Whether the report goes to standard error or into the dialog is decided by the console state (decisions 6 and 7), never by whether a write succeeds. The route stays predictable, and a harness that closes its end of the pipe never gets a dialog.

### 5. Usage errors take the same route

`InspectorArgs::try_parse()` replaces `parse()` (`lib.rs:1131`), so a usage error becomes a value instead of an exit inside clap.

- A request for help or the version is one of clap's non-error outcomes. It prints as today and ends with exit code 0, into the terminal when the Inspector writes to its parent's console.
- A real usage error becomes `InspectorError::Usage` and follows the report's route:
  - on a console or a given standard error, clap prints it in its own rendering, colored where supported, and the Inspector exits with clap's code 2;
  - in the dialog, it appears as clap's plain text.

The existing test `an_unknown_log_level_is_rejected_with_the_level_names` (`lib.rs:1249-1257`) keeps passing, because it already uses `try_parse_from`.

### 6. Windows: the parent's console while starting, a given standard error for the whole run

As its first step, `run()` checks the standard error it was given:

- **A file or a pipe** (`GetFileType`) was given by the parent: a shell redirect, a harness, the acceptance lanes. It is kept for the whole run. The Inspector never attaches, never detaches, and never shows a dialog.
- **Anything else**: no handle, an invalid one, or a character device. The Inspector remembers its three standard handles and calls `AttachConsole(ATTACH_PARENT_PROCESS)`:
  - on success, it writes to the terminal it was started from;
  - if the process already has a console, the attach fails as expected. A debug build is a console program and does nothing further;
  - if the parent has no console (Explorer, the Start menu, a shortcut), there is nowhere to write, and a failure goes to the dialog (decision 7).
- **At the end of the app creator**, when `InspectorApp::new` has returned (`lib.rs:1185-1194`), a process that attached leaves the console again:
  - it first restores the standard handles it remembered (`SetStdHandle`);
  - it then calls `FreeConsole`.

The Inspector leaves the console so that it no longer depends on the terminal. While attached, closing the terminal would end the Inspector, because `CTRL_CLOSE_EVENT` ends every attached process, and Ctrl+C typed there later would reach it (*assumed*, see Context). Today a release Inspector does not depend on the terminal it was started from, and it keeps that independence once its window is up.

The handles are restored before `FreeConsole` for two reasons:

- The standard library looks the handle up on every write. Once the handles are back to what they were, which in a terminal start is none, a write from any thread is a silent no-op again, exactly as today.
- Restoring first leaves no moment in which another thread could write through a closed console handle whose value the system has already given to a new file or socket.

The Inspector leaves the console at the end of the app creator, not at its start, so that what the app's own construction reports still reaches the terminal: the persisted settings, the picker probe, and, once `inspector-xpath-history` lands, a corrupt history file. A failure before that point — a renderer or event loop that fails, or a usage error — also still finds the terminal.

The code is compiled on every Windows build, not only when `debug_assertions` is off. `just clippy-windows` and CI's Windows lint therefore check it, and a debug build runs its decision and finds its own console. On Linux and macOS the same entry reports the state "standard error as given" and does nothing.

*Alternatives considered:*

- **Stay attached for the whole run**, as some GUI programs do. Every diagnostic would reach the terminal, but closing the terminal would end the Inspector. That behavior change was not asked for. Rejected.
- **Attach only when reporting a failure.** The Inspector would never depend on the terminal, but `--help`, usage errors, and the startup warnings that often explain a start failure would stay invisible. Such warnings include an ignored renderer value and a missing platform backend. Rejected.
- **A console twin**, a console-subsystem launcher next to the GUI binary, as `devenv.com` sits next to `devenv.exe`. It would give complete terminal behavior, at the cost of a second binary in the wheel and a hand-over between the two. That is out of proportion for reporting a start failure. Rejected.
- **The console subsystem for release builds.** Every start from Explorer would open a console window. That is why the attribute exists. Rejected.

### 7. Windows: an error dialog when the report has nowhere to go

The report's route is a pure function of the console state:

| Console state | Route |
|---|---|
| standard error given (Linux and macOS: always) | the line |
| own console | the line |
| attached to the parent's console | the line |
| detached, or no console | the dialog |

The dialog is `MessageBoxW`:

- it has no owner window, an OK button and the error icon, and it comes to the foreground;
- its title is `PlatynUI Inspector`;
- its body is the reason, meaning the line without the `Error: ` prefix, or clap's plain message for a usage error;
- a second paragraph says how to see the diagnostics: start the Inspector from a terminal, or redirect its standard error into a file.

The text is converted to UTF-16. The process returns its exit code once the dialog is dismissed.

*Alternatives considered:*

- **A notification.** To show reliably it needs an AppUserModelID and a registered shortcut, and it disappears by itself. Rejected.
- **An egui error window.** The failure is often in the renderer or the window itself. Rejected.
- **The Windows event log.** Nobody who double-clicked an icon looks there. Rejected.

### 8. No log file in this change

The options weighed:

- **A log file written by default** (`%LOCALAPPDATA%\platynui\inspector.log`, `$XDG_STATE_HOME/platynui/…`):
  - it would keep the non-fatal warnings of a start without a terminal, and give a bug report something to attach;
  - it is new persistent state on every start, which needs decisions of its own: location, truncation, and what the trace level writes into it;
  - it needs a second writer in the shared builder, whose only writer hook is a hidden test API (`crates/log-filter/src/lib.rs:173-179`).
- **An opt-in `--log-file` or `PLATYNUI_INSPECTOR_LOG_FILE`.** It adds nothing that `2> inspector.log` does not already give, once decision 6 keeps a given standard error for the whole run.

The start failure itself, the problem of A5, is fully covered by the terminal and the dialog. `crates/log-filter` therefore stays untouched. The priority B entry "`RUST_LOG` silently overrides a requested level" (`crates/log-filter/src/lib.rs:138-159`, and `packages/native/src/log_bridge.rs:217` in the review) keeps its own trigger: the next change to the shared filter builder. If a later change adds a log file, the file belongs in the local state or data directory. The config directory, where `inspector.ron` lives, holds settings only.

### 9. Tests by layer

- **Unit tests, every platform** (`just test-crate platynui-inspector`), all of them pure functions without Win32:
  - the report text for each `InspectorError` variant;
  - the exit codes;
  - the dialog's title and body;
  - the attach decision for each kind of standard error (file, pipe, character device, none);
  - the route for each console state.

  They are compiled on every platform, so the tests run on Linux in CI and not only on a Windows host. On Linux and macOS the running Inspector uses them too, with the state fixed to "standard error as given"; a state that only Windows reaches may need a narrowly scoped dead-code allowance there, which is preferable to hiding the decision behind `cfg(windows)`.
- **Integration tests on Linux**, in `apps/inspector/tests/`. They run the built binary (`CARGO_BIN_EXE_platynui-inspector-rs`) in a controlled environment:
  - `RUST_LOG` and `PLATYNUI_LOG_LEVEL` are removed;
  - `PLATYNUI_INSPECTOR_SETTINGS_PATH` points into a temporary directory, as the acceptance suites do (`inspector.resource:30-34`), so that no user settings leak in;
  - the display variables are set per test.

  The tests:
  - the runtime failure (`XDG_SESSION_TYPE=x11`, a `DISPLAY` that names no X server, no `WAYLAND_DISPLAY`);
  - the window failure (none of the display variables);
  - the usage error.

  The first two fail before the change, as observed: exit code 101 with a panic, and the old prefix. The test file allows `unused_crate_dependencies`, as the bin target does, because an integration test links every dependency of the package.
- **Windows, manual:** a release build on real Windows, following the matrix in tasks 7.3 and 7.4, after the baseline of task 1.1. A debug build cannot take the release path, and Wine does not count.
- **Acceptance:** the existing Inspector suites on the X11, compositor and Windows lanes check that startup still works, including a given standard error.

The change has no Robot Framework surface, so no mock suite applies.

## Risks / Trade-offs

- [The Windows console behavior is taken from documentation, not measured] → Every start path — Command Prompt, PowerShell, Windows Terminal, the Run dialog, a shortcut, `2> file` — is checked on real Windows before the tasks close (7.3, 7.4). If `AttachConsole` does not set up the standard handles, the design adds an explicit `CONOUT$` handle and is updated first.
- [The output appears after the shell's next prompt] → Command Prompt and PowerShell do not wait for a GUI program. This is cosmetic, and `dev-docs/inspector.md` says so and names the `.cmd` script and `start /wait` as ways to wait.
- [Ctrl+C or closing the terminal while the Inspector starts ends it] → Only until its window is up, a matter of seconds. After that it is detached.
- [On Windows, warnings after startup stay invisible without a redirect] → As today, and a non-goal (decision 8). `2> inspector.log` keeps all of them, and the dialog's hint says so.
- [A dialog blocks an unattended start that has no standard handles] → The dialog appears only when nothing else can carry the error. Automation hands over a standard error, as the acceptance lanes do, and those also use debug builds.
- [A console handle value is reused after the detach] → Handled by restoring the standard handles before `FreeConsole` (decision 6).
- [`WGPU_BACKEND=metal` does not fail on some Windows machine] → The start-failure checks then use another failure, such as `--renderer glow --glow-hardware-acceleration required` on a machine without hardware OpenGL, and record which one was used.
- [A runtime failure exits with code 1 instead of 101] → A script that relied on the panic's code is unlikely. The change is listed in the release notes.
- [Panics still vanish on Windows] → Out of scope here (Open Questions).

## Migration Plan

- **Behavioral, for the Inspector only:**
  - a Windows release build uses its parent's console while it starts and shows a dialog when nothing else can carry a failure;
  - on every platform, a runtime failure is a reported error instead of a panic;
  - the Rust binary's line reads `Error: …`.

  Nothing is persisted, and nothing is configured. `platynui_inspector::run()` changes its return type, and its only callers are the two `main` functions in this repository.
- **No native rebuild:** `packages/native` is not touched. The Inspector binaries are rebuilt, and users get the fix with the next `platynui-inspector` wheel. Debug builds, and with them every acceptance lane, keep their console behavior on Windows.
- **Sequence:**
  1. the tests: unit tests and the Linux integration tests;
  2. the error type, the returned runtime failure, the report and both `main` functions;
  3. the Windows console and the dialog;
  4. the docs;
  5. the verification, first on Linux with the cross-checks, then on real Windows.
- **Rollback:** revert the commits. The Windows commit can be reverted alone. The report then stays the line on standard error, and the returned runtime failure remains.

## Open Questions

- Should the main thread's panics also reach the dialog on Windows release builds, through a panic hook? A bug in the UI code still makes the window vanish silently. That would be a later change of its own.
- Does a later change want a log file (decision 8)? If so, the `RUST_LOG`-override warning in `crates/log-filter` rides along with it.
- The exact wording of the dialog's hint is settled when implementing. The spec requires only the reason and a pointer to the diagnostics.
