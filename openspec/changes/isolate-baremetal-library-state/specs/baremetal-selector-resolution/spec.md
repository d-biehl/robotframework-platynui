# BareMetal Selector Resolution

## ADDED Requirements

### Requirement: A selector resolves against the library that is running

A selector reference SHALL carry only selector data — the query and the root chain captured when it was created as a root binding. The library instance, the effective root, the query settings and any per-call overrides SHALL be supplied by the keyword call that resolves it. A selector reference handed to another library instance — through a variable, an argument, or a scope-inherited root — SHALL therefore be evaluated on the resolving instance's runtime, against the resolving instance's root and settings.

#### Scenario: A selector handed to another import evaluates there

- **WHEN** a selector reference obtained from one aliased import is passed as an argument to a keyword of another aliased import
- **THEN** the evaluation SHALL run on the second import's runtime, and the first import's runtime SHALL not be called

#### Scenario: An absolute selector does not resolve the root

- **WHEN** a keyword resolves an absolute selector (`/…` or `//…`) while a root is set
- **THEN** it SHALL evaluate without resolving that root, and SHALL succeed even when the root itself can no longer be resolved

#### Scenario: A relative selector still reports a failing root

- **WHEN** a keyword resolves a relative selector while the current root no longer matches anything
- **THEN** the failure SHALL name the root's selector, not the target's

#### Scenario: Per-call overrides do not outlive their call

- **WHEN** a keyword resolves a selector with `query_overrides` and a later keyword resolves the same selector string without overrides
- **THEN** the second resolution SHALL use the scoped/default settings, and no state from the first call SHALL remain reachable from the selector reference

### Requirement: Selectors are re-evaluated, captures are pinned

A selector-backed reference SHALL be re-evaluated on every use; the library SHALL NOT reuse a node resolved by an earlier call. A captured element — a `UiNode` passed into a keyword, including `Set Root ${node}` and the captured-element form of `Wait Until Gone` — SHALL be pinned to exactly that node and SHALL NOT be re-resolved.

#### Scenario: The same selector under a changed root re-resolves

- **WHEN** a keyword resolves a relative selector under one root, the root is changed to a different container that also matches that selector, and the same selector string is used again
- **THEN** the second call SHALL evaluate against the new root and act on the element inside it, not on the element resolved under the previous root

#### Scenario: A selector stops matching an element that no longer fits it

- **WHEN** the same selector is used twice under the same root and, in between, the element it matched changed so that it no longer satisfies the selector — while remaining a live, valid element (verified against a real provider; the mock tree never changes)
- **THEN** the second call SHALL report that nothing matches, rather than acting on the element it resolved earlier

#### Scenario: A selector follows the element that now fits it

- **WHEN** a selector is used after an element started satisfying it
- **THEN** the call SHALL resolve that element

#### Scenario: A captured element is not re-resolved

- **WHEN** an element is captured and the application then creates another element matching the same selector
- **THEN** a keyword given the captured element SHALL act on the captured one, or fail because it is no longer valid — never on the newly matching element

### Requirement: A captured element belongs to one runtime

A captured element SHALL record the library instance whose runtime produced it. When a keyword of a different instance is asked to resolve, act on, or evaluate against such a capture — including `Query`'s `root` argument and the activation path that brings a target's window to the front — the keyword SHALL fail with a `BareMetalError` naming the mismatch, rather than passing a foreign node to its own runtime.

#### Scenario: A captured element passed to another import fails loudly

- **WHEN** an element captured through one aliased import is passed as the target of a keyword of another aliased import
- **THEN** the keyword SHALL raise a `BareMetalError` explaining that the element belongs to a different library instance

#### Scenario: A foreign node as a query root fails loudly

- **WHEN** `Query` is called with a `root` node that was produced by another import's runtime
- **THEN** the keyword SHALL raise a `BareMetalError` rather than evaluate against it

#### Scenario: A foreign capture is never reported as gone

- **WHEN** `Wait Until Gone` is given a captured element belonging to another import
- **THEN** it SHALL raise the mismatch error and SHALL NOT report the element as gone
