# Spec Delta

## ADDED Requirements

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
