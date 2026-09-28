## 1. Robot Framework tests first

- [x] 1.1 Create the mock suite `tests/BareMetal/window_activation.robot`, following the `robot-test-style` skill. Cover every window-activation scenario the mock can observe:
  - normal → minimized → `Activate Window` comes back normal
  - maximized → minimized → `Activate Window` comes back maximized
  - activating a maximized background window keeps `@IsMaximized` and `@Bounds`
  - activating a normal window keeps it normal
  - `Bring To Front` on an element of a maximized window, and of a minimized window
  - `Pointer Click` into a maximized background window keeps it maximized
  - `Restore Window` un-maximizes

  Verify with `just build-native-mock` and `uv run --no-sync robotcode --profile mock run --suite "Window Activation" tests/BareMetal` (a bare file path skips the `mock` tag set in `tests/BareMetal/__init__.robot`): the maximized-preservation and restore-to-maximized tests fail on the current code, and the rest pass.
- [x] 1.2 Create the acceptance suite `tests/acceptance/egui/window_activation.robot`, which runs under the compositor and X11 like `auto_activate.robot`. Cover the real-only scenarios:
  - `@IsMaximized`/`@IsMinimized` after `Maximize Window`/`Minimize Window`
  - live re-read on the same node
  - no state attributes on a non-window element
  - `@IsTopmost` is `False` (tagged `platform:wayland`)
  - `Activate Window`/`Bring To Front` of minimized and maximized windows
  - a minimized window stays resolvable on the compositor

  Verify with `just headless=true test-acceptance-compositor --suite '*.Egui.WindowActivation'` and the same for `test-acceptance-x11` (RF matches normalized suite names, so no spaces), reading the outcome with `robotcode results`: the new tests fail on the current code because the attributes are missing and activation un-maximizes.
- [x] 1.3 Extend `tests/acceptance/swing/window.robot` with the JAB scenario: maximize → `@IsMaximized` is `True`, minimize → `@IsMinimized` is `True`, `Activate Window` brings the frame back. Verify on a Windows host with `just test-acceptance-windows` (expected red before group 5). Outcome (2026-09-28): green in the Windows run recorded under *Windows verification evidence*. It was never seen red, because the test and the code landed together in `1312085`.

## 2. Rust tests first

- [x] 2.1 Add runtime unit tests in `crates/runtime/src/runtime/window.rs`:
  - `bring_to_front` keeps a maximized mock window maximized;
  - `bring_to_front` brings back a window minimized from maximized as maximized;
  - `bring_to_front` returns `ActionFailed` when the window's activation errors (extend `test_fixtures.rs` with such a window if none exists).
  - the existing `bring_to_front_reports_missing_pattern` stays as the test for "element without an activatable window" (the mock tree has no such element for an RF test).

  Verify with `just test-crate platynui-runtime`: the first two fail on the current code.
- [x] 2.2 Add a unit test in `crates/core/src/platform/window_manager.rs`: a window manager that does not override the state query returns `CapabilityUnavailable` naming the window-state capability. Verify it compiles and fails until task 3.1 adds the method.
- [x] 2.3 Add tests to `crates/provider-mock/src/tests.rs` for the restore-to-maximized memory: maximize → minimize → activate gives maximized; maximize → minimize → restore gives normal; the memory is cleared by `restore`/`maximize`. Verify with `just test-crate platynui-provider-mock` (red before task 5.1).
- [x] 2.4 Add unit tests in `crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs`:
  - decoding a `list_windows` response with a minimized entry;
  - matching a minimized window by pid, title and size;
  - deriving the window state from the decoded info (minimized wins over maximized; topmost is false).

  Verify with `just test-crate platynui-platform-linux-wayland` (red before task 4.5).
- [x] 2.5 Add IPC integration tests in `apps/wayland-compositor/tests/ipc_tests.rs` (real compositor + egui client; `control.rs` has no unit-testable `State`) for the control-socket additions:
  - minimized entries carry content size and `maximized`;
  - `get_window` by `window_id` finds a minimized window and reports it as minimized.

  Verify with `just test-crate platynui-wayland-compositor` (red before task 4.4).

## 3. Core window-state type

- [x] 3.1 Add the window-state value type (exclusive visual state Normal/Minimized/Maximized, plus topmost) and the `WindowManager` state query with a default `CapabilityUnavailable` implementation to `crates/core/src/platform/window_manager.rs` (design D2). Re-export both from `platynui_core::platform`. Verify that test 2.2 passes and `just test-crate platynui-core` is green.

## 4. Platform window managers

- [x] 4.1 Implement the state query in `crates/platform-mock` (Normal, not topmost, plus a new `WindowManagerLogEntry` variant). Verify with a platform-mock unit test that asserts the returned state and the log entry.
- [x] 4.2 Implement the state query in `crates/platform-windows/src/window_manager.rs` (`IsIconic`, `IsZoomed`, `WS_EX_TOPMOST`; design D3). Verify on a Windows host with `just test-crate platynui-platform-windows`, or with `just pre-commit-cross` when no Windows host is available.
- [x] 4.3 In `crates/platform-linux-x11/src/window_manager.rs`, implement the state query (`_NET_WM_STATE` HIDDEN / MAXIMIZED_VERT+HORZ / ABOVE; add the needed atoms). In `activate`, map the client window before `_NET_ACTIVE_WINDOW` when the state is Minimized (design D4). Verify with `just test-crate platynui-platform-linux-x11` and the X11 activation scenarios from 1.2.
- [x] 4.4 In `apps/wayland-compositor/src/control.rs`:
  - give minimized-window entries content size and the maximized flag;
  - let `get_window` by `window_id` search the minimized list;
  - route `focus_window` through the foreign-toplevel activation path, with a `window_id` lookup that includes minimized windows (design D4, D5).

  Verify that tests 2.5 pass.
- [x] 4.5 In `crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs`:
  - decode `maximized` and the minimized array into the window info with a minimized flag;
  - let `resolve_window` and `resolve_window_info` consider minimized windows;
  - implement the state query and forward it in `window_manager/mod.rs`.

  Verify that tests 2.4 pass.

## 5. Providers

- [x] 5.1 In `crates/provider-mock/src/window.rs`, remember the maximized state on `minimize`, bring it back on activation of a minimized window, and clear it on `restore`/`maximize` (design D4). Verify that tests 2.3 pass and that the restore-to-maximized test from 2.1 passes.
- [x] 5.2 In `crates/provider-windows-uia/src/node.rs`, make `activate` call `ShowWindow(SW_RESTORE)` on an iconic native window handle before `SetFocus` (design D4). Verify on a Windows host with the egui activation scenarios in `just test-acceptance-windows`. Outcome (2026-09-28): `Egui.Window Activation` 7/7 and `Egui.Auto Activate` 8/8, see *Windows verification evidence*.
- [x] 5.3 In `crates/provider-atspi/src/node.rs`, add `IsMinimized`, `IsMaximized` and `IsTopmost` wherever `IsActive` is emitted (lazy standard-attribute kinds, index table, value resolution through the WindowManager state query, `False` on failure, a debug log on query errors; design D6). Verify with `just test-crate platynui-provider-atspi` and the attribute scenarios from 1.2 in both Linux lanes.
- [x] 5.4 In `crates/provider-java-jab/src/node.rs`, add the same three attributes next to `IsActiveAttr` for top-level nodes. Verify with the JAB scenario from 1.3 in `just test-acceptance-windows`. Outcome (2026-09-28): `Swing.Window` 4/4, served by the Access Bridge, see *Windows verification evidence*.

## 6. Runtime

- [x] 6.1 Remove the `restore()` step from `Runtime::bring_to_front` in `crates/runtime/src/runtime/window.rs` and update its doc comment to the activation contract. Verify that all 2.1 tests pass and that `just test-crate platynui-runtime` and `just test-crate platynui-cli` are green (including `window_bring_to_front_restores_minimized`).

## 7. Native rebuild and Python/RF surface

- [x] 7.1 Rebuild the mock native module and run the mock suite (`just test-baremetal`). Verify that `tests/BareMetal/window_activation.robot` from 1.1 is fully green.
- [x] 7.2 Update the user-facing texts to the activation contract: a minimized window comes back and a maximized one stays maximized; `Restore Window` is the way to un-maximize. The texts are the `Bring To Front` and `Activate Window` docstrings in `src/PlatynUI/BareMetal/__init__.py`, the `Activatable.activate` docstring in `packages/native/src/runtime.rs`, and the CLI `--bring-to-front` help in `crates/cli/src/commands/window.rs`. Follow the keyword-docstring style. Verify with `just check` (ruff, mypy, clippy).

## 8. Developer docs

- [x] 8.1 Extend `dev-docs/architecture.md` §8.5 (WindowManager) with explanatory prose, not signatures: what activation guarantees about minimized and maximized windows, and what the window-state query reports and who uses it. Verify that the section states the contract normatively, with no status or count wording.
- [x] 8.2 Update the WindowManager sections of `dev-docs/platform-windows.md` §3, `dev-docs/platform-linux.md` §3 and the PlatynUI-compositor window-manager description in `dev-docs/platform-linux-wayland.md`: where each backend reads the state and how activation brings a minimized window back. Flag any other divergence found rather than rewriting silently. Verify by re-reading each section against design D3/D4.

## 9. Verification

- [x] 9.1 Run `just check`, `just test`, `just test-python` and `just test-baremetal`. All must be green.
- [x] 9.2 Run `just test-acceptance-compositor` and `just test-acceptance-x11`, then read each run with `robotcode results`, since the compositor lane exits 0 even on failure. The new `window_activation.robot`, the existing `auto_activate.robot` and `inspector_window_controls.robot` ("Maximize Button Toggles The Window State") must pass. Investigate and record any change in the outcome of the latter.
- [x] 9.3 On a Windows host, run `just test-acceptance-windows` for the egui and Swing suites. Record whether a window minimized from maximized comes back maximized under Win32/UIA (the `SW_RESTORE` assumption in design Risks), and apply the documented fallback if it does not. Outcome (2026-09-28): it does. "A Window Minimized From Maximized Comes Back Maximized" passes under UI Automation, so no fallback is needed; see *Windows verification evidence*.

## Windows verification evidence

### The verifying run (2026-09-28)

The maintainer chose a fresh run at the current head over the earlier run below. Two commits had touched what the tasks verify since then: the log level of a refused foreground change (`4b1fc6c`) and two lines of `window_activation.robot` (`1dcf69d`).

- **Command:** `just test-acceptance-windows --profile real-windows run --suite Tests.Acceptance.Egui.WindowActivation --suite Tests.Acceptance.Egui.AutoActivate --suite Tests.Acceptance.Swing.Window`, on the maintainer's Windows host.
  - The recipe ran the ignored Java live tests of `platynui-provider-java` and `platynui-java-agent` first. They passed, because the recipe starts Robot Framework only after them.
- **Code:** `4b1fc6c`, with only documentation changed in the working tree.
- **Robot Framework:** 7.5 on Python 3.12.12, started 2026-09-28 12:06:26, 19 s.
- **Result:** 19 tests, 19 passed, 0 failed, 0 skipped. No WARN or ERROR message, and the `errors` section is empty.
  - `Egui.Window Activation`, 7/7:
    - Maximized State Is Read Live From The Same Window
    - Minimized State Is Reported And Activation Brings The Window Back
    - A Window Minimized From Maximized Comes Back Maximized
    - Activating A Maximized Background Window Keeps Its Size
    - Bring To Front Brings Back A Minimized Window
    - Pointer Click Into A Maximized Background Window Keeps It Maximized
    - Window State Attributes Exist Only On Windows
  - `Egui.Auto Activate`, 8/8.
  - `Swing.Window`, 4/4, served by the Access Bridge (`resources/testapp.resource` switches the agent off). This includes "Window State Is Reported And Activation Brings A Maximized Frame Back Maximized".
- **Raw results:** kept outside the repository, in the maintainer's untracked `playground/lane-2026-09-28-1206/`.

### A known gap on UI Automation, outside what this run shows

"Bring To Front activates the element's window without restoring it" does not hold for an element *inside* a window under UI Automation today. `pattern_by_name` serves the window patterns on every element (`crates/provider-windows-uia/src/node.rs:560-568`). So Bring To Front activates the element itself: `activate` applies `SW_RESTORE` only to the element's own handle and calls `SetFocus` on the element (`:604-617`).

The spec's scenario for an element of a minimized window is verified against the mock only. `gate-uia-window-patterns` closes the gap by gating those patterns on the window surface and activating the root window through the window manager.

### The earlier run (2026-09-27)

Recorded before the maintainer decided on a fresh run, and kept for reference. The raw results are in `playground/lane-2026-09-27-1809/`.

**The run**

- `just test-acceptance-windows` on the maintainer's Windows host, Robot Framework 7.5 on Python 3.12.12.
- Started 2026-09-27 18:09:29 and took 145 s.
- 119 tests: 119 passed, 0 failed, 0 skipped.
- No WARN or ERROR message, and the `errors` section is empty. The three FAIL-level messages come from `Run Keyword And Expect Error` in `Egui.Interaction` and `Egui.Query`.

**Code state**

- The run used `0802be7` plus the uncommitted working tree of `xdm-snapshot-release`, which was committed afterwards as `33fa58c`…`78bbe61` (18:22–18:25).
- The code these tasks verify has not changed since. `git log -L` gives the last commit that touched each part:

  | Code | Last commit |
  |---|---|
  | UIA `activate` (`crates/provider-windows-uia/src/node.rs:604-636`) | `6983c64` (2026-09-26) |
  | JAB window-state attribute (`crates/provider-java-jab/src/node.rs:1210-1250`, pushed at `:485-488`) | `1312085` |
  | `crates/platform-windows/src/window_manager.rs` | `f9592d2` (2026-09-25) |

- The egui test app did change after the run: `cbf8e1b` gave its button group bounds.

**1.3 and 5.4 (JAB)**

- `Swing.Window` passed 4/4, served by the Access Bridge (`resources/testapp.resource` switches the agent off).
- It includes "Window State Is Reported And Activation Brings A Maximized Frame Back Maximized".
- 1.3's "red before group 5" was never observable, because the test and the code landed together in `1312085`.

**5.2 and 9.3 (UIA)**

- `Egui.Window Activation` passed 7/7 in 7.69 s:
  - Maximized State Is Read Live From The Same Window
  - Minimized State Is Reported And Activation Brings The Window Back
  - A Window Minimized From Maximized Comes Back Maximized
  - Activating A Maximized Background Window Keeps Its Size
  - Bring To Front Brings Back A Minimized Window
  - Pointer Click Into A Maximized Background Window Keeps It Maximized
  - Window State Attributes Exist Only On Windows
- `Egui.Auto Activate` passed 8/8.
- Under Win32/UIA, a window minimized from maximized comes back maximized, so the `SW_RESTORE` assumption holds and the fallback in design Risks is not needed.
