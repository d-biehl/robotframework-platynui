# Tasks

Windows behavior counts as verified only on real Windows (the Windows VM or another Windows host); Wine does not count. The tests change nothing on the user's desktop unless they are ignored tests, which only the Windows lane runs. No test uses the taskbar, the shell or applications that Windows ships: the tests use their own processes, windows and desktops, and the repository's test apps.

## 1. Before the change

- [ ] 1.1 Measure assumption A1 (design, Context) on real Windows with today's build.
  - Build with `just build-native` and `cargo build -p platynui-cli -p platynui-test-app-egui`.
  - Start the egui test app elevated, from the console with the consent prompt or through a scheduled task with the highest run level, and bring its window to the front.
  - From a shell without elevation, run `platynui-cli keyboard type "abc"` and `platynui-cli pointer click --point <x,y>` with a point inside that window.
  - Record here, for each command, whether it succeeds while nothing arrives (A1 holds), or fails. The keyboard fails with "keyboard provider is not ready"; the pointer fails with the `SendInput` error and its code.
  - If either command fails, stop. Change design decision 5 and the third requirement of `specs/windows-input/spec.md` as decision 5 describes, before 3.5 is written.
- [ ] 1.2 Record the Windows lane's state before the change: run `just test-acceptance-windows`, then `uv run --no-sync robotcode results log --level WARN --execution-messages`. Note the result and any warning from PlatynUI here; none is expected.
- [ ] 1.3 Measure assumption A2 on a Windows host with harmless events, in a scratch test that is not committed.
  - Move a new thread to a desktop that the test creates (`CreateDesktopW`, `SetThreadDesktop`).
  - From that thread, call `SendInput` with the press and release of F24, and with a relative pointer move of zero distance.
  - Record the return values and the Win32 error codes here.
  - If `SendInput` inserts the events, choose another way to provoke a refusal before 3.4 is written, and record it here (design, Risks).

## 2. Tests first — Robot Framework acceptance

Follow the `robot-test-style` skill.

- [ ] 2.1 In `tests/acceptance/egui/interaction.robot`, add "Keyboard Type Types A Character Outside The Basic Multilingual Plane", tagged `platform:windows` (spec scenario "A character outside the Basic Multilingual Plane is typed into an application").
  - The test runs `BM.Keyboard Type    .//*[@Id="input-name"]    <Ctrl+A>a\U0001F600z`, then `BM.Wait Until Attribute Value    .//*[@Id="input-name"]    control:Text    ==    a\U0001F600z`. Robot Framework resolves `\U0001F600` to the emoji before the keyword runs.
  - Its documentation says why the test is tagged for Windows (design decision 9).
  - Verify with `uv run --no-sync robotcode analyze code tests/acceptance/egui`.
  - On the Windows lane, run `just test-acceptance-windows --profile real-windows run --suite '*.Egui.Interaction'` and verify that the test fails before the change, with `a`, U+F600 and `z` in the field.

## 3. Tests first — Rust

- [ ] 3.1 `platynui-process`, on every platform — unit tests for:
  - the integrity level: its order, the names of the well-known levels, and the display of a level that has no name;
  - the comparison with an own level of medium, following the spec scenario "Integrity levels are compared": high, system and denied access count as higher; medium and low do not; an undetermined reading stays undetermined;
  - a `ProcessIdentity` as the key of a `HashSet`.

  Verify on Linux with `just test-crate platynui-process` that they fail to compile.
- [ ] 3.2 `platynui-process`, on a Windows host — tests that read real processes:
  - the process's own level can be read, and it is at least low;
  - a child whose token denies the query (`deny_token_query`) reads as denied (spec scenario "A process whose token refuses the query counts as higher");
  - a new child mode of the test binary, next to `waiting_child`, lowers its own token to low integrity and then reports on its output how it reads its parent. The parent reads the child as low, which is not higher, and the child counts its parent as higher (spec scenario "A process at a lower level sees its parent as higher").

  Record here whether the child read its parent's level or was denied access (A3), and whether it could lower its own level (A6). If it could not, start the child with a lowered copy of the parent's token instead (design, Risks). Verify on Linux with `just check-windows` that the tests type-check once 4.1 exists, and on a Windows host with `just test-crate platynui-process` that they fail to compile before 4.1.
- [ ] 3.3 `platynui-platform-windows`, on a Windows host — tests that send nothing:
  - the press of `😀` builds one insertion of two Unicode key-down events, U+D83D and then U+DE00, and its release builds the two matching key-up events, in the same order;
  - `ä`, `A` and `Escape` resolve as before;
  - the insertion check counts 0 of 1 and 1 of 2 inserted events as refused, and 2 of 2 as inserted.

  Change `unicode_for_non_ascii` (`crates/platform-windows/src/keyboard.rs:646-656`) to expect the character instead of a `u16`. Verify with `just check-windows` on Linux that they fail to compile before 5.1.
- [ ] 3.4 `platynui-platform-windows`, on a Windows host — refused insertions, on a thread that the test moves to a desktop it creates, with the records captured by `test_log::logged`. Write it only after 1.3 confirmed A2, or with the other provocation that 1.3 recorded.
  - A key press of F24 fails with `KeyboardError::Platform`, names `SendInput` and gives a Win32 code. It does not fail with `NotReady`.
  - The press of `😀` fails the same way. Neither the error nor the records contain the character, `D83D`, `DE00` or a key code, and exactly one debug record carries the Win32 code.
  - A pointer press fails with the `SendInput` error.

  Add the dev-dependency features that the desktop calls need (`Win32_System_StationsAndDesktops`, `Win32_Security`). Verify on a Windows host that the keyboard cases fail before 5.2, because they get `NotReady`, and that the pointer case already passes.
- [ ] 3.5 `platynui-platform-windows`, on a Windows host — the integrity check, following the scenarios of the spec's third requirement. The tests inject the own level and use real child processes of the test binary, with a small waiting child like the one in `platynui-process`'s tests; the records are captured.
  - Three checks of a child that counts as higher produce one warning, with pid, application, integrity, own level and the remedy, and two debug records.
  - After that child is killed, checking a second higher child produces one debug record that the first is gone, and one warning for the second.
  - A child at the own level produces nothing above debug, and so does a child that ended before its level was read.
  - A denied reading produces the warning that says the level cannot be read.
  - Without an own level, there is one debug record when the check is created, and no check warns.

  Verify on a Windows host that the tests fail to compile before 6.1.
- [ ] 3.6 `platynui-platform-windows`, as an ignored test, because it changes the foreground window (spec scenario "The standard Windows edit control receives the character").
  - The test creates a window with a standard edit control, activates the window with the platform's window manager, focuses the control, and asserts that the window manager reports the window active before it types.
  - It types `a😀z` through the keyboard device, pumps the window's messages, and expects `a😀z` from `WM_GETTEXT`.
  - Add `-p platynui-platform-windows` to the ignored-only `cargo nextest` run of `test-acceptance-windows` (`justfile:391`).

  Verify with `just check-windows` on Linux. On a Windows host, verify that `cargo nextest list -p platynui-platform-windows --run-ignored ignored-only` lists the test, and that it fails before 5.1, with `a`, U+F600 and `z`.

## 4. The integrity reader (`platynui-process`)

- [ ] 4.1 Implement design decisions 6 and 7 in `platynui-process`:
  - the integrity level, the reading (a level, denied access, undetermined) and the comparison, none of them platform-dependent;
  - `ProcessIdentity::integrity` and the calling process's level. The Windows reader opens the recorded process as the crate already does, confirms that it is still the recorded one, opens the token with `TOKEN_QUERY`, and takes the last sub-authority of the `TokenIntegrityLevel` label. Denied access gives "denied"; every other failure gives "undetermined"; on other platforms the answer is "undetermined";
  - `Hash` on `ProcessIdentity`;
  - the crate documentation in `crates/process/src/lib.rs`.

  Verify that 3.1 passes on Linux (`just test-crate platynui-process`) and 3.2 passes on a Windows host, and that `just clippy` and `just clippy-windows` pass.

## 5. The keyboard: characters and refused insertions

- [ ] 5.1 Implement design decision 1 in `crates/platform-windows/src/keyboard.rs`:
  - `WinKey::Unicode` carries a `char`;
  - `key_to_code` keeps today's path for a character of the Basic Multilingual Plane, the CapsLock heuristic included, and sends every other character to the Unicode path;
  - the pure event builder;
  - `send_unicode` inserts all UTF-16 units of the character with one call.

  Verify on a Windows host that 3.3 and the existing keyboard tests pass. 3.6 passes on the Windows lane in 8.3.
- [ ] 5.2 Implement design decision 3:
  - every `SendInput` call of the keyboard goes through the insertion helper;
  - the helper reads the last error first, and writes the debug record with the code, the numbers of given and inserted events and the key state, without naming a key;
  - the pointer's last-error helper moves where both devices can reach it, and the pointer's behavior does not change.

  Verify on a Windows host that 3.4 passes and that the tests in `pointer.rs` stay green.

## 6. The integrity check

- [ ] 6.1 Implement the check of design decisions 4 to 6 as a new module of `platynui-platform-windows`:
  - the own level is read once. If it cannot be read, the check is off, with one debug record;
  - the latch is keyed by `ProcessIdentity`, and forgets the processes that have ended;
  - the target comes from a window through `GetWindowThreadProcessId`. With no window, pid 0 or PlatynUI's own pid, nothing is checked;
  - the records of decision 5;
  - `platynui-process` becomes a dependency, gated to Windows like the crate's other dependencies, with a comment in `Cargo.toml` like the one on `platynui-java-agent`.

  Verify on a Windows host that 3.5 passes, and on Linux that `just clippy-windows` passes.
- [ ] 6.2 Wire the check in:
  - `create_windows_bundle` creates one check and hands it to the keyboard and the pointer;
  - `start_input` checks the foreground window;
  - `press` and `scroll` check the window under the pointer;
  - `release` and `move_to` do not check.

  Verify on a Windows host that `just test-crate platynui-platform-windows` passes, factory tests included, and that a keyboard or pointer action on a window of the own level records nothing above debug.

## 7. Documentation

- [ ] 7.1 Update:
  - `dev-docs/platform-windows.md` §1:
    - keyboard: characters outside the Basic Multilingual Plane as surrogate pairs in one insertion; refused insertions, with their typical causes (a locked workstation, the UAC prompt, a disconnected remote session); the integrity check;
    - pointer: the same check. The pointer already reported refused insertions;
  - `dev-docs/logging.md` §5, the table of latches: a row for input to a process of higher integrity. The subject is the receiving process, by pid and creation time. The owner is the Windows platform's input check, one per runtime. The episode ends when the process is gone;
  - `dev-docs/keyboard-input.md` §6: a character outside the Basic Multilingual Plane is written as it is, or as `\U0001F600` in Robot Framework test data. `\u` takes one character of the Basic Multilingual Plane, and a surrogate half is an invalid escape;
  - the BareMetal library documentation (`src/PlatynUI/BareMetal/__init__.py`): a short section "Applications that run elevated" after "Window control", in the voice of the other sections. On Windows, keys and clicks reach an application that runs as administrator only when PlatynUI runs as administrator too; PlatynUI warns once per application; the remedy.

  Verify by reading, and with `just check`.

## 8. Verification

- [ ] 8.1 On Linux, run `just check`, `just test`, `just check-windows` and `just clippy-windows`. Verify that everything is green, including the platform-independent tests of 3.1.
- [ ] 8.2 On a Windows host, run `just test-crate platynui-process`, `just test-crate platynui-platform-windows` and `just test`, then `just build-native`. Verify that everything is green, including 3.2 to 3.5.
- [ ] 8.3 On Windows, run `just test-acceptance-windows`, then `uv run --no-sync robotcode results log --level WARN --execution-messages`. Verify:
  - everything is green, including 2.1 and the ignored 3.6;
  - there is no WARN and no ERROR from PlatynUI, compared with 1.2.

  Record the outcome here.
- [ ] 8.4 By hand on Windows, the spec scenario "An elevated application is named, and the keys and clicks do not arrive". Use a scratch suite, not committed, that imports `PlatynUI.BareMetal` once, so that one runtime does all four actions: `Keyboard Type    ${None}    abc` twice, and `Pointer Click    x=<x>    y=<y>` twice at a point inside the window. (Each `platynui-cli` call is a runtime of its own and would warn again.) With the elevated egui test app of 1.1 in front, run the suite from a shell without elevation and verify:
  - exactly one warning names the application and its level, and `uv run --no-sync robotcode results log --level WARN --execution-messages` shows it;
  - neither the keys nor the clicks arrive, checked by eye;
  - run from an elevated shell, the suite logs no such warning, and the keys and clicks arrive.

  Record the outcome here.
- [ ] 8.5 CI runs the X11 and compositor lanes on push. Verify there that they are green, that the new acceptance test is not selected, and that their logs have no WARN or ERROR from PlatynUI. Record the run here.

## 9. Commit (only when the user asks)

- [ ] 9.1 Commit in reviewable steps. Each step builds, passes lint and passes its tests on its own:
  - the integrity reader of `platynui-process`, with 3.1 and 3.2;
  - the characters outside the Basic Multilingual Plane, with 3.3, 3.6 (including the justfile line) and 2.1;
  - the refused insertions, with 3.4;
  - the integrity check, with 3.5;
  - the documentation.

  Use Conventional Commits, subjects of at most 72 characters, no `!`. The keyboard commits list the behavior changes of the proposal in their bodies. Do not push without an explicit instruction.
