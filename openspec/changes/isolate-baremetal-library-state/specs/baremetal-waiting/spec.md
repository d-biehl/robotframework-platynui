# BareMetal Waiting — Delta

## MODIFIED Requirements

### Requirement: Wait Until Gone waits for an element to disappear

The library SHALL provide a `Wait Until Gone` keyword that waits until a target is no longer present and then returns nothing. For a selector target, "gone" SHALL mean the selector resolves to an empty node-set; re-evaluation on every attempt follows from the general selector rule (a selector reference never carries a resolved node between calls), so this keyword needs no special-casing. For a captured-element target (a `UiNode` passed in), "gone" SHALL mean the element is no longer valid; a capture from another library instance SHALL raise the mismatch error rather than be reported as gone. The wait SHALL be governed by the effective query settings, configurable per call only via `query_overrides`, and the root SHALL be re-resolved per attempt, consistent with every other keyword.

#### Scenario: Selector already matches nothing

- **WHEN** `Wait Until Gone` is called with a selector that already matches nothing
- **THEN** the keyword SHALL return on the first attempt without error

#### Scenario: Selector remains present for the whole timeout

- **WHEN** `Wait Until Gone` is called with a selector that keeps matching an element for the whole timeout
- **THEN** the keyword SHALL raise `ElementStillPresentError` with a message ending in `within timeout of {timeout} seconds.`

#### Scenario: Captured element stays valid

- **WHEN** `Wait Until Gone` is called with a captured element that remains valid for the whole timeout
- **THEN** the keyword SHALL raise `ElementStillPresentError` naming the captured element

#### Scenario: Captured element becomes invalid

- **WHEN** `Wait Until Gone` is called with a captured element that is destroyed before the timeout (verified against a real accessibility provider, since the mock never invalidates nodes)
- **THEN** the keyword SHALL return once the element is no longer valid

#### Scenario: A selector target is re-evaluated even after an earlier keyword resolved it

- **WHEN** a prior keyword has resolved the same selector, and `Wait Until Gone` is then called for that selector while the element is still present
- **THEN** the keyword SHALL evaluate the selector against the live tree and still time out, rather than reporting "gone" or "present" from anything the earlier call resolved

#### Scenario: Value-producing expression is rejected

- **WHEN** `Wait Until Gone` is given a selector that produces a value rather than an element (for example `count(...)`)
- **THEN** the keyword SHALL raise `ResultTypeError` directing the user to `Wait Until Query`, rather than silently waiting until the timeout

#### Scenario: Swallowed errors do not report gone

- **WHEN** `Wait Until Gone` is called with a persistently failing selector and `ignore_exceptions` enabled
- **THEN** the keyword SHALL keep waiting and ultimately raise `ElementStillPresentError`, never reporting "gone" because of a swallowed evaluation error
