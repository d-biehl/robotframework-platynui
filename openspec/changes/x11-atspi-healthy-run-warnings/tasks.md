# Tasks

The change is Linux-only and about the log. No keyword changes, so there is no Robot Framework test. Rust unit tests with log capture come first, and none of them needs an X server or an accessibility bus. The real providers are checked on the X11 and compositor lanes, whose warnings are inspected, and by hand on a bare `Xvfb`.

## 1. Before the change

- [ ] 1.1 Record the Linux lanes' warnings before the change. Run `just headless=true test-acceptance-x11`, then `uv run --no-sync robotcode results log --level WARN --execution-messages`. Then do the same with `just headless=true test-acceptance-compositor`. Note here, for each lane:
  - the result;
  - every WARN or ERROR that comes from PlatynUI. None is expected (logging-concept task 6.2).

  Verify: both results are recorded here, so that 7.2 can compare against them.

## 2. Tests first — X11 platform (`crates/platform-linux-x11`)

- [ ] 2.1 Add `src/test_log.rs`, declared `#[cfg(test)]` in `src/lib.rs` next to the other modules. It captures records at debug without colours and offers `logged`, `at_level` and `warnings`, like `crates/provider-atspi/src/test_log.rs`. `tracing-subscriber` is already a dev-dependency. Verify: `just test-crate platynui-platform-linux-x11` compiles and stays green.
- [ ] 2.2 In the tests of `src/window_manager.rs`, test the missing-hints function with `test_atoms()` (`:1044-1062`, design decision 2):
  - a supported list with `_NET_CLIENT_LIST`, `_NET_ACTIVE_WINDOW` and `_NET_CLOSE_WINDOW`, but without `_NET_WM_PID`, misses nothing (spec: *`_NET_WM_PID` is not a window-manager hint*);
  - a list with only `_NET_CLIENT_LIST` misses `_NET_ACTIVE_WINDOW` and `_NET_CLOSE_WINDOW`, in that order (*Missing hints are one warning*);
  - an empty list misses the three hints and never `_NET_WM_PID`.

  Verify: the tests fail to compile before 4.1.
- [ ] 2.3 In the same module, test the reporter of design decision 1 with `logged`. Findings are passed in, and every test uses display names of its own, because what was reported is process-wide:
  - no window manager reported twice for `:71`: exactly one warning, which names `:71`, the reason, and that windows cannot be looked up. The second report is debug (spec *A display without a window manager is reported once*);
  - no window manager for `:72` and for `:73`: one warning each (*Each display is reported on its own*);
  - a failed check for `:74` (an `OperationFailed` error), then no window manager for `:74`: exactly one warning, which names `:74` and the error (*A check that fails is the same finding*);
  - a window manager without `_NET_ACTIVE_WINDOW` and `_NET_CLOSE_WINDOW`, reported twice for `:75`: exactly one warning, which names the window manager, `:75` and both hints (*Missing hints are one warning*);
  - a window manager with every hint on `:76`: no warning, and the info record of the detected window manager;
  - a window manager whose `_NET_SUPPORTED` could not be read, on `:77`: no warning and one debug record.

  Verify: the tests fail to compile before 4.1.
- [ ] 2.4 Test the lookup's decision of design decision 3, with the geometry match and the name match passed in as closures:
  - no candidate, looked up ten times: ten errors that name the PID and say that no window belongs to it, no warning or error in the log, and ten debug records with `pid`, `clients` and `without_pid` (spec *A window lookup that finds no window adds no warning*);
  - two candidates that neither closure matches: an error saying that none of the process's windows matches the element, one debug record naming the PID and both window ids, and no warning (*Several windows that match none are named at debug*);
  - regression guards:
    - one candidate is taken without calling a closure;
    - with several candidates, a geometry match wins without calling the name closure;
    - a name match is taken when the geometry does not match.

  Verify: the tests fail to compile before 4.2. Together with `a_window_that_keeps_failing_is_reported_once_across_reads_and_rebuilt_nodes` in `crates/provider-atspi/src/extents.rs:458-471`, which stays green, they cover *The bounds of a window that cannot be found are warned about once*.
- [ ] 2.5 In the `process_id` test module (`src/window_manager.rs:965-1042`), test the lookup's first step (design decision 4):
  - A node without a positive `ProcessId`, whose ancestors have none either, fails. The error says that the element's application has no process ID to look its window up by, and does not contain `UiNode`. The captured log has no warning (spec *An element whose application has no process ID says so*).
  - A node below an application with a process ID yields that process ID.

  Verify: the first test fails before 4.3.

## 3. Tests first — AT-SPI provider (`crates/provider-atspi`)

- [ ] 3.1 In the tests of `src/timeout.rs`, next to the timeout tests (`:275-422`), test slowness in the registry (design decisions 5 and 6). Elapsed times are passed in, just above and exactly at the threshold, and the existing `gedit()` registry (`:1.7`, `gedit`, pid 4711) is the application:
  - One slow operation gives no warning, and one debug record with `bus_name`, `call` and `elapsed_ms`. An operation exactly at the threshold gives no record (spec *A single slow operation stays at debug*).
  - Ten slow operations give exactly one warning, at the third. It names `application="gedit"`, `pid=4711` and `bus_name=:1.7`, and says that queries that include the application are slow. The other nine are debug records (*An application that stays slow is named once*).
  - When `retain` drops `:1.7`, one debug record ends the episode. Three slow operations of a new instance then warn again (same scenario).
  - Two applications with two slow operations each give no warning (*Slow operations of different applications are not added up*).
  - A timeout of `:1.7` through the existing `time_out` helper, then three slow operations: exactly one warning, the timeout's, and three debug records for the slow operations (*An application that times out is not warned about again for being slow*).
  - A slow warning, then a timeout: two warnings (design decision 6).

  Verify: the tests fail to compile before 5.1.
- [ ] 3.2 Test the window-state helper of design decision 7 with a stub window manager like the one in `src/extents.rs:269-330`. If both test modules need it, move it into a shared `#[cfg(test)]` module:
  - A lookup that fails with `OperationFailed` gives `IsActive` `False` and one debug record. The record names the window's description, `bus_name`, `path` and the error. There is no warning (spec *A False from a window that cannot be found can be told apart*).
  - A lookup that succeeds while `is_active` fails gives `False`, and a debug record naming the window, `window_id` and the error (*A window whose activity cannot be read*).
  - A lookup that fails with `CapabilityUnavailable` gives `False` for `IsActive` and `IsMinimized`, and no record (*A window manager without window management adds no record*).
  - A window manager that answers everything gives no record for any of the four attributes (*A window the window manager answers for adds no record*).

  Verify: the tests fail to compile before 5.3.

## 4. X11 platform

- [ ] 4.1 Implement design decisions 1 and 2 in `src/window_manager.rs` and `src/lib.rs`:
  - `check_ewmh_wm_support` returns the finding instead of logging. The checked hints are `_NET_CLIENT_LIST`, `_NET_ACTIVE_WINDOW` and `_NET_CLOSE_WINDOW`, and a pure function computes the missing ones.
  - A `_NET_SUPPORTED` that cannot be read is recorded at debug and misses nothing.
  - The reporter keeps a process-wide set of reported pairs of display and finding. It warns on the first report and records later ones at debug, and it keeps the info record of the detected window manager.
  - `create_x11_bundle` calls the reporter instead of `lib.rs:106-110`.
  - The warnings at `window_manager.rs:908`, `:920` and `:954` are gone. The messages and fields follow `dev-docs/logging.md` §11.

  Verify: 2.2 and 2.3 pass.
- [ ] 4.2 Implement design decision 3:
  - Move the decision of `find_xid_for_pid` (`:188-227`) into a function whose X11 reads come in as closures.
  - Count the clients without `_NET_WM_PID` in the candidate loop (`:180-186`).
  - Replace the warnings at `:190` and `:219` with the debug records of the decision.
  - Leave the error values and the debug records of a resolved window (`:197`, `:206`, `:215`) as they are.

  Verify: 2.4 passes.
- [ ] 4.3 Implement design decision 4. The first step of `resolve_window` (`:684-685`) returns the `resolve X11 window by PID` error that says the element's application has no process ID. Verify: 2.5 passes, `just test-crate platynui-platform-linux-x11` is green and `just clippy` is clean.

## 5. AT-SPI provider

- [ ] 5.1 Implement design decisions 5 and 6 in `src/timeout.rs`:
  - The slow threshold is a constant equal to `TIMEOUT_CALL`, and the count of three is a constant next to it.
  - `AppTimeouts` gets an entry point that takes a measured operation (bus name, call, elapsed time) and alone compares the time with the threshold.
  - Each application's entry records its slow count and whether it is in a timeout episode, both updated under the registry's lock.
  - The third slow operation of an instance warns; every other slow operation is debug, and a slow operation during a timeout episode is debug and not counted.
  - `retain` records the end of a slow episode once at debug.
  - The module's table and the doc comment of `AppTimeouts` say that the registry keeps slowness too.

  Verify: 3.1 passes and the existing timeout tests stay green.
- [ ] 5.2 Hand both measurements to the registry:
  - `children()` in `src/node.rs` (`:384-403`) reports with the call `Accessible.GetChildren`. The per-node warning and its literal `1000` are gone, and the trace record stays.
  - `get_nodes` in `src/lib.rs` (`:334-347`) reports with the call `resolve application`. The literal and the debug record there are gone.

  Verify: `just test-crate platynui-provider-atspi` is green, and `grep -n "as_millis() > 1000" crates/provider-atspi/src` finds nothing.
- [ ] 5.3 Implement design decision 7 in `src/node.rs`:
  - One helper serves `resolve_window`, `resolve_is_active_window` and `resolve_window_state` (`:1604-1644`), with the debug records of the decision.
  - `CapabilityUnavailable` is not recorded.
  - The state record at `:1643` gets the fields `error`, `window_id` and `window`.
  - Every description is built inside the macro's fields.

  Verify: 3.2 passes, `just test-crate platynui-provider-atspi` is green and `just clippy` is clean.

## 6. Documentation

- [ ] 6.1 Update `dev-docs/logging.md`:
  - §5:
    - add a row to the table of subjects: an AT-SPI application that stays slow. Its subject is the application instance, keyed by its bus name; the provider owns the registry, and the episode ends when the instance leaves the bus. Say what "stays slow" means there: three slow operations;
    - add the X11 display's window-manager findings to the examples of *Once per process*, as once per display.
  - §19: map the four new requirements of `diagnostic-logging` to the sections that explain them.

  Keep the prose explanatory and normative, with no counts of findings. Verify: every new requirement in `specs/diagnostic-logging/spec.md` appears in §19.
- [ ] 6.2 Update `dev-docs/platform-linux.md`:
  - §1, *Initialization* (`:78-81`): add the EWMH check, the three hints it checks, and that each finding is warned once per display;
  - §3 (`:195-203`):
    - a failed window lookup is returned and recorded at debug with the process ID and the candidates;
    - a window-state attribute that reads `false` because the window cannot be found is recorded at debug;
    - `_NET_WM_PID` is read from the client windows and is not a hint the window manager has to advertise.

  Keep it as short as the neighbouring bullets. Verify by reading both sections against the spec.
- [ ] 6.3 In `openspec/changes/archive/2026-09-27-logging-concept/review-findings.md`, extend the Status lines of the eight entries of follow-up A1 (`:137`, `:142`, `:147`, `:188`, `:201`, `:211`, `:250`, `:255`), following the file's own convention: each says that `x11-atspi-healthy-run-warnings` implements it. Verify: `grep -n "follow-up A1\|ride-along A1" openspec/changes/archive/2026-09-27-logging-concept/review-findings.md` shows each of the eight with its new status.

## 7. Verification

- [ ] 7.1 Run `just check` and `just test`, then `just build-native`. `just build-native` comes last, because a plain `uv run` inside `just check` can replace the native module with a build that links no providers, and 7.2 and 7.3 need the real build. Verify: everything is green.
- [ ] 7.2 Run `just headless=true test-acceptance-x11`, then `uv run --no-sync robotcode results log --level WARN --execution-messages`. Then do the same for `just headless=true test-acceptance-compositor`. Verify:
  - both lanes are green;
  - neither has a WARN or ERROR from PlatynUI (spec *A healthy run reports no PlatynUI warning*);
  - the results match 1.1.

  Record both runs here.
- [ ] 7.3 Check by hand on a bare `Xvfb` without a window manager (spec *A display without a window manager is reported once*):
  - start `Xvfb :97 -nolisten tcp` in the background;
  - run `uv run --no-sync python -c "from platynui_native import Runtime; [Runtime(config={'platform': {'backend': 'x11', 'x11': {'display': ':97'}}}).shutdown() for _ in range(2)]"`;
  - stop `Xvfb`.

  Verify: stderr shows exactly one warning from PlatynUI about the window manager. It names `:97` and says that windows cannot be looked up. Record the output here.

## 8. Commit (only when the user asks)

- [ ] 8.1 Commit in reviewable steps. Each step carries the tests it turns green, so each one builds, is lint-clean and passes on its own:
  - the X11 platform, with 2.1-2.5 and 4.1-4.3, under the scope `platform-linux-x11`;
  - the AT-SPI provider, with 3.1-3.2 and 5.1-5.3, under the scope `provider-atspi`;
  - the docs and the review-findings status, 6.1-6.3.

  Subjects are at most 72 characters, without `!`. The bodies name the behavior changes of the proposal for the release notes. Do not push.
