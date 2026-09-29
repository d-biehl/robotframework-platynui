# Spec Delta

## Purpose

Reading the value of one attribute of one element with `PlatynUI.BareMetal`: the `Get Attribute Value` keyword, and `Get Attribute`, its deprecated former name. The keyword reads the value once and can check it once; waiting until a value holds is `Wait Until Attribute Value` in `baremetal-waiting`.

## ADDED Requirements

### Requirement: Get Attribute Value reads one attribute of one element

The library SHALL provide a `Get Attribute Value` keyword that reads one attribute of one element and returns the attribute's typed value. The element SHALL be given as a selector, which the keyword waits for under the effective query settings and `query_overrides`, or as a captured element. The attribute name SHALL be given bare, or with a namespace prefix such as `app:` or `native:`. When an assertion operator and an expected value are given, the keyword SHALL check the value it read once and fail at once when the check does not hold. It SHALL NOT read the attribute again to wait for the check to hold. With or without an assertion, the keyword SHALL return the value it read.

#### Scenario: The typed value is returned

- **GIVEN** a window whose `@IsMaximized` is `False`
- **WHEN** `Get Attribute Value` is called with a selector for that window and the attribute `IsMaximized`
- **THEN** it SHALL return the boolean `False`

#### Scenario: A prefixed attribute is read in its namespace

- **GIVEN** a window whose `@native:ProcessId` is `4242`
- **WHEN** `Get Attribute Value` is called for that window with the attribute `native:ProcessId`
- **THEN** it SHALL return `4242`

#### Scenario: An assertion that holds returns the value

- **GIVEN** a window whose `@Name` is `Operations Console`
- **WHEN** `Get Attribute Value` is called for that window and `Name` with the operator `==` and the expected value `Operations Console`
- **THEN** it SHALL return `Operations Console`

#### Scenario: An assertion that fails fails at once

- **GIVEN** a window whose `@Name` is `Operations Console`, and a per-call timeout of 30 seconds
- **WHEN** `Get Attribute Value` is called for that window and `Name` with the operator `==` and the expected value `Wrong Name`
- **THEN** it SHALL fail within a few seconds, well before the timeout, with AssertionEngine's actual-versus-expected diagnostic

#### Scenario: An element that never appears fails as not found

- **GIVEN** a selector that matches nothing for the whole timeout
- **WHEN** `Get Attribute Value` is called with that selector
- **THEN** it SHALL fail with `ElementNotFoundError`, naming the selector and ending in `within timeout of {timeout} seconds.`

#### Scenario: A missing attribute fails at once

- **GIVEN** a window that has no `ToggleState` attribute
- **WHEN** `Get Attribute Value` is called for that window and `ToggleState`
- **THEN** it SHALL fail with `AttributeNotFoundError` naming the attribute, without waiting for the attribute to appear

#### Scenario: A captured element from another library import is rejected

- **GIVEN** an element captured through one import of the library
- **WHEN** a second import of the library calls `Get Attribute Value` with that element
- **THEN** it SHALL fail with the error for an element that belongs to a different library instance

### Requirement: Get Attribute is a deprecated alias of Get Attribute Value

The library SHALL keep `Get Attribute` as a deprecated alias of `Get Attribute Value` until the alias is removed before PlatynUI 1.0. The alias SHALL take the same arguments and SHALL behave exactly like `Get Attribute Value`: the same element lookup, the same typed value, the same assertion checked once, the same errors. Its documentation SHALL start with Robot Framework's deprecation marker, name `Get Attribute Value` as the replacement, and state that the alias will be removed before 1.0. Robot Framework then logs a deprecation warning whenever the alias runs, and documentation tools show the alias as deprecated.

#### Scenario: The alias returns what Get Attribute Value returns

- **GIVEN** a window whose `@Name` is `Operations Console` and whose `@IsMaximized` is `False`
- **WHEN** `Get Attribute` and `Get Attribute Value` each read `Name` and `IsMaximized` of that window
- **THEN** both SHALL return equal values of the same type

#### Scenario: The alias checks an assertion once

- **GIVEN** a window whose `@Name` is `Operations Console`
- **WHEN** `Get Attribute` is called for that window and `Name` with the operator `==` and the expected value `Wrong Name`
- **THEN** it SHALL fail at once with the same diagnostic that `Get Attribute Value` gives for the same call

#### Scenario: Running the alias logs a deprecation warning

- **GIVEN** a suite that calls `Get Attribute`
- **WHEN** the suite runs
- **THEN** Robot Framework SHALL log a warning that the keyword is deprecated and that names `Get Attribute Value` as the replacement

#### Scenario: The keyword documentation marks the alias as deprecated

- **GIVEN** the library's keyword documentation, as Robot Framework's libdoc builds it
- **WHEN** the entry for `Get Attribute` is read
- **THEN** it SHALL be marked as deprecated, and its short documentation SHALL name `Get Attribute Value` and the removal before 1.0
