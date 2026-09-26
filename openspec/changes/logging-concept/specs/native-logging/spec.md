# Spec Delta

## MODIFIED Requirements

### Requirement: Only native warnings and errors are produced by default

By default the native core SHALL produce records only at `WARN` and `ERROR`, independent of Robot Framework's log level. Its level SHALL be set as the diagnostic-logging requirement *The level setting means the same everywhere* defines, with the `PlatynUI.BareMetal` import argument `native_log_level` as the requested level. `native_log_level` SHALL accept the level names of that requirement, case-insensitive. An invalid `native_log_level` SHALL fail the library import with a message naming the argument and the accepted values. The level is process-wide, because the native core is: while several library instances request levels, the most verbose request SHALL apply, and a request SHALL be released when its library instance goes out of scope.

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

- **GIVEN** `PLATYNUI_LOG_LEVEL` set to a value that is not a level, and no requested level
- **WHEN** PlatynUI initializes its logging
- **THEN** only `WARN` and `ERROR` SHALL be produced
- **AND** a warning SHALL name `PLATYNUI_LOG_LEVEL` and the rejected value

#### Scenario: Two library instances, the most verbose request applies while it lives

- **GIVEN** two `PlatynUI.BareMetal` instances in one run, one imported with `native_log_level=debug`, the other without
- **WHEN** both are in scope
- **THEN** native debug records SHALL be produced
- **AND** once the instance that requested `debug` has gone out of scope, only `WARN` and `ERROR` SHALL be produced again

#### Scenario: The Python spelling of a level is accepted

- **WHEN** `PlatynUI.BareMetal` is imported with `native_log_level=WARNING`
- **THEN** the import SHALL succeed and only `WARN` and `ERROR` SHALL be produced
