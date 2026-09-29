## Why

`Get Attribute` reads one attribute of one element and can check it once. That is right for a value that has already settled, but real applications change state asynchronously: a window finishes maximizing, a status label updates after a click. A one-shot check right after the action then fails within milliseconds. Today the only way to wait for an attribute value is `Wait Until Query    <selector>/@Attr    ==    <value>`, the idiom behind all 49 attribute waits in the acceptance suites. It works, but it spells the attribute as an XPath step (`@IsMaximized`) instead of the bare name `Get Attribute` takes, it builds the expression by appending to the selector string, a captured element needs `./@Attr` plus `root=${element}`, and a missing attribute behaves differently from `Get Attribute`.

Reading a value and waiting for a value are different actions with different results, so the wait gets its own keyword. `Get Attribute` stays a keyword that reads and, if asked, checks once.

## What Changes

- Add **`Wait Until Attribute Value`** to `PlatynUI.BareMetal`. It waits until one attribute of one element satisfies a condition, then returns the attribute's value at the moment the condition held.
  - It takes the element and the attribute the way `Get Attribute` does (a selector or a captured element; the attribute name bare, or with its namespace prefix such as `native:`), and the assertion arguments the way `Wait Until Query` does (`assertion_operator`, `assertion_expected`, `assertion_message`). `validate` works; `then`/`evaluate` are rejected because they transform instead of assert. Without an operator it waits until the value is truthy.
  - The effective query settings govern it, and `query_overrides` tunes one call, like the other `Wait Until …` keywords. One deadline covers both the element appearing and the value being reached.
  - A selector is evaluated again on every attempt. A captured element stays the same element; it is refreshed before each read and fails at once when it is no longer valid.
  - An attribute the found element does not have counts as "not yet", not as an error. When the timeout elapses, the failure reports the last attempt: the element was never found, the attribute was missing (naming attribute and element), or AssertionEngine's actual-versus-expected diagnostic.
  - It always returns the value itself, also for `matches` with capture groups, where AssertionEngine (and therefore `Wait Until Query`) returns the groups.
- `Get Attribute` keeps its behavior. Its documentation, and the library documentation's section on reading and checking values, point to the new keyword for waiting.
- The library documentation names the new keyword under "Waiting explicitly" and where it lists the keywords that read the UI again on every attempt.
- The contributor guidance that prescribes `Wait Until Query …/@Attr` for attribute effects (the robot-test-style skill, `dev-docs/testing-strategy.md` §2.6) and `dev-docs/architecture.md` §9.3 name the new keyword.

Not part of this change: converting the existing `Wait Until Query …/@Attr` waits in the acceptance suites. They remain correct.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `baremetal-waiting`: adds a requirement for `Wait Until Attribute Value`, the fourth explicit wait keyword. The existing requirements do not change.

## Impact

- **Affected specs:** `baremetal-waiting` (one added requirement). Its Purpose paragraph names the three existing keywords; it is outside the delta and is updated by hand.
- **Python / Robot Framework:** `src/PlatynUI/BareMetal/__init__.py` gains the keyword, plus documentation changes to `Get Attribute` and to the library introduction ("Elements and attributes", "Reading and checking values", "Waiting explicitly"). It reuses the query settings, the element reference (`UiNodeDescriptor`) and the ownership check (`require_own_node`) as they are. No other keyword changes behavior.
- **Rust / native binding:** none. The keyword uses existing binding API (`UiNode.attribute`, `UiNode.invalidate`, `UiNode.is_valid`, `Runtime.evaluate_single`, `Runtime.clear_cache`), so no native rebuild is needed for it.
- **Tests:** mock coverage in `tests/BareMetal/wait_keywords.robot`, the foreign-element case in `tests/BareMetal/library_instance_isolation.robot`, and real-provider coverage in `tests/acceptance/egui/wait.robot` for a value that arrives after the call started, through a selector and through a captured element. Only a real provider changes state while a keyword waits; the mock does not.
- **Platforms and providers:** the keyword is provider-independent and works wherever a provider exposes the attribute. The egui wait suite carries no platform tag, so every acceptance lane that runs the egui suites verifies it.
- **Compatibility:** additive. Nothing existing changes behavior; nothing is **BREAKING**.
