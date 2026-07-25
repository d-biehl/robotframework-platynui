# baremetal-library-instances Specification

## Purpose
TBD - created by syncing change isolate-baremetal-library-state. Update Purpose after archive.
## Requirements
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

### Requirement: The scope names are Robot Framework's own

`Set Root` and `Set Query Settings` SHALL accept the scope names Robot Framework's `VAR` syntax accepts, with the same meanings, and SHALL store the value through the corresponding `VariableScopes` setter: `LOCAL`, `TEST` (with `TASK` as its alias), `SUITE` (this suite only), `SUITES` (the suite and the suites below it) and `GLOBAL`. `SUITE` SHALL NOT reach the suites below it, matching Robot Framework's own suite variables; `SUITES` is the explicit opt-in that does, so that a directory's `__init__.robot` can pin the context once for every suite it contains.

#### Scenario: A suite-scoped root stops at its suite

- **WHEN** a directory's `__init__.robot` runs `Set Root … scope=SUITE` and a child suite that imports the library under the same name runs a relative selector
- **THEN** the selector SHALL resolve against the desktop, not against the parent suite's root

#### Scenario: A SUITES-scoped root reaches the suites below

- **WHEN** a directory's `__init__.robot` runs `Set Root … scope=SUITES` and a child suite that imports the library under the same name runs a relative selector
- **THEN** the selector SHALL resolve against that root, evaluated on the child suite's own runtime

#### Scenario: TASK is accepted wherever TEST is

- **WHEN** a root is set with `scope=TASK`
- **THEN** it SHALL apply exactly as `scope=TEST` does

#### Scenario: A global root applies beyond its suite

- **WHEN** a selector root is set with `scope=GLOBAL`
- **THEN** it SHALL apply to subsequent suites of the same run until it is reset

### Requirement: A root that pins an element cannot cross a suite boundary

Setting a root that pins an element at `SUITES` or `GLOBAL` SHALL fail with an error explaining that the element belongs to one runtime, and SHALL leave the previously stored root untouched. The check SHALL cover the root's whole parent chain, since a selector root may drill into a captured element. At `LOCAL`, `TEST` and `SUITE` a pinned root SHALL remain allowed, because those scopes never leave the instance that set it.

#### Scenario: A capture is refused at the cross-suite scopes

- **WHEN** an element captured from a query is passed to `Set Root` with `scope=SUITES` or `scope=GLOBAL`
- **THEN** the keyword SHALL fail, naming the element's runtime and pointing to using a selector, and no root SHALL have been stored

#### Scenario: A capture stays usable within its suite

- **WHEN** the same captured element is passed to `Set Root` with `scope=SUITE`
- **THEN** relative selectors in that suite SHALL resolve against it
