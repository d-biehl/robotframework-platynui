# baremetal-selector-resolution Specification

## Purpose
TBD - created by syncing change isolate-baremetal-library-state. Update Purpose after archive.
## Requirements
### Requirement: A selector resolves against the library that is running

A selector reference SHALL carry only selector data — the query and the root chain captured when it was created as a root binding. The library instance, the effective root, the query settings and any per-call overrides SHALL be supplied by the keyword call that resolves it. A selector reference handed to another library instance — through a variable or an argument — SHALL therefore be evaluated on the resolving instance's runtime, against the resolving instance's root and settings.

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

A scoped root is the one exception: the element a root binding resolved to SHALL be reused while it still reports itself valid *and* still belongs to the resolving instance's runtime, and SHALL be resolved again as soon as either stops holding. A root is looked up once per keyword in addition to that keyword's own target — repetition the suite did not write — and it names a container pinned on purpose, whereas re-evaluating a target selector is the observation the keyword exists to make. When a captured element is no longer valid and the reference holds no selector, the failure SHALL name the element and say that there is nothing to look up again.

#### Scenario: A scoped root is looked up once

- **WHEN** several keywords run against the same scoped root
- **THEN** the root SHALL be resolved on the first of them and reused by the others

#### Scenario: A root whose element died is looked up again

- **WHEN** the element a scoped root resolved to stops reporting itself valid, e.g. because its window closed
- **THEN** the next lookup SHALL resolve the root's selector again rather than reuse that element

#### Scenario: A root reused by another import is resolved there

- **WHEN** a selector root binding that one import has already resolved is set as another import's root
- **THEN** the second import SHALL resolve the selector on its own runtime instead of reusing the first import's element

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

A captured element SHALL record the library instance whose runtime produced it. When a keyword of a different instance is asked to resolve, act on, or evaluate against such a capture — including `Query`'s `root` argument and the activation path that brings a target's window to the front — the keyword SHALL fail with a `BareMetalError` naming the mismatch, rather than passing a foreign node to its own runtime. `Set Root` SHALL apply this check to the whole root chain and to every form its argument takes, including a root binding this keyword returned earlier, so a foreign element is refused when the root is set rather than when it is next read.

#### Scenario: A captured element passed to another import fails loudly

- **WHEN** an element captured through one aliased import is passed as the target of a keyword of another aliased import
- **THEN** the keyword SHALL raise a `BareMetalError` explaining that the element belongs to a different library instance

#### Scenario: A foreign node as a query root fails loudly

- **WHEN** `Query` is called with a `root` node that was produced by another import's runtime
- **THEN** the keyword SHALL raise a `BareMetalError` rather than evaluate against it

#### Scenario: A foreign capture is never reported as gone

- **WHEN** `Wait Until Gone` is given a captured element belonging to another import
- **THEN** it SHALL raise the mismatch error and SHALL NOT report the element as gone

#### Scenario: A restored root pinning a foreign element fails when it is set

- **WHEN** the root value returned by `Set Root` of one import — a root that pins an element — is passed to `Set Root` of another import
- **THEN** that call SHALL raise the mismatch error and SHALL NOT store the root, at any scope
