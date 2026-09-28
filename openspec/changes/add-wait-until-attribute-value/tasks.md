# Tasks

No native rebuild is needed: the keyword uses existing binding API (design.md, Migration Plan). The `just` recipes below build the native variant they need themselves.

## 1. Mock tests first

- [ ] 1.1 Add a `Wait Until Attribute Value` section to `tests/BareMetal/wait_keywords.robot`, with one test per mock-verifiable scenario of the spec delta. Name the fourth keyword in the suite's `Documentation`. The tests reuse the suite's `${OPS}` window and its 0.2 s import timeout, and assert failures with `Run Keyword And Expect Error` against the user-facing message. The scenarios:
  - value already satisfied: `IsMaximized    ==    ${False}` returns a boolean equal to what `Get Attribute` reads;
  - truthy default returns `Name` at once;
  - truthy default on `IsMaximized` times out with `*did not become truthy*within timeout of 0.2 seconds*`;
  - `matches    (Operations) (Console)` returns the string, not the groups;
  - `validate    value == True` times out;
  - `then` is rejected;
  - `==    Wrong Name` times out with AssertionEngine's diagnostic;
  - a selector that matches nothing fails with `*No element matched*within timeout of 0.2 seconds*`;
  - a missing attribute times out naming the attribute. Pick one the window does not expose, confirmed with `Query` first; the spec's example is `ToggleState`;
  - `nosuch:Name` fails with a message naming the prefix, not a timeout message;
  - `count(//control:Window)` and `…/@Name` fail with `*Use Wait Until Query*`;
  - `query_overrides={'timeout': 0.6}` is reported as `0.6`;
  - a broken selector with `ignore_exceptions` ends in `*No element matched*`;
  - `Name    >    ${5}` keeps waiting and times out with the comparison error.

  Follow the robot-test-style skill. Verify: `just test-baremetal --suite '*.WaitKeywords' tests/BareMetal` runs; the existing tests pass, and every new test fails only because the keyword does not exist yet.
- [ ] 1.2 Add a test to `tests/BareMetal/library_instance_isolation.robot`: an element captured through import `A` and passed to `B.Wait Until Attribute Value` fails with `*different library instance*`. It mirrors the existing `Wait Until Gone` case. Verify: the suite runs with the new test failing only on the missing keyword.

## 2. Acceptance tests first

- [ ] 2.1 Add three tests to `tests/acceptance/egui/wait.robot` and name the keyword in the suite's `Documentation`:
  - **Selector:** read the count with `Get Click Count`, click `.//*[@Id="btn-click-me"]`, then `BM.Wait Until Attribute Value    .//*[@Id="status-clicks"]    Name    ==    Clicks: ${{ $before + 1 }}`. Assert that the returned value equals the expected text.
  - **Captured label:** capture the label with `BM.Query    .//*[@Id="status-clicks"]    only_first=${True}`, click, then wait on the captured element with the same condition and assert the returned value.
  - **Captured item that goes away:** open the File menu, capture `.//*[@Id="menu-file-new"]` with `Wait Until Exists`, close the menu with `<Escape>`, and confirm the item is gone with `BM.Wait Until Gone    ${item}`. Then expect `*no longer available*` from `BM.Wait Until Attribute Value    ${item}    Name    query_overrides={'timeout': 30}`. A keyword that waited instead would take 30 s and fail with a different message.

  Verify: `uv run --no-sync robotcode analyze code tests/acceptance/egui` reports only the not-yet-existing keyword. If it reports stale results, clear its cache with `robotcode analyze cache clear`.

## 3. Implementation

- [ ] 3.1 In `src/PlatynUI/BareMetal/__init__.py`, add `wait_until_attribute_value(self, descriptor, attribute_name, assertion_operator=None, assertion_expected=None, assertion_message=None, *, query_overrides=None)` next to the other wait keywords, without `@assertable` (design decision 3). Implement the up-front usage checks from design decision 5:
  - reject `then`/`evaluate` with the message `Wait Until Query` uses;
  - check the prefix against the four `Namespace` values (`as_str()`), and raise an error that names the unknown prefix;
  - run `require_own_node` for a captured element before the loop.

  Verify: the mock tests for `then`, the unknown prefix and the foreign import (1.2) pass.
- [ ] 3.2 Implement the loop from design decisions 3, 4 and 6. Per attempt:
  - **Selector:** clear the cache, resolve the root only when `needs_root` says so, and evaluate once. A non-element result raises `ResultTypeError` pointing to `Wait Until Query`.
  - **Captured element:** call `invalidate()`, then `is_valid()`, and raise `PinnedElementGoneError` when it is no longer valid.
  - **Read:** use `node.attribute(name, namespace)`. `AttributeNotFoundError` means "not yet".
  - **Check:** truthiness, or `verify_assertion`. `AssertionError` and `TypeError` mean "not yet"; `RuntimeError` surfaces at once.
  - **Errors:** apply `ignore_exceptions` exactly as the other wait keywords do.
  - **Result:** return the value read in the successful attempt, never `verify_assertion`'s result.

  One deadline from `query_overrides` over the effective query settings bounds the loop. Verify: the mock tests for an already-satisfied value, the truthy return, `matches`, the value selector and the swallowed errors pass.
- [ ] 3.3 Implement failure reporting from the last attempt that did not raise, following the table in design decision 8. Describe the element with a guarded `node.describe()` that falls back to the runtime id. Let the default AssertionEngine message prefix name the attribute and the element, and let `assertion_message` replace it. Verify: the remaining mock tests from 1.1 pass (every timeout case, the per-call timeout, the comparison error).
- [ ] 3.4 Write the keyword's docstring in the style of the other wait keywords: what it waits for, why it exists next to `Get Attribute`, the truthy default, that `then` is rejected, that it returns the value (also for `matches`), and Args/Returns/Examples with a selector and a captured element. Verify: `uv run --no-sync python -m robot.libdoc PlatynUI.BareMetal show "Wait Until Attribute Value"` renders the documentation, and its links (`Get Attribute`, `Wait Until Query`, `Tuning the wait`) resolve in a generated HTML libdoc.

## 4. Documentation and guidance

- [ ] 4.1 In the library documentation (`src/PlatynUI/BareMetal/__init__.py`):
  - The `Get Attribute` docstring says it checks the value once and points to `Wait Until Attribute Value` for waiting on a value; "Reading and checking values" says the same.
  - "Waiting explicitly" lists four keywords instead of three and gains an example line.
  - The sentence under "Elements and attributes" that lists which keywords read the UI again on every attempt names the new keyword.

  Verify with libdoc as in 3.4.
- [ ] 4.2 Name the new keyword in `dev-docs/architecture.md` §9.3, in the sentence listing the BareMetal keywords that clear the cache on every attempt. In `dev-docs/testing-strategy.md` §2.6, attribute effects are now awaited with `Wait Until Attribute Value`, and the note that `Get Attribute    ==` checks its value only once stays. Verify: both documents name the keyword where they list the wait keywords.
- [ ] 4.3 Update `.claude/skills/robot-test-style/SKILL.md`:
  - The waiting-table row "Attribute reaches a value" becomes `BM.Wait Until Attribute Value    <loc>    <attr>    ==    <value>`; a captured element is passed directly, with no `root=`.
  - The paragraph below the table names it for asynchronous effects.

  Verify: the table and the paragraph no longer present `Wait Until Query …/@Attr` as the way to await an attribute, and `Wait Until Query` remains the row for computed conditions.
- [ ] 4.4 Extend the Purpose paragraph of `openspec/specs/baremetal-waiting/spec.md` (outside the delta) to name `Wait Until Attribute Value` next to the other three keywords. Verify: `openspec validate add-wait-until-attribute-value --strict` passes.

## 5. Verification

- [ ] 5.1 Run `just check` (fmt, clippy, ruff, mypy) and verify that it is clean. Run it before the RF recipes below: a plain `uv run` can replace the native build, and the recipes rebuild the variant they need.
- [ ] 5.2 Run the whole mock lane with `just test-baremetal` and verify that it is green, the new tests included. Judge the run with `uv run --no-sync robotcode results summary --failed`, not by the exit code.
- [ ] 5.3 Run the egui wait suite on the Linux acceptance lanes with `just headless=true test-acceptance-x11 --suite '*.Egui.Wait'` and `just headless=true test-acceptance-compositor --suite '*.Egui.Wait'`, and verify that both are green, the three new tests included.
- [ ] 5.4 On a real Windows machine (Wine does not count), run `just test-acceptance-windows --suite '*.Egui.Wait'` and verify that it is green. The captured-label test checks that UI Automation reports a fresh value after `invalidate()` (design.md, Risks). Keep this task open until it has been checked on real Windows.
