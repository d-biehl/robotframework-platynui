# Spec Delta

## Purpose

How the Windows platform delivers synthesized keyboard and pointer input. It covers three things: characters outside the Basic Multilingual Plane are typed as themselves; input that Windows refuses fails with the operating system's error; and input that Windows discards because the receiving process runs at a higher integrity level is reported once per process.

## ADDED Requirements

### Requirement: Characters outside the Basic Multilingual Plane are typed as themselves

The Windows keyboard SHALL type every character of a keyboard sequence as that character, including characters outside the Basic Multilingual Plane, whose code points lie above U+FFFF (emoji, CJK Extension B, historic scripts).

Such a character SHALL be sent as Unicode input made of its UTF-16 surrogate pair:

- Pressing it SHALL deliver the key-down events of both halves, the high surrogate first, in one insertion that no other input can interleave with.
- Releasing it SHALL deliver the key-up events of both halves in one such insertion.
- It SHALL NOT be mapped through the keyboard layout, and the CapsLock state SHALL NOT change it.

Characters of the Basic Multilingual Plane SHALL be typed as before: through the key the keyboard layout assigns them, and as Unicode input when the layout has none.

#### Scenario: A character outside the Basic Multilingual Plane is typed into an application

- **GIVEN** the egui test app on Windows, with its text field `input-name`
- **WHEN** `Keyboard Type` types `<Ctrl+A>a😀z` into the field (the emoji is U+1F600, written `\U0001F600` in Robot Framework test data)
- **THEN** the field's `control:Text` SHALL become `a😀z`
- **NOTE:** Real provider only, on the Windows lane (the test is tagged `platform:windows`). Before this change the field reads `a`, U+F600 and `z`.

#### Scenario: The standard Windows edit control receives the character

- **GIVEN** a standard Windows edit control that is in the foreground window and has the keyboard focus
- **WHEN** the Windows keyboard types `a😀z`
- **THEN** the control's text SHALL be `a😀z`
- **NOTE:** Real Windows only. It changes the foreground window, so it is an ignored test of the Windows platform that `just test-acceptance-windows` runs. The edit control is Windows' own, so a failure here is PlatynUI's and not a toolkit's.

#### Scenario: Both halves are pressed together and released together

- **GIVEN** the Windows keyboard
- **WHEN** it prepares the press and the release of U+1F600 without sending them
- **THEN** the press SHALL be one insertion of two Unicode key-down events, U+D83D and then U+DE00
- **AND** the release SHALL be one insertion of the two matching key-up events, in the same order
- **NOTE:** A unit test on a Windows host (`just test-crate platynui-platform-windows`); it sends nothing.

#### Scenario: Characters of the Basic Multilingual Plane keep their keys

- **GIVEN** the Windows keyboard
- **WHEN** it resolves the character `A`, the character `ä` and the key name `Escape`
- **THEN** `A` SHALL resolve to the key the keyboard layout assigns it, `ä` to the layout's key or, without one, to Unicode input of U+00E4, and `Escape` to its virtual key, as before this change
- **NOTE:** Unit tests on a Windows host; the existing tests of the keyboard stay green.

### Requirement: Input that Windows refuses fails with the operating system's error

Windows can refuse to insert a keyboard or pointer event, for example while the input desktop is not the desktop of the calling thread, as when the workstation is locked or the UAC prompt is up. An insertion counts as refused when Windows inserts fewer events than it was given.

When that happens, the keyboard or pointer action SHALL fail with a platform error. The error SHALL name the refused call and give the Win32 error code that the operating system reported. The keyboard SHALL NOT report the refusal as "not ready".

The failure SHALL be recorded at debug with the Win32 error code, read before anything else can overwrite it. Because the failure is returned, it SHALL NOT be recorded at warning or error level.

Neither the error nor any record below trace SHALL name the key, character or key code that was refused (`diagnostic-logging`, *Keywords do not repeat the text they are given to type*). For a `Secret`, the keyword's error SHALL say that sending the keyboard input failed, and nothing more.

#### Scenario: A key event that Windows refuses fails with the Win32 error

- **GIVEN** a thread whose desktop is not the input desktop, the situation a locked workstation creates
- **WHEN** the Windows keyboard sends a key press from that thread
- **THEN** it SHALL fail with a platform error that names `SendInput` and gives the Win32 error code
- **AND** it SHALL NOT fail with the "not ready" error
- **NOTE:** Real Windows only, `just test-crate platynui-platform-windows` on a Windows host. The test creates a desktop of its own, so nothing reaches the user's desktop. Before this change it fails with "not ready".

#### Scenario: A refused character is not named

- **GIVEN** a thread whose desktop is not the input desktop
- **WHEN** the Windows keyboard sends the press of a character that takes the Unicode path, from that thread
- **THEN** the error SHALL contain neither the character nor any of its code units
- **AND** the records captured up to debug SHALL contain one record with the Win32 error code, and SHALL name no character, code unit or key code
- **NOTE:** Real Windows only; the test captures the records itself.

#### Scenario: A pointer press that Windows refuses fails with the Win32 error

- **GIVEN** a thread whose desktop is not the input desktop
- **WHEN** the Windows pointer presses a button from that thread
- **THEN** it SHALL fail with a platform error that names `SendInput` and gives the Win32 error code
- **NOTE:** Real Windows only. The pointer behaves this way already (`crates/platform-windows/src/pointer.rs:97-106`); the scenario keeps it that way.

#### Scenario: An insertion that Windows completes only in part counts as refused

- **GIVEN** the keyboard's check of an insertion's result
- **WHEN** Windows reports that it inserted one of the two events of a surrogate pair
- **THEN** the send SHALL fail as refused
- **NOTE:** A unit test of the result check on a Windows host; Windows cannot be made to do this on purpose.

#### Scenario: A secret whose sending fails says only that sending failed

- **GIVEN** a `Secret` passed to `Keyboard Type`, and a keyboard device that fails to send with a platform error
- **WHEN** the keyword fails
- **THEN** its error SHALL read "sending the keyboard input failed", without the device's text
- **NOTE:** The runtime's rendering is verified on every platform with the stub keyboard, which fails with a platform error (`crates/runtime/src/runtime/input.rs`, `a_send_failure_keeps_the_reason_and_its_sensitive_rendering_only_says_sending_failed`). That the Windows keyboard fails with a platform error is the first scenario of this requirement.

### Requirement: Input to a process of higher integrity is reported once per process

Windows discards synthesized input to a window whose process runs at a higher mandatory integrity level than the process that sends it (User Interface Privilege Isolation, UIPI), and the sender is not told. The Windows platform SHALL therefore check where input goes before it sends it:

- **When:** before a keyboard sequence starts, and before a pointer button is pressed or the wheel is scrolled. Pointer moves SHALL NOT be checked.
- **Which process:** for the keyboard, the process of the foreground window when the sequence starts. For a pointer press or scroll, the process of the window under the pointer.
- **Comparison:** that process's integrity level against the level of the process PlatynUI runs in. The target counts as higher when its level is higher, and also when its level cannot be read because access to the process or its token is denied.
- **No report:** a target at the same or a lower level, and a target that cannot be determined (no such window, or a process that has ended), SHALL cause no record above debug.
- **No own level:** while PlatynUI's own level cannot be read, no target SHALL cause a record above debug, and that the check is off SHALL be recorded once at debug when the runtime is created.

For a target that counts as higher, the platform SHALL warn once per target process. The warning SHALL name:

- the process: its process id, and its name where that can be read;
- the process's integrity level, or that the level cannot be read;
- PlatynUI's own level;
- the consequence: Windows discards the synthesized keys and clicks for that process;
- the remedy: run PlatynUI at that level, for example elevated, or the application without elevation.

Further input to the same process SHALL be recorded at debug. When the process has ended, the end of the episode SHALL be recorded once at debug. A new process at a higher level SHALL be warned about again, also when it has the id of a process that ended.

The warnings SHALL be tracked per runtime, like all platform state (`per-runtime-platform-lifecycle`). The check SHALL only report. The input SHALL be sent as before, and the action SHALL succeed or fail as the sending does.

#### Scenario: Integrity levels are compared

- **GIVEN** PlatynUI at medium integrity
- **WHEN** a target's level reads high, system, medium or low, or access to it is denied, or it cannot be read
- **THEN** the target SHALL count as higher for high, system and denied access
- **AND** it SHALL NOT count as higher for medium and low
- **AND** it SHALL count as undetermined when its level cannot be read
- **NOTE:** A unit test that runs on every platform; no process is involved.

#### Scenario: A process at a lower level sees its parent as higher

- **GIVEN** a test that runs without elevation, and a child process of it that runs with a low-integrity token
- **WHEN** the child reads its parent's integrity level, and the parent reads the child's
- **THEN** the child SHALL count its parent as higher, whether it reads the parent's level or is denied access
- **AND** the parent SHALL read the child's level as low, which does not count as higher
- **NOTE:** Real Windows only, `just test-crate platynui-process` on a Windows host. No elevation is needed, because a process may always run at a lower level than its parent. This is the situation of a non-elevated PlatynUI and an elevated application, one level lower down.

#### Scenario: A process whose token refuses the query counts as higher

- **GIVEN** a child process whose token denies this user the right to query it
- **WHEN** its integrity level is read
- **THEN** the reading SHALL be denied access, which counts as higher
- **NOTE:** Real Windows only. No elevation is needed, because a token's owner may change the token's access control list.

#### Scenario: Input to a process of higher integrity is warned about once

- **GIVEN** a running process whose integrity level is higher than PlatynUI's
- **WHEN** keyboard input to that process is checked three times
- **THEN** exactly one warning SHALL name its process id and name, its level and PlatynUI's level, say that Windows discards the input, and name the remedy
- **AND** the second and the third check SHALL each be recorded at debug
- **NOTE:** Real Windows only, a unit test of the Windows platform on a Windows host. The test treats its own level as low, so that an ordinary child process is higher, and it checks the child directly; no foreground window changes.

#### Scenario: A process that ends ends its episode

- **GIVEN** a process of higher integrity that was warned about
- **WHEN** it ends, and input to another process of higher integrity is checked
- **THEN** one debug record SHALL say that the first process is gone
- **AND** one new warning SHALL name the second process
- **NOTE:** Real Windows only, with the setup of the previous scenario.

#### Scenario: A target at the same level is not reported

- **GIVEN** a process at the same integrity level as PlatynUI
- **WHEN** input to it is checked
- **THEN** no warning and no error SHALL be recorded
- **NOTE:** Real Windows only, with the same setup. This is the case of every healthy session.

#### Scenario: A target that cannot be determined is not reported

- **GIVEN** input whose receiving process cannot be determined, or a process that has ended before its level is read
- **WHEN** the input is checked
- **THEN** no warning SHALL be recorded
- **AND** the input SHALL be sent
- **NOTE:** Real Windows only, with the same setup.

#### Scenario: Without its own level, the check stays off

- **GIVEN** a runtime for which PlatynUI's own integrity level cannot be read
- **WHEN** it is created, and input is checked afterwards
- **THEN** one debug record SHALL say that input targets are not checked
- **AND** no check SHALL warn
- **NOTE:** A unit test of the Windows platform on a Windows host. The failure to read the own level is simulated, because a real process can always read it.

#### Scenario: An elevated application is named, and the keys and clicks do not arrive

- **GIVEN** the egui test app running elevated with its window in the foreground, and one PlatynUI runtime running without elevation
- **WHEN** `Keyboard Type` types twice without a target element, so that the keys go to the foreground window, and `Pointer Click` clicks twice at absolute coordinates inside that window
- **THEN** exactly one PlatynUI warning SHALL be logged, and it SHALL name the application and its level
- **AND** neither the keys nor the clicks SHALL reach the application
- **AND** when the same run is started elevated, no such warning SHALL appear, and the keys and the clicks SHALL arrive
- **NOTE:** Real Windows only, checked by hand with a scratch suite that imports the library once. Each `platynui-cli` call builds a runtime of its own and would warn again. Starting an application elevated needs the consent prompt, or an administrator's scheduled task, which the Windows lane does not do. A client without elevation cannot read the elevated application's elements either, so whether the input arrived is checked by eye.

#### Scenario: A healthy Windows lane has no such warning

- **GIVEN** the Windows acceptance lane in a session without elevated applications
- **WHEN** it runs
- **THEN** no warning and no error of the run SHALL come from this check
- **NOTE:** Real Windows only, checked with `uv run --no-sync robotcode results log --level WARN --execution-messages` after the lane.
