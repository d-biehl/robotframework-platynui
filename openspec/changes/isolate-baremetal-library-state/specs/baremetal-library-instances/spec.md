# BareMetal Library Instances

## ADDED Requirements

### Requirement: Scoped library state is private to each library instance

State that `Set Root` and `Set Query Settings` store in Robot Framework variables SHALL be private to the library import that set it. The library SHALL derive the variable name from the name the calling instance is registered under in the current Robot Framework namespace: the default (unaliased) import SHALL keep the documented names `${PLATYNUI_ROOT_DESCRIPTOR}` and `${PLATYNUI_QUERY_SETTINGS}`; any other registered name SHALL yield those names with a `_<NAME>` suffix, where `<NAME>` is the registered name uppercased with every character outside `[A-Z0-9_]` replaced by `_`. When there is no execution context to resolve a name from, the unsuffixed names SHALL be used.

#### Scenario: A root set by one import does not reach another

- **WHEN** a suite imports the library twice under different aliases and one alias runs `Set Root` at any scope
- **THEN** a relative selector evaluated through the other alias SHALL resolve against the desktop, not against the first alias's root

#### Scenario: Query settings do not reach another import

- **WHEN** one aliased import runs `Set Query Settings {'timeout': 60} scope=SUITE`
- **THEN** the effective timeout of the other import SHALL remain its own import-time default

#### Scenario: The default import keeps the documented variable name

- **WHEN** the library is imported without an alias and `Set Root` is called
- **THEN** the root SHALL be stored in `${PLATYNUI_ROOT_DESCRIPTOR}` and the settings in `${PLATYNUI_QUERY_SETTINGS}`

#### Scenario: An aliased import uses a normalized suffix

- **WHEN** the library is imported `AS BM` and `Set Root` is called
- **THEN** the root SHALL be stored in `${PLATYNUI_ROOT_DESCRIPTOR_BM}`

### Requirement: A suite-scoped value applies to the whole suite tree

`Set Root` and `Set Query Settings` with `scope=SUITE` SHALL apply to the suite that set them *and to the suites below it*, so that a directory's `__init__.robot` can pin the context once for every suite it contains. Because the library is suite-scoped, Robot Framework creates a new instance per suite; the variable name SHALL therefore depend only on the registered library name, never on the instance, and the inheriting suite SHALL resolve the value on its own runtime.

#### Scenario: A suite-scoped root is inherited by a child suite

- **WHEN** a parent suite's setup runs `Set Root … scope=SUITE` and a child suite that imports the library under the same name runs a relative selector
- **THEN** the selector SHALL resolve against the inherited root

#### Scenario: The inherited root resolves on the inheriting instance's runtime

- **WHEN** a child suite resolves a root inherited from its parent suite
- **THEN** the evaluation SHALL run on the child suite's own runtime, and the resulting node SHALL belong to that runtime

#### Scenario: Suite-scoped query settings are inherited by a child suite

- **WHEN** a parent suite sets `Set Query Settings` at `SUITE` scope and a child suite that imports the library under the same name performs a lookup
- **THEN** the lookup SHALL use the inherited settings rather than the library-import defaults

### Requirement: State from a differently configured import is rejected

A stored value SHALL carry a fingerprint of the import arguments of the instance that wrote it. When an instance reads a value whose fingerprint differs from its own, it SHALL NOT resolve that value; it SHALL behave as if no value were set at that scope and SHALL emit a warning naming the variable.

#### Scenario: Differently configured child suite ignores the inherited root

- **WHEN** a parent suite imports the library with `use_mock=${True}`, sets a suite-scoped root, and a child suite imports the library under the same name without `use_mock`
- **THEN** the child suite SHALL resolve relative selectors against the desktop and SHALL log a warning about the ignored inherited root

#### Scenario: Identical configuration is not a mismatch

- **WHEN** parent and child suite import the library under the same name with identical arguments
- **THEN** the inherited root SHALL be used without a warning
