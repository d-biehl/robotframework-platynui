# Proposal

## Why

The logging review deferred the Windows input diagnostics to follow-up A3 (`openspec/changes/archive/2026-09-27-logging-concept/review-findings.md`, section *Windows & Java*, the entries at `keyboard.rs:81` and `keyboard.rs:164`). Its typed-text sweep also found a functional bug in the same keyboard device, which the triage of 2026-09-28 assigned to a follow-up for the Windows keyboard. All three were re-checked on 2026-09-29 and still hold:

- **A refused key event says "not ready".** When `SendInput` inserts no event, the keyboard device returns `KeyboardError::NotReady` (`crates/platform-windows/src/keyboard.rs:81`, `:98`). It does not read the operating system's error and writes no record. The user reads "keyboard provider is not ready". For a Robot Framework `Secret` the message is "no keyboard device is ready" (`crates/runtime/src/runtime/error.rs:115-117`), although the `diagnostic-logging` spec requires a failure while sending to be given as such. Windows refuses input, for example, while the input desktop is not the session's desktop: the workstation is locked, the UAC prompt is up, or a remote session is disconnected. Nothing in the message points there. For the pointer the same failure already carries the Win32 error code (`crates/platform-windows/src/pointer.rs:97-106`, `:146-149`).
- **Input to an elevated application is lost without a word.** Windows discards synthesized input to a window whose process runs at a higher integrity level than the sender. This is User Interface Privilege Isolation (UIPI), and Microsoft documents that `SendInput` does not report it. A test that runs without elevation and types into an elevated application — Task Manager, a console started as administrator, an installer — types nothing. It then fails later, on an assertion that has nothing to do with the cause. PlatynUI never looks at integrity levels: `start_input` does nothing (`keyboard.rs:164-166`), and no code in `platform-windows` or `provider-windows-uia` reads one.
- **A character outside the Basic Multilingual Plane is typed as a different character.** `key_to_code` keeps only the low 16 bits of the character (`keyboard.rs:140`), and a key code carries a single UTF-16 unit (`:18`, `:84-99`). `😀` (U+1F600) is typed as U+F600, a private-use character, and no error is raised.

## What Changes

- **A refused event fails with the operating system's error.** The keyboard device reads the thread's last error before anything else runs, records the error code at debug, and returns the error as a platform error, the same way the pointer does. Neither the error nor the record names a key, a character or a key code. For a `Secret` the error reads "sending the keyboard input failed". A call that inserts fewer events than it was given also counts as refused.
- **Characters outside the Basic Multilingual Plane are typed as themselves.** Such a character has no key in a keyboard layout, so it always takes the Unicode path, and it is sent as its surrogate pair. Its press sends the two key-down events in one `SendInput` call, and its release sends the two key-up events in one call. Characters of the Basic Multilingual Plane keep today's path unchanged, the CapsLock handling included.
- **Input to a process of higher integrity is reported once per process.** Windows delivers keyboard input to the foreground window and a pointer press or scroll to the window under the pointer. Before a keyboard sequence starts, and before a pointer press or scroll, the Windows platform finds the process that owns that window. It compares that process's integrity level with PlatynUI's own. If the level is higher, or cannot be read (which on a desktop usually means an elevated or a system process), PlatynUI warns once for that process. The warning names the application and says how to fix it. Further occurrences for the same process are recorded at debug. The input is still sent: the check reports the problem, it does not block the input.
- **`platynui-process` reads integrity levels.** It reads a process's mandatory integrity level through the recorded process identity, as it reads the other process facts. It also reads the calling process's own level and compares two levels. The UI Automation provider already depends on the crate and can use the same reader for its `WaitForInputIdle` follow-up from the same review.
- **Deliberately unchanged:**
  - the CapsLock handling for characters of the Basic Multilingual Plane, which tests only the low byte of the UTF-16 unit (`keyboard.rs:142-144`). A correct fix has to take the keyboard layout into account and is recorded as a follow-up;
  - the release of injected modifiers, whose failures are ignored (`let _ =`, dropped in the triage);
  - `KeyboardError::NotReady` for a runtime without a platform;
  - the sequence syntax: `\u` keeps taking exactly four hex digits.

Behavior changes that users see, for the release notes:

- On Windows, a key event that Windows refuses fails with an error that names `SendInput` and the Win32 error code, instead of "keyboard provider is not ready". For a `Secret` the error says "sending the keyboard input failed", instead of "no keyboard device is ready".
- On Windows, characters outside the Basic Multilingual Plane, such as emoji or CJK Extension B, are typed correctly.
- On Windows, a warning appears when keys or clicks go to an application that runs at a higher integrity level than PlatynUI.

## Capabilities

### New Capabilities

- `windows-input`: how the Windows platform delivers synthesized keyboard and pointer input. It covers characters outside the Basic Multilingual Plane, input that Windows refuses, and input that Windows discards because the receiving process runs at a higher integrity level.

### Modified Capabilities

None. The `diagnostic-logging` rules apply unchanged. With this change, the Windows keyboard meets the existing requirement that a failure while sending is given as such, for a `Secret` too.

## Impact

- **Rust crates:**
  - `crates/process`:
    - the integrity level types and their comparison, available on every platform;
    - the Windows reader: a recorded process's level, read through its identity, and the calling process's level;
    - `Hash` on `ProcessIdentity`, so that a process can be the subject of a once-per-episode latch.
  - `crates/platform-windows`:
    - `src/keyboard.rs`: key codes carry a `char`; the surrogate pair; the refusal path; `start_input` runs the integrity check;
    - `src/pointer.rs`: `press` and `scroll` run the integrity check; the keyboard now uses the pointer's last-error helper as well;
    - a new module for the check: finding the receiving window and its process, the comparison, the latch and the records;
    - `src/factory.rs`: one check per platform bundle, shared by its keyboard and pointer;
    - `Cargo.toml`: `platynui-process` as a dependency, a leaf crate like `platynui-java-agent`; dev-dependency features for the desktop and window tests.
  - No change to `platynui-core`, the runtime or the Python binding. `KeyboardError::Platform` exists already (`crates/core/src/platform/keyboard.rs:78`), and the runtime already reports a sending failure in both renderings (`crates/runtime/src/runtime/error.rs:46`, `:98-121`).
- **Python/RF:** no API change. The BareMetal library documentation gets a short paragraph on applications that run elevated.
- **Tests:**
  - `platynui-process`: the types and the comparison on every platform; on a Windows host, the reader, including a child that runs at low integrity and a child whose token refuses the query;
  - `platynui-platform-windows`, on a Windows host: the events built for a character, a refused event on a private desktop, and the check against real child processes;
  - an ignored test that types into an EDIT control of its own, which the Windows lane runs;
  - an egui acceptance test tagged `platform:windows`;
  - a manual check with an elevated application on real Windows. Wine does not count as verification.
- **Native rebuild:** yes.
- **Platforms:** only the Windows platform (Win32) changes. Linux and macOS behave as before; there the new `platynui-process` API answers that a level is unknown.
- **BREAKING:** none in the API. Rust code that expected `KeyboardError::NotReady` from the Windows keyboard for a refused event now gets `KeyboardError::Platform`.
- **Docs:** `dev-docs/platform-windows.md` (§1, keyboard and pointer), `dev-docs/logging.md` (§5, the table of latches), `dev-docs/keyboard-input.md` (§6, characters outside the Basic Multilingual Plane), and the BareMetal library documentation.
- **Coordination:**
  - `gate-uia-window-patterns` routes UIA activation through the Win32 window manager before every pointer and keyboard action. In `platform-windows` it only edits comments in `window_manager.rs`, so there is no conflict. Both changes add a crate to the same `--run-ignored ignored-only` command of `test-acceptance-windows` (`justfile:391`); whichever lands second rebases.
  - Several open changes edit `dev-docs/platform-windows.md` in its JAB, agent and UIA sections. This change edits only §1.
  - `x11-atspi-healthy-run-warnings` also adds to the table of episode subjects in `dev-docs/logging.md` §5, and `uniform-wait-keywords` rewrites parts of the BareMetal library documentation. Both only touch the same files; whichever lands second rebases.
  - The UI Automation follow-up for `WaitForInputIdle` on elevated targets (`crates/provider-windows-uia/src/node.rs:42-60`, from the same review) is not part of this change. It can use the new reader.
