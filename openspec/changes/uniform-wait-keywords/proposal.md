# Proposal

## Why

`PlatynUI.BareMetal` waits in four places, and each place was written on its own. They are the element lookup behind `Wait Until Exists` and every keyword that takes an element, `Wait Until Gone`, `Wait Until Query` and `Wait Until Attribute Value`. The four loops disagree on three questions: when a `Set Root` root is looked up, which errors `ignore_exceptions` may swallow, and what the failure reports once the timeout has elapsed. The disagreement produces wrong results:

- **A missing root is swallowed.** `Wait Until Gone` and `Wait Until Attribute Value` look the root up inside the code that swallows errors. With `ignore_exceptions`, a root that is not found is retried on every attempt, and each attempt waits the root's own timeout. The wait then fails as if the target were at fault: `Wait Until Gone` says the element "was still present", and `Wait Until Attribute Value` says no element matched. `Wait Until Gone`'s documentation promises the opposite: "A `Set Root` root that itself vanishes surfaces as the root's own lookup error."
- **Presence is reported that nobody saw.** When every attempt of `Wait Until Gone` raised, it reports the element as still present, although no attempt ever found it.
- **The cause is lost.** Under `ignore_exceptions`, a wait that times out drops the error its attempts raised. A broken selector then reads like a missing element. `Wait Until Query`'s truthy timeout does not say what the query returned (`0`, `''`, `False`, nothing). With an operator, `Wait Until Query` evaluates once more after the deadline. That extra evaluation can pass and the keyword still fails, and under `ignore_exceptions` it lets the raw error escape.

The maintainer decided on 2026-09-29 that all waiting is implemented one way, and that the failure reports what the last attempt saw, including the last error when `ignore_exceptions` swallowed it.

## What Changes

- **One polling loop for every wait.** The element lookup (the explicit `Wait Until Exists` and the implicit wait of every action keyword and of `Get Attribute Value`), the lookup of a `Set Root` root, `Wait Until Gone`, `Wait Until Query` and `Wait Until Attribute Value` all go through one shared helper. Each keyword supplies only what one attempt does and how it describes the failure.
- **The root is looked up on every attempt, and its failure is never swallowed.** An attempt first looks up the root when its target needs one. A relative selector or expression needs it; an absolute one never does. The lookup reuses the element the root resolved to while that element is still valid. It runs with the root's own settings and outside the code that swallows errors. So a root that cannot be found ends the wait at once with `RootNotFoundError`, which names the root and the target, whatever `ignore_exceptions` says. A root that goes away during the wait is looked up again; if a replacement appears, the wait follows it. Two changes follow for `Wait Until Query`:
  - an absolute expression no longer needs the root;
  - its explicit `root` argument is checked like a root: an element from another import fails before the first attempt, and one that is no longer valid fails at once.
- **One list of errors that waiting cannot fix.** These surface at once in every loop, whatever `ignore_exceptions` says: `ResultTypeError`, `NoQueryError`, `ForeignNodeError`, `PinnedElementGoneError`, `RootNotFoundError`, and the `RuntimeError` AssertionEngine raises for an operator it does not know. `Wait Until Gone` is the exception for `PinnedElementGoneError`: for its captured target, a gone element is the success it waits for.
- **`ignore_exceptions` remembers the last error.** Every other error is swallowed until the timeout. The loop keeps the last one. An attempt that completes without raising clears it. A swallowed error never satisfies a wait.
- **The failure describes the last attempt.** If the last attempt completed, the failure says what it saw: no element, the element still there, or the result or value and why it did not satisfy the condition. `Wait Until Query`'s truthy timeout now names the last result. If the last attempt raised, the failure says how far that attempt got, followed by `The last error was: <Type>: <message>`, the form Robot Framework's own `Wait Until Keyword Succeeds` uses. `Wait Until Gone` then says the element could not be confirmed gone. The error *types* stay the ones each keyword raises today for that situation. The type never depends on which exception was swallowed.
- **Nothing is evaluated after the deadline.** `Wait Until Query` drops its extra evaluation and reports what the loop saw, as `Wait Until Attribute Value` already does.
- **Documentation.** The docs of the four wait keywords and the query-settings section on `ignore_exceptions` state these rules, in the library's user-facing voice.
- `Get Attribute Value` stays a one-shot read and check ("one keyword, one action"). Only its element lookup follows the new rules, like every keyword's.

Behavior changes users can see, for the release notes. Each situation keeps the error type it has today. Some failures now report a different situation, though: the root's failure instead of the target's, or the last attempt instead of an older one. Failure messages change as well.

- **BREAKING (message):** under `ignore_exceptions`, a wait that times out now ends its message with ` The last error was: …` instead of `… seconds.` An expected-error pattern anchored on the old ending no longer matches.
- **BREAKING (message):** under `ignore_exceptions`, `Wait Until Gone` now says "could not be confirmed gone" instead of "still present" when its last attempt raised.
- **BREAKING (behavior):** under `ignore_exceptions`, `Wait Until Attribute Value` now reports how far its last attempt got. When earlier attempts read a wrong value and the last one raised, it fails as not found or not read, with the last error, instead of with the older mismatch.
- **BREAKING (behavior):** under `ignore_exceptions`, a `Set Root` root that cannot be found now fails `Wait Until Gone` and `Wait Until Attribute Value` with `RootNotFoundError`, after the root's own timeout. Before, they failed with `ElementStillPresentError` or `ElementNotFoundError` after the per-call timeout.
- **BREAKING (behavior):** `Wait Until Query` changes in four ways:
  - it rejects an element from another import as its `root`;
  - it fails at once when its `root` is no longer valid;
  - with an operator, it fails with `AssertionError` or `TypeError` that carries the timeout, instead of letting a raw error escape after the deadline;
  - its truthy timeout names the last result.
- Every wait checks its `Set Root` root on every attempt, not once at its start. A root that is replaced during the wait (its window closes and reopens) is followed. A root that goes away ends the wait with the root's error, instead of the target's error after the whole timeout. An absolute `Wait Until Query` expression no longer needs the root at all.
- A captured element in `Wait Until Gone`'s messages is described as `Role "Name" #id`, the one element form of the `diagnostic-logging` spec, instead of the runtime id.

This change absorbs three entries of the logging triage in `openspec/changes/archive/2026-09-27-logging-concept/review-findings.md`:

- `:721`, where the element lookup loses the cause under `ignore_exceptions`;
- `:812`, where `Wait Until Gone` loses the cause and reports "still present";
- `:751`, the B entry that makes the "did not become truthy" timeout name the last result or the last error.

The first two were marked "follow-up A2", and A2 was re-cut into this change. The DEBUG record for each swallowed error, which the two A2 entries also proposed, is not added (design.md, decision 9; Open Questions).

Not in scope:

- the pointer `ensure_move` double report in the runtime (`review-findings.md:18`), a separate small fix;
- the broad `except` in `Highlight` (`:761`), a separate item;
- `Query`, which is a snapshot and not a wait.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `baremetal-waiting`: an ADDED requirement states the one polling rule for every wait: attempts and the deadline, the root, the errors that waiting cannot fix, `ignore_exceptions`, and the failure that describes the last attempt. MODIFIED are the requirements for `Wait Until Gone`, its still-present error, `Wait Until Query` and `Wait Until Attribute Value`, where their text or scenarios pin the old root handling or the old failure messages. `Wait Until Exists` and the value-semantics requirement do not change.

## Impact

- **Python / Robot Framework:** `src/PlatynUI/BareMetal/__init__.py`:
  - a shared polling helper, with the list of errors that waiting cannot fix and the formatting of the last error;
  - `UiNodeDescriptor.resolve`, used for targets and for roots;
  - `RootNotFoundError`, which gains an optional last error;
  - `wait_until_gone`, `wait_until_query` and `wait_until_attribute_value`;
  - `_attribute_wait_error`, which now tells how far a raising attempt got;
  - the docstrings of the four wait keywords, and the sections "Waiting for elements" and "Tuning the wait".

  No keyword signature changes.
- **Rust / native binding:** none. The helper uses existing binding API (`Runtime.evaluate_single`, `is_context_dependent`, `clear_cache`, and `UiNode.is_valid`, `invalidate`, `attribute`, `describe` and `owner_id`). **No native rebuild.**
- **Tests:**
  - `tests/BareMetal/wait_keywords.robot`: three tests pin today's messages and change (`:73-75`, `:133-135`, `:199-202`); three more are tightened (`:57-59`, `:83-85`, `:87-91`); new tests cover the root rule, the last error and the named result.
  - `tests/BareMetal/query_settings.robot`: `:86-92` stays compatible and is tightened to the last error.
  - `tests/BareMetal/library_instance_isolation.robot`: a foreign `Wait Until Query` root.
  - A new pytest module next to `tests/PlatynUI/test_baremetal_root_reuse.py`, with a fake runtime. It covers what the static mock tree cannot show: a root that goes away during a wait, a completed attempt that clears the error, no evaluation after the deadline.
  - One test in `tests/acceptance/egui/app_root_after_exit.robot`: a root pinned to an ended application, under `ignore_exceptions`, on real providers.
  - `tests/PlatynUI/test_keyword_logging_rf.py:236-241` and the acceptance suites' `STARTS:RootNotFoundError` expectations stay compatible.
- **Platforms and providers:** provider-independent Python, so every platform behaves the same. The mock lane covers the rules. The egui acceptance test runs on every lane (X11, the compositor, Windows) and exercises each provider's `is_valid` for an application node.
- **Docs:** only the in-library documentation above. `dev-docs/architecture.md:766` describes when BareMetal discards the runtime's snapshot. That stays true, because the helper keeps each loop's current policy.
- **Bookkeeping:** the Purpose paragraph of `openspec/specs/baremetal-waiting/spec.md` (outside the delta), and the `Status:` lines of the three absorbed triage entries.
