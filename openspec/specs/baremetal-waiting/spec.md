# baremetal-waiting Specification

## Purpose
The explicit waiting keywords. Every BareMetal action and read keyword already waits for
its own target, and `Query` is the deliberate non-waiting snapshot for *asking about* the
UI — but nothing waited for an element and handed it back without acting, waited for one to
disappear, or waited until an arbitrary XPath result satisfied a condition. Users
approximated all three with `Sleep` or `Query` polling wrapped in `Wait Until Keyword
Succeeded`: verbose, flaky, and against the library's "state what you expect, the keyword
waits for exactly that" model.

`Wait Until Exists`, `Wait Until Gone` and `Wait Until Query` close that gap, and `Wait
Until Attribute Value` waits for one attribute of one element to reach a value — the
waiting counterpart to `Get Attribute Value`, which reads and checks a value once. All four are
governed by the effective query settings and tunable per call. Two supporting rules keep the waits
honest: a dedicated `ElementStillPresentError` so a target that never disappeared cannot be
confused with one that was never found, and Python value semantics on evaluated results —
`bool(UiNode)` reflecting node validity, an attribute result behaving like its own value —
so the truthy default and ordinary Robot comparisons mean what they read like.

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

### Requirement: Wait Until Attribute Value waits until an attribute of an element satisfies a condition

The library SHALL provide a `Wait Until Attribute Value` keyword that repeatedly reads one attribute of one element until the attribute's value satisfies a condition, and then returns that value.

**Arguments.** The keyword SHALL take the element as a selector or as a captured element, and the attribute name bare or with a namespace prefix, as `Get Attribute` does. It SHALL accept the same `assertion_operator`, `assertion_expected` and `assertion_message` arguments as `Wait Until Query`, with the same operator spellings. Without an operator it SHALL wait until the value is truthy. It SHALL reject the `then`/`evaluate` operator, which transforms the value instead of asserting on it and therefore cannot express a wait condition.

**Result.** The keyword SHALL return the attribute's value as read in the attempt that satisfied the condition, typed as `Get Attribute` returns it. It SHALL NOT return a result an operator derives from the value, such as the capture groups AssertionEngine returns for `matches`.

**Waiting.** The effective query settings (`timeout`, `retry_interval`, `ignore_exceptions`) SHALL govern the wait, configurable per call only via `query_overrides`, and one `timeout` SHALL bound the whole call, including the time until the element appears. Every attempt SHALL observe the current UI: a selector SHALL be evaluated against the live tree on each attempt, and a captured element SHALL be read afresh on each attempt while remaining the same element. An attempt in which the selector matches nothing, in which the element does not have the attribute, or in which the operator cannot yet compare the value SHALL count as not yet satisfied. With `ignore_exceptions` enabled, an attempt that raises SHALL count as not yet satisfied and SHALL never satisfy the condition.

**Failing.** When the timeout elapses, the keyword SHALL fail with an error that describes the last attempt that did not raise — the element was not found, the attribute was missing, or the condition did not hold — and states the timeout. The keyword SHALL fail at once, without waiting, on a usage error: a selector that resolves to a value or an attribute instead of an element, the `then`/`evaluate` operator, an attribute prefix that names no namespace, or a captured element from another library import. A captured element that is no longer valid SHALL also fail at once, because it can never reach the value.

#### Scenario: A value that already satisfies the condition is returned at once

- **GIVEN** a window whose `@IsMaximized` is `False`
- **WHEN** `Wait Until Attribute Value` is called with a selector for that window, the attribute `IsMaximized`, the operator `==` and the expected value `${False}`
- **THEN** it SHALL return on the first attempt with the boolean `False`, equal in value and type to what `Get Attribute` reads for the same element and attribute

#### Scenario: Without an operator a truthy value is returned at once

- **GIVEN** a window whose `@Name` is `Operations Console`
- **WHEN** `Wait Until Attribute Value` is called for that window and `Name` without an operator
- **THEN** it SHALL return `Operations Console` on the first attempt

#### Scenario: Without an operator a falsy value is waited for until the timeout

- **GIVEN** a window whose `@IsMaximized` stays `False` for the whole timeout
- **WHEN** `Wait Until Attribute Value` is called for that window and `IsMaximized` without an operator
- **THEN** it SHALL keep waiting, and then fail with a message that says the value did not become truthy, names the attribute and the last value read, and ends in `within timeout of {timeout} seconds.`

#### Scenario: A value that changes after the call started is returned as it was when the condition held

- **GIVEN** a status label whose `@Name` is `Clicks: N`
- **WHEN** the button that increments the count is clicked, and `Wait Until Attribute Value` is then called with a selector for the label, the attribute `Name`, the operator `==` and the expected value `Clicks: N+1`
- **THEN** it SHALL keep reading until the label shows the new count, and return `Clicks: N+1`
- **NOTE** Real provider only: the mock never changes a value while a keyword waits.

#### Scenario: A captured element is read afresh on every attempt

- **GIVEN** the same status label captured as an element with `Query`, showing `Clicks: N`
- **WHEN** the button is clicked, and `Wait Until Attribute Value` is then called with the captured label, the attribute `Name`, the operator `==` and the expected value `Clicks: N+1`
- **THEN** it SHALL return `Clicks: N+1`, read from the same captured element
- **NOTE** Real provider only, for the same reason.

#### Scenario: The matches operator returns the value, not the capture groups

- **GIVEN** a window whose `@Name` is `Operations Console`
- **WHEN** `Wait Until Attribute Value` is called for that window and `Name` with the operator `matches` and the pattern `(Operations) (Console)`
- **THEN** it SHALL return the string `Operations Console`, not the captured groups

#### Scenario: The validate operator is a wait condition

- **GIVEN** a window whose `@IsMaximized` stays `False` for the whole timeout
- **WHEN** `Wait Until Attribute Value` is called for that window and `IsMaximized` with the operator `validate` and the expression `value == True`
- **THEN** it SHALL keep waiting and fail at the timeout with AssertionEngine's diagnostic, rather than returning after the first attempt

#### Scenario: The transforming then/evaluate operator is rejected

- **GIVEN** a window that exists
- **WHEN** `Wait Until Attribute Value` is called for that window with the operator `then` (or `evaluate`)
- **THEN** it SHALL fail at once with an error that directs to `validate` or a comparison operator

#### Scenario: A condition that never holds fails with the assertion diagnostic

- **GIVEN** a window whose `@Name` is `Operations Console`
- **WHEN** `Wait Until Attribute Value` is called for that window and `Name` with the operator `==` and the expected value `Wrong Name`
- **THEN** it SHALL fail after the timeout with an `AssertionError` that carries AssertionEngine's actual-versus-expected diagnostic, names the attribute, and states the timeout

#### Scenario: An element that never appears fails as not found

- **GIVEN** a selector that matches nothing for the whole timeout
- **WHEN** `Wait Until Attribute Value` is called with that selector and any attribute and condition
- **THEN** it SHALL fail with `ElementNotFoundError`, naming the selector and ending in `within timeout of {timeout} seconds.`

#### Scenario: A missing attribute is waited for, and named when it never appears

- **GIVEN** a window that has no `ToggleState` attribute
- **WHEN** `Wait Until Attribute Value` is called for that window and `ToggleState` with the operator `==` and the expected value `On`
- **THEN** it SHALL keep waiting for the whole timeout instead of failing at once, and then fail with `AttributeNotFoundError` naming the attribute and the element and ending in `within timeout of {timeout} seconds.`

#### Scenario: An unknown namespace prefix fails at once

- **GIVEN** a window that exists
- **WHEN** `Wait Until Attribute Value` is called for that window with the attribute name `nosuch:Name`
- **THEN** it SHALL fail at once with an error naming the unknown prefix, without waiting for the timeout

#### Scenario: A selector that resolves to a value fails at once

- **GIVEN** a selector that resolves to a value or an attribute instead of an element, such as `count(//Window)` or `//Window/@Name`
- **WHEN** `Wait Until Attribute Value` is called with that selector
- **THEN** it SHALL fail at once with `ResultTypeError`, directing to `Wait Until Query` for value conditions

#### Scenario: A captured element that is no longer valid fails at once

- **GIVEN** a menu item captured while its menu is open, and the menu closed again so that the captured item is no longer valid
- **WHEN** `Wait Until Attribute Value` is called with the captured item and a generous timeout
- **THEN** it SHALL fail at once with the error for a pinned element that is no longer available, not with a timeout error
- **NOTE** Real provider only: the mock never invalidates a captured element.

#### Scenario: A captured element from another library import is rejected

- **GIVEN** an element captured through one import of the library
- **WHEN** a second import of the library calls `Wait Until Attribute Value` with that element
- **THEN** it SHALL fail at once with the error for an element that belongs to a different library instance

#### Scenario: A per-call timeout override is honored

- **GIVEN** a condition that never holds
- **WHEN** `Wait Until Attribute Value` is called for it with `query_overrides={'timeout': T}`
- **THEN** the wait SHALL last up to `T` seconds, and the failure SHALL report `T`

#### Scenario: Swallowed errors never satisfy the condition

- **GIVEN** a selector that fails to evaluate on every attempt, and `ignore_exceptions` enabled for the call
- **WHEN** `Wait Until Attribute Value` is called with that selector
- **THEN** it SHALL keep waiting and fail after the timeout with `ElementNotFoundError`, never returning because of a swallowed error

#### Scenario: A value that cannot be compared yet does not end the wait

- **GIVEN** a window whose `@Name` is a string
- **WHEN** `Wait Until Attribute Value` is called for that window and `Name` with the order operator `>` and the number `${5}`, which cannot be compared with a string
- **THEN** it SHALL keep waiting, and fail at the timeout with the comparison error and the timeout, rather than on the first attempt
