# Tasks

No native rebuild is needed: the change is Python only and uses existing binding API (design.md, Migration Plan). The `just` recipes build the native variant they need themselves. Run ad-hoc Python and robotcode commands with `uv run --no-sync`. A plain `uv run` syncs the environment and can replace the native build. Robot Framework glob patterns treat `[…]` as a character class, so expected-error patterns below leave out bracketed selector text and match around it with `*`.

## 1. Robot Framework mock tests first

Follow the `robot-test-style` skill. The suites keep their 0.2 s import timeout and assert failures with `Run Keyword And Expect Error` against the user-facing message.

- [ ] 1.1 In `tests/BareMetal/wait_keywords.robot`, change the tests that pin today's messages. They become the tests of the MODIFIED scenarios:
  - `:73-75` "Wait Until Gone With Ignore Exceptions Never Reports Gone" expects `*could not be confirmed gone within timeout of 0.2 seconds. The last error was: EvaluationError: *`;
  - `:133-135` "Wait Until Query With Ignore Exceptions Times Out On A Bad Expression" expects `*did not become truthy within timeout of 0.2 seconds. The last error was: EvaluationError: *`;
  - `:199-202` "Wait Until Attribute Value With Ignore Exceptions Never Succeeds On A Bad Selector" expects `*No element matched*within timeout of 0.2 seconds. The last error was: EvaluationError: *`;
  - tighten `:57-59` to `*Captured element Window "Operations Console" was still valid within timeout of 0.2 seconds.`, `:83-85` to `*was 0 and did not become truthy within timeout of 0.2 seconds.`, and `:87-91` to `*was False and did not become truthy within timeout of 0.2 seconds.`

  Verify: `just test-baremetal --suite '*.WaitKeywords' tests/BareMetal` runs, these six tests fail on the old messages, and every other test of the suite passes. Judge the run with `uv run --no-sync robotcode results summary --failed`.
- [ ] 1.2 Add tests to `tests/BareMetal/wait_keywords.robot`, one per mock-verifiable scenario of the spec delta. Add `${MISSING_ROOT}` (`//control:Window[@Name="NoSuchWindow"]`) and `${MISSING_INSIDE_ROOT}` (`.//control:Button[@Name="NoSuchButton"]`) to its variables, as in `query_settings.robot`. Set roots with `Set Root … scope=TEST`. The tests:
  - `Wait Until Exists    //control:Window[broken    query_overrides={'ignore_exceptions': True}` fails with `*No element matched*within timeout of 0.2 seconds. The last error was: EvaluationError: *`;
  - `Wait Until Exists    count(//control:Window)    query_overrides={'ignore_exceptions': True}` fails with `*did not return an element*`. This one is already green and documents the rule;
  - under the missing root, `Wait Until Gone    ${MISSING_INSIDE_ROOT}    query_overrides={'timeout': 1, 'ignore_exceptions': True}` fails with `RootNotFoundError: *NoSuchWindow*within timeout of 0.2 seconds; *NoSuchButton*was not evaluated.`;
  - the same for `Wait Until Attribute Value    ${MISSING_INSIDE_ROOT}    Name    query_overrides={'timeout': 1, 'ignore_exceptions': True}`;
  - under the missing root, `Wait Until Query    count(.//control:Button)` fails with `RootNotFoundError: *NoSuchWindow*within timeout of 0.2 seconds; 'count(.//control:Button)' was not evaluated.`;
  - under the missing root, `Wait Until Query    count(//control:Window)    >    ${0}` returns a count above 0;
  - `Wait Until Query    ${MISSING}/@Name` fails with `*matched nothing and did not become truthy within timeout of 0.2 seconds.`;
  - `Wait Until Query    count(//control:Window[broken    >    ${0}    query_overrides={'ignore_exceptions': True}` fails with `*did not satisfy the assertion within timeout of 0.2 seconds. The last error was: EvaluationError: *`;
  - `Wait Until Query    ${OPS_NAME}    >    ${5}` fails with `*not supported between*within timeout of 0.2 seconds*`;
  - `Wait Until Query    ${OPS_NAME}    validate    valu == 'x'    query_overrides={'ignore_exceptions': True}` fails with `*was 'Operations Console' and could not be checked within timeout of 0.2 seconds. The last error was: Evaluating expression*`;
  - `Wait Until Attribute Value    ${OPS}    IsMaximized    validate    valu == True    query_overrides={'ignore_exceptions': True}` fails with `*'IsMaximized' of Window "Operations Console" was False and could not be checked within timeout of 0.2 seconds. The last error was: Evaluating expression*`.

  Name the shared polling rule in the suite's `Documentation`. Verify: the suite runs as in 1.1. The value-selector test passes and every other new test fails, for the reason its expectation names.
- [ ] 1.3 In `tests/BareMetal/query_settings.robot`, tighten "Ignore Exceptions Keeps Retrying On Errors" (`:86-92`), the spec's scenario for the wait of a keyword that reads an element. It expects `*No element matched*within timeout of 0.2 seconds. The last error was: EvaluationError: *` and says so in its documentation. Verify: `just test-baremetal --suite '*.QuerySettings' tests/BareMetal` fails only this test.
- [ ] 1.4 In `tests/BareMetal/library_instance_isolation.robot`, add "A Node From Another Import Is Rejected As A Wait Until Query Root": an element from `A.Query    ${OPS}    only_first=${True}` passed as `root=` to `B.Wait Until Query    count(.//item:ListItem)` fails with `*different library instance*`. Verify: `just test-baremetal --suite '*.LibraryInstanceIsolation' tests/BareMetal` fails only this test, because `B` evaluates today instead of rejecting the element.

## 2. Acceptance test first

- [ ] 2.1 In `tests/acceptance/egui/app_root_after_exit.robot`, add "A Root Of An Ended Application Is Not Swallowed By Ignore Exceptions". It mirrors "A Root Pinned To An Ended Application Is Looked Up Again" (`:20-31`), with a title and app id of its own:
  1. launch the app;
  2. `BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=LOCAL`;
  3. resolve the root with `BM.Wait Until Exists    ./(Frame|Window)`;
  4. end the app;
  5. set `BM.Set Query Settings    {'timeout': 2}    scope=LOCAL`;
  6. expect `STARTS:RootNotFoundError: The root set by Set Root, '/app:Application[@ProcessId=${pid}]', was not found` from `BM.Wait Until Gone    ./(Frame|Window)    query_overrides={'timeout': 10, 'ignore_exceptions': True}`.

  Keep the teardown that ends the app. Update the suite's `Documentation` to name `ignore_exceptions`. Verify: `uv run --no-sync robotcode analyze code tests/acceptance/egui` reports nothing new (clear a stale cache with `robotcode analyze cache clear`). On a Linux lane (`just headless=true test-acceptance-x11 --suite '*.Egui.AppRootAfterExit'`) the test fails before the change, with `ElementStillPresentError` after about 10 s.

## 3. Unit tests first

- [ ] 3.1 Add `tests/PlatynUI/test_baremetal_wait_loop.py`, built like `tests/PlatynUI/test_baremetal_root_reuse.py`:
  - a counting fake runtime whose `evaluate_single` answers by query *and* context;
  - fake elements as `MagicMock(spec=UiNode)`, with `owner_id`, `is_valid` and `describe` set;
  - the root handed to the library by monkeypatching the `BareMetal.root` property to resolve a root binding, since there is no Robot Framework context.

  One test per scenario that the static mock cannot show:
  - an attempt that completes discards the remembered error: the first evaluation raises, later ones match nothing, the message has no last error. It is already green;
  - a root replaced during the wait is followed: the root's first element becomes invalid, the next lookup finds a second one, and the target is found under it;
  - a root that goes away for good ends `Wait Until Exists` and `Wait Until Gone` with `RootNotFoundError`, with `ignore_exceptions` on for the call;
  - a pinned root (`UiNodeDescriptor(node, None, is_root_binding=True)`) that stops being valid raises `PinnedElementGoneError` on the next attempt;
  - an explicit `root` of `Wait Until Query` that stops being valid raises `PinnedElementGoneError`;
  - an attribute read that raises a named test exception on every attempt ends `Wait Until Attribute Value` with `AttributeNotFoundError` `… could not be read … The last error was: <Name>: …`;
  - the final exception's `__cause__` is the last swallowed error;
  - `Wait Until Query` with `>` and a first evaluation that sleeps past a 0.05 s timeout evaluates exactly once.

  Verify: `just build-native-mock`, then `uv run --no-sync pytest tests/PlatynUI/test_baremetal_wait_loop.py tests/PlatynUI/test_baremetal_root_reuse.py -v`. The existing root-reuse tests and the first test pass, and every other new test fails.

## 4. Implementation

- [ ] 4.1 Check whether `robot.utils.ErrorDetails` takes the error as its first argument at the project's floor, Robot Framework 7.0: `uv run --no-project --with 'robotframework==7.0' python -c "import inspect; from robot.utils import ErrorDetails; print(inspect.signature(ErrorDetails))"`. Record the result here. If it does not, the loop formats the error inside its `except` with `robot.utils.get_error_message()` (design.md, Risks). Verify: the result and the chosen formatting are recorded here.
- [ ] 4.2 In `src/PlatynUI/BareMetal/__init__.py`, add the shared polling helper (design decisions 1, 3, 4, 6 and 7):
  - the module-level tuple of errors that waiting cannot fix;
  - the formatting of the last error;
  - an optional, keyword-only last error on `RootNotFoundError`, appended after `… was not evaluated.`;
  - the `QuerySettings` docstring (`:143-148`), which names a `UiNodeDescriptor.__call__` that no longer exists; it now names the helper.

  Move `UiNodeDescriptor.resolve` onto the helper, for targets and for roots:
  - the context step looks the root up on every attempt, outside the swallowing `try`, only when `needs_root` says so;
  - a `RootNotFoundError` is re-raised with the target named and the root's last error carried along;
  - the snapshot is discarded only after an attempt that found nothing;
  - the captured-target checks stay before the loop.

  Verify:
  - the pytest tests of 3.1 for the element lookup pass, and `test_baremetal_root_reuse.py` stays green;
  - the `Wait Until Exists` tests of 1.2 and the test of 1.3 pass;
  - `just test-baremetal --suite '*.QuerySettings' --suite '*.SelectorResolution' --suite '*.SetRootScope' --suite '*.ScopeLadder' tests/BareMetal` is green.
- [ ] 4.3 Move `Wait Until Gone` onto the helper:
  - the root step comes out of the `try` (`:1739`);
  - a reference with neither a selector nor an element raises `NoQueryError` before the first attempt;
  - its failure builder gives "still present", "still valid" and "could not be confirmed gone", with the captured element described in the one form (`_element_text`).

  Verify: the `Wait Until Gone` tests of 1.1 and 1.2 and the `Wait Until Gone` root test of 3.1 pass. The existing `Wait Until Gone` tests of `wait_keywords.robot` and `library_instance_isolation.robot` stay green.
- [ ] 4.4 Move `Wait Until Query` onto the helper:
  - the expression goes through the same `needs_root` memo (`descriptor_from_query`);
  - an explicit `root` is checked with `require_own_node` before the first attempt and for validity on every attempt;
  - the loop counts `AssertionError` and `TypeError` from the check as "not yet", and treats a `TypeError` raised by the evaluation as an error;
  - its failure builder gives the truthy messages (result named, "matched nothing"), the mismatch, the comparison error with expression and timeout, and the raising attempts, as in design decision 5;
  - the evaluation after the deadline (`:1874-1885`) goes away.

  Verify: the `Wait Until Query` tests of 1.1, 1.2 and 1.4 and the pytest tests of 3.1 for the explicit root and the single evaluation pass. The existing `Wait Until Query` tests stay green.
- [ ] 4.5 Move `Wait Until Attribute Value` onto the helper:
  - the root step comes out of the `try` (`:1980`);
  - the attempt records how far it got: no element, the element, the value;
  - `_attribute_wait_error` gains the kinds for a raising attempt. The element was not found gives `ElementNotFoundError`. The attribute could not be read gives `AttributeNotFoundError`. The value could not be checked gives `AssertionError`, naming the value. Each ends with the last error.

  Verify: the `Wait Until Attribute Value` tests of 1.1 and 1.2 and the attribute-read test of 3.1 pass, and the existing ones stay green.
- [ ] 4.6 Update the documentation in `src/PlatynUI/BareMetal/__init__.py` (design decision 10):
  - "Waiting for elements" and its query-settings text (`:970-1012`): which errors are never waited out, that the root is checked on every attempt, and that a failure quotes the last swallowed error;
  - the `ignore_exceptions` row of the settings table (`:995`);
  - the subtlety paragraph of "Tuning the wait" (`:1041-1043`);
  - one sentence on the failure in each wait keyword's docstring. `Wait Until Query`'s also says that an absolute expression does not use the root and that `root` must come from the same import.

  Keep the user-facing voice, with no platform internals. Verify: `uv run --no-sync python -m robot.libdoc PlatynUI.BareMetal show "Wait Until Gone"` renders, and likewise for the other three wait keywords. A generated HTML libdoc resolves the new links. `just check` passes.

## 5. Bookkeeping

- [ ] 5.1 Extend the Purpose paragraph of `openspec/specs/baremetal-waiting/spec.md` (outside the delta; its "two supporting rules") to name the shared polling rule. Verify: `openspec validate uniform-wait-keywords --strict` passes.
- [ ] 5.2 Add a `Status:` line saying "implemented by `uniform-wait-keywords`" to the three absorbed entries in `openspec/changes/archive/2026-09-27-logging-concept/review-findings.md` (`:721`, `:751`, `:812`). Say that the DEBUG record the two A2 entries proposed is not added (design decision 9). Verify: each of the three entries carries the line.

## 6. Verification

- [ ] 6.1 Run `just check` (fmt, clippy, ruff, mypy) and verify that it is clean. Run it before the recipes below, because a plain `uv run` inside it can replace the native build, and those recipes rebuild the variant they need.
- [ ] 6.2 Run `just test-python` and verify that it is green, the new `test_baremetal_wait_loop.py` included.
- [ ] 6.3 Run the whole mock lane with `just test-baremetal` and verify with `uv run --no-sync robotcode results summary --failed` that it is green. The loop behind every keyword's element lookup changed, so no suite is skipped.
- [ ] 6.4 Run both Linux acceptance lanes in full, with `just headless=true test-acceptance-compositor` and `just headless=true test-acceptance-x11`, because every keyword's element lookup changed. Verify with `uv run --no-sync robotcode results summary --failed` that both are green, 2.1 included. Record the outcome here.
- [ ] 6.5 On a real Windows machine (Wine does not count), run `just test-acceptance-windows`. At the least run the egui suites `*.Egui.AppRootAfterExit` and `*.Egui.Wait` with the recipe's build steps and `uv run --no-sync robotcode --profile real-windows run --suite '*.Egui.AppRootAfterExit' --suite '*.Egui.Wait'`, with `PLATYNUI_TEST_APP_BIN` set. Verify that they are green: 2.1 checks that UI Automation's application node ends the wait with `RootNotFoundError`. Record the outcome here.

## 7. Commit (only when the user asks)

- [ ] 7.1 Commit in reviewable steps. Each step carries the tests it turns green, and builds, passes lint (`just check` covers the whole project) and passes its tests on its own:
  - the helper and the element lookup, with 1.3, the `Wait Until Exists` tests of 1.2, and their unit tests;
  - `Wait Until Gone`, with its tests;
  - `Wait Until Query`, with its tests and 1.4;
  - `Wait Until Attribute Value`, with its tests;
  - the acceptance test 2.1;
  - the documentation;
  - the OpenSpec bookkeeping of section 5.

  Use Conventional Commits, subjects ≤ 72 characters, no `!`. The bodies of the behavior commits list the behavior changes of the proposal that they bring. Do not push.
