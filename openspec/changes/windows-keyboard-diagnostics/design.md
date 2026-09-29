# Design

## Context

See proposal.md for the motivation and specs/windows-input/spec.md for the requirements. The facts below were checked in the working tree at `7180d20b` on 2026-09-29. The machine was Linux, so no Windows behavior was run: whatever Windows does is listed at the end of this section as an assumption, with the task that measures it. Wine does not count as a measurement.

**The Windows keyboard today** (verified):

- A key code is one of three kinds: a virtual key, one UTF-16 unit, or a character the keyboard layout maps to a key with modifiers (`crates/platform-windows/src/keyboard.rs:15-20`).
- `key_to_code` resolves a single character like this:
  - it casts the character to `u16` (`:140`);
  - if CapsLock is on and the *low byte* of that unit is an ASCII letter, it takes the layout's key and inverts Shift (`:141-158`);
  - otherwise `VkKeyScanW` maps the character, and Unicode input is the fallback (`:110-124`).
  The named aliases that stand for characters (`PLUS`, `LESS`, …) are all ASCII (`:511-516`).
- `send_vk` and `send_unicode` send one event each, and map "`SendInput` inserted nothing" to `KeyboardError::NotReady` (`:60-99`). A mapped character sends its modifiers and its key in separate calls (`:175-219`).
- `start_input` and `end_input` do nothing (`:164-166`, `:223-225`).
- The keyboard and the pointer are unit structs that the factory creates for each bundle (`crates/platform-windows/src/factory.rs:72-80`, `pointer.rs:20`).

**How the runtime drives the keyboard** (verified):

- The sequence parser passes each character to `key_to_code` as a string of its own (`crates/runtime/src/keyboard_sequence.rs:236-240`). A character outside the Basic Multilingual Plane therefore reaches the device whole; only the device cuts it.
- `\u` takes four hex digits, and a surrogate half fails as an invalid escape, because it is no character (`keyboard_sequence.rs:410-420`).
- A keyword parses and resolves the whole sequence, then starts the device, sends, and ends the device (`crates/runtime/src/runtime/input.rs:350-366`, `crates/runtime/src/keyboard.rs:25-66`). A key's press and release are separate device calls with delays between them (`keyboard.rs:183-199`).
- BareMetal brings the element's window to the front and focuses the element before it calls the runtime (`src/PlatynUI/BareMetal/__init__.py:2708-2717`).

**The Windows pointer today** (verified):

- `send_mouse_input` reads the last error before it logs anything, records the error at debug and returns `CapabilityUnavailable { capability: "SendInput", details: "failed: <code>" }` (`crates/platform-windows/src/pointer.rs:89-108`, `:146-149`).
- The runtime moves the pointer to the target before a press (`crates/runtime/src/pointer.rs:387-408`). It calls `press` and `release` once per click (`:408-410`, `:428-436`, `:495-505`), `scroll` once per step (`:462`), and `move_to` for every step of an interpolated motion (`:546-629`).

**Errors and how they reach the user** (verified):

- `KeyboardError::Platform(PlatformError)` exists; `NotReady` displays as "keyboard provider is not ready" (`crates/core/src/platform/keyboard.rs:76-94`).
- The runtime wraps a device failure as "sending the keyboard input failed: <device error>" (`crates/runtime/src/runtime/error.rs:45-47`). For a `Secret` it drops the device's text: `NotReady` becomes "no keyboard device is ready", and a platform error becomes "sending the keyboard input failed" (`error.rs:98-121`). A test pins the second case with the stub keyboard, which fails with a platform error (`input.rs:694-700`, `crates/runtime/src/runtime/test_fixtures.rs:694-732`).
- `dev-docs/error-handling.md` lists "send_event failed" under `OperationFailed`, but the pointer reports a refused `SendInput` as `CapabilityUnavailable` (decision 3).

**The logging rules that bind this change** (verified):

- A record that names a key, character, key code or keysym, or that carries a keyboard device's error text, is trace-level (`dev-docs/logging.md` §3, §16; `.github/instructions/logging.instructions.md` §3).
- A `SendInput` failure is read from the operating system first, then recorded at debug, then returned (`logging.md` §4).
- A condition that recurs is reported once per episode through `platynui_core::diagnostics::Transitions`, and the owner of the latch defines what ends an episode (`logging.md` §5; `crates/core/src/diagnostics.rs:30-92`).
- The model for a process as the subject is the Java agent's version mismatch: it warns once per JVM, records later occurrences at debug, and ends the episode when the process is gone (`crates/provider-java/src/agent/backend.rs:103`, `:204-217`, `:225-242`).
- A runtime owns its platform state; two runtimes share no mutable process-global platform state (`openspec/specs/per-runtime-platform-lifecycle/spec.md`).

**Process facts** (verified):

- `platynui-process` records a process by its pid and creation time. It opens a process with the limited query right, and adds `SYNCHRONIZE` where that is granted (`crates/process/src/lib.rs:110-229`).
- It reads process attributes only while the process is still the recorded one (`crates/process/src/attributes/win32.rs:54-58`), and it already opens a process token and reads `TokenUser` from it (`win32.rs:124-192`).
- It depends on no PlatynUI crate (`crates/process/Cargo.toml:24-25`). UIA, JAB, the Java provider and AT-SPI depend on it. `platform-windows` does not; its only PlatynUI dependencies are `platynui-core` and the leaf crate `platynui-java-agent` (`crates/platform-windows/Cargo.toml:15-22`).
- Windows-only logic in `platynui-process` is unit-tested on every platform through `#[cfg(any(windows, test))]` helpers (`crates/process/src/attributes.rs:133-190`). `platform-windows`, by contrast, is empty on other platforms (`Cargo.toml:15-17`, `src/lib.rs:15-43`).
- Its tests already change a child's token access control list without elevation (`deny_token_query`, `crates/process/src/attributes/tests.rs:351-414`).
- `ProcessIdentity` derives `Debug`, `Clone`, `PartialEq` and `Eq`, but not `Hash` (`crates/process/src/lib.rs:64-70`).

**Integrity levels in the code** (verified): nothing reads one. Two places touch the subject:

- The JAB pump reads its own token's *elevation flag*, to open the UIPI message filter when PlatynUI runs elevated (`crates/provider-java-jab/src/pump.rs:226-279`).
- The UIA `WaitForInputIdle` checker opens the target with `PROCESS_QUERY_INFORMATION` and answers "not idle" when access is denied (`crates/provider-windows-uia/src/node.rs:42-60`).

**The `windows` crate** (verified in the 0.62.2 sources):

- `TokenIntegrityLevel`, `TOKEN_MANDATORY_LABEL`, `GetSidSubAuthority` and `GetSidSubAuthorityCount` are in `Win32_Security`, which `platynui-process` enables.
- `WindowFromPoint` and `GetCursorPos` are in `Win32_UI_WindowsAndMessaging`, which `platform-windows` enables.
- `CreateDesktopW` and `SetThreadDesktop` are in `Win32_System_StationsAndDesktops`; `CreateDesktopW` also needs `Win32_Security` and `Win32_Graphics_Gdi`.
- `SetTokenInformation`, `CreateWellKnownSid` and `DuplicateTokenEx` are in `Win32_Security`. `CreateProcessAsUserW` needs `Win32_Security` with `Win32_System_Threading`.

**What Windows is assumed to do** (not run here; from Microsoft's documentation; measured by the tasks named):

- **A1.** Windows discards input that UIPI blocks, and `SendInput` reports this neither in its return value nor through `GetLastError` (its documented remarks). Task 1.1.
- **A2.** `SendInput` called from a thread whose desktop is not the input desktop inserts nothing and sets an error, usually `ERROR_ACCESS_DENIED`. This is the locked-workstation case. Task 1.3.
- **A3.** A process can open a process of higher integrity with the limited query right; `platynui-process` already relies on this for process attributes. Whether it may then open that process's token depends on the token's access control list: either the level is read, or access is denied. Task 3.2 records which.
- **A4.** Windows delivers keyboard input to the foreground thread, and button and wheel input to the window under the pointer (for the wheel, with Windows' default of scrolling inactive windows under the pointer). UIPI judges the process of the window that receives the input.
- **A5.** A standard edit control, and the toolkits, join a high and a low surrogate that arrive as consecutive `WM_CHAR` messages. The order "both key-downs, then both key-ups" is the one commonly used with `SendInput` for such a pair. Tasks 3.6 and 2.1.
- **A6.** A process may lower the integrity level of its own token, and a lower level takes effect for its later access checks. Raising it needs a privilege. Task 3.2.

## Goals / Non-Goals

**Goals:**

- A key event that Windows refuses fails the way a refused mouse event already does: with the operating system's error, and without naming the key.
- Every character is typed as itself.
- Input that Windows will discard because of UIPI is reported once per receiving process, where the user reads it, with the remedy. A healthy session stays silent.
- The integrity reader lives where the UI Automation provider can use it later.

**Non-Goals:**

- Failing or blocking input to a process of higher integrity (decision 5).
- Telling a locked workstation from other refusals in the error (see Open Questions).
- CapsLock handling that follows the keyboard layout for characters of the Basic Multilingual Plane (decision 2).
- Changing how injected modifiers are released, or sending a mapped character's modifiers and key in one call.
- The UIA `WaitForInputIdle` fix for elevated targets, JAB's own elevation check, and a host process that carries `uiAccess`.
- The Linux keyboards. The X11 keyboard already maps every character to a Unicode keysym (`crates/platform-linux-x11/src/keyboard.rs:490-508`); whether the Linux lanes type characters outside the Basic Multilingual Plane is not examined here (decision 9).
- A `\U` escape with eight hex digits. Robot Framework test data already has `\U0001F600`.

## Decisions

### 1. A key code carries a character, and the Unicode path sends all of its UTF-16 units in one call

`WinKey::Unicode` carries a `char` instead of a `u16`. `key_to_code` looks at the scalar value of a single character:

- A character of the Basic Multilingual Plane takes today's path, unchanged: the CapsLock heuristic, `VkKeyScanW`, and Unicode input as the fallback. The code units are the same as before, because such a character is one UTF-16 unit.
- A character outside it goes straight to the Unicode path. `VkKeyScanW` takes a single UTF-16 unit, no keyboard layout has a key for such a character, and Unicode input does not depend on CapsLock.

To send a Unicode key, the device builds one keyboard event per UTF-16 unit of the character: one for a character of the Basic Multilingual Plane, two for a surrogate pair. Every event is marked as Unicode input, and a release also as key-up. The events follow the UTF-16 order, and one `SendInput` call inserts them. So a press sends high-down and low-down, and a release sends high-up and low-up. Building the events is a pure function of the character and the key state, and is unit-tested without sending anything.

Why one call: `SendInput` inserts the events of one call without interleaving any other input. With two calls, a key of the user's or of another thread could land between the halves, and a failure between the calls would leave half a character.

Why both key-downs and then both key-ups: this is the device's own model. A key code has one press and one release, which the runtime sends separately with delays in between (`crates/runtime/src/keyboard.rs:183-199`). A character is produced on its key-down (the key-down becomes a `WM_CHAR` message in the application), so the application receives the two halves as consecutive `WM_CHAR` messages. This rests on A5. Task 3.6 checks it against Windows' own edit control, and task 2.1 against egui.

*Alternatives rejected:*

- **Keep the `u16` and add a variant for pairs.** That gives one concept two representations. A `char` is what the parser has, and `encode_utf16` is the one source of the units.
- **Tap each unit inside the press** (high-down, high-up, low-down on press; low-up on release). This splits the character's key-down and key-up unevenly between press and release, so `Keyboard Press` would hold half a character. It remains the fallback if task 3.6 shows that the edit control rejects the chosen order.
- **Reject characters outside the Basic Multilingual Plane with `UnsupportedKey`.** That would be honest, but it is needless, because Windows can type them.

### 2. The CapsLock heuristic for characters of the Basic Multilingual Plane stays as it is

The heuristic tests the low byte of the UTF-16 unit (`keyboard.rs:141-158`), so U+0141 (`Ł`) reads as `A`. It is also wrong the other way round: CapsLock changes the case of the non-ASCII letters of many layouts, the German `ä` for one, and the heuristic never adjusts those.

A correct rule depends on the layout. The question is whether the key that the layout assigns to the character is one that CapsLock affects, which for example `ToUnicodeEx`, called with the CapsLock state, can answer. Replacing the low-byte test with an ASCII test would only trade one guess for another. It would change the outcome for letters whose low byte happens to be an ASCII letter, such as `Ł` (U+0141, low byte 0x41) or `š` (U+0161, low byte 0x61), without knowing whether their keys follow CapsLock in the user's layout — which is the layout question again.

So characters of the Basic Multilingual Plane keep exactly today's path. The follow-up is listed under Open Questions. Characters outside the Basic Multilingual Plane never reach the heuristic (decision 1).

### 3. A refused insertion: read the last error first, record its code at debug, return it as a platform error

Every `SendInput` call of the keyboard goes through one helper. The helper takes the prepared events, calls `SendInput` once, and counts the call as refused when fewer events were inserted than given. On a refusal it:

1. reads `GetLastError` *first*, because a log subscriber that runs in between could overwrite it (`logging.md` §4);
2. writes a debug record;
3. returns `KeyboardError::Platform`, built by the pointer's last-error helper (`pointer.rs:146-149`). That helper moves to a place that both devices reach.

The error is therefore `CapabilityUnavailable { capability: "SendInput", details: "failed: WIN32_ERROR(<code>)" }`, exactly what the pointer returns. A user reads "sending the keyboard input failed: platform error: platform capability unavailable: SendInput: failed: WIN32_ERROR(5)", and for a `Secret` "sending the keyboard input failed" (`error.rs:98-121`).

The debug record carries the Win32 error code as a number, how many events were given and how many were inserted, and whether it was a press or a release. It carries no key, no character, no code unit, and not the device's error text (the typed-text rule). If a key code is ever recorded, then only at trace. A suggested message: "SendInput inserted no key event".

A partial insertion can only happen for a surrogate pair, where one of the two events is inserted. It counts as refused. `GetLastError` is still read and reported, even though it may be 0. The comparison of inserted and given events is a small pure check with its own unit test.

The path of a mapped character keeps sending its modifiers and its key in separate calls (`keyboard.rs:175-219`); each call goes through the helper.

*Alternatives rejected:*

- **`OperationFailed { operation: "SendInput" }`**, as `error-handling.md` suggests for "send_event failed". It is more precise by the document, but the keyboard and the pointer would then report the same refusal in two ways; mirroring the pointer keeps one report for one kind of refusal. Whether both devices should switch is listed under Open Questions.
- **Naming the likely cause in the error**, such as a locked workstation. The review criticized a message that guesses its cause (the `SetForegroundWindow` finding). The typical causes go into the documentation instead; a detection based on facts is listed under Open Questions.
- **A new `KeyboardError` variant.** `KeyboardError::Platform` exists, so `platynui-core` does not change.

### 4. Where the integrity check runs, and what "target" means

UIPI judges the process that *receives* the input. The device cannot know which element the test meant, but it can know where Windows will deliver the input (A4):

- **Keyboard:** the foreground window's thread receives keyboard input. The check runs in `start_input`, which the runtime calls once per keyword. It runs after the sequence is parsed and resolved, and, in BareMetal, after the automatic activation and the focus. So the foreground window at that moment is the one the keys go to.
- **Pointer:** button and wheel input goes to the window under the pointer. The check runs in `press` and in `scroll`, on `WindowFromPoint(GetCursorPos())`. The runtime has already moved the pointer to the target, so this is where the click lands. `release` is not checked, because it goes to the same window as its press. Neither is `move_to`, which the runtime calls for every step of a motion.
- **From the window to the process:** `GetWindowThreadProcessId`. With no window, or pid 0, nothing is checked. PlatynUI's own process is not checked either, because its level is PlatynUI's own.

The check takes the target — a window and its process — as a value. The keyboard and the pointer find it as described above, and the tests hand one in directly.

Limits: a sequence that changes the foreground window itself (`<Alt+Tab>`) is checked where it starts; the next keyword checks again. A mouse capture held by another window is not considered.

*Alternatives rejected:*

- **The element's process** (its `ProcessId`). It is wrong exactly when it matters: when the element's window is not where the input goes, for example because an activation failed and an elevated window stayed in front. The runtime also has no notion of integrity levels.
- **Checking every key event.** It costs something per key and repeats the records, for a condition that changes between keywords, not between keys.
- **A new device hook that the runtime calls.** That would change a core trait for a concern that only Windows has. `start_input` exists, and the architecture describes it as the hook before a burst of input (`dev-docs/architecture.md` §8.2).

### 5. The check reports and does not block: one warning per receiving process and runtime

**Level.** A warning: PlatynUI knowingly works with less than it was asked for. The input is sent, but Windows discards it; the cause is the environment or the target application; and it is not a returned failure (A1).

**Once per episode** (`logging.md` §5):

- The subject is the receiving process as a `ProcessIdentity`, its pid together with its creation time, so a reused pid is a new subject.
- Further occurrences are recorded at debug.
- The episode ends when the process is gone. This is recorded once, at debug. Before each check, the latch forgets the processes whose identity reports them as ended (`Transitions::retain` with `ProcessIdentity::check`). That is cheap: the latch only holds processes that were warned about, and in a healthy run it holds none.

**Owner.** Each platform bundle has one check, and its keyboard and its pointer share it; the factory creates it. The latch therefore lives per runtime, as `per-runtime-platform-lifecycle` requires. Each suite's runtime warns once more about the same elevated application, as the other latches do (the agent's per-JVM latch, the AT-SPI timeout latch). Each `platynui-cli` call is a runtime of its own, so it warns once per call, which is what a command-line user needs.

**Content:**

- `pid`;
- `application`, the process name read through the identity. It is built inside the logging macro, so it costs nothing while the level is off (`logging.md` §12);
- `integrity`, such as `high`, or `unreadable`;
- `own_integrity`;
- `hwnd`, in hexadecimal, as `crates/provider-java/src/provider.rs:359` writes it.

Suggested messages:

- level known: "input goes to a process with a higher integrity level; Windows discards the keys and clicks PlatynUI synthesizes for it (run PlatynUI at that level, for example elevated, or the application without elevation)";
- access denied: "input goes to a process whose integrity level PlatynUI may not read, which usually means that it runs elevated; Windows discards synthesized input to a process of higher integrity (run PlatynUI elevated, or the application without elevation)";
- at debug, while the episode continues: "input still goes to a process with a higher integrity level";
- at debug, when it ends: "a process with a higher integrity level that received input is gone".

The input is sent regardless of the check.

**The plan rests on A1, and task 1.1 measures it with today's build.** Windows might refuse UIPI-blocked input instead, so that `SendInput` inserts fewer events than it was given. Then the refusal already fails the action (decision 3), and a warning would report the same failure a second time (`logging.md` §4). In that case the check's finding goes into the returned error's details and the debug record instead of a warning. The third requirement of the spec is changed to match, and the design and the spec are updated before the check's tests are written (task 3.5).

*Alternatives rejected:*

- **Failing the action when the target is higher.** The finding is an inference: a token that cannot be read counts as higher (decision 6), a host with `uiAccess` is not considered, and a sequence can change the foreground window itself. A failed test would turn a possibly wrong inference into a failure. A warning with the remedy gives the diagnosis without that risk.
- **One warning per process for the whole PlatynUI process** (`std::sync::Once`, or a global set). That is mutable process-global platform state, which the lifecycle spec forbids.
- **Keying the latch by pid alone.** A reused pid would suppress the warning for a new elevated process. The agent latch can use the pid, because the agent's handshake file tells it when the JVM is gone; here only the creation time can.

### 6. What counts as higher, and why denied access counts

Levels are compared by the RID of the token's mandatory label, which is what UIPI compares. The elevation flag, which JAB reads, is not enough: levels other than medium and high exist (system, medium plus), and the review of this finding asked for the comparison of levels for that reason.

When access to the process or its token is denied, the process counts as higher. On a desktop, the processes whose token a non-elevated user cannot read are expected to be the user's own elevated processes and the system's processes (A3), and both are higher. A process of another user at the same level (started with "Run as different user") would be denied too, and would get a false warning. That setup is rare; the warning says that the level could not be read, and the documentation says what it means.

Any other failure to read the level counts as "undetermined" and produces no warning: the process has ended, it is no longer the recorded one, or a call failed unexpectedly.

PlatynUI's own level is read once per bundle. If it cannot be read, the check is off for that bundle, and this is recorded once at debug.

A host process that carries `uiAccess` is exempt from UIPI, but the check does not consider it. PlatynUI's executables carry no `uiAccess` manifest; a host that does would get a warning that Windows does not enforce. See Open Questions.

### 7. The reader lives in `platynui-process`, the policy in `platynui-platform-windows`

`platynui-process` gains, as facts:

- an integrity level: the RID, ordered, with names for the well-known levels (untrusted, low, medium, medium plus, high, system, protected process) and a display form that uses those names;
- a reading with three outcomes: a level, denied access, or undetermined;
- `ProcessIdentity::integrity`. On Windows it opens the recorded process as the crate does today, confirms that it is still the recorded one, opens the token with `TOKEN_QUERY`, and takes the last sub-authority of the `TokenIntegrityLevel` label. Elsewhere it answers "undetermined";
- the level of the calling process;
- the comparison of a reading with PlatynUI's own level, following decision 6, as a method with no platform dependency;
- `Hash` on `ProcessIdentity`.

The types and the comparison compile and are tested on every platform. The Windows reader, and its mapping of failed calls to readings, are Windows-only and are tested on a Windows host.

`platynui-platform-windows` gains the check: it finds the target, reads the level through the identity, keeps the latch and writes the records. It gains `platynui-process` as a dependency, gated to Windows like its other dependencies.

Why the split falls here:

- `platynui-process` already opens processes in one way (the limited query right, with the identity check) and already reads a token (`TokenUser`). A second copy of that code in the platform would drift.
- Providers may not depend on platform crates (`dev-docs/architecture.md:681-690`, and decision 1 of `gate-uia-window-patterns`). So only a reader in `platynui-process` can also serve the UIA `WaitForInputIdle` fix. UIA, JAB and the Java provider already depend on the crate.
- `platynui-process` depends on no PlatynUI crate, so the new dependency of the platform creates no cycle. The same reasoning let the platform depend on `platynui-java-agent` (`crates/platform-windows/Cargo.toml:19-22`).

*Alternatives rejected:*

- **Everything in `platform-windows`.** It would be shorter now, but UIA could not share it, and the comparison could be tested only on Windows, because the crate is empty elsewhere.
- **An integrity attribute on application nodes** (`app:IntegrityLevel`). It would be useful in the Inspector, but it is a new public attribute that needs the `application-process-attributes` spec and every provider. That is a change of its own.
- **Reusing JAB's elevation check.** It reads the elevation flag, not the level (decision 6). JAB can move to the new reader later.

### 8. Tests are placed by what they need

- **Every platform, `just test`:** in `platynui-process`, the level type, the readings, and the comparison's truth table.
- **A Windows host, `just test-crate`, without the ignored tests:**
  - `platynui-process`:
    - the process's own level can be read;
    - a child whose token denies the query reads as denied, using the existing `deny_token_query`;
    - a child that lowers its own token to low integrity reads as low, and it reads its parent as higher. The child is the test binary started again, as `WaitingChild` is, and it reports its reading on its output, as `WaitingChild` reports that it runs. The test records whether the child read its parent's level or was denied access (A3), and whether the child could lower its own level (A6).
  - `platynui-platform-windows`:
    - the events built for `😀` and for `ä` (decision 1);
    - the check of a partial insertion;
    - on a thread moved to a desktop that the test creates, a key press, the press of a character on the Unicode path, and a pointer press all fail with the `SendInput` error, and the captured records name no character (decision 3, A2);
    - the check with an injected own level of low, against real child processes of the test binary: one warning, then debug records; the end of the episode, and a new warning for a new process; silence at the same level; silence when the target is undetermined; and no check, with one debug record, without an own level.
  - None of these tests affects the user's desktop. The private desktop swallows the input, and the tests of the check hand in a target instead of looking at the foreground window.
- **The Windows lane, as an ignored test** (it changes the foreground window): the test types `a😀z` into an edit control in a window of its own, which it activates with the platform's own window manager, as the ignored activation test of `gate-uia-window-patterns` does. It reads the text back with `WM_GETTEXT`. `test-acceptance-windows` gains `-p platynui-platform-windows` in its ignored-only run (`justfile:391`).
- **The Windows lane, in Robot Framework:** `Keyboard Type` of `<Ctrl+A>a😀z` into the egui field `input-name`, read back through `control:Text` with `Wait Until Attribute Value`, in `tests/acceptance/egui/interaction.robot`, tagged `platform:windows` (decision 9).
- **By hand, on real Windows:** the measurement of A1 (task 1.1) and the scenario with an elevated application. The lane cannot start an elevated application without the consent prompt.

### 9. The acceptance test is tagged for Windows

The bug and the fix belong to the Windows keyboard. The X11 keyboard maps every character to a Unicode keysym (`crates/platform-linux-x11/src/keyboard.rs:490-508`), but nobody has sent an emoji through it or through the Wayland backends yet. An untagged test could turn two lanes red for reasons outside this change. The tag follows `acceptance-lane-selection`: it selects, it never skips. Whether to drop it later is listed under Open Questions.

The test types `a😀z` rather than the emoji alone. That shows that the characters around the pair stay in order, and that the layout path still works next to the Unicode path.

## Risks / Trade-offs

- **[A1 is wrong, and Windows refuses UIPI-blocked input]** → Task 1.1 measures it before anything is built, and decision 5 says what changes then. The spec and the design are updated first.
- **[A2 is wrong, and `SendInput` succeeds from a private desktop]** → Task 1.3 measures it with harmless events first: key F24 and a relative pointer move of zero distance. Only then are the refusal tests written, the pointer press among them. If A2 does not hold, another way to provoke a refusal is chosen and recorded (for example, an event with an invalid type). Failing that, the refusal is checked by hand on a locked workstation.
- **[A5 does not hold for a toolkit]** → The test against the edit control separates PlatynUI from the toolkits. If egui fails while the edit control passes, the limit is in egui's text input: the Robot Framework test then moves to the Qt test app's text field, and the egui limit is recorded here.
- **[A6 does not hold, and a process cannot lower its own level]** → The child is started with a lowered copy of the parent's token instead (`CreateProcessAsUserW`, as Microsoft's guide to low-integrity processes describes). If that needs a privilege the lane does not have, the "lower sees higher" reading is checked only by hand, with the elevated application, and the denied-token test remains the automated check of the denied reading.
- **[A false warning]** → Possible for a process of another user at the same level, or under a host with `uiAccess`. Both are rare, the warning says when the level could not be read, and the documentation explains it.
- **[The warning repeats per runtime]** → As with the other latches. The alternative would break `per-runtime-platform-lifecycle`.
- **[Cost]** → Per keyword, press or scroll step: a handful of system calls (`GetForegroundWindow` or `WindowFromPoint`, `GetWindowThreadProcessId`, opening the process and its token, `GetTokenInformation`). That is nothing against the default 50 ms per keystroke. Moves are not checked.
- **[Debug records while an episode lasts]** → The runtime scrolls in steps (`crates/runtime/src/pointer.rs:462`), so one scroll into an elevated window writes several debug records. They are debug only.
- **[Only a Windows host runs the platform tests]** → Linux type-checks and lints them with `just check-windows` and `just clippy-windows`, which build `--all-targets`. The tasks require a run on real Windows; Wine does not count.
- **[Coordination]** → `gate-uia-window-patterns` edits the same ignored-only line of the justfile and the BareMetal documentation near "Bringing windows to the front"; this change adds its paragraph after "Window control". `uniform-wait-keywords` rewrites other parts of the BareMetal documentation, and `x11-atspi-healthy-run-warnings` adds to the same table of episode subjects in `dev-docs/logging.md` §5. Several open changes edit other sections of `dev-docs/platform-windows.md`. Whichever lands second rebases.

## Migration Plan

- **Behavioral, and additive in the API.** `platynui-process` gains types and methods, and `ProcessIdentity` gains `Hash`. Nothing changes in the public API of `platynui-core`, the runtime or the Python binding. The behavior changes are listed in the proposal.
- **Native rebuild:** yes, on Windows. On Linux and macOS the additions to `platynui-process` compile and answer "undetermined".
- **Sequence:**
  1. The measurements (tasks 1.1–1.3).
  2. The tests: Robot Framework and Rust, red where the spec says so.
  3. The integrity reader.
  4. The keyboard: characters, and refused insertions.
  5. The check.
  6. The documentation.
  7. The lanes, and the check by hand.
- **Rollback:** revert. The three parts are independent:
  - the surrogate pair (`keyboard.rs`);
  - the refusal path (`keyboard.rs` and the shared helper);
  - the check (the new module, its wiring, and the reader).
  Each can be reverted on its own; the reader does no harm without its user.

## Open Questions

- Should a refused insertion become `OperationFailed` for both devices, as `error-handling.md` suggests for "send_event failed"? That would change the pointer's error text as well, so the decision is the maintainer's. This change keeps the pointer's variant for both.
- Should a refused insertion also check whether the input desktop can be opened (`OpenInputDesktop`) and then say, as a fact, that the input desktop is not accessible (a locked workstation, a secure desktop)? That needs measurements on a locked workstation and in a remote session; it is a follow-up.
- CapsLock handling that follows the layout for characters of the Basic Multilingual Plane (decision 2) is a follow-up for the Windows keyboard. It needs tests on real Windows with German, Polish and Czech layouts.
- Should the check read `TokenUIAccess` of its own token, and stay off when it is set? Only if someone runs PlatynUI from a host with `uiAccess`.
- The `platform:windows` tag of the acceptance test can go once the X11 and compositor lanes are shown to type characters outside the Basic Multilingual Plane.
- JAB's elevation check (`pump.rs:233-279`) and the UIA `WaitForInputIdle` fix can use the new reader, each in its own change.
