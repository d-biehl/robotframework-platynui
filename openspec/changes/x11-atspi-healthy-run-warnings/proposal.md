# Proposal

## Why

A healthy run has no warning from PlatynUI. That is the promise of the logging concept (`dev-docs/logging.md` §3 and §18) and of the `diagnostic-logging` spec: a warning is never produced in a healthy session, a failure that is returned is not logged again above debug, and a condition that recurs on every call is reported once per episode. The X11 platform and the AT-SPI provider still break these rules. The triage of the logging review (`openspec/changes/archive/2026-09-27-logging-concept/review-findings.md`, follow-up A1) lists five findings of priority A for them, and a re-check on 2026-09-29 found all five unchanged:

- **The EWMH check warns twice for one condition, and once with an interpolated message.** When no EWMH window manager is found, `check_ewmh_wm_support` warns (`crates/platform-linux-x11/src/window_manager.rs:908` or `:920`), and the bundle factory warns again (`crates/platform-linux-x11/src/lib.rs:108`). A failed check warns with `"EWMH WM detection failed: {e}"` (`lib.rs:109`). Both repeat for every runtime a process creates.
- **The hint check raises a false alarm.** Every hint missing from `_NET_SUPPORTED` gets its own warning (`window_manager.rs:946-956`). `_NET_WM_PID` is among them, although applications set it on their own windows and the window lookup reads it regardless.
- **A window lookup warns about a failure it returns, in two findings.** When no window belongs to the element's process (`window_manager.rs:188-195`), and when several do and none matches (`:219-226`), the X11 window manager warns and returns the error. The lookup is not cached. Every top-level `Bounds` read, every `IsActive`, `IsMinimized`, `IsMaximized` and `IsTopmost` read, every window action and every frame the picker checks runs it again.
- **Every slow child list warns.** Reading a node's children for longer than one second warns per node (`crates/provider-atspi/src/node.rs:396-403`), in a function-prefixed, shouting message, against a literal `1000` that `lib.rs:345` repeats.

Three smaller findings of the same code ride along (priority B): no warning names an AT-SPI application that stays slow; `IsActive` and the window-state attributes read `False` without any record when the window cannot be found (`node.rs:1605-1610`, `:1633-1636`); and the error for an element without a process ID reads `extract PID from UiNode` (`window_manager.rs:684-685`).

It matters more now for two reasons:

- **Polling.** `Wait Until Attribute Value` re-reads the element on every poll (`src/PlatynUI/BareMetal/__init__.py:1975`, `:1990`). A window lookup that fails therefore warns on every poll.
- **Avalonia.** Since `atspi-application-level`, an Avalonia `PopupRoot` is a `control:Frame` directly under the application, so it is a top-level window. With a modeless dialog open, its X11 lookup is ambiguous (`openspec/changes/add-avalonia-test-app/design.md:266-270`). `add-avalonia-test-app` would then show a warning on every popup read in an otherwise healthy X11 run.

## What Changes

- **The X11 window manager check reports each finding once.**
  - There are two findings: no usable EWMH window manager (none runs, its check is inconsistent, or the check fails), and a window manager that does not advertise some of the hints PlatynUI uses.
  - Each finding is one warning. It names the display and states the consequence. A process reports it once per display, and later runtimes on the same display record it at debug.
  - The duplicate warning and the interpolated message are gone.
- **`_NET_WM_PID` is no longer checked as a window-manager hint.** The remaining missing hints (`_NET_CLIENT_LIST`, `_NET_ACTIVE_WINDOW`, `_NET_CLOSE_WINDOW`) are named together in one warning.
- **A failed X11 window lookup is recorded at debug and returned.**
  - A lookup that finds no window records the process ID and the numbers of managed windows it looked at.
  - An ambiguous lookup records the candidate windows and the hints it compared.
  - Its consumers keep their own reports: the once-per-window substitution warning for bounds, the error of a window action, and the new debug record of a window-state read.
- **An element without a process ID says so.** The lookup's error says that the element's application has no process ID to look its window up by, without naming internal types.
- **Slow AT-SPI operations are debug, and an application that stays slow is warned once.**
  - Reading a node's children and resolving an application during enumeration share one threshold constant.
  - Each slow operation is a debug record.
  - An application instance whose operations have exceeded the threshold three times is warned about once, through the provider's per-application registry. The warning names the application and says that queries that include it are slow.
  - An application already warned about for timing out is not warned about again for being slow. The episode ends when the instance leaves the accessibility bus.
- **A window-state read that falls back to `False` is traceable.** When the window manager cannot find the window or cannot answer, `IsActive`, `IsMinimized`, `IsMaximized` and `IsTopmost` still read `False`. The AT-SPI provider now records at debug which window it was and why. A window manager that reports window management as an unavailable capability is not recorded per read; this is the case for a generic Wayland compositor.
- **Docs:**
  - `dev-docs/logging.md`: the table of episode subjects, the once-per-process examples and the map of spec requirements;
  - `dev-docs/platform-linux.md`: the X11 initialization and window-manager sections;
  - the Status lines of the implemented entries in `review-findings.md`.
- **Deliberately unchanged:**
  - The bounds substitution warning (`crates/provider-atspi/src/extents.rs:128-139`, once per window and episode). It is the report that the `wayland-compositor-detection` spec requires.
  - The RANDR and X-Resource warnings, and the X11 connect errors of `x11util.rs` (priority B, "when touched").
  - The AT-SPI functional bugs the review found: role memoization and `Rect(0,0,0,0)` extents.

Behavior changes that users see, for the release notes:

- A healthy X11 or Wayland session no longer shows these warnings.
- A session without an EWMH window manager shows one warning per display instead of two or more per runtime.
- An AT-SPI application that stays slow is named once, instead of one warning per slow node.
- The error of a window action on an element without a process ID says so.

PlatynUI is at 0.x, and nothing here is breaking: no keyword, argument, attribute value or configuration key changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `diagnostic-logging`: four ADDED requirements. The existing requirements on levels, log-or-return and once per episode already forbid the records this change removes. The added ones state the promises this change makes concrete and testable:
  - a subject that stays slow is warned once per episode;
  - a failed window lookup is reported by the layer that decides what it means;
  - an AT-SPI window-state read that falls back to `False` is traceable;
  - an X11 display's window manager support is reported once.

## Impact

- **Rust:**
  - `crates/platform-linux-x11`:
    - `src/lib.rs`: the bundle factory hands the EWMH finding to one reporter;
    - `src/window_manager.rs`: the EWMH check returns findings, the reporter logs them once per display, the hint list, the window lookup's decision and its debug records, and the process-ID error;
    - a new `#[cfg(test)]` log capture, `src/test_log.rs`.
  - `crates/provider-atspi`:
    - `src/timeout.rs`: the threshold constant, and slowness in the per-application registry;
    - `src/node.rs`: the child-list measurement and the window-state reads;
    - `src/lib.rs`: the application-resolution measurement.
  - No change to `platynui-core`, the runtime or any other window manager.
- **Python / Robot Framework:** no keyword, argument or API change.
- **Tests:**
  - Rust unit tests with log capture, written first: the X11 check, the X11 lookup, the AT-SPI registry and the window-state reads.
  - The X11 and compositor acceptance lanes, with their warnings inspected.
  - A manual check on a bare Xvfb without a window manager.
- **Native rebuild:** yes. Both crates are linked into `packages/native`, the CLI and the Inspector.
- **Platforms:** Linux only.
  - X11, where window management is "⚠️ partial EWMH" in the README: all parts.
  - The PlatynUI compositor and generic Wayland: the AT-SPI parts, meaning the slow application and the window-state reads.
  - Windows and macOS are untouched. JAB discards its window-lookup errors the same way (`crates/provider-java-jab/src/node.rs:1204-1205`, `:1244`), and stays out of scope.
- **Coordination:**
  - `add-avalonia-test-app` should land after this change, or together with it, before its X11 popup checks (its task 2.4) rely on a warning-free run. What the provider does with a `PopupRoot` stays an open question (design).
  - `java-provider-linux` adds a window enumeration and a window-to-process rule to `platform-linux-x11` (its tasks 1.1-1.2). It touches the same file as this change, but not the same functions, so whichever lands second rebases. Its design already calls `_NET_WM_PID` client-claimed (`openspec/changes/java-provider-linux/design.md:80`).
  - No other open change edits this code.
