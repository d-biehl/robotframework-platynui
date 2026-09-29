# Proposal

## Why

On Windows, a release build of the Inspector is a GUI program without a console. Both binaries set `windows_subsystem = "windows"` (`apps/inspector/src/main.rs:4`, `packages/inspector/src/main.rs:1`). Started from Explorer, or from a terminal without a redirect, the process therefore has no standard error. When the Inspector cannot start, it writes the reason with `eprintln!` (`apps/inspector/src/main.rs:10`, `packages/inspector/src/main.rs:5`) and exits with code 1. Typical reasons are that the renderer finds no graphics adapter, that the PlatynUI runtime cannot be created, or that an argument is wrong. Rust's standard library discards writes to a missing handle without an error, so the Inspector simply does not appear, and nothing tells the user why. `--help` and every startup warning are lost the same way. This is follow-up A5 of the logging triage, "the Inspector's start failure on Windows". Its entries are in `openspec/changes/archive/2026-09-27-logging-concept/review-findings.md`: `apps/inspector/src/main.rs:4` is the priority A entry, and `lib.rs:1182` is the priority B entry that rides along.

A second defect affects every platform. A runtime that cannot be created ends the Inspector with a panic instead of an error. `Runtime::new().expect(…)` is at `apps/inspector/src/lib.rs:1150`, and the panic is documented as `# Panics` at `:1127-1129`. On Linux, a `DISPLAY` that names no X server makes the current build print the error's `Debug` form and a backtrace hint, and exit with code 101.

## What Changes

- **A failure that ends the Inspector is reported once, on every platform.**
  - `platynui_inspector::run()` reports its own failure and returns the process exit code. Both binaries' `main` shrink to that one call.
  - The report is one line, `Error: <reason>`. The Rust binary's `Inspector exited with error:` prefix goes away, and the wheel's `Error:` prefix and the command-line tool's `Error:` prefix already match it.
  - A usage error keeps clap's own message and exit code 2. Every other failure exits with code 1.
  - The failure is not logged as a diagnostic in addition.
- **A runtime that cannot be created is a returned failure, not a panic.** The report reads `Error: cannot create the PlatynUI runtime: <reason>`. Creating the runtime becomes one fallible function that the running Inspector can call again later.
- **Windows: started from a terminal, a release build prints there while it starts.**
  - Before it parses its arguments, a process whose standard error is not a file or a pipe attaches to its parent's console, if the parent has one. `--help`, a usage error, the startup warnings and a start failure then appear in that terminal.
  - Once the Inspector's window is up, it detaches from that console again, and closing the terminal or pressing Ctrl+C there no longer affects it.
- **Windows: a standard error the Inspector was given is kept.** This applies to a file or a pipe, for example `2> inspector.log` or a test harness that captures the output. Diagnostics go there for the whole run, and the Inspector neither attaches to a console nor shows a dialog.
- **Windows: a failure with nowhere else to go is shown in an error dialog.** This covers a start from Explorer, the Start menu, the Run dialog or a shortcut, and a failure after the Inspector has detached. The dialog carries the same text as the terminal line and says how to see the diagnostics.
- **Not part of this change:**
  - A log file, whether written by default or on request.
  - The warning that `RUST_LOG` overrides a requested level. That priority B entry rides along only when the shared filter builder in `crates/log-filter` is touched, and this change does not touch it (design decision 8).
  - A dialog on Linux or macOS.
  - Panics. They still write to a standard error that may be missing (design, Open Questions).

Behavior changes for the release notes:

- On Windows, a release build started from a terminal prints its help, usage errors, startup warnings and start failure there. Started without a terminal, it shows a start failure in a dialog.
- On every platform, a runtime that cannot be created ends the Inspector with exit code 1 and an `Error:` line, where it used to panic with exit code 101.
- The Rust binary `platynui-inspector-rs` reports a fatal error as `Error: …`, the same way the wheel's binary does.

## Capabilities

### New Capabilities

- `inspector-fatal-errors`: how the Inspector reports a failure that ends it. It covers:
  - the returned runtime failure;
  - the one-line report and its exit codes, identical for both binaries;
  - a Windows release build's use of its parent's console while it starts;
  - a standard error the Inspector was given;
  - the error dialog when neither exists.

### Modified Capabilities

None.

- `diagnostic-logging` stays unchanged. The fatal report is not a log record (`dev-docs/logging.md` §2, *Not diagnostics: results and failures*), and the level knob, the records and their levels stay as they are.
- No existing Inspector capability (`inspector-live-mouse-picker`, `-status-bar`, `-theme`, `-toolbar`, `-window-controls`) covers how the Inspector starts or exits.

## Impact

- **Rust, `apps/inspector`:**
  - `src/lib.rs`:
    - `run()` returns `std::process::ExitCode` and reports its own failure;
    - a private error type distinguishes a usage error, a runtime failure, and a window or renderer failure;
    - a fallible function creates the runtime;
    - arguments are read with clap's `try_parse`;
    - the Inspector detaches from the console at the end of the app creator;
    - `run()`'s documentation loses `# Panics`.
  - New module(s):
    - the report and the Windows console and dialog handling, as pure decision and formatting functions;
    - a Windows-only part for the Win32 calls, with `#[allow(unsafe_code)]` scoped narrowly, as `src/modifiers.rs:166-171` does.
  - `src/main.rs` becomes one call.
  - `Cargo.toml` adds the features `Win32_System_Console`, `Win32_Storage_FileSystem` and `Win32_UI_WindowsAndMessaging` to the Windows-only `windows` 0.62 dependency.
  - `tests/` gains a Linux integration test that runs the built binary.
- **Rust, `packages/inspector/src/main.rs`** (the wheel's binary) becomes one call.
- **Not touched:**
  - `crates/log-filter`;
  - `crates/runtime`, because `Runtime::new` already returns its error;
  - `packages/native`;
  - the Python and Robot Framework library;
  - the acceptance suites, which start the debug build with its standard error redirected to a file (`tests/acceptance/egui/resources/inspector.resource:30-34`).
- **Tests:**
  - unit tests of the report text, the exit codes and the route decisions, on every platform;
  - Linux integration tests for a runtime failure and a window failure;
  - manual checks of a release build on real Windows (the Windows 11 VM counts, Wine does not);
  - the Inspector acceptance suites as a startup regression check.
- **Docs:**
  - `dev-docs/inspector.md`: the module listing, the *Tracing* bullet and *Troubleshooting*;
  - `dev-docs/logging.md` §2: where the Windows release build's diagnostics go;
  - `.github/instructions/logging.instructions.md` §6: an entry point reports the failure that ends it once;
  - `packages/inspector/README.md`: *Notes*.
- **Native rebuild:** no, because `packages/native` is not touched. The Inspector binaries are rebuilt, and users get the fix with the next `platynui-inspector` wheel.
- **Platforms:**
  - Windows (UIA): the console, the kept standard error and the dialog, in release builds only. Debug builds are console programs and behave as before.
  - Every platform: the returned runtime failure and the unified line.
  - The README's support table lists the Inspector as supported on Windows, Linux X11 and the PlatynUI compositor, partial on other Wayland sessions, and unsupported on macOS.
- **Compatibility:**
  - Nothing is **BREAKING** for the library or its users.
  - A runtime failure exits with code 1 instead of 101.
  - `platynui_inspector::run()` changes its return type from `eframe::Result` to `ExitCode`. Its only callers are the two `main` functions in this repository.
- **Coordination:**
  - `inspector-xpath-history` (open) plans an empty history plus a logged warning for a corrupt history file (its design D4, task 2.4). Today that warning is invisible on Windows release builds. The history loads while the app is being created, before the Inspector detaches, so after this change a terminal start shows the warning. A start without a terminal still does not, like every non-fatal warning.
  - `runtime-provider-selection` (dormant) task 3.1 rebuilds the runtime inside the running Inspector. It reuses the fallible creation and shows a failure in the UI instead of ending the Inspector.
  - The dropped review entry on the wording at `apps/inspector/src/main.rs:10` is satisfied by the shared report.
