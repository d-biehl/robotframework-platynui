# Spec Delta

## ADDED Requirements

### Requirement: A subject that stays slow is warned once per episode

A call across a process boundary, or an operation made of such calls, that takes longer than the threshold of its kind SHALL be recorded at debug level, naming what was called, the subject it went to and how long it took. Each kind SHALL have one threshold, defined in one place and used by every place that measures that kind. A single slow call SHALL NOT be reported above debug level, because the same call is slow on a loaded machine and fast on an idle one.

A subject that stays slow, because it keeps exceeding its threshold, SHALL be warned about once per episode. The warning SHALL name the subject and state that queries that include it are slow. The subject's further slow calls SHALL be recorded at debug level. A subject that is already warned about for not answering in time SHALL NOT be warned about again for being slow. The component that owns the report defines when a subject stays slow and when its episode ends, and the end SHALL be recorded once, at debug level. This applies to every slow-call record that PlatynUI adds or changes.

For the AT-SPI provider, the subject is an application instance, identified by its bus name. The measured operations are reading a node's children and resolving an application during enumeration. An application stays slow once three of its operations have exceeded the threshold, whether or not they came one after another. Its episode ends when the application instance leaves the accessibility bus, and a new instance of the application is a new subject.

#### Scenario: A single slow operation stays at debug

- **GIVEN** an AT-SPI application one of whose child lists takes longer than the threshold to read, while its other operations answer in time
- **WHEN** a query reads that child list
- **THEN** the log SHALL contain no warning
- **AND** one debug record SHALL name the application's bus name, the call and the time it took
- **NOTE** Verified with a unit test of the provider's per-application record, with the elapsed time passed in. No lane fixture is slow on purpose.

#### Scenario: An application that stays slow is named once

- **GIVEN** an AT-SPI application whose operations keep taking longer than the threshold
- **WHEN** a `Wait Until …` keyword polls a query that includes it for ten seconds
- **THEN** the log SHALL contain exactly one warning naming the application (its name, and its pid where known) and its bus name, and saying that queries that include it are slow
- **AND** its other slow operations SHALL be recorded at debug level
- **AND** when the application quits, one debug record SHALL say that its episode ended
- **AND** when a new instance of the application stays slow, one further warning SHALL be logged
- **NOTE** Unit test of the registry, like the one behind *An application that stops answering is named once*.

#### Scenario: Slow operations of different applications are not added up

- **GIVEN** two AT-SPI applications, each with two operations that took longer than the threshold
- **WHEN** both are measured
- **THEN** no warning SHALL be logged
- **NOTE** Unit test.

#### Scenario: An application that times out is not warned about again for being slow

- **GIVEN** an AT-SPI application that was warned about because a call to it timed out
- **WHEN** three of its operations then take longer than the threshold
- **THEN** no further warning SHALL be logged
- **AND** each of the slow operations SHALL be recorded at debug level
- **NOTE** Unit test.

### Requirement: A failed window lookup is reported by the layer that decides what it means

When a window manager cannot find the native window of an accessible element, it SHALL return the failure to its caller. It SHALL NOT report the failure at warning or error level, however often the lookup is repeated. It MAY record the failure at debug level with what it tried: the process ID it looked up and, when several windows of that process were candidates, their identifiers and the hints it compared.

Only the layer that decides what the failure means SHALL report it, each in its own way:

- a top-level window's bounds fall back to the toolkit's geometry, with the one warning of *A substituted window geometry is visible in the log* (capability `wayland-compositor-detection`);
- a window action fails with the error;
- the AT-SPI provider's window-state attributes read `False` and record the failure at debug level (*An AT-SPI window-state read that falls back to False is traceable*).

On X11, the error SHALL say why no window was found:

- the element's process has no managed window;
- several managed windows belong to the process and none matches the element;
- or the element's application has no process ID to look its window up by.

The error SHALL NOT name internal types.

#### Scenario: A window lookup that finds no window adds no warning

- **GIVEN** an X11 session, and an accessible top-level window whose process has no window in the window manager's client list
- **WHEN** its window is looked up ten times, as polling its `@IsActive` does
- **THEN** every lookup SHALL fail with an error that names the process ID and says that no window belongs to it
- **AND** the log SHALL contain no warning or error from the lookups
- **AND** it SHALL contain one debug record per lookup, naming the process ID
- **NOTE** Unit test of the X11 lookup, with the managed windows passed in. That no lane fixture produces such a warning is checked after each Linux lane.

#### Scenario: Several windows that match none are named at debug

- **GIVEN** an X11 process with two managed windows, and an accessible top-level window of that process that matches neither of them by geometry or by name
- **WHEN** its window is looked up
- **THEN** the lookup SHALL fail with an error saying that none of the process's windows matches the element
- **AND** a debug record SHALL name the process ID and both candidate windows
- **AND** no warning SHALL be logged
- **NOTE** Unit test. The real case is an Avalonia popup while a modeless dialog is open (`add-avalonia-test-app`), which no lane runs yet.

#### Scenario: The bounds of a window that cannot be found are warned about once

- **GIVEN** an accessible top-level window whose native window the X11 window manager cannot find
- **WHEN** its bounds are read many times
- **THEN** every read SHALL return the toolkit's geometry
- **AND** the only warning in the log SHALL be the one substitution warning for that window
- **NOTE** Unit tests of the X11 lookup together with the provider's substitution tests. No lane fixture has such a window today.

#### Scenario: An element whose application has no process ID says so

- **GIVEN** an accessible top-level window whose application has no process ID, because the accessibility bus daemon could not tell it
- **WHEN** the X11 window manager looks up its window
- **THEN** the lookup SHALL fail with an error saying that the element's application has no process ID to look its window up by
- **AND** the error SHALL NOT name an internal type
- **AND** no warning SHALL be logged by the lookup
- **NOTE** Unit test of the X11 lookup. On a real bus this is the case of the `sidecar-deployment` topology, whose harness does not assert on this text.

### Requirement: An AT-SPI window-state read that falls back to False is traceable

The AT-SPI provider reads `IsActive`, `IsMinimized`, `IsMaximized` and `IsTopmost` of a top-level window from the window manager. When the window manager cannot find the window or cannot answer for it, the attribute SHALL read `False`, as `window-state` requires. The provider SHALL record at debug level which window it was and the window manager's error. It SHALL NOT report the fallback above debug level, because windows that the window manager cannot find are normal in some sessions and the value read does not change.

A window manager that reports window management as an unavailable capability gives the same answer on every read. The provider SHALL NOT record such a read.

#### Scenario: A False from a window that cannot be found can be told apart

- **GIVEN** a top-level window that the window manager cannot find
- **WHEN** its `@IsActive` is read
- **THEN** it SHALL read `False`
- **AND** a debug record SHALL name the window and the window manager's error
- **AND** no warning SHALL be logged
- **NOTE** Unit test of the AT-SPI provider with a stub window manager. The mock provider takes no window state from a window manager.

#### Scenario: A window whose activity cannot be read

- **GIVEN** a top-level window that the window manager finds, but for which it cannot say whether it is active
- **WHEN** its `@IsActive` is read
- **THEN** it SHALL read `False`
- **AND** a debug record SHALL name the window, its window identifier and the error
- **NOTE** Unit test with a stub window manager.

#### Scenario: A window manager without window management adds no record

- **GIVEN** a Wayland compositor for which PlatynUI has no window management, so that the window manager reports window management as an unavailable capability
- **WHEN** a top-level window's `@IsActive` and `@IsMinimized` are read
- **THEN** both SHALL read `False`
- **AND** no record SHALL be logged for them
- **NOTE** Unit test with a stub window manager that answers with the unavailable capability. The real case is a generic Wayland compositor, which no lane runs.

#### Scenario: A window the window manager answers for adds no record

- **GIVEN** a top-level window that the window manager finds and answers for
- **WHEN** its `@IsActive`, `@IsMinimized`, `@IsMaximized` and `@IsTopmost` are read
- **THEN** no record SHALL be logged for them
- **NOTE** Unit test with a stub window manager.

### Requirement: An X11 display's window manager support is reported once

When a runtime is created on an X11 display, PlatynUI SHALL check two things: whether an EWMH window manager runs there, and whether it advertises the hints that PlatynUI's window lookup and window actions use (`_NET_CLIENT_LIST`, `_NET_ACTIVE_WINDOW` and `_NET_CLOSE_WINDOW`). It SHALL report two findings. Each is one warning that names the display and states the consequence:

- **No usable EWMH window manager.** None runs, its `_NET_SUPPORTING_WM_CHECK` window is inconsistent, or the check itself fails. The warning SHALL say which of these it is, and that windows cannot be looked up, so window actions and window state do not work.
- **Missing hints.** The window manager does not advertise some of these hints. One warning SHALL name the window manager and every missing hint, and say that the window operations that use them may have no effect.

Each finding SHALL be warned about at most once per process for each display. A later runtime on the same display SHALL record it at debug level.

`_NET_WM_PID` is a property that applications set on their own windows, not a window-manager hint. A property of that kind SHALL NOT be reported as missing.

A window manager that advertises all of these hints SHALL produce no warning.

#### Scenario: A display without a window manager is reported once

- **GIVEN** an X11 display on which no EWMH window manager runs
- **WHEN** two runtimes are created on it, one after the other
- **THEN** exactly one warning SHALL name the display and say that windows cannot be looked up, so window actions and window state do not work
- **AND** the second runtime SHALL record the finding at debug level
- **NOTE** Unit test of the report with the finding passed in, and a check by hand on a bare Xvfb without a window manager.

#### Scenario: Each display is reported on its own

- **GIVEN** two X11 displays, neither with an EWMH window manager
- **WHEN** a runtime is created on each
- **THEN** each display SHALL get its own warning
- **NOTE** Unit test.

#### Scenario: A check that fails is the same finding

- **GIVEN** an X11 display on which the window manager check fails with an error
- **WHEN** a runtime is created on it, and a later runtime finds no window manager there
- **THEN** exactly one warning SHALL be logged, naming the display and the error
- **NOTE** Unit test.

#### Scenario: `_NET_WM_PID` is not a window-manager hint

- **GIVEN** a window manager that advertises `_NET_CLIENT_LIST`, `_NET_ACTIVE_WINDOW` and `_NET_CLOSE_WINDOW`, but not `_NET_WM_PID`
- **WHEN** a runtime is created on its display
- **THEN** no warning SHALL be logged about the window manager
- **NOTE** Unit test.

#### Scenario: Missing hints are one warning

- **GIVEN** a window manager that advertises `_NET_CLIENT_LIST`, but neither `_NET_ACTIVE_WINDOW` nor `_NET_CLOSE_WINDOW`
- **WHEN** two runtimes are created on its display
- **THEN** exactly one warning SHALL name the window manager, the display and both missing hints
- **NOTE** Unit test.
