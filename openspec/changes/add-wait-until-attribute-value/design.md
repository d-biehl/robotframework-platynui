# Design

## Context

See proposal.md for why the keyword is needed, and the spec delta for its behavior. This section covers only the code the keyword builds on. Everything below was read in the current code unless it is marked as assumed.

- **`Get Attribute`** ([src/PlatynUI/BareMetal/__init__.py:2417-2445](../../../src/PlatynUI/BareMetal/__init__.py#L2417-L2445)) splits a `prefix:name` attribute name, waits for the element with `descriptor.resolve`, and returns `node.attribute(name, namespace)`. The `@assertable` decorator checks the value once and returns the value itself, not what AssertionEngine returns ([src/PlatynUI/_assertable.py:118-123](../../../src/PlatynUI/_assertable.py#L118-L123)).
- **`descriptor.resolve`** ([__init__.py:240-325](../../../src/PlatynUI/BareMetal/__init__.py#L240-L325)) waits up to its own `timeout`. It accepts a captured element only when it belongs to this import and is still valid; otherwise it raises `ForeignNodeError` or `PinnedElementGoneError` (lines 247-256). It raises `ResultTypeError` when a selector yields something that is not an element (lines 319-320).
- **`Wait Until Gone`** ([__init__.py:1638-1692](../../../src/PlatynUI/BareMetal/__init__.py#L1638-L1692)) is the model for a loop that handles both selectors and captured elements.
  - It checks ownership of a captured element before the loop.
  - For a selector, each attempt clears the cache, resolves the root only when the selector needs it, and evaluates once without an inner wait.
  - For a captured element, each attempt calls `invalidate()` and then `is_valid()` (lines 1660-1668).
  - With `ignore_exceptions`, a swallowed error never counts as success (lines 1669-1676).
- **`Wait Until Query`** ([__init__.py:1694-1800](../../../src/PlatynUI/BareMetal/__init__.py#L1694-L1800)) is the model for asserting inside the loop.
  - It rejects `then` up front.
  - It treats `AssertionError` and `TypeError` from `verify_assertion` as "not yet", and lets `RuntimeError` (an unknown operator) through at once.
  - It returns what `verify_assertion` returns.
  - At the deadline it evaluates the expression once more, to produce AssertionEngine's diagnostic.
- **AssertionEngine** (installed version 5.0.1, `assertionengine/assertion_engine.py:188-219`): `verify_assertion` returns the value for every operator except two. `then` returns the transformed result. `matches` returns the capture groups (a tuple, or a dict for named groups) when the pattern has any (`_matches`, lines 121-129).
- **Native binding** ([packages/native/src/runtime.rs:101-115](../../../packages/native/src/runtime.rs#L101-L115)): `UiNode.attribute` raises `ValueError` for an unknown namespace prefix before it asks the provider, and `AttributeNotFoundError` when the node lacks the attribute. `UiNode.invalidate()` asks the provider to refresh the node's cached information (lines 218-221).
- **Namespaces:** the prefixes are exactly `control`, `item`, `app` and `native`, matched case-sensitively ([crates/core/src/ui/namespace.rs:59-76](../../../crates/core/src/ui/namespace.rs#L59-L76)). Python sees the same four through `Namespace.Control` and the others, each with `as_str()` ([packages/native/python/platynui_native/_native.pyi:115-124](../../../packages/native/python/platynui_native/_native.pyi#L115-L124)).
- **The snapshot model:** [dev-docs/architecture.md:764](../../../dev-docs/architecture.md#L764) lists which BareMetal keywords clear the cache on every attempt. The library introduction repeats the list ([__init__.py:518-523](../../../src/PlatynUI/BareMetal/__init__.py#L518-L523)).

## Goals / Non-Goals

**Goals:**

- One keyword with one purpose: wait until one attribute of one element satisfies a condition. It takes its inputs the way `Get Attribute` does and its assertion arguments the way `Wait Until Query` does.
- A failure that tells the user which of three things went wrong: the element was never found, the attribute was never there, or the value never satisfied the condition.
- It behaves like the other `Wait Until …` keywords where they agree: query settings, `ignore_exceptions`, clearing the cache on every attempt, and usage errors that fail at once.

**Non-Goals:**

- Changing `Get Attribute`. It keeps reading once and checking once.
- Changing `Wait Until Query`, in particular that it returns the `matches` capture groups and raises `ResultTypeError` when the truthy default times out.
- Converting existing `Wait Until Query …/@Attr` waits in the suites.
- Waiting on several attributes, or on a value computed from them. That stays `Wait Until Query`.
- A `root=` argument. It is not needed, see decision 7.

## Decisions

### 1. A separate keyword, not a `Get Attribute` that waits

Reading a value and waiting for a value are two actions with two results: `Get Attribute` reports what is there now, while the wait reports the value at the moment a condition held. The library keeps one keyword per action.

*Alternatives considered:*

- Letting `Get Attribute` retry a failing assertion, as the Browser library's getters do (`retry_assertions_for`). Rejected: one keyword would then do two things, and a wrong value would take a whole timeout to fail in a keyword whose job is to read.
- Keeping `Wait Until Query …/@Attr` as the only way. Rejected for the reasons in proposal.md.

### 2. The name `Wait Until Attribute Value`

The name says what is waited on, which is the value. `Wait Until Attribute    …    IsEnabled`, called without an operator, would read as "wait until the attribute exists". With an operator the call reads as a sentence: `Wait Until Attribute Value    ${win}    IsMaximized    ==    ${True}`.

### 3. Its own loop with one deadline, not `descriptor.resolve` inside a loop

The keyword declares the three assertion parameters on its own signature and checks inside its own loop, as `Wait Until Query` does. It does not use `@assertable`, which checks once. `query_overrides` is keyword-only, as on the other wait keywords. `assertion_expected` given without an operator is ignored, as in `Wait Until Query` and `Get Attribute`.

Each attempt looks the element up once, without waiting:

- **A selector:** clear the cache, resolve the root only if the selector needs it, and evaluate once. This is `Wait Until Gone`'s selector branch.
- **A captured element:** check before the loop that it belongs to this import. On each attempt call `invalidate()`, then `is_valid()`, then read, as `Wait Until Gone` polls liveness.

One `timeout` then bounds the whole call. The root lookup is the exception: it keeps its own wait, as it does for every keyword ([__init__.py:958-960](../../../src/PlatynUI/BareMetal/__init__.py#L958-L960)).

*Alternatives considered:*

- Calling `descriptor.resolve` inside the loop. Rejected: each attempt could then block for a whole `timeout`, so the call could run for several.
- Building `<selector>/@Attr` and evaluating it as `Wait Until Query` does. Rejected for two reasons. Joining strings changes the meaning of some selectors (`a | b` + `/@X` becomes `a | b/@X`). And an empty result cannot tell a missing element from a missing attribute, which the failure has to report.

### 4. A missing attribute means "not yet"

A found element without the attribute counts as an attempt that did not succeed, and the loop continues. There are three reasons:

- `Wait Until Query` already treats a missing attribute as an empty result and keeps waiting.
- The library waits out a selector that finds nothing yet, a typo included.
- An element that is being rebuilt can match the selector before it has all its attributes.

The cost is that a misspelled attribute name takes the whole timeout to fail. The failure then names the attribute and the element, so the cause is still plain.

This was decided on the author's recommendation, because the user went on to the proposal without answering. Failing at once, as `Get Attribute` does, remains possible. It would change one scenario ("A missing attribute is waited for…") and this decision.

### 5. Usage errors fail at once, whatever `ignore_exceptions` says

Four inputs can never succeed, so they are checked before the first attempt or recognized on it:

- **An unknown namespace prefix:** checked before the loop against the four values of `Namespace`. The `ValueError` from `UiNode.attribute` would come only once an element is found, and `ignore_exceptions` would swallow it.
- **`then` / `evaluate`:** rejected up front, with the message `Wait Until Query` uses.
- **A captured element from another import:** `require_own_node` raises `ForeignNodeError`.
- **A selector that yields a value or an attribute:** `ResultTypeError`, pointing to `Wait Until Query`. This is `Wait Until Gone`'s check (lines 1654-1658).

A captured element that is no longer valid also fails at once with `PinnedElementGoneError`, because it can never reach the value. This is the error `descriptor.resolve` raises for a captured element without a selector.

### 6. The result is the value read in the successful attempt

The keyword returns the value it read in that attempt, not what `verify_assertion` returns. For `matches` with capture groups the two differ: AssertionEngine returns the groups. Returning the value matches `Get Attribute` (`@assertable` returns the value), and the result is always "the value the attribute had". The value is typed the same way `Get Attribute` types it, because both come from `UiNode.attribute`.

### 7. No `root=` argument

`Wait Until Query` needs `root=` because its argument is an arbitrary expression. This keyword takes an element reference: a captured element is passed as the element itself, and a relative selector follows `Set Root` like in every other keyword that takes a selector.

### 8. The failure reports the last attempt that did not raise

The keyword keeps the outcome of the last attempt that did not raise. Before any such attempt, the outcome is "element not found". When the timeout elapses, the keyword raises for that outcome, without the extra evaluation `Wait Until Query` runs after the deadline. The error therefore always matches what the loop saw.

| Last outcome | Error |
|---|---|
| Selector matched nothing | `ElementNotFoundError`, with the message element resolution uses (`No element matched … within timeout of {timeout} seconds.`, [__init__.py:312-314](../../../src/PlatynUI/BareMetal/__init__.py#L312-L314)) |
| Attribute missing | `AttributeNotFoundError`, the class `Get Attribute` raises for a missing attribute, naming the attribute and the element and ending in `within timeout of {timeout} seconds.` |
| Operator not satisfied | The `AssertionError` from that attempt, with ` (within timeout of {timeout} seconds)` appended, as in `Wait Until Query` |
| Operator could not compare the value (`TypeError`) | That comparison error, with the timeout appended |
| No operator, value not truthy | `AssertionError`: the attribute did not become truthy, with the last value and the timeout |

For the truthy case the keyword deliberately differs from `Wait Until Query`, which raises `ResultTypeError`. A condition that did not hold is an assertion failure, not a type error. `Wait Until Query` stays as it is.

Where the message names the element, it uses `UiNode.describe()` (`Role "Name" #Id`). That call asks the provider, so it is guarded and falls back to the runtime id, as `_describe` does ([__init__.py:412-424](../../../src/PlatynUI/BareMetal/__init__.py#L412-L424)). `_describe` itself cannot be used here, because it returns nothing unless DEBUG logging is enabled.

AssertionEngine's message prefix names the attribute and the element. A given `assertion_message` takes the prefix's place, the way `Get Attribute` and `Wait Until Query` pass it.

## Risks / Trade-offs

- **A provider may keep an attribute value in its own cache, so a captured element reads a stale value even after `invalidate()`.** This is assumed, not verified: `invalidate()` exists, but whether each provider refreshes attribute values on it has not been checked. → The acceptance scenario for a captured label verifies it on every lane that runs the egui suite. If one provider fails, the selector form still works, and the gap becomes a provider bug to fix.
- **A misspelled attribute name takes the whole timeout to fail.** → The failure names the attribute and the element. `query_overrides` shortens the wait.
- **Users still reach for `Get Attribute    …    ==` right after an action and get a check that runs once.** → The two keywords' documentation points at each other, and the robot-test-style table names `Wait Until Attribute Value` for "attribute reaches a value".
- **Each attempt evaluates the selector against a cleared cache.** → This is the same cost as `Wait Until Query` and `Wait Until Gone`, and `retry_interval` limits it.
- **The failure message asks the provider for a description.** → The call is guarded and falls back to the runtime id. The error still carries the attribute name and the timeout.

## Migration Plan

- **Additive:** a new keyword. No existing keyword changes behavior, and nothing is removed.
- **No native rebuild:** the change is Python only and uses existing binding API. The RF recipes build the native package they need anyway (`just test-baremetal` builds the mock variant).
- **Rollback:** revert the commits. Only suites that call `Wait Until Attribute Value` depend on it; they can go back to `Wait Until Query    <selector>/@Attr`.
- **Main spec Purpose:** the Purpose paragraph of `openspec/specs/baremetal-waiting/spec.md` names the three existing keywords. It is outside the delta and is extended by hand.
