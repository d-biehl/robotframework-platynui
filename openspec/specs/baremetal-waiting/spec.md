# baremetal-waiting Specification

## Purpose
TBD - created by archiving change add-baremetal-wait-keywords. Update Purpose after archive.
## Requirements
### Requirement: Wait Until Exists waits for an element and returns it

The library SHALL provide a `Wait Until Exists` keyword that repeatedly evaluates a selector against the live UI tree until it resolves to a single element, then returns that element. The wait SHALL be governed by the effective query settings (`timeout`, `retry_interval`, `ignore_exceptions`), configurable per call only via the `query_overrides` argument. The keyword SHALL be element-only: a selector that resolves to a value or an attribute rather than an element SHALL fail loudly rather than wait until timeout.

#### Scenario: Element appears within the timeout

- **WHEN** `Wait Until Exists` is called with a selector that matches an element within the timeout
- **THEN** the keyword SHALL return that element as a `UiNode` handle

#### Scenario: Element never appears

- **WHEN** `Wait Until Exists` is called with a selector that matches nothing for the whole timeout
- **THEN** the keyword SHALL raise `ElementNotFoundError` with a user-facing message that names the selector and ends in `within timeout of {timeout} seconds.`

#### Scenario: Per-call timeout override is honored

- **WHEN** `Wait Until Exists` is called with `query_overrides={'timeout': T}`
- **THEN** the wait SHALL last up to `T` seconds and the timeout error SHALL report `T`

#### Scenario: Selector resolving to a non-element fails loudly

- **WHEN** `Wait Until Exists` is given a selector that resolves to a value or attribute (for example `count(...)` or `.../@Name`)
- **THEN** the keyword SHALL raise `ResultTypeError` rather than waiting until the timeout

#### Scenario: Per-call override does not leak across the shared descriptor cache

- **WHEN** `Wait Until Exists` is called for a selector with a `query_overrides` timeout, and is then called again for the same selector without overrides
- **THEN** the second call SHALL use the scoped/default timeout, not the previous override

### Requirement: Wait Until Gone waits for an element to disappear

The library SHALL provide a `Wait Until Gone` keyword that waits until a target is no longer present and then returns nothing. For a selector target, "gone" SHALL mean the selector resolves to an empty node-set; the keyword SHALL re-evaluate the selector fresh on every attempt and SHALL NOT trust any node cached on the shared descriptor. For a captured-element target (a `UiNode` passed in), "gone" SHALL mean the element is no longer valid. The wait SHALL be governed by the effective query settings, configurable per call only via `query_overrides`, and the root SHALL be re-resolved per attempt, consistent with every other keyword.

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

#### Scenario: A stale cached descriptor node is ignored

- **WHEN** a prior keyword has cached a resolved node for the same selector, and `Wait Until Gone` is then called for that selector while the element is still present
- **THEN** the keyword SHALL re-evaluate fresh and still time out, rather than reporting "gone" from the stale cache

#### Scenario: Value-producing expression is rejected

- **WHEN** `Wait Until Gone` is given a selector that produces a value rather than an element (for example `count(...)`)
- **THEN** the keyword SHALL raise `ResultTypeError` directing the user to `Wait Until Query`, rather than silently waiting until the timeout

#### Scenario: Swallowed errors do not report gone

- **WHEN** `Wait Until Gone` is called with a persistently failing selector and `ignore_exceptions` enabled
- **THEN** the keyword SHALL keep waiting and ultimately raise `ElementStillPresentError`, never reporting "gone" because of a swallowed evaluation error

### Requirement: Wait Until Query waits until an XPath result satisfies an assertion

The library SHALL provide a `Wait Until Query` keyword that repeatedly evaluates an XPath expression against the live UI tree until its result satisfies a condition, then returns the satisfying result. It SHALL accept the same `assertion_operator`, `assertion_expected`, and `assertion_message` arguments as `Get Attribute`, with the same operator spellings, by declaring those parameters on its own signature and calling AssertionEngine's `verify_assertion` inside its retry loop; it SHALL NOT use the `@assertable` decorator. When no operator is given, the keyword SHALL wait until the result is truthy. When an operator is given, the value tested by `verify_assertion` SHALL be the meaningful value of the result (the typed value of an attribute, the element, or the native value). The wait SHALL be governed by the effective query settings, configurable per call only via `query_overrides`.

#### Scenario: Default waits for a truthy value

- **WHEN** `Wait Until Query` is called without an operator on an expression that yields a falsy value (for example `count(...)` returning 0, or an attribute whose value is `False`)
- **THEN** the keyword SHALL keep waiting and time out, and once the value becomes truthy SHALL return it

#### Scenario: Default returns the same kind of result as Query

- **WHEN** `Wait Until Query` succeeds without an operator
- **THEN** it SHALL return the raw evaluation result for the expression — an `EvaluatedAttribute` for an attribute step, a `UiNode` for an element expression, or a native value for a computed expression — matching what `Query` returns for the same expression

#### Scenario: Comparison operator is satisfied

- **WHEN** `Wait Until Query` is called with an operator and an expected value that the result eventually satisfies
- **THEN** the keyword SHALL return the value returned by `verify_assertion`

#### Scenario: Comparison operator times out with the engine's diagnostic

- **WHEN** `Wait Until Query` is called with an operator that the result never satisfies
- **THEN** the keyword SHALL raise an `AssertionError` carrying AssertionEngine's actual-vs-expected diagnostic together with the timeout context

#### Scenario: Order or regex operators survive pre-appearance attempts

- **WHEN** `Wait Until Query` uses an order or regex operator while early results are missing or of an incomparable type, causing `verify_assertion` to raise `TypeError`
- **THEN** the keyword SHALL treat those attempts as not-yet-satisfied and keep polling, surfacing the real error only on timeout, rather than failing on the first attempt

#### Scenario: Expected without an operator does not raise the mandatory-operator error

- **WHEN** `Wait Until Query` is called with `assertion_expected` set but no operator
- **THEN** the keyword SHALL follow the truthiness path and SHALL NOT route through `verify_assertion`, so the "assertion operator is mandatory" `ValueError` never fires

#### Scenario: The transforming then/evaluate operator is rejected

- **WHEN** `Wait Until Query` is called with the `then` (or `evaluate`) operator
- **THEN** the keyword SHALL raise a clear error directing the user to `validate` or a comparison operator, because `then` transforms rather than asserts and cannot express a wait condition

#### Scenario: The validate operator polls correctly

- **WHEN** `Wait Until Query` is called with the `validate` operator and a boolean expression that is initially false and later true
- **THEN** the keyword SHALL keep waiting while the expression is false and return once it is true

### Requirement: Wait Until Gone exposes a dedicated still-present error

The library SHALL provide a public `ElementStillPresentError` exception, a subclass of `BareMetalError`, raised by `Wait Until Gone` when its target is still present or valid after the timeout. Its message SHALL end in `within timeout of {timeout} seconds.` to remain consistent with the existing timeout-error convention.

#### Scenario: Still-present timeout raises the dedicated error

- **WHEN** `Wait Until Gone` times out with the target still present
- **THEN** it SHALL raise `ElementStillPresentError`, distinct from `ElementNotFoundError`

### Requirement: Evaluated query results expose Python value semantics

The native binding SHALL give evaluated query results truthiness and equality that reflect their meaning, so the wait keywords' truthy default and ordinary Robot Framework comparisons behave intuitively. `bool(UiNode)` SHALL reflect the node's validity. An `EvaluatedAttribute` SHALL behave like its underlying value for truthiness, equality, string conversion, and hashing.

#### Scenario: A UiNode is truthy when valid

- **WHEN** `bool()` is taken of a `UiNode`
- **THEN** the result SHALL be the node's `is_valid()` state, not unconditionally `True`

#### Scenario: An attribute result reflects its value

- **WHEN** an `EvaluatedAttribute` whose value is `False`, `0`, or empty is tested for truthiness, and one whose value equals an expected value is compared with `==`
- **THEN** truthiness SHALL reflect the value (falsy), and the equality SHALL hold, rather than being unconditionally `True`/`False`
