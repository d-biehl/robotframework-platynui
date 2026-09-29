# Design

## Context

See proposal.md for the motivation. Facts were verified in the working tree at `7180d20b` (2026-09-29). *Assumed* marks what was inferred and not run.

**The rules this change applies.**

- A warning is never produced in a healthy session. A condition that recurs on every call is warned once per episode, and a slow call is debug (`dev-docs/logging.md:94`, §3 *Slow calls* `:143-151`, §5 `:199-243`). The rule for slow calls has three parts:
  - each call class has one threshold, kept as a named constant;
  - a slow call is recorded at debug with `call` and `elapsed_ms`;
  - a subject that *stays* slow is warned once per episode.
- Log or return: a layer that returns a failure logs it at most at debug (§4, `:153-197`).
- Field names: §11 (`:410-430`). `window` is a window's description there, not its id, and `bus_name` names the D-Bus peer.
- The `diagnostic-logging` spec states the same rules (`openspec/specs/diagnostic-logging/spec.md:38-84`), including *A healthy run reports no PlatynUI warning* (`:59-64`).
- The logging-concept change left the slow-call threshold values to its follow-ups (`openspec/changes/archive/2026-09-27-logging-concept/design.md:420`).

**X11: the EWMH check.**

- `create_x11_bundle` runs once per runtime (`crates/platform-linux-x11/src/lib.rs:63-127`), and every suite of an acceptance lane builds its own runtime (`dev-docs/testing-strategy.md` §2.6).
- `check_ewmh_wm_support` (`crates/platform-linux-x11/src/window_manager.rs:890-959`):
  - warns and returns `Ok(false)` when `_NET_SUPPORTING_WM_CHECK` is missing (`:907-910`), or when the check window does not point back to itself (`:912-922`);
  - logs the window manager's name at info (`:936`);
  - reads `_NET_SUPPORTED`, and turns a failed read into an empty list (`:939-944`);
  - warns once for each of four atoms missing from `_NET_SUPPORTED`, `_NET_WM_PID` included (`:946-956`).
- The factory warns again on `Ok(false)`, and puts the error of a failed check into the message text (`lib.rs:106-110`).
- The window lookup reads `_NET_WM_PID` from every client window, whatever `_NET_SUPPORTED` says (`window_manager.rs:125-128`, `:180-186`).
- The atom cache is process-wide (`:50`). The own-window verdict is kept per window manager instance (`:536-566`).

**X11: the window lookup.**

- `resolve_window` (`:683-699`) starts with `extract_pid` (`:377-392`), which walks up to the first positive `control:ProcessId`. Since `atspi-application-level`, only the application-level node carries that attribute. Without one, the error reads `platform operation failed: extract PID from UiNode` (`:684-685`).
- `find_xid_for_pid` (`:170-229`) takes as candidates the managed clients whose `_NET_WM_PID` equals the PID. Clients without `_NET_WM_PID` are skipped silently (`:181`). The number of candidates decides:
  - none: it warns and returns an error (`:188-195`);
  - one: it takes that window (`:196-199`);
  - several: it tries a geometry match, then a name match. When neither fits, it warns with only the count and returns an error (`:200-227`).
- Nothing caches the result, and the AT-SPI provider builds a fresh attribute context for every attribute read (`crates/provider-atspi/src/node.rs:439-449`, `:1098-1109`). The lookup therefore runs again for each of these:
  - every top-level `Bounds` read, through `extents::window_manager_bounds` (`crates/provider-atspi/src/extents.rs:68-83`, `node.rs:1659-1664`), including the reads of a nested element's bounds, which sum up to the top-level;
  - every `IsActive`, `IsMinimized`, `IsMaximized` and `IsTopmost` read (`node.rs:1604-1644`, `:1809-1823`);
  - every window action (`node.rs:666-673`), whose error the provider wraps as `AT-SPI D-Bus error in resolve_window: …`;
  - every frame the point hit-test checks (`crates/provider-atspi/src/lib.rs:594-625`, the call at `:620`).
- A top-level whose bounds the window manager cannot answer for is already reported once per window and episode (`extents.rs:94-150`, the warning at `:128-139`). The `wayland-compositor-detection` spec requires it (*A substituted window geometry is visible in the log*, `openspec/specs/wayland-compositor-detection/spec.md:120`).

**AT-SPI: slow operations.**

- `children()` (`node.rs:362-427`) measures `Accessible.GetChildren` together with the liveness checks of grafted popups (`:365-385`). Above a literal `1000` it warns per node, with the field `bus` and the message `children: SLOW get_children (>1000ms)` (`:396-403`).
- `get_nodes` measures each application's resolution: identity, proxy, child count, interfaces, role and name (`lib.rs:264`, `:334-347`). Above the same literal it records debug.
- `TIMEOUT_CALL` is 1 s (`crates/provider-atspi/src/timeout.rs:38-39`). A compound operation exceeds 1 s only when several of its calls add up to it, or when one of them timed out. A timed-out call is already reported through the registry.
- `AppTimeouts` (`timeout.rs:146-239`) is the provider's per-application registry, keyed by bus name. It holds:
  - a `Transitions` latch;
  - what enumeration learned about each application (name, pid).

  `get_nodes` prunes it against the registry's applications at each enumeration (`lib.rs:249-252`, `timeout.rs:178-193`). A timeout warns once per application instance (`:195-238`).

**AT-SPI: window-state reads.**

- Two places discard the window manager's error:
  - `LazyNodeData::resolve_window` discards the lookup error (`node.rs:1604-1610`);
  - `resolve_is_active_window` discards the error of `is_active` (`:1629-1636`).
- `resolve_window_state` records a failed state read at debug, with a bare `%err` and `window = %wid`, although §11 reserves `window` for a description (`:1638-1644`).
- All four attributes read `False` when this fails (`:1809-1823`), as `window-state` requires (`openspec/specs/window-state/spec.md:64`).
- The two Wayland window managers fail differently:
  - A generic Wayland compositor answers every window-manager call with `CapabilityUnavailable` (`crates/platform-linux-wayland/src/window_manager/mod.rs:99-108`, `crates/platform-linux-wayland/src/capabilities.rs:142-152`).
  - The PlatynUI compositor returns `OperationFailed` when it finds no window (`crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs:70-83`).

**Tests today.**

- `provider-atspi`:
  - `src/test_log.rs` captures records at debug (`logged`, `at_level`, `warnings`). It is used by the tests of `timeout.rs` (`:275-422`), `extents.rs` and `lib.rs`.
  - `extents.rs` has a stub window manager that fails on demand (`:269-330`).
- `platform-linux-x11`:
  - `x11util.rs` has a capture for warnings only (`:125-148`).
  - The tests of `window_manager.rs` exercise pure functions. X11 calls are passed in as closures, as in `skips_as_own` (`:488-504`) and `OwnershipCell` (`:540-566`), and a set of test atoms stands in for a real server (`:1044-1062`).
  - The only tests against a real `Xvfb` are the ignored PID-namespace tests (`tests/pidns_tests.rs`, `just test-x11-pidns`).

**Assumed.**

- The X11 lane's window manager, icewm (`scripts/startxsession.sh:284`), advertises `_NET_CLIENT_LIST`, `_NET_ACTIVE_WINDOW` and `_NET_CLOSE_WINDOW`. Inferred from the logging-concept verification (task 6.2), whose X11 lane showed no PlatynUI warning.
- No lane fixture has a top-level window that the X11 lookup cannot find, and no lane application is slow three times. Inferred from the same run.

## Goals / Non-Goals

**Goals:**

- None of the sites in the proposal warns in a healthy X11 or compositor run, however many runtimes, polls and reads the run makes.
- Every warning that remains names its subject and its consequence, and fires once per process and display, or once per episode.
- Every failure stays traceable at debug, with what was tried.
- Every decision can be tested with log capture, without an X server or an accessibility bus.

**Non-Goals:**

- **The substitution warning** (`extents.rs:128-139`). It stays as `wayland-compositor-detection` requires. How the provider classifies Avalonia's `PopupRoot` is an open question.
- **Caching window lookups.** That would change liveness: a window that appears or is re-created has to be found by the next read.
- **More EWMH hints** in the check, such as `_NET_CLIENT_LIST_STACKING` for the hit-test or `_NET_WM_STATE` for window state (decision 2).
- **Warnings outside the A1 list:**
  - the per-runtime RANDR warning (`lib.rs:102`) and the per-instance X-Resource warning (`window_manager.rs:548-554`), which fire only where the server lacks the extension;
  - the X11 connect errors (`x11util.rs:111-112`, priority B "when touched");
  - the Inspector's slow-call warning (`apps/inspector/src/model/tree_data.rs:399`, priority B "when touched"). The new requirement binds it once it is changed.
- **Error wrapping outside the lookup:**
  - the AT-SPI wrapper that labels a window-manager failure a D-Bus error (`node.rs:671`). It shapes errors and is not about logging;
  - JAB's discarded lookup errors (`crates/provider-java-jab/src/node.rs:1204-1205`, `:1244`), which are Windows and not in A1.
- **The AT-SPI functional bugs** the review found: role memoization, and `Rect(0,0,0,0)` extents.
- **Renaming** `AppTimeouts`, and a sweep over field names in records this change does not touch. The triage dropped that sweep.

## Decisions

### 1. The EWMH check returns a finding, and one reporter logs it once per process for each display

`check_ewmh_wm_support` stops logging and returns what it found:

- a window manager that advertises every checked hint, with its name;
- a window manager that lacks some hints, with its name and the missing hints;
- no usable window manager, with the reason:
  - no `_NET_SUPPORTING_WM_CHECK`;
  - a check window that does not point back to itself;
  - a check that failed, with its error.

A failed check is a finding, not an `Err`. The bundle factory hands the finding to one reporting function in `window_manager.rs`, which replaces `lib.rs:106-110`. The reporter does three things:

- **Info.** It keeps the info record of the detected window manager, as today (`:936`).
- **Warnings.** It warns the first time a display shows a finding, and records at debug when that display shows it again. There are two findings:
  - no usable window manager: `display`, and `reason` or `error`. The message says that there is no EWMH window manager on this display, and that windows cannot be looked up, so window actions and window state do not work;
  - missing hints: `display`, `wm`, `missing`. The message says that the window manager does not advertise these EWMH hints, and that the window operations that use them may have no effect.
- **The record of what was reported.** "Already reported" is a process-wide set of pairs of display and finding. It sits beside the process-wide atom cache: a `Transitions` that is never told of a recovery, or a plain set.

*Alternatives rejected:*

- **Keep the warnings in the check and drop `lib.rs:108`** (the review's proposal). The once-per-process guard would then be needed at three sites: two in the check, and the error in the factory. The check would also stay untestable without an X server.
- **A process-wide `Once`.** A second display without a window manager would stay silent.
- **An episode latch with recovery.** The check runs only when a runtime is created, so an episode could end only at a later creation, which no test run produces. The set reports the same with less.
- **Warn at the first window operation instead,** as *A capability that is not there says so once* does for other capabilities. On X11 the AT-SPI provider asks the window manager for every top-level window's bounds, so the capability matters with the first query that reaches a window. Moving the report would add a latch to the hottest path for the same outcome.

### 2. `_NET_WM_PID` is not a window-manager hint, and missing hints are one warning

- The checked hints are the ones PlatynUI needs from the window manager:
  - `_NET_CLIENT_LIST`, for the window lookup;
  - `_NET_ACTIVE_WINDOW`, for activation and `IsActive`;
  - `_NET_CLOSE_WINDOW`, for Close Window.
- A pure function computes the missing hints from the supported atoms and the atom cache. The existing test atoms can drive it.
- `_NET_WM_PID` is a property that applications set on their own windows, and the lookup reads it regardless. `java-provider-linux` treats it the same way (`openspec/changes/java-provider-linux/design.md:80`).
- A `_NET_SUPPORTED` that cannot be read (request or reply error) is recorded at debug and does not count as "every hint missing". The check cannot tell, and a warning on a transient error would fire in a healthy session. A `_NET_SUPPORTED` that is absent or empty does count, because then the window manager advertises nothing.

*Alternatives rejected:*

- **Extending the list.** New hints would add warnings on window managers that were never checked against the lanes. A later change can add them together with a lane check.
- **One warning per missing hint,** as today: one condition, up to three warnings.

### 3. A failed window lookup is returned and recorded at debug with what it tried

- The decision part of `find_xid_for_pid` (`:188-227`) becomes a function of the PID, the candidate windows, the number of clients looked at, and two closures for the geometry match and the name match. This is the pattern of `skips_as_own`. The X11 reads stay in `find_xid_for_pid`.
- It writes two debug records:
  - no candidate: `no X11 window found for the process`, the form `.github/instructions/logging.instructions.md:88-89` already shows. Its fields are `pid`, `clients` (the managed windows looked at) and `without_pid` (those that set no `_NET_WM_PID`, which the lookup skips silently today);
  - several candidates and no match: `several X11 windows belong to the process and none matches the element`, with `pid`, `candidates` (the window ids), and the hints `name` and `extents`.
- The debug records of a resolved window (`:197`, `:206`, `:215`) stay; the triage dropped their consolidation.
- The error values stay as they are.
- The consumers' reports stay the only ones above debug:
  - the substitution warning, once per window and episode;
  - the error of a window action;
  - a frame the picker skips;
  - the debug record of a window-state read (decision 7).

*Alternatives rejected:*

- **A latch in the X11 window manager that warns once per window or PID.** The window manager does not know what a failure means to its caller: for the picker it is normal, for bounds the provider already warns, and for an action the error reaches the user. A second once-per-window warning would only double the provider's.
- **Caching the resolved window.** It changes liveness and is not a logging question (Non-Goals).

### 4. The error for an element without a process ID says so

- The first step of `resolve_window` returns an `OperationFailed` with the operation `resolve X11 window by PID`, like the no-window error (`:192`), and the details *the element's application has no process ID to look its window up by*. The three lookup failures then read alike, and none names an internal type.
- Nothing new is logged. The substitution warning already prints the error, and the triage chose this text over a new record in `identity.rs`.
- The step becomes a small function of the node, tested with the process-ID test nodes (`window_manager.rs:965-1042`).

### 5. One threshold, debug per slow operation, and a warning once per application episode

- **The threshold.** It is one constant in `timeout.rs`, next to the call budgets, and equals `TIMEOUT_CALL`: an operation that takes longer than one call's whole budget is slow. Two sites measure, and both hand their elapsed time to the registry, which alone compares it with the threshold:
  - a node's child list, including the popup checks (`node.rs:384-403`);
  - an application's resolution (`lib.rs:334-347`).

  The literal disappears from both sites, and so does the per-node warning.
- **Debug per slow operation.** Every slow operation is a debug record with `bus_name`, `call` (`Accessible.GetChildren`, or `resolve application`), `elapsed_ms` and `threshold_ms`.
- **The warning.** An application instance's third slow operation warns: *application answers AT-SPI calls slowly; queries that include it are slow*. The warning carries `application` and `pid` where enumeration learned them, `bus_name`, `call`, `elapsed_ms` and `threshold_ms`. Later slow operations are debug.
- **The count.** Three slow operations per instance, counted whether or not they came one after another.

*Alternatives rejected:*

- **Warn on the first slow operation** (the review's proposal). `dev-docs/logging.md` §3 rules it out: a single slow call depends on machine load, and a loaded CI runner would warn in a healthy run.
- **Count only consecutive slow operations.** Think of an application whose one big table is slow on every query while its other reads are fast. It would never warn, although every query that includes it loses time. That is the case the review names (a large LibreOffice tree).
- **A threshold independent of `TIMEOUT_CALL`.** That would be two numbers to keep in step, without a reason for them to differ.

### 6. The per-application registry keeps slowness, and slowness gives way to timeouts

- For each bus name, `AppTimeouts` records in its per-application entry the number of slow operations and whether the application is in a timeout episode. `platynui_core::diagnostics::Transitions` needs no new method.
- The count is updated under the registry's lock, so when several threads report at once, exactly one of them warns. `Transitions::failed` gives timeouts the same guarantee.
- While an application is in a timeout episode, its slow operations are debug and do not count. A timeout after a slow warning still warns, because it adds a consequence: elements are missing.
- `retain` at each enumeration forgets the instances that left the bus. It records the end of a slow episode once at debug, like the end of a timeout episode.
- The registry keeps its name, and its doc comment says that it keeps slowness too.

*Alternatives rejected:*

- **One latch for both conditions** (the review's "one user-facing signal per application"). An application first reported as slow would then never say that its elements are missing once its calls time out. Each consequence deserves its own warning. Slowness after a timeout adds nothing, and stays debug.
- **A separate registry.** The subject, the key, the pruning and what enumeration learned are all the same.

### 7. Window-state reads that fall back to `False` are recorded at debug, except for an unavailable capability

- One helper looks the window up and asks whether it is active. `resolve_window`, `resolve_is_active_window` and `resolve_window_state` all use it (`node.rs:1604-1644`). It takes the window manager, the node and a closure that describes the node, so a unit test can drive it with a stub window manager.
- It writes three debug records:
  - the lookup fails: `window manager cannot find this top-level window; IsActive, IsMinimized, IsMaximized and IsTopmost read False`. Its fields are `window` (the description of `dev-docs/logging.md` §13), `bus_name`, `path` and `error`;
  - `is_active` fails: `window manager cannot say whether this window is active; IsActive reads False`, with `window`, `window_id` and `error`;
  - the state read fails: the existing record at `:1643` keeps its level. Its fields are brought in line with §11: `error = %err`, the id as `window_id`, and the description as `window`.
- A `PlatformError::CapabilityUnavailable` is not recorded. A generic Wayland compositor answers every call that way, and `window-state` already defines the value as `False`.
- The description is built inside the macro's fields, so only when debug is enabled (§12).

*Alternatives rejected:*

- **Warn once per window.** Windows the window manager cannot find are normal in some sessions (withdrawn frames, popups exposed as frames), and the value read does not change.
- **Also record `CapabilityUnavailable`.** That would add a record on every read, restating what the compositor identification record already says.

### 8. Tests are placed by what they touch

- **X11.** A `#[cfg(test)]` log capture like the AT-SPI one, and unit tests of four things: the missing hints, the reporter, the lookup's decision, and the process-ID error. None of them needs an X server, so `just test` runs them everywhere.
- **AT-SPI.** Registry tests next to the timeout tests, and tests of the window-state helper with a stub window manager.
- **Real provider:**
  - the X11 and compositor lanes, with their warnings inspected;
  - a check by hand on a bare `Xvfb` without a window manager: create two runtimes, and see one warning.

  No new ignored `Xvfb` test is added. The X11 reads do not change, and a new ignored binary would need a recipe of its own.
- **No Robot Framework test.** No keyword changes, and the lanes are the real-provider check.

### 9. The spec delta adds requirements and modifies none

- The four requirements are ADDED to `diagnostic-logging`. The existing requirements on levels, log-or-return and once per episode already forbid what this change removes. The added ones state the new promises concretely, and none of the existing requirements has to be carried in full text.
- Two of them are narrower than the rest:
  - the window-state requirement names the AT-SPI provider, because JAB discards its errors the same way and is not part of this change;
  - the error-text sentence of the lookup requirement names X11, because the PlatynUI compositor's lookup error has its own wording.
- `skip_specs` was rejected. The log is user interface (`dev-docs/logging.md` §1), and this change alters what users see there, including one new warning.

## Risks / Trade-offs

- **[A slow application warns later than today]** → It warns on the third slow operation instead of the first. For an application that is slow on every query, that is the third query. A single stall stays visible at debug, with `native_log_level=debug`.
- **[A loaded CI runner still reaches three slow operations of one application]** → Then the run did lose time, and the warning names the application. The count and the threshold are named constants, and the check of the lanes' warnings shows whether they fire.
- **[Fewer warnings hide a real lookup problem]** → The consumers still report once. The substitution warning prints the lookup's error, and an action fails with it. Every lookup stays in the debug log, with its PID and its candidates.
- **[Avalonia's `PopupRoot` still warns once per popup window]** → See Open Questions.
- **[The process-wide set of reported findings grows]** → It is bounded by the number of displays times two.
- **[Process-wide state in unit tests]** → nextest runs every test in its own process. The tests also use distinct display names, so a plain `cargo test` is safe too.
- **[`java-provider-linux` edits `window_manager.rs` too]** → It edits different functions. Whichever change lands second rebases.
- **[The healthy-lane check stays manual]** → By the maintainer's decision in the logging concept (no lane-report tooling), the tasks run `robotcode results log` after each lane.

## Migration Plan

- **Behavioral, in the log only**, plus one error text. No keyword, API, attribute or configuration changes. The tests and docs are additive.
- **Native rebuild:** yes. Both crates are linked into `packages/native`, the CLI and the Inspector. The lane recipes rebuild the native package themselves.
- **Sequence:**
  1. Record the lanes' warnings before the change.
  2. The tests, which fail to compile or fail.
  3. The X11 platform.
  4. The AT-SPI provider.
  5. The docs.
  6. The verification: unit tests, lanes, and the bare `Xvfb`.
- **Order with other changes:**
  - before `add-avalonia-test-app`'s X11 popup checks (its task 2.4), or together with them;
  - independent of `java-provider-linux`, apart from a rebase.
- **Rollback:** revert. The X11 commit and the AT-SPI commit revert independently.

## Open Questions

- **Avalonia's `PopupRoot` on X11.** Since `atspi-application-level`, it is a `control:Frame` under the application level, so it is a top-level window whose bounds come from the window manager (`openspec/changes/add-avalonia-test-app/design.md:266-270`).
  - With a modeless dialog open, this change turns its ambiguous lookup into debug. `extents.rs:128-139` still warns once per popup window and episode that the window manager cannot answer for it. Since a popup is a new accessible each time it opens, *assumed* not verified, that is one warning per popup opened.
  - With the main window alone, the lookup takes the main window and nothing warns, but the bounds are wrong. That is `add-avalonia-test-app`'s provider defect, not a logging question.

  Two answers are possible:
  - the provider treats a `PopupRoot` as a transient popup, which `add-avalonia-test-app`'s Risks foresee as a provider change with its own test;
  - or the once-per-window warning is accepted as the correct report of a substituted geometry, and the Avalonia X11 run is not warning-free.

  The answer changes neither this change's specs nor its tasks. It is needed before `add-avalonia-test-app` relies on a warning-free X11 run.
