# Logging

<!-- Living document. For history see CHANGELOG.md and git log. -->

> **Scope of this document.** This is PlatynUI's logging concept: what each level means, where
> and how often a record is written, what it contains, and how a user controls what they see. It
> is normative for the PlatynUI library — the native core, the platforms and the providers, the
> Python bindings and the Robot Framework library — and for the command-line tool and the
> Inspector, in Rust and in Python alike. The development tools (the Wayland compositor, the EIS
> test client and the egui test app) keep their own log setup and are outside it.
>
> The one-line-per-rule checklist that code reviews and agents apply is
> [`.github/instructions/logging.instructions.md`](../.github/instructions/logging.instructions.md).
> The behavior users can rely on is specified in the `diagnostic-logging`, `native-logging` and
> `runtime-session-config` specs under `openspec/specs/`; §19 maps their requirements to the
> sections here. How native records travel into Python is in
> [`python-bindings.md`](python-bindings.md) (*Logging*); how errors are shaped is in
> [`error-handling.md`](error-handling.md).

## 1. Why logging is user interface

Native records do not stay in a developer's terminal. The Python extension hands every native
record to Python `logging`, and under Robot Framework the record lands in the log of the keyword
during which it happened. A native warning therefore *is* a Robot Framework warning: it is printed
on the console while the suite runs, it is listed under *Test Execution Errors* in `log.html`, and
a test author reads it as "something is wrong with my test or with my system". The command-line
tool and the Inspector print the same records on stderr.

That changes the question a record has to answer. It is not "what was the code doing here?" but
"what does the reader have to do now?" Three kinds of records fail that question, and the rules
in this document exist to prevent them:

- a warning that also fires in a healthy session teaches users to ignore warnings, including the
  one that matters;
- a warning that fires on every poll of a `Wait Until …` keyword — up to ten times a second —
  buries everything else on the console;
- an error that repeats the exception the keyword already raised makes one failure look like two.

## 2. The channels

PlatynUI produces diagnostics through two channels. Two further outputs look similar but are not
diagnostics, and one component, the Java agent, logs somewhere else entirely.

**Native diagnostics.** The Rust crates log through the `tracing` crate. The command-line tool and
the Inspector install a formatting subscriber that writes to stderr, because stdout carries the
commands' output. The Python extension installs a subscriber that only queues: its records are
delivered on the thread that calls into PlatynUI — after each runtime call, when a runtime is
constructed or shut down, around every keyword of the Robot Framework libraries, and on
`platynui_native.flush_logs()` — and from there reach Python `logging` under
`platynui.native.<module>`. A record from `platynui_provider_atspi::extents` arrives on the logger
`platynui.native.provider_atspi.extents`, and Robot Framework shows its text as
`[provider_atspi.extents] message field=value`, because Robot Framework shows only the text of a
record. [`python-bindings.md`](python-bindings.md) explains why delivery happens on the calling
thread and what that means for records from background threads.

**The Python library's own records.** The Python code of the library logs through the standard
`logging` module under `platynui.` plus its module path: `platynui.core.adapter_devices`,
`platynui.ui.application`, and `platynui.baremetal` for the keyword library, including its keyword
action lines (§15). Under Robot Framework, a `logging` record written on the keyword's thread
reaches the keyword's log exactly like a `robot.api.logger` message, and Robot Framework keeps the
root logger's level equal to `--loglevel` and to `Set Log Level`. Because native records live
under `platynui.native`, `logging.getLogger("platynui")` covers both channels at once: plain Python
code can attach a handler or set a level there and gets everything PlatynUI reports. Without any
configuration, Python itself prints warnings and errors to stderr.

**Not diagnostics: results and failures.**

- A keyword's own output, such as the screenshot that `Take Screenshot` embeds in the log, is the
  keyword's result. It stays at Robot Framework's INFO level and is written through
  `robot.api.logger`, which offers the HTML embedding it needs.
- A failure returned to the caller — an error value in Rust, an exception in Python, a failed
  keyword in Robot Framework — is the primary report of that failure. Logging is not a second
  channel for it (§4).

**Diagnostics inside a target JVM.** The Java agent that PlatynUI injects into a target JVM runs
inside somebody else's application, so it deliberately uses no logging framework there. It writes
to the target's own stderr, with lines prefixed `[PlatynUI agent]`: errors always, debug output only
when the target runs with `-Dplatynui.agent.debug=true`. Neither the level knob (§17) nor the Robot
Framework log reaches that output. The Rust side therefore reports what it can observe from the
outside — for example, an agent that was injected but never published its handshake — as a
warning that names the target's log, because nothing else would point the user there. The details
are in [`java-toolkits.md`](java-toolkits.md) (*What has to be true for a target to be served*) and
in `java/agent/src/main/java/platynui/agent/AgentLog.java`.

## 3. The levels

Each level has one meaning, the same in Rust and in Python. The meaning decides the level — not how
important the author feels a message is, and not how often it fires; the one exception is the
typed-text rule at the end of this section.

| Level | Meaning | Who sees it |
|---|---|---|
| error | Something PlatynUI had to do failed unexpectedly and the failure was swallowed, so its results or the system's state are wrong, and nothing else reports it. States the consequence. Never for a failure that is returned or raised. A condition that recurs on every call is reported once per episode. | By default: the Robot Framework console and *Test Execution Errors*; stderr of the command-line tool and the Inspector. |
| warn | PlatynUI knowingly works with less than it was asked for, because of the environment, the configuration or the target application, and says what the user loses. Never in a healthy session, never for a failure that is returned or raised. A condition that recurs on every call — a failure, or a subject that stays slow — is warned once per episode. | By default, like error. A level of `error` — native, or Robot Framework's `--loglevel ERROR` — hides them. |
| info | A lifecycle transition of a long-lived native resource, once per transition: a runtime is created or shut down, the Java agent is injected into a JVM. Never per operation. PlatynUI's Python code logs no diagnostic at info. | Native records only when switched on (`native_log_level=info`, `--log-level info`, `PLATYNUI_LOG_LEVEL=info` or `RUST_LOG`); Robot Framework then shows them at its default `--loglevel INFO`. |
| debug | What a single operation did or decided; a fallback that is normal in some sessions; a call slower than its call class's threshold; a returned failure with its context; a keyword action line. | Native records only when switched on to debug, and in Robot Framework also at `--loglevel DEBUG`; keyword action lines at `--loglevel DEBUG` alone. |
| trace | One record per element, tick, key or character, or message. A record that names a key, character, key code or keysym, or carries a keyboard device's error text, is trace-level however often it occurs. | Only when switched on to trace, and in Robot Framework at `--loglevel TRACE`. |

**error** is for damage nobody else reports. When a provider fails to list its top-level elements
during a desktop enumeration, the enumeration carries on without it: the query still returns a
result, but the provider's elements are missing from it, and no error value tells the caller. That
is an error, and its message says exactly that consequence. Keys that remain held down because
releasing them failed after a keyboard error are the other typical case: the keyword reports the
original keyboard error, and without the record nobody would learn that the keyboard is now in a
wrong state. A screenshot that fails and is returned as an error is *not* an error record; the
caller already has the failure.

**warn** is for "PlatynUI does less than you asked, and here is what you lose": an application that
stops answering accessibility calls, so its elements are missing or incomplete; a pointer target
outside the desktop that is moved to the nearest edge; a setting that cannot be used and falls back
to its default; no platform backend, so pointer, keyboard, screenshot, highlight and window control
are unavailable. A warning names the subject and states the consequence, so that the reader can
decide whether to fix the environment, the configuration or the test. The strictest part of the
definition is *never in a healthy session*: a fallback that some sessions take as a matter of course
is debug, however unusual it looks to the author. An automatic activation that fails because the
element's window cannot be activated — the desktop itself, for instance — is such a fallback; the
action proceeds, and whether it mattered shows in the action's own result.

**info** marks a transition that a reader can place on a timeline of long-lived resources: this
runtime came up with this backend and these providers, this JVM got the agent, this runtime shut
down. Resources that come and go get one record per transition, not one per program run — a test
run creates and closes many runtimes, and attaches to several JVMs. A lookup, a click or an
enumeration is never info, however rarely it happens. PlatynUI's Python code records its own
lifecycle transitions at debug instead: Robot Framework shows Python INFO records at its default
log level, so a Python info record would change every user's default log.

**debug** is the level for diagnosing a problem after the fact: what a single operation did or
decided, with its inputs and its outcome — an XPath evaluation and how many items it found, a
keyboard sequence's mode and length, a pointer click and where it landed. Fallbacks that are normal
in some sessions, calls slower than their threshold, failures that are returned together with the
context the error value does not carry, and the keyword action lines all belong here.

**trace** is for records that come per item: per element of an enumeration, per tick of an event
loop, per key or character, per protocol message. When in doubt between debug and trace, ask
whether the record would be noise with five hundred elements in the tree.

**Typed text takes precedence.** A record that names a key, character, key code or keysym, or that
carries a keyboard device's error text, is trace-level however rarely it occurs. Neither the
runtime nor a keyboard device can tell whether the text it types is an ordinary string or a Robot
Framework `Secret`, so nothing below trace may name what was typed (§16). Below trace, keyboard
records carry counts, modes and positions.

### Slow calls

A slow call is not by itself a warning: the same call is slow on a loaded CI machine and fast on a
developer's desktop, and a warning that depends on machine load fires in healthy sessions. Each
call class — an AT-SPI property read, a UIA call, a window manager request — has one threshold,
kept as a named constant next to the code that measures it. A call slower than that threshold is
recorded at debug with `call` and `elapsed_ms`. When a subject *stays* slow — the same application
or provider keeps exceeding its threshold — the user does lose something, namely time on every
query, and that is warned once per episode (§5), naming the subject.

## 4. Log or return

A failure is reported once, by the layer that knows what it means.

> A layer that returns a failure logs it at most at debug. The layer that swallows it, or decides
> what happens because of it, logs it above debug — and only that layer.

A failure is *returned* when it is an error value in Rust, an exception in Python, or a failed
keyword in Robot Framework. The caller of such a function sees the failure and decides; if the
function also logged a warning or an error, a Robot Framework user would see the failure twice —
once as a warning on the console and once as the keyword's error — and would reasonably assume two
separate problems. The returning layer may still record the failure at debug when it knows context
the error value does not carry: which endpoint it tried, how long it waited, what the operating
system reported.

Some consequences of the rule:

- The AT-SPI provider's connection code returns its failures. When `Get Element At Point` fails
  because the accessibility bus does not answer, the keyword fails with the timeout, and the log
  contains no PlatynUI warning or error about it. When a query meanwhile enumerates the desktop, the
  *enumeration* swallows the same provider failure — it continues with the other providers — so the
  enumeration is the layer that reports it, as an error once per episode that names the provider
  (§5).
- When a keyboard sequence fails, the runtime releases the keys that are still pressed and returns
  the keyboard error. Releasing is recorded at debug with the number of keys. Only when the release
  itself fails is there an error record, because that second failure is swallowed — the keyword
  reports the first one — and it leaves keys held down.
- On Windows, the failure of a call such as `SendInput` is read from the operating system *before*
  anything is logged, then recorded at debug, then returned. A subscriber that runs in between may
  itself make system calls and overwrite the thread's last-error value:

  ```rust
  let err = last_error("SendInput");
  tracing::debug!(error = %err, "SendInput failed");
  return Err(err);
  ```

- In Python, a keyword that raises does not also log the failure; Robot Framework shows the
  exception. Where Python code deliberately swallows a failure — the automatic activation before an
  action, for example, continues when the window cannot be brought to the front — it logs the
  failure at the level its consequence deserves (debug in that case, §7).

The same rule is stated from the error side in [`error-handling.md`](error-handling.md) (*Logging*):
when both a record and an error value are useful, record the context at debug and return the typed
error.

## 5. Once per episode

Some conditions can recur on every operation: every query, every poll of a `Wait Until …`
keyword, every node read, every call into an application. Reporting such a condition each time
floods the console with identical lines and hides everything else. It is therefore reported per
*episode*:

- when the condition begins for a subject — a provider, an application instance, a process — it is
  reported at warning or error level, naming the subject;
- while it persists, each further occurrence is recorded at debug;
- when the subject recovers or goes away, the end of the episode is recorded once, at debug, naming
  the subject. The end of an episode is not a lifecycle transition, so it is never info, and it is
  good news, so it is never a warning;
- a failure after recovery starts a new episode and is reported again.

**The latch.** `platynui_core::diagnostics::Transitions` records which subjects are currently
failing. The call site tells it that a subject failed and learns whether this starts a new episode
or continues one; it tells it that a subject recovered and learns whether the subject had been
failing; and it can make the latch forget subjects that no longer exist, learning which ones it
forgot. The latch logs nothing — `platynui-core` has no `tracing` dependency — so the call site
logs: warn or error when an episode starts, debug while it continues, one debug record for each
subject that recovers or is forgotten. When several threads report the first failure of a subject
at the same time, exactly one of them is told that the episode started.

**Choosing the subject and what ends an episode.** The owner of a latch decides both, because only
the owner knows what "working again" means for its subject. The existing uses show the range:

| What recurs | Subject, and who owns the latch | The episode ends when |
|---|---|---|
| A provider fails to list its top-level elements during desktop enumeration | The provider; the runtime owns the latch | The provider lists its elements again |
| An application's accessibility calls time out on AT-SPI | The application instance, keyed by its D-Bus bus name; the provider owns the latch | The application instance is gone |
| The Java agent in a JVM reports a different version | The JVM process; the agent backend owns the latch | The process is gone |
| Window bounds are substituted because the window manager does not answer for a window | The window; the injected window manager owns the latch | The window manager answers for that window again |

The AT-SPI row shows why recovery is not always "the next call succeeded". Each call into an
application has its own time budget, so an application with one slow handler times out on that
call while its next call answers. If a successful call ended the episode, the provider would warn
again on every poll. The episode instead lasts as long as the application instance: D-Bus unique
bus names are never reused, so the provider forgets bus names that have left the registry at each
enumeration, and a restarted application is a new subject that warns again.

**Once per process.** Some reports concern the whole process rather than a subject: a UI tree
provider that is only a stub on its platform, a test build of the Python extension used for real
work, a rejected value of a log-level variable, a missing Java Access Bridge DLL. These use a
process-wide `std::sync::Once` (or an equivalent set of already-reported values) instead of a latch.

## 6. Decisions name what was decided

When PlatynUI chooses something on the user's behalf — the platform backend, the active UI tree
providers, the Wayland input backend — the record says what it chose and why, and at debug which
alternatives it rejected and why. "3 providers" or "platform = true" does not help a user who
wonders why the X11 backend was used in a Wayland session; "backend x11, not forced, providers
atspi" does.

The runtime's initialization record is the model. It is the one info record of a runtime's
creation and names the chosen backend, whether configuration forced it, the active provider ids,
the desktop bounds and the monitor count:

```rust
tracing::info!(backend = "x11", forced = false, providers = ?ids, desktop = %bounds, monitors = 2, "runtime initialized");
```

When no alternative is usable, the decision becomes a warning that names the candidates it tried
and what stops working as a result: no platform backend could serve the session, so pointer,
keyboard, screenshot, highlight and window control are unavailable; no UI tree provider is active,
so queries find only the desktop node; on Wayland, no input backend could be initialized, with
each backend's reason. When a later Wayland input backend succeeds, each rejected one is recorded
at debug with its reason.

A test build of the Python extension — built with the mock provider for the test suites — that is
used without selecting the mock backend is the one deliberate exception: its missing platform
backend and missing providers are consequences of the build, not of the session, so the single
test-build warning (§8) is the report, and those two records are debug for that construction. The
extension decides this and tells the runtime; the command-line tool's and the Inspector's mock
builds keep both warnings.

## 7. Fallbacks that change an action's effect

A fallback in which an action proceeds differently from what was asked is recorded, at the level
its consequence deserves:

- **A pointer target outside the desktop** is moved to the nearest edge. That is a warning with the
  requested and the actual coordinates (`requested_x`, `requested_y`, `x`, `y`): the click lands
  somewhere the test did not ask for, and nothing in the keyword's result shows it.
- **Keys that could not be released** after a keyboard error are an error, with the number of keys
  that may remain held down — the count only, because a held key may be a character of a `Secret`
  (§16).
- **A window that did not become the foreground window** after activation is recorded at debug. The
  check also runs for automatic activation before an action, where the outcome is normal in some
  sessions.
- **A failed automatic activation** before an action is recorded at debug with the element and the
  reason, and the action proceeds as it would have without activation. Elements whose window
  cannot be activated, such as the desktop, are ordinary targets.

The first two change the target application in a way the test cannot see; the last two are normal
in some sessions, and the action's own result shows whether they mattered.

## 8. A capability that is not there says so once

When PlatynUI runs where a capability it would normally provide is not implemented or not
installed, it says so once, at the moment the capability matters. It does not behave as if the
capability were present and simply find nothing — that is the hardest symptom for a user to reason
backwards from. Where nothing needs the capability, it stays silent above debug.

- **A UI tree provider that is only a stub on its platform** warns once per process when a runtime
  is created, saying that queries on this desktop find nothing because support for the platform is
  not implemented.
- **Watching for UI events without an event source.** `platynui-cli watch` warns when no active
  provider emits UI events, saying that the command will wait without output. When a provider does
  emit events, the same check is recorded at debug.
- **The Java Access Bridge.** A missing bridge DLL is recorded at debug at startup, with the
  discovery paths that were tried, because most Windows machines have no JDK and need none. It
  matters only when a Swing or AWT window is found that no Java backend serves. Then the Java
  provider's router, the one component that knows whether another backend — the in-JVM agent —
  served the window, warns once per process, naming `providers.java.jab.dll_path`,
  `PLATYNUI_JAB_DLL` and a 64-bit JDK as the remedies. A window the agent serves is not reported.
- **A test build used for real work.** A build of the Python extension that links only the mock
  platform and provider, and is used without selecting the mock backend, warns once per process
  that it links no real platform or providers, and how to get a real build. The warning reaches
  Python `logging` before the constructor returns, and that construction records no other warning
  (§6), so the one actionable message is not buried.

## 9. Configuration mistakes are reported

A configuration mistake that falls back to a default in silence costs a user hours: the test runs,
but against the wrong display, with popups not surfaced, with default timing. Mistakes are
therefore reported, and the component that reads a setting is the one that checks it.

**Components check their own section.** Every PlatynUI platform and provider checks its section of
the runtime configuration at the start of its build, before anything can fail, so that its warnings
are logged even when the build then fails — a mistyped X11 `display` warns even though the
construction afterwards fails for lack of an X server. It warns for each key it does not know and
for each value of the wrong type, naming the component in bucket form (`platform.x11`,
`providers.atspi`) and the key, and for a type mismatch also the expected and the found type; the
default then applies. The known keys are the constants the reads themselves use, so the list
cannot drift from the code. `enabled` is reserved on every provider. Nested maps are checked by the
code that receives them: the Java provider's backends report their keys as `agent.<key>` and
`jab.<key>` under `providers.java`. The runtime checks the one key it reads itself,
`platform.backend`, the same way. `platynui-core` offers the checks as plain functions on the
configuration map that return facts; the component logs.

```python
Runtime(config={'platform': {'x11': {'dispaly': ':1'}}})
# warning naming platform.x11 and the unknown key dispaly; the display comes from the environment

Runtime(config={'providers': {'atspi': {'surface_popups': 'False'}}})
# warning naming providers.atspi, surface_popups, the expected boolean and the found string
```

**Components that are not built stay quiet.** A configuration dictionary may carry every operating
system's blocks, so that one dictionary serves every platform. A component id that no active
component claims — `platform.windows` on Linux, for example — is recorded at debug only, and its
keys are not checked. The "active" rule is what keeps portable dictionaries free of warnings.

**The Python binding reports what it cannot pass on.** Some mistakes would be dropped before any
component sees them, so the binding reports them: a top-level key other than `platform` and
`providers` (such as `platfrom`), naming the key and the accepted buckets; a bucket that is not a
dict, naming the bucket and its Python type; and a key that is not a string, or a value of a type
the configuration cannot hold (such as a `pathlib.Path`), naming its dotted path and its Python
type.

**Profiles are stricter.** A pointer or keyboard profile value of the wrong type — `'100ms'` where a
number of milliseconds is expected — raises a `TypeError` naming the key and the expected type, as
the profiles' other fields already do. A profile key that no field reads, such as `speed_factr`,
is reported as a warning naming the key, and the call proceeds: rejecting it would break suites
that carry a harmless extra key, while the warning catches the typo.

## 10. External boundaries

PlatynUI spends most of its time waiting for other processes: D-Bus calls into applications, UIA
calls, X11 requests, the Java Access Bridge, the Java agent's RPC, the compositor's control socket.
When something goes wrong at such a boundary, the questions are always the same — what was called,
on whom, how long did it take, and what was the budget — so records about a call across a process
boundary carry the same fields:

- `call`: what was called, such as the D-Bus method or property, or the API function;
- the target: who was called — `application`, `bus_name` and `pid` for an application, `provider`
  for a provider, `window` for a window;
- `elapsed_ms`: how long the call took;
- `timeout_ms`: the time budget, when the call timed out.

Log-or-return (§4) and once-per-episode (§5) decide the level. A timeout that is returned to the
caller is debug. A timeout that is swallowed — enumeration skips an application that does not
answer — is a warning once per episode for that application, naming it (its name, and its pid
where known), its bus name, the call and the timeout, and saying that its elements are missing from
query results. A slow call is debug (§3, *Slow calls*).

## 11. Message style and field names

**Messages** are English fragments in lower case; proper names such as AT-SPI, X11, JAB or
`SendInput` keep their case. They have no trailing period and no type, function or module prefix —
the record's target already names the module. A warning or an error states its consequence after a
semicolon:

```text
provider failed to list its top-level elements; its elements are missing from query results
```

Native messages carry no interpolated values; values go into fields, where a handler can filter on
them and the Python bridge attaches them to the record. Python records carry their values in the
message itself, through `logging`'s lazy `%` arguments, because Robot Framework shows only a
record's text:

```python
_LOGGER.debug('clicked %s at (%d, %d), %s; button %s, %d click', element, x, y, source, button, clicks)
```

The style is for log records. Error messages — the text of an error value or an exception — are not
log records: they are written for the person whose call or keyword failed, and follow
[`error-handling.md`](error-handling.md).

**Field names.** The same fact carries the same name everywhere, so that a reader, a filter or a
handler can rely on it:

| Field | Meaning | Example |
|---|---|---|
| `error` | The error, always written `error = %err` — never `err`, `e` or a bare `%err` | `error = %err` |
| `provider` | A UI tree provider's id | `atspi` |
| `component` | A configuration component in bucket form | `providers.atspi`, `platform.x11` |
| `key` | A setting key, dotted and relative to the component | `surface_popups`, `agent.enabled` |
| `expected`, `found` | The expected and the found type of a setting | `bool`, `str` |
| `application`, `bus_name`, `pid` | The application a call went to | `gedit`, `:1.57`, `4711` |
| `call` | What was called across a process boundary | `GetExtents` |
| `elapsed_ms`, `timeout_ms` | How long a call took; its budget when it timed out | `1000` |
| `element` | An element, in the description form of §13 | `Button "OK" #ok` |
| `window` | A window, in the description form of §13 | `Frame "Untitled – Editor"` |
| `requested_x`, `requested_y`, `x`, `y` | A requested point and the point actually used | |
| `position` | A 1-based character position in a keyboard sequence | `5` |
| `backend`, `forced`, `providers`, `suppressed`, `desktop`, `monitors` | The runtime's initialization record: chosen backend, whether configuration forced it, active and suppressed provider ids, desktop bounds, monitor count | see §6 |

A new record uses these names and this style from the start, and a record that is changed for any
reason is brought in line with them.

## 12. Producer rules for native code

Three rules keep native records reachable and cheap.

**Log under a target that starts with `platynui`.** The level knob (§17) turns a single level into
`warn,platynui=<level>`, and a filter directive matches every target that *starts with* its name.
A record's target is its module path, so every crate named `platynui-*` complies without doing
anything. A record that overrides its target with a name outside that prefix escapes the knob:
`--log-level debug` and `native_log_level=debug` would never show it.

**Diagnostics are events, and their context is in their fields.** Spans are not carried to Python
or to Robot Framework: the Python extension's subscriber queues events only, and Robot Framework
shows a record's text only. A field recorded on an enclosing span is therefore lost for most
readers. Every event carries the context it needs — the provider, the application, the call — in
its own fields, even when the same values would also sit on a span.

**Build expensive message parts only when their level is enabled.** The `tracing` macros evaluate
their field expressions only when the event is enabled, so `element = %describe(node)` inside the
macro costs nothing when debug is off; a description computed into a variable before the macro
costs the provider calls every time. Python's lazy `%` arguments delay only the *formatting*, not
the evaluation of the arguments: `_LOGGER.debug('… %s', node.describe())` queries the application
even when debug is off. Guard such a record with `_LOGGER.isEnabledFor(logging.DEBUG)`, as the
keyword action lines do (§15).

## 13. Describing elements

An element is described in one form wherever a person reads about it — in keyword action lines, in
error messages, in the element fields of native records, and from Python through
`UiNode.describe()`:

```text
Button "OK" #ok
```

- the role's local name (`Button`);
- the name in double quotes, cut to at most 60 characters — a longer name keeps its first 59
  characters and `…` — and then escaped, so that quotes, backslashes and control characters such as
  a line break cannot break the line;
- `#` and the element's id, when it has one that is not empty. An id of whitespace is shown as it
  is, like every value (architecture §5.7).

A description is produced by `platynui_core::ui::describe` in Rust and by `UiNode.describe()` in
Python. It asks the provider for the name and the id, which on AT-SPI can cost D-Bus calls per
element, so it is built only where it is logged or raised, and only when its level is enabled
(§12).

**Why `repr` stays cheap.** A Python element's `repr` and `str` show its runtime id and ask the
provider nothing. Robot Framework converts every value a keyword returns and a test assigns to
text, so a `repr` that queried the application would cost D-Bus calls for every result of every
`Query` — up to two calls with a one-second timeout each for an AT-SPI element's id alone. The full
description is produced when it is asked for, not when an element is displayed.

**Error messages name no internal types.** An error that a user reads names the element the way
they wrote it, not the class that carried it: no `UiNodeDescriptor`, no internal component names. When an
element cannot be found, the error says which of two lookups failed — the element the keyword was
given, or the root set by `Set Root` that the lookup depends on:

```text
No element matched "//control:Button[@Name='OK']" within timeout of 10.0 seconds.
The root set by Set Root, "//control:Window[@Name='Editor']", was not found within timeout of 0.5 seconds; './control:Button' was not evaluated.
```

A runtime without a platform backend says so in the user's terms — the runtime has no platform
backend, because none could serve this session or the runtime is shut down — instead of naming the
missing internal component.

## 14. Python: `logging`, Robot Framework and `warnings`

**Loggers.** Python code logs through `logging.getLogger("platynui.<module path>")`, where the
module path is the module's dotted path below `PlatynUI`, in lower case. The whole logger name is
therefore the module's `__name__` in lower case: `platynui.core.adapter_devices`,
`platynui.ui.application`, `platynui.baremetal`. Keyword modules use `logging` too, for three
reasons:

- under Robot Framework, a `logging` record from the keyword's thread reaches the keyword's log with
  the same level mapping as `robot.api.logger`;
- only a logger offers `isEnabledFor`, which lets a keyword skip expensive work when its level is
  off (§12, §15);
- outside Robot Framework, `robot.api.logger` writes to a logger named `RobotFramework`, outside the
  `platynui` hierarchy that users configure.

Earlier versions logged the adapter devices under `platynui.devices`; configurations that name that
logger must switch to `platynui.core.adapter_devices`.

**Levels in Python.** The meanings of §3 apply unchanged, with one addition: PlatynUI's Python code
logs no diagnostic at INFO, because Robot Framework shows INFO by default. Robot Framework keeps the
root logger's level equal to `--loglevel` and `Set Log Level`; no user has to call `setLevel` to see
PlatynUI's debug records in the Robot Framework log.

**`robot.api.logger`** stays for keyword output that needs Robot Framework's own features, such as
embedding a screenshot as HTML. It is not used for diagnostics.

**`warnings.warn`** is for the Python API — `platynui_native`, `PlatynUI.core`, `PlatynUI.ui` —
whose users are Python programmers: `DeprecationWarning` for a deprecation, a `UserWarning`
subclass for misuse. Robot Framework does not capture Python warnings, so a deprecation that a
Robot Framework user can hit — a keyword, a keyword argument, an import argument — is reported as a
warning in the log instead, once per run and use site. A deprecated keyword's documentation starts
with `*DEPRECATED ...*`, which Robot Framework reports by itself.

## 15. Keyword action lines

Every action keyword of `PlatynUI.BareMetal` logs one DEBUG line after it succeeds, saying what it
did. When a click in a long suite did not have the effect a test expected, this line answers the
first two questions — which element did it hit, and where exactly — without re-running anything.

```text
clicked Button "OK" #ok at (412, 305), its activation point; button LEFT, 1 click
```

The line has the form `<verb> <what> at <point>, <source>; <parameters>`:

- the element the keyword acted on, in the description form of §13;
- the point or strategy it used and where that point came from: the activation point, the center of
  the bounds, an offset, absolute coordinates, or the current pointer position;
- its parameters: the button, the number of clicks, the scroll direction and ticks.

A keyword without an element names the point and its source instead. A keyboard keyword gives the
length of the sequence as written, or only `secret` for a `Secret` (§16), and when it was given no
element it says that it typed into the focused element — without querying which one that is.
`Highlight` names each element, or the rectangles it was given.

The action keywords are `Pointer Click`, `Pointer Multi Click`, `Pointer Press`, `Pointer Release`,
`Pointer Move To`, `Pointer Scroll`, `Focus`, `Activate Window`, `Restore Window`,
`Maximize Window`, `Minimize Window`, `Close Window`, `Move Window`, `Resize Window`,
`Move And Resize Window`, `Bring To Front`, `Keyboard Type`, `Keyboard Press`, `Keyboard Release`
and `Highlight`. `Query`, the `Wait Until …` keywords, the `Get …` and `Set …` keywords and
`Take Screenshot` write no action line; they act on nothing.

Three rules keep the lines honest and free:

- **Described before the action.** A keyword builds the descriptions of its elements while it
  resolves the element and its point anyway, before it acts. After `Close Window`, or a click that
  closes a dialog, the element may be gone; a keyword never queries an element after an action.
- **Only when DEBUG is on.** The descriptions are built only when `platynui.baremetal` is enabled for
  DEBUG. At Robot Framework's default `--loglevel INFO`, an action keyword makes no provider call
  for its line, and the default log stays exactly as it is.
- **After the native records.** The line is written after the native records of the same action
  have been delivered, so that it closes the keyword's log instead of interleaving with it.

The line does not repeat the text a keyword types. It does not say how long the element resolution
waited either: that belongs to the resolution, which is shared by every keyword that looks an
element up, not to the action.

## 16. Typed text and `Secret`

PlatynUI is a test automation system. Values it reads back from the application — an attribute, a
text field's content — and the way it converted characters into keys may be reported: they are
what a tester needs to see. The text a keyword is *given* to type is different. Robot Framework
already records every keyword's arguments, so a keyword does not repeat them; and when a keyword is
given a `Secret`, nothing PlatynUI produces may give it away.

**What Robot Framework records itself.** Robot Framework records a keyword's arguments as they are
written in the test data: a literal verbatim, a variable by its name. At `--loglevel TRACE` it also
logs the resolved values, and there only a `Secret` is hidden. A value also appears wherever the
suite assigns or logs it. Repeating the argument therefore adds nothing, and only a `Secret` keeps a
value out of a TRACE log.

**Keywords do not repeat what they type.** The keyboard keywords repeat the text or sequence they
were given neither in their keyword line nor in the error they raise. Their keyword line gives the
length of a string as written, or only `secret` for a `Secret`.

**Errors say why, and where.** When a sequence cannot be typed, the error gives:

- the position of the problem in the text the keyword received — after Robot Framework has resolved
  its own escapes and variables — counted in characters from 1;
- which character or key name could not be converted into a key, and the reason the keyboard
  backend gives, for example: the character 'ä' at position 5 cannot be typed: unsupported key: ä;
- for a key name inside `<…>`, that a literal `<` is written `\<` in a sequence — and, for a plain
  string, that Robot Framework test data needs `\\<` for that, because Robot Framework removes a
  backslash before a character it does not know before the keyword runs. `pa<ss>wd` fails on the key
  name `ss` with exactly this hint;
- for a sequence that does not parse, what is wrong at that position in the user's terms — a `<`
  whose key block is never closed with `>`, a missing key name, a `\` that escapes nothing because
  it ends the text or stands before a line break — each with how to write the character literally.
  `Qz7<Kq9` fails at position 4, because that `<` opens a key block that is not closed. Grammar rule
  names never appear;
- for an invalid escape (`\x` without two hex digits, `\u` without four), the escape and its
  position: `C:\users` fails at position 3.

An error that is not tied to a character or key — no keyboard device is ready, another keyboard
input is still active, starting the input failed, sending failed — says so without a position.
Resolving happens before anything is sent, so a sequence that fails to parse or to resolve types
nothing at all. The escape rules themselves are in [`keyboard-input.md`](keyboard-input.md) §6.

**`Secret`.** The keyboard keywords accept Robot Framework's `Secret` when Robot Framework 7.4 or
later is installed, and work unchanged with earlier versions; the import is conditional and the
minimum stays Robot Framework 7.0.

- A `Secret` uses the same sequence syntax as a string: `\\` types a backslash, `\<` and `\>` type
  `<` and `>`, `\xHH` and `\uHHHH` type the character with that code and are an error when not
  followed by two or four hex digits, and a backslash before any other character is dropped.
- A `Secret` cannot be written as a literal in test data; it comes, for example, from
  `VAR    ${password: Secret}    %{PASSWORD}` (see the Robot Framework User Guide on secret
  variables).
- A `Secret` must be the whole argument. Embedded in a longer argument, Robot Framework inserts the
  text `<secret>`, which the sequence grammar reads as a key name. Call `Keyboard Type` once for the
  `Secret` and once for `<Return>`, or build the whole sequence as a `Secret` with
  `VAR    ${sequence: Secret}    %{PASSWORD}<Return>`.
- For a `Secret`, an error gives the position and the kind of failure — the sequence does not
  parse, a key cannot be typed, sending failed — with the syntax hint where one applies. It contains
  no character, no key name, no device message text and no other part of the value. From Python,
  the keyboard calls of `platynui_native.Runtime` select this rendering with their keyword-only
  `sensitive=True` argument, which BareMetal passes for a `Secret`.

**Keys are named only at trace.** Neither the runtime nor a keyboard device knows whether it is
typing a `Secret`. Every record that names a key, character, key code or keysym — the X11 backend's
record of a dynamic keysym remap, the mock keyboard's press and release records — or that carries a
keyboard device's error text is therefore trace-level (§3). Up to debug, nothing PlatynUI derives
from a `Secret` names one of its characters. Trace shows every typed key, a `Secret`'s included, so
a trace log of a run that types secrets is not for sharing.

## 17. The level knob

Every entry point — the command-line tool, the Inspector and the Python extension — reads its level
from the same sources, and the first one present wins. They share one implementation, the
`platynui-log-filter` crate (`crates/log-filter`), so the meaning cannot drift between them:

| Source | What it takes | Filter |
|---|---|---|
| `RUST_LOG` | Filter directives in the standard syntax, target-only directives included | Used verbatim |
| The requested level | A single level: `--log-level` of the command-line tool and the Inspector; the `native_log_level` import argument of `PlatynUI.BareMetal`, or `platynui_native.set_log_level()` from plain Python | See below |
| `PLATYNUI_LOG_LEVEL` | A single level, with the same meaning as the requested level | See below |
| None of them | | `warn` |

**What a single level means.** A single level of `warn`, `info`, `debug` or `trace` applies to
PlatynUI's own modules only, while every other module stays at `warn`: `debug` becomes the filter
`warn,platynui=debug`. A level of `error` or `off` applies to every module. The knob is for
PlatynUI's diagnostics: it never makes a third-party crate such as zbus, wgpu or eframe more
verbose, because their debug output would drown PlatynUI's; and `error` has to silence the whole
process, third-party warnings included.

**`RUST_LOG` is the way into third-party modules,** and the only source of filter directives.
It replaces the whole filter, and a target it does not name is off. Keep a bare level in it, so that
PlatynUI's warnings stay visible:

```sh
RUST_LOG=warn,platynui=debug,zbus=debug   # PlatynUI and zbus at debug, everything else at warn
RUST_LOG=zbus                             # every zbus record, and nothing else
```

**Level names** are case-insensitive: `off`, `error`, `warn`, `info`, `debug`, `trace`, and the Python
spellings `warning` (meaning `warn`) and `critical` and `fatal` (meaning `error`). So
`PLATYNUI_LOG_LEVEL=WARNING` and `native_log_level=WARNING` behave exactly like `warn`.

**Rejected values count as absent.** A `RUST_LOG` that does not parse, and a `PLATYNUI_LOG_LEVEL`
that is not a single level, are rejected, and the next source applies. `PLATYNUI_LOG_LEVEL` takes no
directives: a value such as `platynui_runtime=trace` is rejected with the hint to put directives in
`RUST_LOG`. A plain filter would read an unknown word such as `verbose` as the name of a target and
switch every other target off, so a misspelled level would silence every warning; rejecting it
keeps the default. Each rejected variable and value is reported once per process as a warning that
names both. The extension rebuilds its filter, and reads the environment again, whenever a library
instance requests or withdraws a level, and the report still appears only once. An empty or blank
variable counts as absent without a report.

An invalid level given directly is an error, not a fallback: `--log-level verbose` is a usage error
that lists the accepted names, and `native_log_level=verbose` fails the library import with a
message naming the argument and the accepted values.

**The extension's level is process-wide,** because its subscriber is. Each `PlatynUI.BareMetal`
instance registers its `native_log_level` as a request; while several are live, the most verbose
request applies, and a request ends when its library instance goes out of scope. To see native
debug records in Robot Framework, both are needed: `native_log_level=debug` and a Robot Framework
log level that keeps `DEBUG`.

**The Inspector's GUI stack** — eframe, egui, wgpu, winit — logs through the `log` crate, not
through `tracing`. The Inspector forwards those records into the same filter, so
`RUST_LOG=warn,eframe=debug` shows eframe's debug records, while `--log-level debug` alone shows
PlatynUI's.

**Earlier versions** passed `PLATYNUI_LOG_LEVEL` to the filter verbatim and let `--log-level debug`
lower every crate. Where a setup relied on that, move directives, and any wish for third-party debug
output, into `RUST_LOG`.

**The compositor lane.** The development tools keep their own log setup. `scripts/startcompositor.sh`
hands `PLATYNUI_LOG_LEVEL` to the Wayland compositor as its `--log-level`, which accepts only its
five lower-case names and applies to every crate. `PLATYNUI_LOG_LEVEL=debug` therefore makes the
compositor print third-party debug output, and values the library accepts but the compositor does
not (`WARNING`, `off`, `critical`) stop the lane from starting. In that lane, raise the library's
level with `native_log_level`, or with `RUST_LOG` — which the compositor reads as well, so name the
crates you want (`RUST_LOG=warn,platynui_provider_atspi=debug`) rather than all of `platynui`.

## 18. Looking at a run's warnings

A healthy run has no warning and no error from PlatynUI. Robot Framework's own tools are enough to
check that:

- the console prints every warning and error while the suite runs;
- `log.html` lists them under *Test Execution Errors*, each linked to the keyword that logged it;
- `robotcode results log` lists them from `output.xml`, per test, without opening a browser:

  ```sh
  uv run --no-sync robotcode results log --level WARN --execution-messages
  ```

  `--level WARN` keeps warnings and errors; `--execution-messages` adds the run's execution errors
  from `output.xml`, including those logged outside any test, such as at library import. Run it with
  the same profile as the run, or point `--output` at the run's `output.xml`. In this repository,
  `--no-sync` keeps `uv` from re-syncing the environment, which can replace the native build.

Native records are recognizable by their `[module]` prefix. Every PlatynUI warning in a run means
one of two things: the session really lacks something — then the warning names what, and the fix
is in the environment, the configuration or the application — or a record fires in a healthy
session, and then the record is at the wrong level and is a bug by §3.

To report a problem, run the failing suite with `native_log_level=debug` and `--loglevel DEBUG` and
attach `output.xml`. Leave trace off unless it is asked for: trace names every typed key, a
`Secret`'s included (§16).

## 19. Where this is specified

The requirements of the `diagnostic-logging` spec, and the sections that explain them:

- *Each level has one meaning* — §3, including the typed-text precedence and the Python INFO rule.
- *Warnings and errors state their consequence and are reported once per episode* — §3 (warn and
  error), §5, and §18 for the healthy-run rule.
- *A failure returned to the caller is not reported again* — §4.
- *Decisions name what was decided* — §6.
- *Fallbacks that change an action's effect are reported* — §7.
- *Keyword actions are described at debug level* — §15.
- *Elements are described in one form* — §13.
- *Keywords do not repeat the text they are given to type* — §16.
- *The level setting means the same everywhere* — §17.
- *Configuration mistakes are reported* — §9.
- *A capability that is not there says so once* — §8.

The `native-logging` spec (*Only native warnings and errors are produced by default*) is §17 together
with [`python-bindings.md`](python-bindings.md); the reporting rules of the `runtime-session-config`
spec (*Unclaimed configuration keys are tolerated*) are §9.
