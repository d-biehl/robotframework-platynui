# Spec Delta

## Purpose

What PlatynUI's diagnostics promise to the people who read them: Robot Framework users in their log and console, users of the command-line tools on stderr, and Python users through `logging`. Each level has one meaning. Warnings and errors state their consequence and do not repeat per operation. Failures are reported once. Runtime decisions say what was decided. Keyword actions can be traced. Elements are described one way. Keywords do not repeat the text they type. The level setting means the same everywhere. The listed configuration mistakes do not go unnoticed.

## ADDED Requirements

### Requirement: Each level has one meaning

PlatynUI SHALL place every diagnostic record it adds or changes, in its native core and in its Python code, by one meaning per level:

- **error:** something PlatynUI had to do failed unexpectedly, and the failure was swallowed, so its results or the system's state are wrong and nothing else reports it (a provider's elements missing from every query, keys left held);
- **warning:** PlatynUI knowingly works with less than it was asked for, because of the environment, the configuration or the target application, and says what the user loses (an application that stops answering, a pointer target outside the desktop, a setting it cannot use, a missing backend or capability);
- **info:** a lifecycle transition of a long-lived native resource, such as a runtime being created or shut down, or the Java agent being injected into a JVM, recorded once per transition and never per operation;
- **debug:** what a single operation did or decided: its inputs and outcome (an XPath evaluation, a keyboard sequence's mode and length, a pointer click), a fallback that is normal in some sessions (such as an automatic activation that fails), a call slower than its call class's threshold, a returned failure with its context, or a keyword action line;
- **trace:** a record per element, tick, key or character, or message.

One rule takes precedence over these meanings: a record that names a key, character, key code or keysym, or that carries a keyboard device's error text, is trace-level however often it occurs (see *Keywords do not repeat the text they are given to type*).

Errors and warnings SHALL reach the user by default: in the Robot Framework console and *Test Execution Errors*, and on the standard error of the command-line tools. Native info, debug and trace records SHALL appear only when the user switches their level on. PlatynUI's Python code SHALL NOT log a diagnostic at info, because Robot Framework shows info at its default log level; it records its lifecycle transitions at debug.

A keyword's own output, such as an embedded screenshot, is not a diagnostic and keeps Robot Framework's INFO level.

#### Scenario: Info records are off by default

- **GIVEN** a Robot Framework run at the default log level, without `native_log_level` and without `RUST_LOG` or `PLATYNUI_LOG_LEVEL`
- **WHEN** a runtime is created and keywords run against the mock provider
- **THEN** the log SHALL contain no native info record

#### Scenario: Info records mark lifecycle, not operations

- **GIVEN** `native_log_level=info` and a Robot Framework run at the default log level
- **WHEN** a runtime is created and then ten pointer keywords run against the mock provider
- **THEN** the log SHALL contain the runtime's initialization record at INFO
- **AND** the ten keywords SHALL add no native info record

### Requirement: Warnings and errors state their consequence and are reported once per episode

A diagnostic at warning or error level that PlatynUI adds or changes SHALL state its consequence. It SHALL NOT be produced in a healthy session.

Some conditions can recur on every operation: every query, poll, node read or call. Such a condition SHALL be reported at warning or error level once when it begins for a given subject (a provider, an application, a process), and at debug level while it persists. It SHALL be reported at warning or error level again only after the subject has recovered, as the component that owns the report defines recovery, and then failed anew. The end of an episode SHALL be recorded once, at debug level. The report SHALL name the subject.

#### Scenario: A provider that keeps failing is reported once, by name

- **GIVEN** a UI tree provider whose listing of top-level elements fails on every desktop enumeration
- **WHEN** a `Wait Until Exists` polls the tree for ten seconds
- **THEN** the log SHALL contain exactly one error naming that provider and saying that its elements are missing from query results
- **AND** once the provider lists its elements again and later fails anew, one further error SHALL be logged

#### Scenario: An application that stops answering is named once

- **GIVEN** an application whose accessibility calls time out, while some of its other calls still answer
- **WHEN** many of its element properties are read
- **THEN** the log SHALL contain one warning naming the application (its name, and its pid where known), its bus name, the call and the timeout, and debug records for the further timeouts
- **AND** after the application quits and a new instance of it times out, one further warning SHALL be logged
- **NOTE** The latch is verified with a unit test; the real AT-SPI provider in the X11 lane.

#### Scenario: A healthy run reports no PlatynUI warning

- **GIVEN** the mock lane, the X11 lane and the compositor lane on a healthy session
- **WHEN** they run
- **THEN** none of their execution errors SHALL come from PlatynUI
- **NOTE** X11 and compositor lanes: real sessions only, checked with `robotcode results log --level WARN --execution-messages` after each lane.

### Requirement: A failure returned to the caller is not reported again

When PlatynUI returns a failure to its caller, no record it adds or changes SHALL also log that failure at warning or error level. A failure is returned when it is an error value in Rust, an exception in Python, or a failed keyword in Robot Framework. PlatynUI MAY record the returned failure at debug level with context.

The layer that swallows a failure, or decides what happens because of it, SHALL be the only one that reports it at warning or error level. A failure swallowed while enumerating the tree is therefore reported by the enumeration, and not again by the call that failed inside it.

#### Scenario: A returned connection failure appears once

- **GIVEN** an accessibility bus that does not answer the provider's connection attempt
- **WHEN** `Get Element At Point`, which returns the provider's failure to its caller, fails with the connection timeout
- **THEN** the caller SHALL see the timeout
- **AND** the log SHALL contain no PlatynUI warning or error for it
- **NOTE** Real AT-SPI provider only. A query that enumerates the desktop meanwhile reports the provider once, through the enumeration's error (*Warnings and errors state their consequence and are reported once per episode*).

#### Scenario: A failed keyboard sequence appears once

- **GIVEN** a keyboard sequence that fails, after which all pressed keys are released
- **WHEN** the keyword returns its error
- **THEN** the log SHALL contain no warning or error for that failure

### Requirement: Decisions name what was decided

When PlatynUI chooses the platform backend, the active UI tree providers or the Wayland input backend, it SHALL record which it chose and why, and at debug level the alternatives it rejected. A decision record that PlatynUI adds or changes SHALL do the same. When no alternative is usable, it SHALL warn, naming the candidates it tried and what stops working as a result. In a test build of the Python extension that is used without its mock backend, the test-build warning (*A test build used without its mock backend says so once*) is the one report: the missing platform backend and the missing providers are then recorded at debug level.

#### Scenario: Runtime initialization names backend and providers

- **GIVEN** native diagnostics at info level
- **WHEN** a runtime is created
- **THEN** exactly one info record SHALL name:
  - the chosen platform backend, or that none was chosen;
  - whether the backend was forced by configuration;
  - the active provider ids;
  - the desktop bounds;
  - the monitor count

#### Scenario: No platform backend can serve the session

- **GIVEN** a build that links the real platform backends, in an environment in which none of them can serve the session
- **WHEN** a runtime is created
- **THEN** a warning SHALL name the backends that were tried
- **AND** it SHALL state that pointer, keyboard, screenshot, highlight and window control are unavailable

#### Scenario: No UI tree provider is active

- **GIVEN** a build that links the real providers, and a runtime created without any active one
- **WHEN** it is created
- **THEN** a warning SHALL say that queries find only the desktop node

#### Scenario: No input backend is available on Wayland

- **GIVEN** a Wayland session in which every input backend fails to initialize
- **WHEN** the platform initializes its input
- **THEN** one warning SHALL name, for each backend tried, why it failed
- **NOTE** Real Wayland session only.

#### Scenario: A test build used for real work says so

- **GIVEN** a build of the Python extension made for tests, which links only the mock providers
- **WHEN** a runtime is created without selecting the mock backend
- **THEN** a warning SHALL say that the build links no real platform or providers, and how to get a real build

### Requirement: Fallbacks that change an action's effect are reported

PlatynUI SHALL record the following fallbacks, in which an action proceeds differently from what was asked, and every fallback record it adds or changes, as stated:

- A pointer target outside the desktop, moved to the nearest edge, SHALL be reported as a warning.
- Keys that could not be released after a keyboard error SHALL be reported as an error.
- A window that did not become the foreground window after activation SHALL be recorded at debug level.
- A failed automatic activation before an action SHALL be recorded at debug level, with the element and the reason, and the action SHALL proceed as before.

#### Scenario: A clamped pointer target is reported

- **GIVEN** a pointer target outside the desktop bounds
- **WHEN** the pointer moves there
- **THEN** a warning SHALL give the requested and the actual coordinates

#### Scenario: Keys that stay pressed are reported

- **GIVEN** a keyboard sequence that fails, and a release of the pressed keys that fails as well
- **WHEN** the keyword returns its error
- **THEN** an error record SHALL say how many keys may remain held down

#### Scenario: A failed automatic activation is traceable

- **GIVEN** `auto_activate` enabled, and an element whose window cannot be brought to the front
- **WHEN** a pointer or keyboard keyword acts on it
- **THEN** the keyword SHALL proceed
- **AND** the Robot Framework log at debug level SHALL name the element and the activation error

### Requirement: Keyword actions are described at debug level

Every action keyword of the Robot Framework library SHALL, after it succeeds, log one debug-level line that describes what it did:

- the element it acted on;
- the point or strategy it used, and where that point came from: activation point, center of the bounds, an offset, absolute coordinates, or the current pointer position;
- the button, the number of clicks, or the scroll direction and ticks; for a keyboard keyword, only the length of the sequence it was given, or for a `Secret` only that it is secret.

A keyword that acts without an element says what it acted on instead: the point and its source for a pointer keyword, the focused element for a keyboard keyword.

The element SHALL be described as it was before the action. When the debug level is off, a keyword SHALL NOT query the element for its line.

Keyword lines SHALL NOT appear in a log kept at the default info level. Text a keyword types SHALL NOT appear in them.

#### Scenario: A click can be traced

- **GIVEN** a Robot Framework run at `--loglevel DEBUG` against the mock provider
- **WHEN** `Pointer Click` clicks the mock's OK button
- **THEN** the keyword's log SHALL contain one debug line with `Button "OK"`, the click coordinates, their source and the mouse button

#### Scenario: The default log stays as it is

- **GIVEN** a Robot Framework run at the default log level
- **WHEN** action keywords run
- **THEN** the log SHALL contain no keyword action lines

#### Scenario: Typing is traced without its text

- **GIVEN** a Robot Framework run at `--loglevel DEBUG`
- **WHEN** `Keyboard Type` types a string
- **THEN** the keyword's debug line SHALL give the length of the string as written
- **AND** it SHALL NOT contain the string

### Requirement: Elements are described in one form

Keyword lines, error messages that PlatynUI adds or changes, and the element description available from Python SHALL describe a UI element in one form:

- the role;
- the name in double quotes, cut to at most 60 of its characters and then escaped, so that the description is always one line;
- `#` followed by the element's id, when it has one.

Native diagnostics introduced or changed from now on SHALL use the same form for an element field.

Displaying an element — its Python representation, or a Robot Framework assignment that shows it — SHALL NOT query the application. The full description SHALL be produced only when it is asked for or logged.

Error messages that PlatynUI adds or changes, those of element lookup and those for a missing platform backend SHALL NOT name internal types. When an element cannot be found, the error SHALL say which of two lookups failed:

- the element the keyword was given;
- the root set by `Set Root` that the lookup depends on.

#### Scenario: The description of a mock button

- **WHEN** the description of the mock's OK button is asked for from Python
- **THEN** it SHALL read `Button "OK"`

#### Scenario: A name with a line break

- **GIVEN** an element whose name contains a line break
- **WHEN** it is described
- **THEN** the description SHALL be one line, with the line break escaped

#### Scenario: An element with an id

- **GIVEN** an element with role `Button`, name `OK` and id `ok`
- **WHEN** it is described
- **THEN** the description SHALL read `Button "OK" #ok`

#### Scenario: A vanished root is named as the cause

- **GIVEN** a root set with `Set Root` that no longer exists
- **WHEN** `Wait Until Exists` looks for an element relative to it
- **THEN** the error SHALL say that the root set by `Set Root` was not found, with the root's query and the timeout
- **AND** it SHALL NOT blame the element query the keyword was given

### Requirement: Keywords do not repeat the text they are given to type

Robot Framework already records the arguments of every keyword: as written in the test data, and at `--loglevel TRACE` also their values. The keyboard keywords SHALL therefore not repeat the text or sequence they were given, neither in their keyword lines nor in the error they raise when it cannot be typed. Their keyword line SHALL give the length of a string sequence as written, and for a `Secret` only that the input is secret.

When a character or key of the sequence cannot be typed, the error SHALL say why:

- the position in the text the keyword received, after Robot Framework has resolved its own escapes and variables, counted in characters from 1;
- which character or key name could not be converted into a key, and the reason the keyboard backend gives;
- for a key name from a shortcut, that a literal `<` is written `\<` in the sequence, and, for a plain string, that this is `\\<` in Robot Framework test data;
- for a sequence that does not parse, what is wrong there in the user's terms: a `<` whose shortcut is not closed with `>`, a missing key name, or a `\` that escapes nothing, each with how to write the character literally. Grammar rule names SHALL NOT appear.

An error that is not tied to a character or key, such as a missing keyboard device, a failure to start or end the input, or a failure while sending, SHALL say so without a position.

Diagnostics MAY report values read from the application, and how characters were converted into keys. A record that names a key, character, key code or keysym, or that carries a keyboard device's error text, belongs to the trace level, because neither the runtime nor a device can tell whether the sequence is a `Secret`.

The keyboard keywords SHALL accept Robot Framework's `Secret` values when Robot Framework 7.4 or later is installed, and SHALL work unchanged with earlier versions. A `Secret` uses the same sequence syntax as a string.

For a `Secret`, an error SHALL give the position and the kind of failure, including the sequence syntax involved (an unclosed `<`, a `\` that escapes nothing, how to write a literal `<`). It SHALL contain no other character, no key name, no device message text and no other part of the value. A failure while sending SHALL be given as such, without the device's text.

No keyword line, error or record up to the debug level that PlatynUI produces from the `Secret` it was given SHALL contain its value or name any of its characters or key names. Values that PlatynUI later reads back from the application are not covered.

#### Scenario: A character that cannot be typed says why

- **WHEN** `Keyboard Type` is given a text containing a character or key name the keyboard cannot produce
- **THEN** the error SHALL name its position, the character or key name, and the backend's reason
- **AND** it SHALL NOT repeat the rest of the text

#### Scenario: A key name that is not known hints at the escape

- **WHEN** `Keyboard Type` receives `pa<ss>wd`, whose `<ss>` is read as a shortcut
- **THEN** the error SHALL name `ss`, its position, and that a literal `<` is written `\<`, which is `\\<` in Robot Framework test data
- **AND** it SHALL NOT repeat the rest of the text

#### Scenario: A text that does not parse is not repeated

- **WHEN** `Keyboard Type` receives `Qz7<Kq9`
- **THEN** the error SHALL name position 4, say that this `<` opens a key block that is not closed with `>`, and say how to write a literal `<`
- **AND** it SHALL NOT repeat the text or name grammar rules

#### Scenario: A secret stays secret

- **GIVEN** Robot Framework 7.4 or later and a `Secret` value
- **WHEN** it is passed to `Keyboard Type` at `--loglevel TRACE` and `native_log_level=debug`
- **THEN** its value SHALL be typed
- **AND** it SHALL appear nowhere in `output.xml`
- **AND** no record up to the debug level SHALL name any of its characters or key names

#### Scenario: A secret that cannot be typed gives its position and the kind of failure

- **GIVEN** a `Secret` containing a key name the keyboard does not know
- **WHEN** it is passed to `Keyboard Type`
- **THEN** the error SHALL name the position and say that a key cannot be typed, with the hint on writing a literal `<`
- **AND** it SHALL NOT contain the key name or any other part of the value

#### Scenario: Earlier Robot Framework versions

- **GIVEN** a Robot Framework without `Secret`
- **WHEN** `Keyboard Type` types a string
- **THEN** it SHALL work as before

### Requirement: The level setting means the same everywhere

Every PlatynUI entry point SHALL read its log level from the same sources, in this order of precedence:

1. `RUST_LOG`
2. the level given on the command line or to the library
3. `PLATYNUI_LOG_LEVEL`
4. `warn`

The entry points are the command-line tool, the Inspector and the Python extension.

`RUST_LOG` SHALL keep the standard filter-directive syntax, including target-only directives. It is the only source that takes directives, and the only one that makes third-party modules more verbose.

The level given on the command line or to the library, and `PLATYNUI_LOG_LEVEL`, SHALL each be a single level. A level of `warn`, `info`, `debug` or `trace` SHALL apply to PlatynUI's own modules only, while third-party modules stay at `warn`. A level of `error` or `off` SHALL apply to every module.

Level names SHALL be case-insensitive and SHALL include:

- `off`;
- the Python spelling `warning`, meaning `warn`;
- `critical` and `fatal`, meaning `error`.

A rejected value SHALL count as absent, so that the next source applies. It SHALL be reported as a warning naming the variable and the value, once per process. Rejected are a `RUST_LOG` that does not parse, and a `PLATYNUI_LOG_LEVEL` that is not a level, directive syntax included.

#### Scenario: The command-line level opens PlatynUI's modules only

- **WHEN** the command-line tool runs with `--log-level debug`
- **THEN** debug records from PlatynUI's modules SHALL be shown
- **AND** debug records from third-party modules SHALL NOT be shown unless `RUST_LOG` enables them

#### Scenario: A single level in the environment opens PlatynUI's modules only

- **GIVEN** `PLATYNUI_LOG_LEVEL=debug`
- **WHEN** any entry point starts
- **THEN** debug records from PlatynUI's modules SHALL be shown
- **AND** debug records from third-party modules SHALL NOT be shown

#### Scenario: Error silences every module's warnings

- **WHEN** the command-line tool runs with `--log-level error`
- **THEN** no warning SHALL be shown, neither from PlatynUI's modules nor from third-party modules

#### Scenario: The Python spelling is understood

- **GIVEN** `PLATYNUI_LOG_LEVEL=WARNING`, or `Library    PlatynUI.BareMetal    native_log_level=WARNING`
- **WHEN** it takes effect
- **THEN** warnings and errors SHALL be shown, exactly as with `warn`

#### Scenario: A misspelled level is reported, not obeyed

- **GIVEN** `PLATYNUI_LOG_LEVEL=verbose`
- **WHEN** any entry point starts, and a library instance then requests and withdraws a level twice
- **THEN** exactly one warning SHALL name `PLATYNUI_LOG_LEVEL` and `verbose`
- **AND** warnings and errors SHALL be shown as by default

#### Scenario: Directives belong in RUST_LOG

- **GIVEN** `PLATYNUI_LOG_LEVEL=platynui_runtime=trace`
- **WHEN** any entry point starts
- **THEN** a warning SHALL say that `PLATYNUI_LOG_LEVEL` takes a single level and that directives belong in `RUST_LOG`
- **AND** warnings and errors SHALL be shown as by default

#### Scenario: A RUST_LOG that does not parse falls through

- **GIVEN** `RUST_LOG=zbus=loud` and `native_log_level=debug`
- **WHEN** PlatynUI initializes its logging
- **THEN** a warning SHALL name `RUST_LOG` and the rejected value
- **AND** debug records from PlatynUI's modules SHALL be produced, as the requested level says

#### Scenario: A target-only RUST_LOG is accepted

- **GIVEN** `RUST_LOG=zbus`
- **WHEN** any entry point starts
- **THEN** every record from `zbus` SHALL be shown, as the directive syntax defines

### Requirement: Configuration mistakes are reported

The following configuration mistakes SHALL NOT be dropped silently. Components are named in bucket form, such as `providers.atspi` or `platform.x11`.

- A provider or platform setting of the wrong type SHALL be reported as a warning naming the component, the key, the expected type and the found type, and the default SHALL apply.
- A configuration value that the Python binding cannot pass on (a key that is not a string, or a value of an unsupported type) SHALL be reported as a warning naming its dotted path and its Python type.
- A pointer or keyboard profile value of the wrong type SHALL be rejected with an error naming the key and the expected type, as the profiles' other fields already are.
- A profile key that no field reads SHALL be reported as a warning naming the key.

A component checks its settings before it builds anything, so that its warnings are logged even when the build then fails.

#### Scenario: A string where a flag was expected

- **GIVEN** `config={'providers': {'atspi': {'surface_popups': 'False'}}}`
- **WHEN** the AT-SPI provider is built
- **THEN** a warning SHALL name `providers.atspi`, `surface_popups`, the expected boolean and the found string
- **AND** the default SHALL apply

#### Scenario: A string where a flag was expected, Java

- **GIVEN** `config={'providers': {'java': {'agent': {'enabled': 'False'}}}}`
- **WHEN** a runtime is created
- **THEN** a warning SHALL name `providers.java` and `agent.enabled`
- **NOTE** Windows only.

#### Scenario: A wrong type that makes the build fail

- **GIVEN** `config={'platform': {'backend': 'x11', 'x11': {'display': 1}}}` and no reachable X server
- **WHEN** a runtime is created
- **THEN** a warning SHALL name `platform.x11` and `display`
- **AND** the construction SHALL fail as it would without the setting

#### Scenario: A value the binding cannot pass on

- **GIVEN** a setting whose value is a `pathlib.Path`
- **WHEN** a runtime is created from Python
- **THEN** a warning SHALL name the setting's dotted path and the type `Path`

#### Scenario: A profile value of the wrong type

- **WHEN** `pointer_profile={'after_click_delay_ms': '100ms'}` is given
- **THEN** the call SHALL fail with an error naming `after_click_delay_ms` and the expected number

#### Scenario: A misspelled profile key

- **WHEN** `pointer_profile={'speed_factr': 2}` is given
- **THEN** a warning SHALL name `speed_factr` as an unknown key
- **AND** the call SHALL proceed

### Requirement: A capability that is not there says so once

When PlatynUI runs where a capability it would normally provide is not implemented or not installed, it SHALL say so once, at the moment it matters. It SHALL NOT behave as if the capability were present and simply find nothing.

This applies to:

- a UI tree provider that is a stub on its platform;
- watching for UI events when no active provider emits them;
- Java Access Bridge support, when a Swing or AWT window is found that no Java backend serves because the bridge DLL is missing.

Where nothing needs the capability, PlatynUI SHALL stay silent above debug level.

#### Scenario: Watching without an event source

- **GIVEN** no active provider that emits UI events
- **WHEN** `platynui-cli watch` starts
- **THEN** a warning SHALL say that no active provider emits UI events and that the command will wait without output

#### Scenario: Watching with an event source

- **GIVEN** the mock provider, which emits UI events
- **WHEN** `platynui-cli watch` starts
- **THEN** no warning SHALL be logged

#### Scenario: A test build used without its mock backend says so once

- **GIVEN** a build of the Python extension made for tests
- **WHEN** runtimes are created without selecting the mock backend
- **THEN** exactly one warning per process SHALL say so, and the construction SHALL record no other warning
- **AND** the warning SHALL have reached Python `logging` when the constructor returns, without a further PlatynUI call

#### Scenario: The macOS provider stub

- **GIVEN** macOS, where the accessibility provider is not implemented yet
- **WHEN** a runtime is created
- **THEN** one warning per process SHALL say that queries on this desktop find nothing because macOS support is not implemented yet
- **NOTE** macOS only.

#### Scenario: Java Access Bridge is missing only when it matters

- **GIVEN** Windows without the Java Access Bridge DLL
- **WHEN** no Swing or AWT window is open, or every such window is served by the in-JVM agent
- **THEN** no warning SHALL be logged about the bridge
- **AND** when a Swing or AWT window is found that no Java backend serves, one warning per process SHALL name the missing bridge DLL and how to provide it
- **NOTE** Windows only.
