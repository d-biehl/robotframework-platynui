# Spec Delta

## Purpose

How diagnostics emitted by PlatynUI's native core reach Python's `logging` module and the Robot Framework log, so that warnings, identification records and requested debug output are visible where Python and Robot Framework users look for them, without ever blocking or deadlocking the native code that emits them.

## ADDED Requirements

### Requirement: Native diagnostics reach Python logging

A diagnostic event emitted by PlatynUI's native core at an enabled level SHALL be delivered to Python's `logging` module as a log record. The record's logger SHALL be named after the native module that emitted it, under the `platynui.native` hierarchy (for example `platynui.native.provider_atspi.extents` for an event of the AT-SPI provider's extents module), so that Python code can select and filter native records like any other logger. The record's level SHALL correspond to the event's level: error → `ERROR`, warn → `WARNING`, info → `INFO`, debug → `DEBUG`, trace → a level below `DEBUG`. The record's message SHALL contain the event's message, the name of the native module that emitted it, and every structured field of the event with its value. A record SHALL be delivered at most once.

#### Scenario: A native warning becomes a Python warning record

- **GIVEN** a Python program that uses PlatynUI and has a handler on the `platynui.native` logger
- **WHEN** the native core emits a warning with a message and two structured fields during a PlatynUI call
- **THEN** the handler SHALL receive exactly one record at level `WARNING` from a logger under `platynui.native`, whose message contains the event's message, the emitting module's name and both fields with their values

#### Scenario: Levels map one to one

- **GIVEN** native levels enabled down to trace
- **WHEN** the native core emits one event at each level
- **THEN** Python SHALL receive records at `ERROR`, `WARNING`, `INFO`, `DEBUG` and a level below `DEBUG`, in that correspondence

#### Scenario: The XPath trace function is visible

- **GIVEN** native debug output enabled
- **WHEN** an XPath expression using `fn:trace()` is evaluated through PlatynUI
- **THEN** a debug record under `platynui.native` SHALL carry the trace label and value
- **NOTE** Verifiable against the mock provider.

### Requirement: Native diagnostics appear in the Robot Framework log of the keyword they belong to

When PlatynUI is used from Robot Framework, every delivered native record SHALL appear in the Robot Framework log with the corresponding RF level (`ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE`), subject to RF's own log level (`--loglevel`, `Set Log Level`). Records SHALL be delivered on the thread that calls into PlatynUI, so that Robot Framework records them. A record emitted while a PlatynUI keyword runs — by the calling thread or by any native background thread — SHALL appear in that keyword's log, or, when it is emitted after that keyword's last delivery, in the log of the next PlatynUI keyword at the latest. A record emitted by a thread other than the one that delivers it SHALL name the thread that emitted it and the time it was emitted, because Robot Framework stamps a message with its delivery time.

#### Scenario: A warning during a keyword is a warning of that keyword

- **GIVEN** a Robot Framework suite that uses `PlatynUI.BareMetal`
- **WHEN** the native core emits a warning while a PlatynUI keyword runs
- **THEN** the RF log SHALL contain that message at level `WARN` inside that keyword
- **AND** it SHALL be listed among the run's execution errors, as every RF warning is

#### Scenario: A background thread's record is not lost

- **GIVEN** a Robot Framework suite that uses `PlatynUI.BareMetal`
- **WHEN** a native background thread emits a warning while a PlatynUI keyword runs
- **THEN** the RF log SHALL contain that message inside that keyword or the next PlatynUI keyword
- **AND** the message SHALL name the thread that emitted it and the time it was emitted

#### Scenario: RF's own log level still applies

- **GIVEN** native debug output enabled and RF running with `--loglevel INFO`
- **WHEN** the native core emits a debug event during a keyword
- **THEN** the RF log SHALL NOT contain it
- **AND** after `Set Log Level    DEBUG`, a later debug event SHALL appear

### Requirement: Only native warnings and errors are produced by default

By default the native core SHALL produce records only at `WARN` and `ERROR`, independent of Robot Framework's log level. The level SHALL be lowerable for PlatynUI's own native modules through a `PlatynUI.BareMetal` import argument, `native_log_level`, accepting `error`, `warn`, `info`, `debug` and `trace` (case-insensitive), and through the environment variables the command-line tool uses: `RUST_LOG` and `PLATYNUI_LOG_LEVEL`, with the same directive syntax. Their precedence SHALL be the command-line tool's: `RUST_LOG`, then the explicitly requested level, then `PLATYNUI_LOG_LEVEL`, then the `WARN` default. Native modules that are not part of PlatynUI SHALL stay at `WARN` unless an environment directive enables them; the import argument SHALL NOT lower them. An invalid `native_log_level` SHALL fail the library import with a message naming the argument and the accepted values; an invalid environment value SHALL leave the default in effect and SHALL be reported as a warning naming the variable. The level is process-wide, because the native core is: while several library instances request levels, the most verbose request SHALL apply, and a request SHALL be released when its library instance goes out of scope.

#### Scenario: The default shows warnings and nothing below

- **GIVEN** a Robot Framework run with `--loglevel DEBUG`, no `native_log_level` and neither environment variable set
- **WHEN** the native core emits a debug event and a warning during a keyword
- **THEN** the RF log SHALL contain the warning and SHALL NOT contain the debug event

#### Scenario: The import argument lowers the floor for PlatynUI's modules only

- **GIVEN** `Library    PlatynUI.BareMetal    native_log_level=debug` and RF running with `--loglevel DEBUG`
- **WHEN** a PlatynUI module and a third-party module each emit a debug event during a keyword
- **THEN** the RF log SHALL contain the PlatynUI module's debug record and SHALL NOT contain the third-party module's

#### Scenario: The environment overrides as in the command-line tool

- **GIVEN** `RUST_LOG=zbus=debug` in the environment and `native_log_level=warn` on the import
- **WHEN** zbus emits a debug event
- **THEN** it SHALL be delivered, because `RUST_LOG` takes precedence over the import argument

#### Scenario: An invalid import value fails the import by name

- **WHEN** `PlatynUI.BareMetal` is imported with `native_log_level=verbose`
- **THEN** the import SHALL fail with a message naming `native_log_level` and the accepted values

#### Scenario: An invalid environment value keeps the default and says so

- **GIVEN** `PLATYNUI_LOG_LEVEL` set to a value that is not a valid directive
- **WHEN** PlatynUI initializes its logging
- **THEN** only `WARN` and `ERROR` SHALL be produced
- **AND** a warning SHALL name `PLATYNUI_LOG_LEVEL` and the rejected value

#### Scenario: Two library instances, the most verbose request applies while it lives

- **GIVEN** two `PlatynUI.BareMetal` instances in one run, one imported with `native_log_level=debug`, the other without
- **WHEN** both are in scope
- **THEN** native debug records SHALL be produced
- **AND** once the instance that requested `debug` has gone out of scope, only `WARN` and `ERROR` SHALL be produced again

### Requirement: Logging never blocks or deadlocks native code

Emitting a native diagnostic SHALL NOT call into Python, wait for the Python interpreter, or wait for a PlatynUI call to finish, on any thread. A native background thread SHALL be able to log while the Python caller waits for it — including while a PlatynUI call holds the interpreter and waits for that thread to finish. A Python log handler that calls back into PlatynUI while it handles a native record SHALL NOT deadlock; records emitted during that nested call SHALL still be delivered.

#### Scenario: A thread that logs while the caller waits for it

- **GIVEN** a PlatynUI call that waits for a native background thread to finish while the calling Python thread holds the interpreter
- **WHEN** that background thread emits a warning before it finishes
- **THEN** the call SHALL return normally
- **AND** the warning SHALL be delivered afterwards on the calling thread

#### Scenario: Shutdown with logging background threads

- **GIVEN** a runtime whose native background threads log while they stop
- **WHEN** the runtime is shut down from Python
- **THEN** the shutdown SHALL complete, and the records those threads emitted SHALL be delivered on the calling thread
- **NOTE** Verifiable against the real AT-SPI provider (popup watcher) or the Wayland backend (event loop); the mock has no background threads.

#### Scenario: A log handler that calls back into PlatynUI

- **GIVEN** a Python log handler on `platynui.native` that calls a PlatynUI function while handling a record
- **WHEN** a native warning is delivered to it
- **THEN** the nested call SHALL complete without deadlock
- **AND** records emitted during the nested call SHALL be delivered, each exactly once

### Requirement: Undelivered records are bounded and their loss is reported

Records waiting for delivery SHALL be held in a bounded buffer, so that a burst of native output cannot exhaust memory while no PlatynUI call delivers them. When the buffer is full, further records SHALL be dropped, not delivered out of order, and the next delivery SHALL report as a warning how many records were dropped.

#### Scenario: A flood is cut off and reported

- **GIVEN** native trace output enabled
- **WHEN** more records are emitted between two deliveries than the buffer holds
- **THEN** the next delivery SHALL deliver the records that fit, in the order they were emitted
- **AND** it SHALL deliver one warning stating how many records were dropped
