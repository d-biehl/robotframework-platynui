## ADDED Requirements

### Requirement: Walks end when siblings share an identity

When a model reports two or more siblings as the same node but tells each listed node's position among its siblings, as the runtime does for every provider, every step over children and siblings SHALL end, SHALL reach every node that follows those siblings in document order, and SHALL count positions over the listed children. A result SHALL hold no more nodes than the tree has; which equal sibling it keeps is not specified.

#### Scenario: A descendant search past equal siblings finds the node after them

- **GIVEN** the keyless tree `W:[A,B,E]` in which `A` and `B` share one runtime id
- **WHEN** `//E` and `count(//*)` are evaluated
- **THEN** the first result SHALL be `[E]`, and the count SHALL be at most `4`
- **NOTE:** Exercised at the runtime integration level with a fake provider; the real-provider case is the WinForms suite of capability `uia-runtime-id`.

#### Scenario: A child step past equal siblings ends

- **GIVEN** the same tree
- **WHEN** `/W/*`, `/W/E` and `/W/*[3]` are evaluated
- **THEN** the first result SHALL contain `E` and at most three nodes, and the others SHALL be `[E]` and `[E]`

#### Scenario: Sibling axes over equal siblings end

- **GIVEN** the same tree
- **WHEN** `/W/*[1]/following-sibling::*` and `/W/E/preceding-sibling::*` are evaluated
- **THEN** the first result SHALL contain `E`, and both results SHALL hold at most two nodes

#### Scenario: A search for a missing node over many equal siblings finds nothing

- **GIVEN** a keyless tree of one `W` with 50 children that all share one runtime id
- **WHEN** `//Edit` and `/W/*[@Name='missing']` are evaluated
- **THEN** both results SHALL be empty, and each list of children SHALL have been read at most once

### Requirement: Cancellation stops every walk

When the cancel flag is set while an evaluation walks the tree, the evaluation SHALL stop with the cancellation error before it reads the children of more than one further node. This SHALL hold for every node test, whether or not a predicate filters the walk, and whether a path, a streamed result or a function such as `count()` consumes it.

#### Scenario: count() over a wildcard walk stops

- **GIVEN** a provider that sets the cancel flag when its fifth list of children is read
- **WHEN** `count(//*)` is evaluated
- **THEN** the evaluation SHALL fail with the cancellation error, and the provider SHALL have been asked for at most six lists of children
- **NOTE:** Exercised at the runtime integration level with a counting fake provider.

#### Scenario: A walk whose node test rejects every node stops

- **GIVEN** the same provider, in a tree that holds no `Edit`
- **WHEN** the first item of `//Edit` is taken
- **THEN** it SHALL be the cancellation error, and the provider SHALL have been asked for at most six lists of children

#### Scenario: A walk over equal siblings stops

- **GIVEN** the keyless tree `W:[A,B,E]` in which `A` and `B` share one runtime id, and a cancel flag set before the evaluation starts
- **WHEN** `count(//*)` is evaluated
- **THEN** the evaluation SHALL fail with the cancellation error
