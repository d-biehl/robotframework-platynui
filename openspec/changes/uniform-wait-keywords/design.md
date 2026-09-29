# Design

## Context

See proposal.md for the motivation and the spec delta for the behavior. This section covers only the code the change builds on. All of it was read at `7180d20b`. *Inferred* marks a conclusion that was not run, and *assumed* marks one that was not checked. Line numbers are in `src/PlatynUI/BareMetal/__init__.py` unless a file is named.

**The four loops today.**

| Loop | Where it looks the root up | What it always re-raises, even under `ignore_exceptions` | What the timeout reports |
|---|---|---|---|
| `UiNodeDescriptor.resolve` (`:241-322`): `Wait Until Exists` (`:1688`), every keyword that takes an element, and the root's own lookup | Once, before the loop (`:276-282`); a `RootNotFoundError` is re-raised with the target named (`:279-282`) | Nothing inside the loop; a value result raises `ResultTypeError` after it (`:316-317`) | `No element matched … within timeout of N seconds.` (`:309-311`), or `RootNotFoundError` for a root (`:307-308`) |
| `Wait Until Gone` (`:1690-1779`) | On every attempt, *inside* the swallowing `try` (`:1739`) | `ResultTypeError` (`:1756-1757`) | `… was still present …` / `Captured element {repr} was still valid …` (`:1768-1777`), whatever the attempts saw |
| `Wait Until Query` (`:1781-1887`) | Once, before the loop, without `needs_root` and without an ownership check of `root` (`:1839`) | `RuntimeError` (`:1857-1858`). `AssertionError` and `TypeError` count as "not yet", including a `TypeError` of the evaluation itself (`:1844-1860`) | Truthy: `… did not become truthy …` without the result (`:1870-1873`). With an operator: one more evaluation after the deadline (`:1874-1885`) |
| `Wait Until Attribute Value` (`:1889-2028`, failure in `_attribute_wait_error` `:403-432`) | On every attempt, inside the `try` (`:1980`) | `ResultTypeError`, `PinnedElementGoneError`, `RuntimeError` (`:2017-2018`) | The last attempt that did not raise (`:1969-1971`, `:2025-2026`) |

Every loop swallows any other `Exception` when `ignore_exceptions` is on, and drops it (`:296-299`, `:1760-1763`, `:1861-1864`, `:2021-2023`).

**What the loops share and must keep.**

- **The root's settings.** The `root` property (`:1441-1454`) resolves the stored root with `as_root=True` and no per-call override. The root's lookup therefore runs with the scope or import settings, never with `query_overrides`. The library documents this (`:1036-1043`), and `tests/BareMetal/query_settings.robot:120-136` pins it.
- **Reusing the root.** A root reuses the element it resolved to while that element belongs to this runtime and `is_valid()` (`:264-271`). `is_valid()` costs one provider call: AT-SPI reads the role over D-Bus (`crates/provider-atspi/src/node.rs:510-516`), UI Automation reads `CurrentProcessId` (`crates/provider-windows-uia/src/node.rs:743-749`). It is `true` where a provider does not implement it (`crates/core/src/ui/node.rs:106-108`); the mock does not implement it.
- **Whether a selector needs the root.** `needs_root` (`:219-234`) memoizes `is_context_dependent` per selector. An expression that does not parse counts as needing the root, so the real evaluation reports the error. `is_context_dependent` classifies only the top-level node selection (`crates/xpath/src/parser/ast.rs:95-123`, tests in `crates/xpath/tests/it/parser_context_dependence.rs`). A function call counts as independent whatever its arguments hold: `count(.//x)` does, and so do `name()` and `position()`, although the commit that added the classifier (`3b4b5def`) names zero-argument context functions as dependent. An `if` condition and a `for` or `let` binding are ignored as well, so `let $a := .//x return $a` counts as independent. That suits the top-level node selection of an element selector, but it classifies an expression that computes a value wrongly (checked on the mock build). Decision 12 changes this.
- **The snapshot policy.** The element lookup discards the runtime's snapshot only after an attempt that found nothing (`:313-314`). `Wait Until Gone`, `Wait Until Query` and `Wait Until Attribute Value` discard it before every attempt (`:1738`, `:1845`, `:1975`). The library docs (`:592-598`) and `dev-docs/architecture.md:766` describe exactly this.

**What can raise inside an attempt.**

- Native errors derive from `PlatynUiError(Exception)`, and none of them from `RuntimeError` (`packages/native/src/runtime.rs:1765-1772`). `evaluate_single` maps evaluation failures to `EvaluationError` (`:908-923`, `:1667-1669`). It raises `TypeError` only for a context that is not a `UiNode` (`:913`).
- `UiNode.is_valid()`, `invalidate()` and `bool(UiNode)` return plain values and cannot raise (`runtime.rs:216-231`).
- `UiNode.attribute()` raises `ValueError` for an unknown namespace and `AttributeNotFoundError` when the element returns nothing (`runtime.rs:103-115`). The element interface has no error path at all: `attribute` returns an `Option` and `value` a plain `UiValue` (`crates/core/src/ui/node.rs:63`, `:225`). Neither do `children` and `attributes` (`:50`, `:60`). An attribute read therefore never raises a provider error. *Inferred:* what reaches a wait as an error today comes from the XPath engine (`crates/runtime/src/xpath.rs:152-159`), not from a provider.
- The runtime evaluates against a held element as it is, without a resolver (`crates/runtime/src/runtime/evaluation.rs:15-17`, `xpath.rs:306-324`). *Inferred:* for an element that died, the provider decides what the evaluation sees, typically no children.
- AssertionEngine 5.0.1 (`assertionengine/assertion_engine.py:188-219`): `verify_assertion` raises `AssertionError` on a mismatch and the operator's own `TypeError` when the values cannot be compared. It raises `RuntimeError` itself only for an operator without a handler (`:208-211`). Enumerating `AssertionOperator` shows that every member except `then` has a handler, and `then` is rejected before the loops.
- `validate` evaluates its expression with Robot Framework's `BuiltIn().evaluate` (`assertion_engine.py:151-152`). Robot Framework reports any failure of the expression as `DataError` (`robot/variables/evaluation.py:30-65`), and `BuiltIn.evaluate` turns that into a `RuntimeError` with the same message (`robot/libraries/BuiltIn.py:4570-4571`, RF 7.5; RF 7.0 does the same). A `DataError` stands for invalid test data (`robot/errors.py:56-61`); the `RuntimeError` makes a failing expression an ordinary keyword failure. So a `validate` expression that raises reaches the loops as a `RuntimeError`, whether it is misspelled or fails on the current value, such as `int(value) > 3` on `'Loading'`. Robot Framework shows a `RuntimeError` without its type name: the message reads `Evaluating expression … failed: …`.
- Robot Framework's own timeout is a `BaseException` (`robot/errors.py:90`), so `except Exception` does not catch it.

**Robot Framework's form of a last error.** `Wait Until Keyword Succeeds` fails with `… The last error was: {err}` (`robot/libraries/BuiltIn.py:3419-3420`). The error text there is formatted by `ErrorDetails` (`robot/utils/error.py:44-66`, `:117-134`): `Type: message`, with the type left out for `AssertionError`, `Error`, `Exception`, `RuntimeError` (`:52`) and for Robot Framework's own errors. Checked in RF 7.5: `ErrorDetails(EvaluationError('x')).message` is `EvaluationError: x`, and a `DataError` or a `RuntimeError` gives its bare message. RF 7.0, the project's floor (`pyproject.toml:31`), takes the error in the same constructor argument (task 4.1). BareMetal's errors set no name suppression, so Robot Framework shows their type. `tests/acceptance/egui/app_root_after_exit.robot:29` expects `STARTS:RootNotFoundError: `.

**Where the code and the specs or docs disagree today.** The code is reality; these are the places where it misses the stated intent.

- `Wait Until Query` evaluates against a `root` from another import (`:1839`). `baremetal-selector-resolution` requires the mismatch error for every capture a keyword evaluates against (`openspec/specs/baremetal-selector-resolution/spec.md:85-87`).
- `Wait Until Query` looks the root up for an absolute expression (`:1839`). The same spec says an absolute selector does not resolve the root (`spec.md:29-32`).
- `Wait Until Gone`'s documentation promises the root's own error (`:1710-1711`), but the code swallows it (`:1739`, `:1760-1763`).
- `Wait Until Attribute Value` under `ignore_exceptions` reports "No element matched" for an element it found on every attempt when the *check* raises, for example `matches` with a pattern that is not a valid regular expression (`re.error`; checked on the mock). The outcome is assigned only in branches that the raise skips (`:1995-2016`), so it keeps its initial "not found" (`:1971`). The maintainer did not list this defect; the new failure rule fixes it.
- `Wait Until Gone` reports a reference that holds neither a selector nor an element as gone (`:1749-1751`), while `resolve` and `Wait Until Attribute Value` raise `NoQueryError` (`:255-256`, `:1962-1963`). Robot Framework's argument conversion never produces such a reference.

**Tests that pin today's behavior.**

- They change: `tests/BareMetal/wait_keywords.robot:73-75`, `:133-135` and `:199-202`. The last one ends in `seconds.` with no trailing `*`.
- They stay compatible: `tests/BareMetal/query_settings.robot:86-92`, `tests/PlatynUI/test_keyword_logging_rf.py:236-241`, where no error is swallowed, and the `STARTS:RootNotFoundError` expectations of the acceptance suites.
- `tests/PlatynUI/test_baremetal_root_reuse.py:41-71` is the model for unit tests with a fake runtime.

**The absorbed triage entries** are in `openspec/changes/archive/2026-09-27-logging-concept/review-findings.md`: `:721-725` for the element lookup and `:812-816` for `Wait Until Gone` (both follow-up A2), and `:751-755` for the truthy message (B). Their proposed wording, `; the last attempt failed with: …`, gives way to the maintainer's `The last error was: …`.

## Goals / Non-Goals

**Goals:**

- One loop implementation serves the element lookup of a target, the lookup of a root, `Wait Until Gone`, `Wait Until Query` and `Wait Until Attribute Value`. Each supplies only what one attempt does and how it describes the failure.
- The root is looked up on every attempt, and its failure is never swallowed.
- A failure describes the last attempt, including the last swallowed error.
- Nothing else changes. These stay as they are:
  - the snapshot policy;
  - the split between per-call settings and the root's settings;
  - what each keyword returns on success;
  - the checks each keyword makes before its first attempt;
  - the error type for each situation.
- The check whether a selector or expression needs the root sees every place it reads its context, a function's arguments included (decision 12).

**Non-Goals:**

- Retrying in `Get Attribute Value`. It reads and checks once ("one keyword, one action").
- Changing error types. In particular, `Wait Until Query`'s truthy timeout stays a `ResultTypeError`, where `Wait Until Attribute Value` raises `AssertionError` for the same situation. The archived design of `add-wait-until-attribute-value` kept the two apart on purpose (its decision 8).
- A DEBUG record per swallowed error (decision 9).
- Waiting in `Query`, which stays a snapshot. Only its lookup of the root follows decision 12, like every keyword's.
- Waiting out a `validate` expression that raises. It arrives as a `RuntimeError`, which ends every wait at once (decision 3).
- `Highlight`'s broad `except` and the runtime's pointer `ensure_move` double report. Both are separate items.
- The text of `Wait Until Query`'s mismatch message. It keeps AssertionEngine's diagnostic plus the timeout, without naming the expression.

## Decisions

### 1. One polling helper; each keyword supplies an attempt and a failure

A private module-level helper in `BareMetal/__init__.py` runs every wait. It takes these inputs:

- the effective settings;
- a *context step*, which looks up the root or checks an explicit root and yields the element to evaluate against;
- an *attempt*, which takes that context and reports "done, with this result" or "not yet";
- a *failure builder*, which turns what the last attempt saw, and the last error, into the exception to raise;
- whether to discard the runtime's snapshot before the first attempt.

A loop through the helper goes like this:

1. It runs the context step, outside the error handling (decision 2). The clock starts after the first context step.
2. It runs the attempt. An error that waiting cannot fix is re-raised at once (decision 3). Any other `Exception` is re-raised without `ignore_exceptions`. With it, the error is kept as the last error, and an attempt that completes clears it (decision 4).
3. After the attempt, if the timeout has elapsed, it raises what the failure builder returns, chained to the last error. Otherwise it sleeps `retry_interval`, discards the snapshot and starts again.

The helper makes at least one attempt. An attempt records what it has seen as it goes: the element found, the value read. So when it raises, the failure builder knows how far it got (decision 5).

`UiNodeDescriptor.resolve` uses the helper both for a target and for a root. The three wait keywords use it with their own attempts. The checks that come before the first attempt keep their place: the rejected operator, the unknown namespace prefix, and the ownership and validity of a captured target.

*Alternatives rejected:*

- **Fixing each loop in place.** The defects come from four copies drifting apart, and the root's own lookup inside `resolve` would be a fifth. Five copies of the same rules would drift again.
- **Letting the wait keywords call `resolve` in a loop.** Each attempt could then block for a whole timeout. `add-wait-until-attribute-value` rejected this in its decision 3.
- **A class hierarchy of waits.** Five call sites do not need more than a function with three callbacks.

### 2. The root is looked up on every attempt, outside the error swallowing

Each attempt starts with the root's own lookup, when the target needs a root. That lookup reuses the root's element while it is valid, as `:264-271` does today, so it costs one provider call. It runs before the swallowing `try`, so its failure ends the wait whatever the target's `ignore_exceptions` says.

The alternative is today's `resolve` and `Wait Until Query`: look the root up once, before the loop, and evaluate every attempt against that element.

| Situation | Once before the loop | On every attempt (chosen) |
|---|---|---|
| The root stays valid | One check | One check per attempt, one provider call each (free on the mock) |
| The root is replaced during the wait (its window closes and reopens) | Every attempt evaluates against the dead element. *Inferred:* nothing matches, so `Wait Until Exists` blames the target after its whole timeout, and `Wait Until Gone` reports "gone" although the replacement still holds a match | The next attempt looks the root up again and the wait continues against the replacement, as a selector root does between keywords |
| The root goes away for good | As above: a failure that blames the target, or a false "gone" | `RootNotFoundError` after the root's own timeout, naming the root and the target |
| A pinned root (`Set Root ${element}`) goes away | As above | `PinnedElementGoneError` on the next attempt |
| Longest possible wait | Root timeout plus target timeout | The same. The deadline is checked after each attempt, so at most one attempt runs past it, and it holds at most one lookup of the root |

Looking the root up on every attempt makes a failing root fail the same way for every provider and names the right cause. It follows a replaced root, which is what `Set Root` promises between keywords (`:892-893`). The spec already says so for `Wait Until Gone`: the root "SHALL be re-resolved per attempt, consistent with every other keyword" (`openspec/specs/baremetal-waiting/spec.md:53`).

Consequences:

- **The per-call timeout does not bound the root's lookup.** This is documented (`:1036-1039`) and stays so. What changes is that a missing root costs one root timeout and then ends the wait with `RootNotFoundError`. Before, the root was retried until the per-call timeout, and the wait failed with an error that blamed the target.
- **The wait's clock starts after the first lookup of the root**, as it did when the root was looked up before the loop. A root that is found late therefore does not use up the target's timeout, and a lookup during the wait counts as part of its attempt. The first version started the clock before that lookup; the review of 2026-09-30 found it, since a keyword with a short `query_overrides` timeout then failed while an application was still starting.
- **`Wait Until Gone` cannot tell whether a target left together with its root.** A target inside a root that goes away for good fails with `RootNotFoundError` after the root's timeout. The keyword cannot tell "the container closed" from "the root selector is wrong". To wait for a container to close, wait on the container itself. The keyword's documentation already says a vanishing root surfaces as the root's error (`:1710-1711`), and it names that way.
- **What surfaces from the root's lookup:**
  - `RootNotFoundError`, re-raised with the target named, as `:279-282` already does. It gains an optional last error, carried from the root's own lookup when that lookup swallowed errors under the root's settings, and appended to the message.
  - `PinnedElementGoneError`.
  - Anything else, unchanged, such as the `ResultTypeError` of a root selector that yields a value (`tests/BareMetal/set_root_scope.robot:82-92`).

  *Rejected:* wrapping every root failure in `RootNotFoundError`. Its message states that the timeout elapsed, which would be false for an error raised on the first attempt.
- **`Wait Until Query` follows the same rule.**
  - Its expression goes through the same `needs_root` memo, so an absolute expression never looks the root up. That takes the classifier of decision 12: the one before it calls `count(.//x)` absolute, and `Wait Until Query` would count on the whole desktop, silently.
  - An explicit `root` is checked with `require_own_node` before the first attempt, as `Query` does (`:1654`).
  - Its validity is checked in the context step of every attempt. An element that is no longer valid raises `PinnedElementGoneError`, as a pinned `Set Root` root does.

  This closes the first two gaps listed under Context.

### 3. One list of errors that waiting cannot fix

One module-level tuple lists the errors that end a wait at once, whatever `ignore_exceptions` says. It holds these errors, each raised where the table says:

| Error | Where it arises in an attempt |
|---|---|
| `ResultTypeError` | A selector yields a value where an element is needed (element lookup, `Wait Until Gone`, `Wait Until Attribute Value`) |
| `NoQueryError` | A reference with neither a selector nor an element. Checked before the first attempt, and now also in `Wait Until Gone`, which reported it as gone (`:1749-1751`) |
| `ForeignNodeError` | A captured target or an explicit `root` from another import, checked before the first attempt |
| `PinnedElementGoneError` | A captured element that is no longer valid where the wait needs it: `Wait Until Attribute Value`'s target, a pinned root, `Wait Until Query`'s `root` |
| `RootNotFoundError` | The root's lookup. It already runs outside the `try` (decision 2); the entry keeps the rule complete should it ever be raised inside an attempt |
| `RuntimeError` | AssertionEngine, for an operator it has no handler for; Robot Framework, for a `validate` expression that raises |

`Wait Until Gone` never raises `PinnedElementGoneError` for its captured target, because a target that is no longer valid is the success it waits for.

`RuntimeError` has two sources in an attempt, and no native error is one, since they derive from `PlatynUiError`:

- AssertionEngine raises it for an operator without a handler (`assertion_engine.py:208-211`). Robot Framework cannot reach that, since its argument conversion yields an `AssertionOperator` and every member but `then` has a handler.
- Robot Framework raises it for a `validate` expression that raises (Context).

The maintainer decided on 2026-09-29 that a `RuntimeError` ends a wait at once, like `SystemExit`, also with `ignore_exceptions`: a `validate` expression that cannot be evaluated is wrong, and waiting does not make it right. Both assertion loops already re-raise it today (`:1857-1858`, `:2017-2018`). The user documentation says what follows: write a `validate` expression so that it copes with every value it can get (decision 10). Listing `RuntimeError` for the loops that never call AssertionEngine changes nothing for them.

*Rejected:* treating every `BareMetalError` as an error that waiting cannot fix. The timeout errors are `BareMetalError`s too, and a rule by class would silently turn every future library error into one. An explicit list is also the "one shared list" the maintainer asked for.

`KeyboardInterrupt`, `SystemExit` and Robot Framework's timeout pass through, because the loop catches `Exception` only.

### 4. `ignore_exceptions` remembers the last error

- An error that is not on the list is re-raised at once without `ignore_exceptions`.
- With `ignore_exceptions`, the error is kept as the last error, and the attempt counts as not satisfied. Only a completed attempt can satisfy a wait.
- An attempt that completes, whatever it saw, clears the last error.
- The final exception is chained to the last error (`raise … from last_error`). Robot Framework's debug traceback then shows where the error came from.

`ignore_exceptions` also governs the root's own lookup, but with the root's settings (scope or import), as today.

### 5. The failure describes the last attempt, up to where it got

At the deadline the failure builder looks at the last attempt:

- **It completed:** the failure says what it saw, as each keyword does today. `Wait Until Query`'s truthy timeout additionally names the result.
- **It raised:** the failure says how far the attempt got before the error, and continues with ` The last error was: ` and the error.

The error's type follows the situation and never depends on the swallowed exception. Each type is the one the keyword already raises for that situation.

| Wait | Last attempt | Error | Message |
|---|---|---|---|
| Element lookup | matched nothing | `ElementNotFoundError` | `No element matched {q!r} within timeout of {t} seconds.` (unchanged) |
| | raised | `ElementNotFoundError` | the same, then ` The last error was: {err}` |
| Root lookup | matched nothing | `RootNotFoundError` | `The root set by Set Root, {r!r}, was not found within timeout of {t} seconds; {q!r} was not evaluated.` (unchanged) |
| | raised (under the root's own settings) | `RootNotFoundError` | the same, then ` The last error was: {err}` |
| `Wait Until Gone`, selector | matched an element | `ElementStillPresentError` | `Element matching query {q!r} was still present within timeout of {t} seconds.` (unchanged) |
| | raised | `ElementStillPresentError` | `Element matching query {q!r} could not be confirmed gone within timeout of {t} seconds. The last error was: {err}` |
| `Wait Until Gone`, capture | still valid | `ElementStillPresentError` | `Captured element {E} was still valid within timeout of {t} seconds.` |
| | raised (cannot happen today, see below) | `ElementStillPresentError` | `Captured element {E} could not be confirmed gone within timeout of {t} seconds. The last error was: {err}` |
| `Wait Until Query`, no operator | falsy result | `ResultTypeError` | `Query {e!r} was {v} and did not become truthy within timeout of {t} seconds.`, or `… matched nothing and …` |
| | raised | `ResultTypeError` | `Query {e!r} did not become truthy within timeout of {t} seconds. The last error was: {err}` |
| `Wait Until Query`, operator | mismatch | `AssertionError` | `{diagnostic} (within timeout of {t} seconds)` (unchanged) |
| | cannot compare | `TypeError` | `Query {e!r}: {error} (within timeout of {t} seconds)` |
| | the evaluation raised | `AssertionError` | `Query {e!r} did not satisfy the assertion within timeout of {t} seconds. The last error was: {err}` |
| | the check raised | `AssertionError` | `Query {e!r} was {v} and could not be checked within timeout of {t} seconds. The last error was: {err}` |
| `Wait Until Attribute Value` | completed | as today | not found, attribute missing, not truthy, mismatch, cannot compare (`:403-432`) |
| | the selector evaluation raised | `ElementNotFoundError` | `No element matched {q!r} within timeout of {t} seconds. The last error was: {err}` |
| | reading the attribute raised | `AttributeNotFoundError` | `Attribute {a!r} of {E} could not be read within timeout of {t} seconds. The last error was: {err}` |
| | the check raised | `AssertionError` | `Attribute {a!r} of {E} was {v!r} and could not be checked within timeout of {t} seconds. The last error was: {err}` |

Notes on the table:

- **`{err}`** is Robot Framework's `ErrorDetails(error).message`, the form the same error has when it fails a keyword directly. That puts the type name in front of the message, which the `diagnostic-logging` requirement *Elements are described in one form* (`openspec/specs/diagnostic-logging/spec.md:188-199`) does not forbid. That requirement keeps internal names such as `UiNode` or `UiNodeDescriptor` out of messages (`test_keyword_logging_rf.py:241`). A public exception type is not one of them, and Robot Framework already shows it for every BareMetal failure.
- **`{E}`** is an element in the one form (`Role "Name" #id`), described by `_element_text` (`:489-498`), which falls back to the runtime id. `Wait Until Gone`'s captured element used to appear as its `repr` (`UiNode(runtime_id=…)`, `runtime.rs:245-248`). That spec requirement asks for the one form in every message PlatynUI adds or changes, and both of `Wait Until Gone`'s captured-element messages are touched here.
- **`{v}` in `Wait Until Query`** is the result as the check sees it (`_assertion_value`, `:354-364`). A value shows its `repr` (`0`, `False`, `'Operations Console'`). The `repr` of an attribute result would show only `EvaluatedAttribute(namespace=…, name=…)` (`runtime.rs:690-692`), so an attribute shows its value. An element shows the one form, and "no result" reads "matched nothing".
- **`Wait Until Query` with an operator** uses `AssertionError` for "did not satisfy the assertion", the type of its condition. Today's `ResultTypeError` with that text (`:1883-1885`) is reachable only when the evaluation after the deadline passes, and that evaluation goes away (decision 6). Today, an evaluation that raises under `ignore_exceptions` escapes raw from that evaluation (`:1876`).
- **A captured element in `Wait Until Gone` cannot raise today**, because `invalidate()` and `is_valid()` cannot raise. The row exists so that the rule holds without exceptions.
- **The "reading the attribute raised" row of `Wait Until Attribute Value` is not reachable either**, because the element interface cannot raise on a read. It keeps the type of today's "unread" outcome.

*Alternatives rejected:*

- **Describing the last attempt that completed, and adding the last error.** This is today's rule for `Wait Until Attribute Value` (archived decision 8, "the last attempt that did not raise"). It states things the last attempt never saw. `Wait Until Gone` would say "still present" after its last attempts failed, which is exactly the defect the maintainer named. A value from an older attempt would be reported as the value at the end.
- **One neutral message whenever the last attempt raised.** It loses the difference between "no element" and "could not read", and changes `Wait Until Attribute Value`'s types. Its existing scenario expects `ElementNotFoundError` when every attempt raised.

Consequence: under `ignore_exceptions`, `Wait Until Attribute Value` can report a different kind than today when a completed attempt is followed by raising ones. A mismatch followed by failing evaluations now reports `ElementNotFoundError` with the last error, not the older mismatch.

### 6. Nothing is evaluated after the deadline

The helper compares the elapsed time with the timeout after each attempt, and the attempt that crossed it is the one the failure describes. `Wait Until Query`'s extra evaluation (`:1874-1885`) goes away, as `add-wait-until-attribute-value` decided for its keyword (archived decision 8). With it go three effects:

- a raw exception escaping after the deadline (`:1876` runs outside any `try`);
- a pass after the deadline that still fails with `ResultTypeError` (`:1883-1885`);
- a comparison `TypeError` without timeout context.

### 7. Each loop keeps its snapshot policy

The helper takes the snapshot policy as an input:

- The element lookup keeps the snapshot for its first attempt, so an action right after another reuses what that one read. It discards the snapshot before each retry (`:313-314`).
- The three waits for a change discard it before every attempt (`:1738`, `:1845`, `:1975`).

In both cases the discard comes before the context step. A root that is looked up again therefore reads the current UI. The documentation (`:592-598`) and `dev-docs/architecture.md:766` stay true.

### 8. `Get Attribute Value` stays one-shot

`_read_attribute` (`:2686-2694`) waits for the element through `resolve`, reads once, and `@assertable` checks once. Only the element lookup changes, through the helper, like every keyword's.

### 9. No record for each swallowed error

The two A2 entries also proposed a DEBUG record on the first swallowed error, and again whenever its text changes. It is not added:

- A2 is "a failing keyword reports once, with its real cause". The failure now carries that cause.
- A wait that recovers has lost nothing. `dev-docs/logging.md` §4 has a layer that swallows a failure log it at the level its consequence deserves. For a recovered wait that is debug at most, which is optional, not required.

A later change can add such a record without touching these rules (Open Questions).

### 10. The documentation states the rules in the library's voice

The rules are said briefly where users look for them, without naming provider internals:

- The text on `ignore_exceptions` under "Waiting for elements" says that a failure quotes the error the last attempt raised, and names the few errors that end a wait even with `ignore_exceptions`: a selector that yields a value, an element from `Query` or a root pinned to one that no longer exists, a root that is not found within its own timeout, and a `validate` expression that raises. It explains the setting with an XPath example (`xs:integer(@Name)` on a label that shows no number yet) instead of provider errors, which cannot reach a wait (Open Questions, answered).
- The section "Scope" says that a root keeps the element it found while that element exists, that a waiting keyword checks the root on every attempt, and that a root that cannot be found fails the keyword with its own error.
- Each wait keyword's docstring states its failure in a sentence. `Wait Until Query` also says that an absolute expression does not use the root and that its `root` must come from the same import. `Wait Until Query` and `Wait Until Attribute Value` say that a `validate` expression that raises ends the wait at once, with an expression that copes with every value it can get.
- The type documentation of an element argument is written for users. Libdoc shows the converter's docstring, and falls back to the class docstring of `UiNodeDescriptor`, which is written for developers, when the converter has none; `UiNodeDescriptor.convert` gets one.

A first version gave the rules a section of their own, "When a wait gives up". The maintainer found it too long and too complicated (2026-09-30), and its content went into the places above. The review of the same day also corrected older text that the waits touch: the examples compare numbers with `${0}` rather than the string `0`, `Query` is no longer recommended for checking that a dialog has gone, and `Get Attribute Value` says that it waits for its element.

### 11. Tests are placed by what they need

- **Robot Framework mock suites** (`tests/BareMetal`) cover everything the static mock tree can show: the changed messages, a root that matches nothing, an absolute expression and an expression that computes a value, each under a root, a foreign `root`, a value selector and a `validate` expression that raises, both under `ignore_exceptions`, and a `matches` pattern that is not a valid regular expression. They come first.
- **A pytest module with a fake runtime**, next to `test_baremetal_root_reuse.py`, covers what needs something to change between attempts: a completed attempt that clears the last error, a root replaced or gone during the wait, a pinned root or an explicit `root` that stops being valid, a root found late that leaves the target its whole timeout, a root whose own lookup raises under its own `ignore_exceptions`, an attribute read that raises, and a single evaluation when the first attempt already overruns the timeout.
- **One egui acceptance test** covers a root pinned to an application that has ended, under `ignore_exceptions`, on real providers. It has no platform tag, so every lane runs it.
- **Rust tests** pin the classifier of decision 12: one case per kind of context read and per place where the focus changes, in the XPath crate (`crates/xpath/tests/it/parser_context_dependence.rs`), and the main cases in the runtime crate, whose function the binding calls.

### 12. The classifier sees every place an expression reads its context

`Expr::is_context_dependent` answers whether evaluating an expression reads its context: the context item, its position or its size. It used to answer that only for the top-level node selection (Context). This change makes it look into every operand, condition, binding and function argument, and counts three kinds of context read:

- a relative path (`.//x`, `child::x`) and the context item (`.`);
- a standard function that falls back to the context when an argument is left out: `data()`, `number()`, `string()`, `string-length()`, `normalize-space()`, `name()`, `local-name()`, `namespace-uri()`, `root()`, `base-uri()` and `document-uri()`, and `lang(…)`, `id(…)`, `element-with-id(…)` and `idref(…)` with one argument. These are the registered forms (`crates/xpath/src/engine/functions/mod.rs`) whose implementations call `require_context_item`;
- `position()` and `last()`, which the compiler turns into opcodes that read the focus (`crates/xpath/src/compiler/mod.rs:169-179`).

It does not look where the focus changes: into a predicate, and into the steps of a path after the first, which evaluate against the items of the step before. An absolute path is independent, since it starts at the root of the tree whichever of its nodes is the context. A function name counts when it has no prefix or the prefix `fn`. The match lists every kind of expression, with no catch-all arm, so a new kind of expression needs a decision.

Consequences:

- `Wait Until Query` can skip the root for an absolute expression (decision 2): `count(//x)` evaluates without it, and `count(.//x)` inside it.
- The element lookup and `Set Root` get the same answer. A selector that reads its context only in such a place now counts as relative: `root()/x`, `id('a')`, `let $a := .//x return $a` or `if (exists(.//x)) then //a else //b`. The element lookup looks the root up for it, and `Set Root` drills into the current root instead of starting at the desktop. Both follow what the expression says. No suite or document of the repository passes such a selector (checked with grep).
- `Query` follows the same rule, as the maintainer chose on 2026-09-30: it evaluates an absolute expression without the root, and a root that cannot be found names the expression it kept from being evaluated (`BareMetal.root_for`). Before, it looked the root up for every expression, against the scenario *An absolute selector does not resolve the root* of `baremetal-selector-resolution`.

*Alternatives rejected:*

- **Looking the root up for every expression of `Wait Until Query`.** It keeps today's behavior, but an absolute expression then fails when the root is gone, and `Wait Until Query` and the element lookup answer the same question differently. The maintainer chose the classifier on 2026-09-29, in this change.
- **A second classifier only for `Wait Until Query`.** Two answers to one question: `Set Root` and the element lookup would keep the wrong one.
- **Reading the list of context functions from the function registry.** The registry records no such property. The list follows the implementations, and the tests name every entry.

## Risks / Trade-offs

- **[Every keyword's element lookup moves into the helper]** → The helper keeps each current behavior that the spec does not change: the snapshot policy, the root settings, the checks before the first attempt, the success values. The whole mock lane, the pytest suite and the egui lanes run before any commit.
- **[One provider call per retry to check the root]** → Only retries pay it, and `retry_interval` bounds it. A keyword whose element is present at once costs what it costs today.
- **[A root that goes away ends `Wait Until Gone` with `RootNotFoundError`]** → Intended (decision 2). The keyword documentation already says so, and the documentation update names the remedy: wait on the container itself.
- **[Suites whose expected-error patterns end in `seconds.` under `ignore_exceptions`, or match "still present"]** → The proposal marks these message changes as breaking. The error types stay.
- **[A provider whose `is_valid()` answers `False` for a live element now fails `Wait Until Query` with an explicit `root`]** → The same check already guards captured targets and pinned roots. No provider is known to do this.
- **[`ErrorDetails(error)` is checked only in RF 7.5; the project supports RF ≥ 7.0]** → A task checks the 7.0 constructor. If it differs, the loop formats the error inside its `except` with `robot.utils.get_error_message()`, which reads the error being handled.
- **[A long last error makes a long message]** → Robot Framework's `Wait Until Keyword Succeeds` quotes errors the same way.

## Migration Plan

- **Behavioral, not additive.** No keyword signature and no error class changes. Behavior changes:
  - failure messages under `ignore_exceptions`;
  - every wait checks its root on every attempt: it follows a replaced root, and a root that goes away ends it with the root's error;
  - `Wait Until Gone` says "could not be confirmed gone" when its last attempt raised;
  - `Wait Until Attribute Value` reports its last attempt, not an older completed one, when that attempt raised;
  - a missing root ends `Wait Until Gone` and `Wait Until Attribute Value` with `RootNotFoundError`, also under `ignore_exceptions`;
  - `Wait Until Query`:
    - names its last result;
    - needs no root for an absolute expression;
    - checks its `root`;
    - reports a failing operator wait as `AssertionError` or `TypeError` with the timeout, where the raw error used to escape;
  - `Wait Until Gone` names a captured element in the one form;
  - the element lookup and `Set Root` treat a selector that reads its context inside a function argument, a condition or a binding as relative (decision 12).

  The proposal lists which of these are breaking.
- **A native rebuild.** The classifier of decision 12 is Rust; the binding's signature does not change. The helper uses existing binding API: `Runtime.evaluate_single`, `is_context_dependent` and `clear_cache`, and `UiNode.is_valid`, `invalidate`, `attribute`, `describe` and `owner_id`. The `just` recipes build the native variant they need.
- **Sequence:**
  1. the tests: mock suites, the pytest module, the acceptance test;
  2. the helper and the element lookup;
  3. `Wait Until Gone`;
  4. the classifier (Rust);
  5. `Wait Until Query`;
  6. `Wait Until Attribute Value`;
  7. the documentation;
  8. the verification lanes.
- **Rollback:** revert the commits. Nothing is persisted and no format changes, and the tests revert together with the code. After the classifier's commit is reverted, the native module has to be rebuilt.
- **Bookkeeping at archive:**
  - The Purpose paragraph of `openspec/specs/baremetal-waiting/spec.md` names only the "two supporting rules". It is outside the delta and is extended by hand to name the shared polling rule.
  - The three triage entries get a `Status:` line saying this change implements them.

## Open Questions

- **A DEBUG record for swallowed errors.** Should a later change add one, as the absorbed A2 entries proposed? It would add a record and leave the rules of this change as they are (decision 9).
- **`Wait Until Query`'s truthy type.** Should its truthy timeout later become an `AssertionError`, like `Wait Until Attribute Value`'s? The maintainer kept the types for this change.
- **The rationale for `ignore_exceptions`** (answered 2026-09-30). The query-settings section explained it with provider errors: "a node disappears in the middle of a traversal, or the accessibility bridge returns a transient error". But the element interface has no error path (`crates/core/src/ui/node.rs:50-108`), so what reaches a wait as an error is an XPath error. The review reworded it with an XPath example that raises until the application has settled: `xs:integer(@Name)` on a label that does not show a number yet (checked on the mock, `FORG0001`).
