# Spec Delta

## MODIFIED Requirements

### Requirement: Each level has one meaning

PlatynUI SHALL place every diagnostic record it adds or changes, in its native core and in its Python code, by one meaning per level:

- **error:** something PlatynUI had to do failed unexpectedly, and the failure was swallowed, so its results or the system's state are wrong and nothing else reports it (a provider's elements missing from every query, keys left held);
- **warning:** PlatynUI knowingly works with less than it was asked for, because of the environment, the configuration or the target application, and says what the user loses (an application that stops answering, a pointer target outside the desktop, a setting it cannot use, a missing backend or capability);
- **info:** a lifecycle transition of a long-lived native resource, such as a runtime being created or shut down, or the Java agent being injected into a JVM, recorded once per transition and never per operation;
- **debug:** what a single operation did or decided: its inputs and outcome (a keyword action line, a keyboard sequence's mode and length, a pointer click), a fallback that is normal in some sessions (such as an automatic activation that fails), a call slower than its call class's threshold, or a returned failure with its context;
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

## ADDED Requirements

### Requirement: XPath evaluation's own diagnostics exist in debug builds only

Evaluating an XPath expression acts on nothing in the UI, so the records that the evaluation writes about itself serve only the development of PlatynUI. They are the records of the XPath engine and of the runtime's adapter that presents the UI tree to the engine. They SHALL exist in debug builds only: a release build, such as the one the published packages contain, SHALL contain none of them, at any level. In a debug build they SHALL be treated like every other PlatynUI record: recorded under the name of their module and shown as the level setting says (*The level setting means the same everywhere*), with no target name, filter rule or `RUST_LOG` exception of their own.

The record of the XPath function `fn:trace()` is not a diagnostic of the evaluation. It is output that the user asked for in their own expression, so it SHALL be produced in every build, at debug level, with the trace's label and value.

XPath evaluation SHALL NOT log a warning or an error. Every failure of an evaluation SHALL be returned to its caller, which decides what it means (*A failure returned to the caller is not reported again*): a keyword fails with it, unless its query settings tell it to ignore such errors while it waits. This covers an expression that does not compile, an error raised while the expression runs, and a call of `fn:error()`.

The records that providers and the enumeration of the desktop write while an evaluation reads the UI tree are not the evaluation's own. They are outside this requirement and exist in every build.

#### Scenario: A release build records only the XPath trace function

- **GIVEN** a release build, native diagnostics at trace level, and a UI tree of one window whose nodes write no records of their own
- **WHEN** `trace(count(//Window), 'windows')` is evaluated
- **THEN** the evaluation SHALL produce exactly one record: the debug record of `fn:trace()`, with the label `windows` and the value `1`
- **NOTE** Verified by a runtime unit test in a test run of a release build; the regular test run builds in debug mode. No provider or platform is involved.

#### Scenario: A debug build keeps the evaluation's records under the level setting

- **GIVEN** a debug build, native diagnostics at trace level, and the same tree
- **WHEN** the same expression is evaluated
- **THEN** the evaluation's own records SHALL be produced at their debug and trace levels, next to the record of `fn:trace()`
- **AND** each of them SHALL come from a PlatynUI module, which the level setting reaches like any other
- **NOTE** The same unit test, in the regular test run.

#### Scenario: The XPath trace function reaches the Robot Framework log from a release build

- **GIVEN** a release build of the Python extension with the mock provider, `native_log_level=debug`, and Robot Framework at `--loglevel DEBUG`
- **WHEN** `Query    trace(count(//*), 'node-count')` runs
- **THEN** the keyword's log SHALL contain the record of `fn:trace()` with the label `node-count`, at DEBUG
- **NOTE** Verifiable against the mock provider: the existing test of this record, run once against a release build.

#### Scenario: A failing expression is returned, not logged

- **GIVEN** native diagnostics at trace level, in a debug or a release build
- **WHEN** an expression is evaluated that does not compile, one that fails while it runs because a value cannot be cast, and one that calls `error()`
- **THEN** each evaluation SHALL return an XPath error to its caller
- **AND** none of them SHALL produce a warning or an error record
- **NOTE** Verified by a runtime unit test in both builds. The mock lane evaluates broken selectors on purpose, and its check for PlatynUI warnings covers the keyword's side.
