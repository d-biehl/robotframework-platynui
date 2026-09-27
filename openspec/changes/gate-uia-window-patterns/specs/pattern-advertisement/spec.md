# Spec Delta

## Purpose

What a node's advertised patterns promise about the pattern instances it hands out: for every provider, an action pattern is advertised exactly when the node serves an instance of it. It also covers the contract testkit check that proves this for each provider.

## ADDED Requirements

### Requirement: Advertised action patterns and served instances agree

For every node of every provider, `pattern_by_name` SHALL return an instance of an action pattern of the core vocabulary (Focusable, Activatable, Minimizable, Maximizable, Restorable, Closeable, Movable, Resizable, Responsive) exactly when the node's `supported_patterns()` lists that pattern. A capability marker (TextEditable) SHALL never have an instance. Patterns that only promise attributes (for example Element, TextContent, ActivationTarget, Application) SHALL be neither required nor forbidden to have an instance.

#### Scenario: The mock provider conforms

- **GIVEN** every node of the mock tree
- **WHEN** the contract testkit's pattern-instance check runs on each
- **THEN** it SHALL report no issue
- **NOTE:** Mock, in CI with `just test`.

#### Scenario: The UI Automation provider conforms

- **GIVEN** the test window of the UI Automation tests (a child process shows it off screen, with three standard buttons) and every node listed below it
- **WHEN** the check runs on each
- **THEN** it SHALL report no issue
- **NOTE:** Real provider only, `just test` on a Windows host. Fails before this change on every button.

#### Scenario: The Java backends conform

- **GIVEN** every node of the Swing fixture, served once by the Java Access Bridge backend and once by the Java agent backend
- **WHEN** the check runs on each
- **THEN** it SHALL report no issue
- **NOTE:** Real provider only: the ignored live tests that `just test-acceptance-windows` runs.

### Requirement: The contract testkit checks pattern instances

The core contract testkit SHALL provide a reusable check of a single node that reports every action pattern whose advertisement and instance disagree, in either direction, and every capability marker that has an instance. It SHALL exempt attribute-only patterns, and it SHALL NOT run any pattern action. The action patterns it checks SHALL come from one list in the core vocabulary, so that a new action pattern is checked without changing the testkit.

#### Scenario: A conforming node passes

- **GIVEN** a node that advertises Focusable and Activatable with an instance for each, and Element without an instance
- **WHEN** the check runs
- **THEN** it SHALL report no issue

#### Scenario: An instance that is not advertised is reported

- **GIVEN** a node whose `pattern_by_name` returns an Activatable instance while its `supported_patterns()` does not list Activatable
- **WHEN** the check runs
- **THEN** it SHALL report exactly one issue, naming Activatable as served but not advertised

#### Scenario: An advertised pattern without an instance is reported

- **GIVEN** a node that lists Minimizable but whose `pattern_by_name(Minimizable)` returns nothing
- **WHEN** the check runs
- **THEN** it SHALL report an issue naming Minimizable as advertised without an instance

#### Scenario: A capability marker with an instance is reported

- **GIVEN** a node that lists TextEditable and returns an instance for it
- **WHEN** the check runs
- **THEN** it SHALL report an issue naming TextEditable

#### Scenario: Attribute-only patterns are exempt

- **GIVEN** a node that lists Element, TextContent and ActivationTarget without instances
- **WHEN** the check runs
- **THEN** it SHALL report no issue

#### Scenario: The check runs no action

- **GIVEN** a node whose pattern actions record every call
- **WHEN** the check runs
- **THEN** no action SHALL have been called
