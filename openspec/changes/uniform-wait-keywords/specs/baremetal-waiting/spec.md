# Spec Delta

## ADDED Requirements

### Requirement: Every wait polls and fails the same way

The library SHALL wait the same way wherever it waits:

- in `Wait Until Exists`, `Wait Until Gone`, `Wait Until Query` and `Wait Until Attribute Value`;
- in every keyword that takes an element and waits for it before it acts on it or reads it, such as the pointer keywords and `Get Attribute Value`;
- in the lookup of a root set with `Set Root`.

**Attempts.** A wait SHALL make attempts, pausing `retry_interval` between them, until an attempt satisfies it or `timeout` has elapsed. The effective query settings SHALL govern the wait. A keyword's `query_overrides` SHALL govern only the keyword's own target, not the lookup of the root. A wait SHALL make at least one attempt, and it SHALL compare the elapsed time with `timeout` only after an attempt. The attempt after which the timeout has elapsed SHALL be the last one. The wait SHALL NOT evaluate anything after it, and its failure SHALL describe that attempt.

**The root.** A relative selector or expression needs the root; an absolute one never does. An expression is relative when it reads its context anywhere outside a predicate and outside the later steps of a path: through a relative path, the context item, or a standard function that reads the context when an argument is left out. So `count(.//x)` is relative and `count(//x)` is absolute. When a target needs the root, every attempt SHALL look the root up first. The lookup SHALL reuse the element the root resolved to for as long as that element is still valid. The lookup SHALL apply the root's own settings to its own attempts. The waiting target SHALL NOT swallow a failure of that lookup, whatever `ignore_exceptions` says: when the lookup fails, the wait SHALL fail at once with the root's error. The wait's `timeout` SHALL start when the lookup before its first attempt has found the root: finding the root takes the root's own timeout. A lookup during the wait, when the root has to be found again, SHALL count as part of its attempt.

- A root selector that matches nothing within its timeout SHALL fail with `RootNotFoundError`, naming the root, its timeout and the target that was not evaluated.
- A root that pins an element which is no longer valid SHALL fail with the error for a pinned element that is no longer available.
- Any other error of the root's lookup SHALL surface unchanged.

When the element that a root selector resolved to stops being valid during a wait, the next attempt SHALL look the root up again, and the wait SHALL continue against the element that lookup finds.

**Errors that waiting cannot fix.** When an attempt raises one of these errors, the wait SHALL end at once, whatever `ignore_exceptions` says:

- `ResultTypeError`: a selector that yields a value or an attribute where an element is needed;
- `ForeignNodeError`: an element from another library import;
- `PinnedElementGoneError`: a captured element that is no longer valid where the wait needs it;
- `NoQueryError`: an element reference that holds neither a selector nor an element;
- `RootNotFoundError`: a root that cannot be found;
- `RuntimeError`, which AssertionEngine raises for an operator it does not know, and Robot Framework for a `validate` expression that cannot be evaluated.

For `Wait Until Gone`, a captured target that is no longer valid is not an error: it is what the keyword waits for.

**Other errors.** Without `ignore_exceptions`, any other error an attempt raises SHALL fail the wait at once. With `ignore_exceptions`, such an error SHALL count as an attempt that did not satisfy the wait, and the wait SHALL keep it as its last error. An attempt that completes without raising SHALL discard the last error. A swallowed error SHALL never satisfy a wait.

**The failure.** When the timeout has elapsed, the wait SHALL fail with an error that describes its last attempt. Its message SHALL state the timeout as `within timeout of {timeout} seconds`.

- When the last attempt completed, the message SHALL say what that attempt saw.
- When the last attempt raised, the message SHALL say how far that attempt got before the error, and SHALL NOT claim anything that an earlier attempt saw. It SHALL end with `The last error was: ` and the error, as Robot Framework reports a failure: the error's type, a colon and its message, with the type left out where Robot Framework leaves it out.

The error's type SHALL be the one the keyword raises for what it was waiting for and for how far its last attempt got. It SHALL NOT depend on the type of a swallowed error.

#### Scenario: A swallowed error is named when Wait Until Exists times out

- **GIVEN** the selector `//control:Window[broken`, which fails to evaluate, and `ignore_exceptions` enabled for the call
- **WHEN** `Wait Until Exists` is called with that selector
- **THEN** it SHALL keep waiting until the timeout
- **AND** it SHALL then fail with `ElementNotFoundError`, whose message names the selector, states `within timeout of {timeout} seconds.` and ends with `The last error was: EvaluationError: ` followed by the evaluation error's message

#### Scenario: The wait of a keyword that reads an element names the swallowed error

- **GIVEN** the selector `//control:Window[broken`, and `ignore_exceptions` enabled for the call
- **WHEN** `Get Attribute Value` is called with that selector and the attribute `Name`
- **THEN** it SHALL fail after the timeout with `ElementNotFoundError`, whose message ends with `The last error was: EvaluationError: ` followed by the evaluation error's message

#### Scenario: An attempt that completes discards the remembered error

- **GIVEN** a selector whose evaluation raises on the first attempt and matches nothing on every later attempt, and `ignore_exceptions` enabled
- **WHEN** `Wait Until Exists` is called with that selector
- **THEN** it SHALL fail after the timeout with `ElementNotFoundError`, whose message ends in `within timeout of {timeout} seconds.` and names no last error
- **NOTE** Verified with a unit test on a fake runtime: the mock tree never changes between attempts.

#### Scenario: A value selector fails at once even when errors are ignored

- **GIVEN** the selector `count(//control:Window)`, which yields a number, and `ignore_exceptions` enabled for the call
- **WHEN** `Wait Until Exists` is called with that selector
- **THEN** it SHALL fail on the first attempt with `ResultTypeError`, not with a timeout error

#### Scenario: A root that matches nothing ends Wait Until Gone even when errors are ignored

- **GIVEN** a root set with `Set Root` that matches nothing, and effective query settings with a timeout of 0.2 seconds
- **WHEN** `Wait Until Gone` is called with a relative selector and `query_overrides={'timeout': 1, 'ignore_exceptions': True}`
- **THEN** it SHALL fail with `RootNotFoundError`, which names the root and the selector and states `within timeout of 0.2 seconds`
- **AND** it SHALL NOT fail with `ElementStillPresentError`

#### Scenario: A root that matches nothing ends Wait Until Attribute Value even when errors are ignored

- **GIVEN** a root set with `Set Root` that matches nothing, and effective query settings with a timeout of 0.2 seconds
- **WHEN** `Wait Until Attribute Value` is called with a relative selector, the attribute `Name` and `query_overrides={'timeout': 1, 'ignore_exceptions': True}`
- **THEN** it SHALL fail with `RootNotFoundError`, which names the root and the selector
- **AND** it SHALL NOT fail with `ElementNotFoundError` for the selector

#### Scenario: Wait Until Query names the expression a missing root kept from being evaluated

- **GIVEN** a root set with `Set Root` that matches nothing
- **WHEN** `Wait Until Query` is called with the relative expression `count(.//control:Button)`
- **THEN** it SHALL fail with `RootNotFoundError`, which names the root and says that `count(.//control:Button)` was not evaluated

#### Scenario: An absolute expression does not need the root

- **GIVEN** a root set with `Set Root` that matches nothing
- **WHEN** `Wait Until Query` is called with the absolute expression `count(//control:Window)`, the operator `>` and the expected value `${0}`
- **THEN** it SHALL return the count, without looking the root up

#### Scenario: An expression that computes a value is evaluated against the root

- **GIVEN** a root set with `Set Root` to a window that holds 4 of the desktop's 8 list items
- **WHEN** `Wait Until Query` is called with `count(.//item:ListItem)`, whose relative path is a function's argument
- **THEN** it SHALL return 4, the count inside the root

#### Scenario: A root that is replaced during the wait is followed

- **GIVEN** a root whose element stops being valid after the first attempt, while the root's selector then matches a new element that holds the target
- **WHEN** `Wait Until Exists` waits for a relative selector under that root
- **THEN** the next attempt SHALL look the root up again, and the keyword SHALL return the target found under the new element
- **NOTE** Verified with a unit test on a fake runtime: the mock provider never invalidates an element.

#### Scenario: A root that goes away for good ends the wait with the root's error

- **GIVEN** a root `/app:Application[@ProcessId=${pid}]` that has resolved while the application ran, a timeout of 2 seconds in the effective query settings, and an application that has ended since
- **WHEN** `Wait Until Gone` is called with a relative selector under that root and `query_overrides={'timeout': 10, 'ignore_exceptions': True}`
- **THEN** it SHALL fail with `RootNotFoundError`, which names the root
- **AND** it SHALL NOT fail with `ElementStillPresentError`
- **NOTE** Real provider only, on every acceptance lane: the mock never invalidates an element. A unit test covers the same logic on a fake runtime.

#### Scenario: A pinned root that stops being valid during the wait fails at once

- **GIVEN** a root set with `Set Root` to a captured element, which stops being valid after the first attempt
- **WHEN** `Wait Until Exists` waits for a relative selector under that root, with `ignore_exceptions` enabled and a long timeout
- **THEN** the next attempt SHALL fail at once with the error for a pinned element that is no longer available
- **NOTE** Verified with a unit test on a fake runtime: the mock never invalidates an element.

#### Scenario: A root that is found late leaves the target its whole timeout

- **GIVEN** a root that is found only after 0.5 seconds, within its own timeout of 2 seconds, and a target under it that appears 0.1 seconds after the root
- **WHEN** `Wait Until Exists` waits for a relative selector for that target with `query_overrides={'timeout': 0.3}`
- **THEN** it SHALL return the target
- **NOTE** Verified with a unit test on a fake runtime.

#### Scenario: Nothing is evaluated after the deadline

- **GIVEN** an expression whose first evaluation takes longer than the timeout
- **WHEN** `Wait Until Query` is called with that expression and an operator that the result does not satisfy
- **THEN** the expression SHALL be evaluated exactly once, and the failure SHALL describe that evaluation
- **NOTE** Verified with a unit test on a fake runtime that counts evaluations.

## MODIFIED Requirements

### Requirement: Wait Until Gone waits for an element to disappear

The library SHALL provide a `Wait Until Gone` keyword that waits until a target is no longer present and then returns nothing. For a selector target, "gone" SHALL mean the selector resolves to an empty node-set. Re-evaluation on every attempt follows from the general selector rule (a selector reference never carries a resolved node between calls), so this keyword needs no special-casing. For a captured-element target (a `UiNode` passed in), "gone" SHALL mean the element is no longer valid. A capture from another library instance SHALL raise the mismatch error rather than be reported as gone. The effective query settings SHALL govern the wait, configurable per call only via `query_overrides`. The keyword SHALL look the root up on every attempt, as every wait does (*Every wait polls and fails the same way*). A root that cannot be found SHALL end the wait with the root's error, also with `ignore_exceptions` enabled. It SHALL never be reported as the target being gone or still present.

#### Scenario: Selector already matches nothing

- **WHEN** `Wait Until Gone` is called with a selector that already matches nothing
- **THEN** the keyword SHALL return on the first attempt without error

#### Scenario: Selector remains present for the whole timeout

- **WHEN** `Wait Until Gone` is called with a selector that keeps matching an element for the whole timeout
- **THEN** the keyword SHALL raise `ElementStillPresentError` with a message ending in `within timeout of {timeout} seconds.`

#### Scenario: Captured element stays valid

- **WHEN** `Wait Until Gone` is called with a captured element that remains valid for the whole timeout
- **THEN** the keyword SHALL raise `ElementStillPresentError` that names the captured element in the one form in which PlatynUI describes elements: its role, its name in double quotes, and `#` with its id when it has one, for example `Window "Operations Console"`

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

- **GIVEN** the selector `//control:Window[broken`, which fails to evaluate on every attempt, and `ignore_exceptions` enabled for the call
- **WHEN** `Wait Until Gone` is called with that selector
- **THEN** it SHALL keep waiting until the timeout, and then raise `ElementStillPresentError`
- **AND** the error's message SHALL say that the element could not be confirmed gone, state `within timeout of {timeout} seconds.` and end with `The last error was: EvaluationError: ` followed by the evaluation error's message
- **AND** the keyword SHALL NOT report the target as gone, and its message SHALL NOT say the target was still present

### Requirement: Wait Until Query waits until an XPath result satisfies an assertion

The library SHALL provide a `Wait Until Query` keyword that repeatedly evaluates an XPath expression against the live UI tree until its result satisfies a condition, then returns the satisfying result. It SHALL accept the same `assertion_operator`, `assertion_expected`, and `assertion_message` arguments as `Get Attribute Value`, with the same operator spellings, by declaring those parameters on its own signature and calling AssertionEngine's `verify_assertion` inside its retry loop; it SHALL NOT use the `@assertable` decorator. When no operator is given, the keyword SHALL wait until the result is truthy. When an operator is given, the value tested by `verify_assertion` SHALL be the meaningful value of the result (the typed value of an attribute, the element, or the native value). The effective query settings SHALL govern the wait, configurable per call only via `query_overrides`, and the keyword SHALL poll as every wait does (*Every wait polls and fails the same way*).

**Context.** When a `root` element is given, the keyword SHALL evaluate the expression against it. A `root` element from another library import SHALL fail before the first attempt, and a `root` element that is no longer valid SHALL fail at once. Without a `root`, the keyword SHALL evaluate a relative expression against the `Set Root` root, and an absolute expression without looking that root up.

**Failing.** When the timeout elapses, the error SHALL describe the last attempt:

- without an operator, a `ResultTypeError` that names the expression and the last result (the attribute's value for an attribute step), or says that the expression matched nothing;
- with an operator, the `AssertionError` with AssertionEngine's actual-versus-expected diagnostic, or the comparison error of the last attempt, each with the timeout added.

When the last attempt raised, the error SHALL say how far that attempt got, and SHALL end with the last error:

- when the evaluation raised: that the expression did not become truthy (a `ResultTypeError`), or did not satisfy the assertion (an `AssertionError`);
- when the expression was evaluated and only the check raised: the result, and that it could not be checked (an `AssertionError`).

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
- **THEN** the keyword SHALL treat those attempts as not-yet-satisfied and keep polling, rather than failing on the first attempt
- **AND** it SHALL surface the comparison error only at the timeout, as a `TypeError` that names the expression and states the timeout

#### Scenario: Expected without an operator does not raise the mandatory-operator error

- **WHEN** `Wait Until Query` is called with `assertion_expected` set but no operator
- **THEN** the keyword SHALL follow the truthiness path and SHALL NOT route through `verify_assertion`, so the "assertion operator is mandatory" `ValueError` never fires

#### Scenario: The transforming then/evaluate operator is rejected

- **WHEN** `Wait Until Query` is called with the `then` (or `evaluate`) operator
- **THEN** the keyword SHALL raise a clear error directing the user to `validate` or a comparison operator, because `then` transforms rather than asserts and cannot express a wait condition

#### Scenario: The validate operator polls correctly

- **WHEN** `Wait Until Query` is called with the `validate` operator and a boolean expression that is initially false and later true
- **THEN** the keyword SHALL keep waiting while the expression is false and return once it is true

#### Scenario: The truthy default names a falsy result

- **GIVEN** the expression `count(//control:Button[@Name="NoSuchButton"])`, which yields 0 for the whole timeout
- **WHEN** `Wait Until Query` is called with that expression without an operator
- **THEN** it SHALL fail with `ResultTypeError`, whose message names the expression and ends in `was 0 and did not become truthy within timeout of {timeout} seconds.`

#### Scenario: The truthy default names the value of a falsy attribute

- **GIVEN** a window whose `@IsMaximized` stays `False` for the whole timeout
- **WHEN** `Wait Until Query` is called with that window's `…/@IsMaximized` step without an operator
- **THEN** it SHALL fail with a message that ends in `was False and did not become truthy within timeout of {timeout} seconds.`: the attribute's value, not a representation of the attribute result

#### Scenario: The truthy default reports an expression that matched nothing

- **GIVEN** the expression `//control:Button[@Name="NoSuchButton"]/@Name`, whose element does not exist
- **WHEN** `Wait Until Query` is called with that expression without an operator
- **THEN** it SHALL fail with a message that ends in `matched nothing and did not become truthy within timeout of {timeout} seconds.`

#### Scenario: A truthy wait names the last error

- **GIVEN** the expression `count(//control:Window[broken`, which fails to evaluate on every attempt, and `ignore_exceptions` enabled for the call
- **WHEN** `Wait Until Query` is called with that expression without an operator
- **THEN** it SHALL fail after the timeout with `ResultTypeError`
- **AND** the error's message SHALL say that the expression did not become truthy, state `within timeout of {timeout} seconds.` and end with `The last error was: EvaluationError: ` followed by the evaluation error's message

#### Scenario: An operator wait names the last error instead of letting it escape

- **GIVEN** the expression `count(//control:Window[broken`, which fails to evaluate on every attempt, and `ignore_exceptions` enabled for the call
- **WHEN** `Wait Until Query` is called with that expression, the operator `>` and the expected value `${0}`
- **THEN** it SHALL fail after the timeout with `AssertionError`
- **AND** the error's message SHALL say that the expression did not satisfy the assertion, state `within timeout of {timeout} seconds.` and end with `The last error was: EvaluationError: ` followed by the evaluation error's message
- **AND** the keyword SHALL NOT raise the evaluation error itself

#### Scenario: A check that raises is reported with the result it checked

- **GIVEN** a window whose `@Name` is `Operations Console`, and `ignore_exceptions` enabled for the call
- **WHEN** `Wait Until Query` is called with that window's `…/@Name` step, the operator `matches` and the pattern `(`, which is not a valid regular expression
- **THEN** it SHALL fail after the timeout with an `AssertionError`
- **AND** the error's message SHALL say that the result was `'Operations Console'` and could not be checked, state `within timeout of {timeout} seconds.` and end with the last error, which says what is wrong with the pattern

#### Scenario: A validate expression that cannot be evaluated ends the wait at once

- **GIVEN** a window whose `@Name` is `Operations Console`, and `ignore_exceptions` enabled for the call
- **WHEN** `Wait Until Query` is called with that window's `…/@Name` step, the operator `validate` and the expression `valu == 'x'`, which cannot be evaluated
- **THEN** it SHALL fail on the first attempt with the error that names the expression, not with a timeout error

#### Scenario: A root element from another library import fails at once

- **GIVEN** an element captured through one import of the library
- **WHEN** a second import of the library calls `Wait Until Query` with a relative expression and that element as `root`
- **THEN** it SHALL fail at once with the error for an element that belongs to a different library instance

#### Scenario: A root element that stops being valid fails at once

- **GIVEN** an element passed as `root`, which stops being valid after the first attempt
- **WHEN** `Wait Until Query` waits on a relative expression under that element, with `ignore_exceptions` enabled and a long timeout
- **THEN** the next attempt SHALL fail at once with the error for a pinned element that is no longer available
- **NOTE** Verified with a unit test on a fake runtime: the mock never invalidates an element.

### Requirement: Wait Until Gone exposes a dedicated still-present error

The library SHALL provide a public `ElementStillPresentError` exception, a subclass of `BareMetalError`. `Wait Until Gone` SHALL raise it when the timeout elapses before its target is gone, in two cases:

- the last attempt found the target still present or still valid;
- the last attempt raised, so that the target could not be confirmed gone.

Its message SHALL state the timeout as `within timeout of {timeout} seconds.`, consistent with the existing timeout-error convention. When the last attempt raised, the message SHALL say that the target could not be confirmed gone, and SHALL end with the last error.

#### Scenario: Still-present timeout raises the dedicated error

- **WHEN** `Wait Until Gone` times out with the target still present
- **THEN** it SHALL raise `ElementStillPresentError`, distinct from `ElementNotFoundError`, with a message that ends in `within timeout of {timeout} seconds.`

#### Scenario: A disappearance that could not be confirmed raises the dedicated error

- **GIVEN** a selector that fails to evaluate on every attempt, and `ignore_exceptions` enabled for the call
- **WHEN** `Wait Until Gone` times out
- **THEN** it SHALL raise `ElementStillPresentError`, whose message says `could not be confirmed gone within timeout of {timeout} seconds.` and continues with `The last error was: `

### Requirement: Wait Until Attribute Value waits until an attribute of an element satisfies a condition

The library SHALL provide a `Wait Until Attribute Value` keyword that repeatedly reads one attribute of one element until the attribute's value satisfies a condition, and then returns that value.

**Arguments.** The keyword SHALL take the element as a selector or as a captured element, and the attribute name bare or with a namespace prefix, as `Get Attribute Value` does. It SHALL accept the same `assertion_operator`, `assertion_expected` and `assertion_message` arguments as `Wait Until Query`, with the same operator spellings. Without an operator it SHALL wait until the value is truthy. It SHALL reject the `then`/`evaluate` operator, which transforms the value instead of asserting on it and therefore cannot express a wait condition.

**Result.** The keyword SHALL return the attribute's value as read in the attempt that satisfied the condition, typed as `Get Attribute Value` returns it. It SHALL NOT return a result an operator derives from the value, such as the capture groups AssertionEngine returns for `matches`.

**Waiting.** The effective query settings (`timeout`, `retry_interval`, `ignore_exceptions`) SHALL govern the wait, configurable per call only via `query_overrides`. One `timeout` SHALL bound the whole call, including the time until the element appears. The lookup of a `Set Root` root SHALL keep its own timeout, as in every wait (*Every wait polls and fails the same way*). Every attempt SHALL observe the current UI: a selector SHALL be evaluated against the live tree on each attempt, and a captured element SHALL be read afresh on each attempt while remaining the same element. An attempt in which the selector matches nothing, in which the element does not have the attribute, or in which the operator cannot yet compare the value SHALL count as not yet satisfied. With `ignore_exceptions` enabled, an attempt that raises SHALL count as not yet satisfied and SHALL never satisfy the condition, unless its error is one that waiting cannot fix (*Every wait polls and fails the same way*).

**Failing.** When the timeout elapses, the keyword SHALL fail with an error that describes its last attempt and states the timeout.

- When that attempt completed, the error SHALL say what it saw: the element was not found, the attribute was missing, or the value and why it did not satisfy the condition.
- When that attempt raised, the error SHALL say how far it got: no element was found, the element's attribute could not be read, or the value (which the error names) could not be checked. It SHALL end with the last error.

The keyword SHALL fail at once, without waiting, on a usage error:

- a selector that resolves to a value or an attribute instead of an element;
- the `then`/`evaluate` operator;
- an attribute prefix that names no namespace;
- a captured element from another library import.

A captured element that is no longer valid SHALL also fail at once, because it can never reach the value.

#### Scenario: A value that already satisfies the condition is returned at once

- **GIVEN** a window whose `@IsMaximized` is `False`
- **WHEN** `Wait Until Attribute Value` is called with a selector for that window, the attribute `IsMaximized`, the operator `==` and the expected value `${False}`
- **THEN** it SHALL return on the first attempt with the boolean `False`, equal in value and type to what `Get Attribute Value` reads for the same element and attribute

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
- **THEN** it SHALL keep waiting, never returning because of a swallowed error
- **AND** it SHALL fail after the timeout with `ElementNotFoundError`, which names the selector, states `within timeout of {timeout} seconds.` and ends with `The last error was: ` followed by the evaluation error

#### Scenario: A value that cannot be compared yet does not end the wait

- **GIVEN** a window whose `@Name` is a string
- **WHEN** `Wait Until Attribute Value` is called for that window and `Name` with the order operator `>` and the number `${5}`, which cannot be compared with a string
- **THEN** it SHALL keep waiting, and fail at the timeout with the comparison error and the timeout, rather than on the first attempt

#### Scenario: A check that raises is reported with the value it checked

- **GIVEN** a window whose `@Name` is `Operations Console`, and `ignore_exceptions` enabled for the call
- **WHEN** `Wait Until Attribute Value` is called for that window and `Name` with the operator `matches` and the pattern `(`, which is not a valid regular expression
- **THEN** it SHALL fail after the timeout with an `AssertionError`
- **AND** the error's message SHALL name the attribute and the element, say that the value was `'Operations Console'` and could not be checked, and end with the last error, which says what is wrong with the pattern
- **AND** it SHALL NOT report that no element matched

#### Scenario: A validate expression that cannot be evaluated ends the wait at once

- **GIVEN** a window whose `@IsMaximized` is `False`, and `ignore_exceptions` enabled for the call
- **WHEN** `Wait Until Attribute Value` is called for that window and `IsMaximized` with the operator `validate` and the expression `valu == True`, which cannot be evaluated
- **THEN** it SHALL fail on the first attempt with the error that names the expression, not with a timeout error

#### Scenario: An attribute that cannot be read is reported with the last error

- **GIVEN** an element whose attribute read raises a provider error on every attempt, and `ignore_exceptions` enabled for the call
- **WHEN** `Wait Until Attribute Value` is called for that element and attribute
- **THEN** it SHALL fail after the timeout with `AttributeNotFoundError`, which says that the attribute of that element could not be read and ends with the last error
- **NOTE** Verified with a unit test on a fake element. No provider can produce this today: the element interface reports a value it cannot read as a missing attribute, so a real or mock read never raises.
