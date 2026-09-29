# Spec Delta

## Purpose

How the PlatynUI Inspector reports a failure that ends it, so that the user always learns why it did not appear or went away. Such a failure can be a command line it cannot use, a PlatynUI runtime it cannot create, or a window or renderer that fails. A Windows release build has no console of its own, so it reports through the terminal it was started from, through a standard error it was given, or through an error dialog.

## ADDED Requirements

### Requirement: A failure that ends the Inspector is reported once

The Inspector SHALL report each failure that ends it: a command line it cannot use, a PlatynUI runtime it cannot create, and a window, renderer or event loop that fails. A panic is not such a failure and is not covered here. The report takes one of two forms:

- A command line it cannot use SHALL be reported with the command-line parser's usage message, which names the rejected argument, and SHALL end the Inspector with exit code 2.
- Every other failure SHALL be reported as one line, `Error: ` followed by the reason, and SHALL end the Inspector with exit code 1.

Both Inspector binaries SHALL report alike: `platynui-inspector-rs`, built from the repository, and `platynui-inspector`, which the wheel installs. On Linux and macOS the report SHALL go to standard error. Where it goes on Windows is set by the requirements below. The Inspector SHALL NOT also log the failure as a diagnostic record.

#### Scenario: A window that cannot be opened is reported in one line

- **GIVEN** a Linux environment in which none of `DISPLAY`, `WAYLAND_DISPLAY` and `XDG_SESSION_TYPE` is set
- **WHEN** `platynui-inspector-rs` starts
- **THEN** it SHALL exit with code 1
- **AND** the last line of its standard error SHALL start with `Error: `, followed by the reason the windowing system gave
- **AND** no line SHALL start with `Inspector exited with error:`
- **NOTE:** Automated on Linux; the test runs the built binary, and no display is needed.

#### Scenario: A command line the Inspector cannot use

- **GIVEN** any environment
- **WHEN** the Inspector is started with `--log-level verbose`
- **THEN** it SHALL exit with code 2
- **AND** its report SHALL name `verbose` and list the accepted level names
- **NOTE:** Automated on Linux by running the built binary. On Windows, where the report goes is covered by the terminal and dialog scenarios below.

#### Scenario: Both binaries report alike

- **GIVEN** `platynui-inspector-rs` and the `platynui-inspector` binary of the wheel, both built from the same sources
- **WHEN** each starts in the environment of *A window that cannot be opened is reported in one line*
- **THEN** both SHALL exit with code 1 and print the same last line
- **NOTE:** Checked by running both builds on Linux in the verification step.

### Requirement: A runtime that cannot be created is a reported failure, not a crash

When the Inspector cannot create its PlatynUI runtime, it SHALL report the failure as `Error: cannot create the PlatynUI runtime: ` followed by the runtime's own reason, and SHALL end with exit code 1. It SHALL NOT panic. This holds on every platform.

#### Scenario: A Linux session whose X server does not answer

- **GIVEN** a Linux environment with `XDG_SESSION_TYPE=x11`, no `WAYLAND_DISPLAY`, and a `DISPLAY` that names no running X server
- **WHEN** `platynui-inspector-rs` starts
- **THEN** it SHALL exit with code 1
- **AND** its standard error SHALL contain exactly one line that starts with `Error: cannot create the PlatynUI runtime: `, followed by the runtime's reason
- **AND** no other line SHALL repeat that reason, and no line SHALL report a panic
- **NOTE:** Automated on Linux by running the built binary. Before this change the Inspector panics here with exit code 101.

### Requirement: On Windows, a release build started from a terminal prints there while it starts

A Windows release build of the Inspector can start without a standard error that is a file or a pipe while its parent process has a console, as when it is started from Command Prompt, PowerShell or Windows Terminal. In that case the Inspector SHALL write to that console from the moment it starts until its window is up. That output covers:

- its help and version output;
- a usage error;
- the diagnostics that the level setting lets through;
- a failure that ends it before its window is up.

Once its window is up, the Inspector SHALL stop using that console, so that neither closing the terminal nor pressing Ctrl+C in it ends the Inspector. A Windows debug build, which is a console program, SHALL keep its behavior.

#### Scenario: The help reaches the terminal

- **GIVEN** a release build and a Command Prompt or PowerShell terminal
- **WHEN** `platynui-inspector --help` is run in that terminal
- **THEN** the help text SHALL appear in that terminal
- **AND** neither the Inspector's window nor a dialog SHALL appear
- **NOTE:** Real Windows only, release build. Wine does not count.

#### Scenario: A usage error reaches the terminal

- **GIVEN** a release build and a terminal
- **WHEN** `platynui-inspector --log-level verbose` is run in that terminal
- **THEN** the usage message naming `verbose` SHALL appear in that terminal
- **AND** no dialog SHALL appear, and the Inspector SHALL end with exit code 2
- **NOTE:** Real Windows only, release build.

#### Scenario: A start failure reaches the terminal

- **GIVEN** a release build, a terminal, and `WGPU_BACKEND=metal`, a backend Windows does not have, so the renderer finds no graphics adapter
- **WHEN** `platynui-inspector` is run in that terminal
- **THEN** a line starting with `Error: WGPU error:` SHALL appear in that terminal
- **AND** no dialog SHALL appear, and the Inspector SHALL end with exit code 1
- **NOTE:** Real Windows only, release build.

#### Scenario: A startup warning reaches the terminal

- **GIVEN** a release build, a terminal, and `PLATYNUI_INSPECTOR_RENDERER=vulkan`, which is not a renderer the Inspector knows
- **WHEN** `platynui-inspector` is run in that terminal
- **THEN** the warning that the renderer value is ignored SHALL appear in that terminal
- **AND** the Inspector's window SHALL appear, drawn by the default renderer
- **NOTE:** Real Windows only, release build.

#### Scenario: Once its window is up, the Inspector no longer depends on the terminal

- **GIVEN** a release build started from a terminal, whose window has appeared
- **WHEN** Ctrl+C is pressed in that terminal, and then the terminal window is closed
- **THEN** the Inspector SHALL keep running, and its window SHALL stay usable
- **NOTE:** Real Windows only, release build.

### Requirement: On Windows, a standard error the Inspector was given is kept

When a Windows release build starts with its standard error connected to a file or a pipe, the Inspector SHALL keep writing there for the whole run. This is the case when a shell redirects its standard error or a program that starts it captures its output. The Inspector SHALL write its diagnostics and a failure that ends it to that standard error. It SHALL NOT attach to a console, and it SHALL NOT show a dialog.

#### Scenario: A redirect receives the startup diagnostics

- **GIVEN** a release build, a Command Prompt terminal, and `PLATYNUI_INSPECTOR_RENDERER=vulkan`
- **WHEN** `platynui-inspector 2> inspector.log` is run in that terminal and its window appears
- **THEN** `inspector.log` SHALL contain the warning that the renderer value is ignored
- **AND** the terminal SHALL show nothing of it
- **NOTE:** Real Windows only, release build.

#### Scenario: A start failure goes to the redirect, not to a dialog

- **GIVEN** a release build and `WGPU_BACKEND=metal`
- **WHEN** `platynui-inspector 2> inspector.log` is run in a Command Prompt terminal
- **THEN** the last line of `inspector.log` SHALL start with `Error: WGPU error:`
- **AND** no dialog SHALL appear
- **NOTE:** Real Windows only, release build.

### Requirement: On Windows, a failure with nowhere else to go is shown in a dialog

A Windows release build can end with a failure while it has neither a standard error it was given nor its parent's console. This happens when it is started from Explorer, the Start menu, the Run dialog or a shortcut, or after it has stopped using the terminal it was started from. In that case the Inspector SHALL show an error dialog before it ends. The dialog SHALL:

- name the PlatynUI Inspector;
- show the reason the report carries, which for a usage error is the parser's message;
- say how to see the Inspector's diagnostics.

The Inspector SHALL end once the dialog is dismissed, with the exit code the report defines. It SHALL NOT show the dialog in any other case.

#### Scenario: A start failure without a terminal

- **GIVEN** a release build and the user environment variable `WGPU_BACKEND=metal`
- **WHEN** the Inspector is started from the Run dialog (Win+R)
- **THEN** an error dialog naming the PlatynUI Inspector SHALL show the WGPU error's text and how to see the diagnostics
- **AND** the Inspector process SHALL end once the dialog is dismissed
- **NOTE:** Real Windows only, release build.

#### Scenario: A usage error from a shortcut

- **GIVEN** a release build and a shortcut that starts it with `--log-level verbose`
- **WHEN** the shortcut is opened
- **THEN** an error dialog SHALL show the usage message, which names `verbose`
- **AND** the Inspector process SHALL end once the dialog is dismissed
- **NOTE:** Real Windows only, release build.

#### Scenario: The dialog is chosen only when nothing else can carry the report

- **GIVEN** the Inspector's decision where the report of a failure goes
- **WHEN** the Inspector was given a standard error that is a file or a pipe
- **THEN** the report SHALL go to that standard error
- **WHEN** the Inspector writes to its parent's console, or has a console of its own
- **THEN** the report SHALL go to that console
- **WHEN** the Inspector has none of these, or has stopped using its parent's console
- **THEN** the report SHALL be shown in the dialog
- **NOTE:** A unit test of the decision alone, on every platform (`just test-crate platynui-inspector`). On Linux and macOS the Inspector always has its standard error, so the report always goes there.
