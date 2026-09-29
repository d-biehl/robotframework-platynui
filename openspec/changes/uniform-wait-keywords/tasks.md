# Tasks

The classifier of 4.7 is Rust, so the native module has to be rebuilt after it; the `just` recipes build the native variant they need themselves (design.md, Migration Plan). Run ad-hoc Python and robotcode commands with `uv run --no-sync`. A plain `uv run` syncs the environment and can replace the native build. Robot Framework glob patterns treat `[…]` as a character class, so expected-error patterns below leave out bracketed selector text and match around it with `*`.

## 1. Robot Framework mock tests first

Follow the `robot-test-style` skill. The suites keep their 0.2 s import timeout and assert failures with `Run Keyword And Expect Error` against the user-facing message.

- [x] 1.1 In `tests/BareMetal/wait_keywords.robot`, change the tests that pin today's messages. They become the tests of the MODIFIED scenarios:
  - `:73-75` "Wait Until Gone With Ignore Exceptions Never Reports Gone" expects `*could not be confirmed gone within timeout of 0.2 seconds. The last error was: EvaluationError: *`;
  - `:133-135` "Wait Until Query With Ignore Exceptions Times Out On A Bad Expression" expects `*did not become truthy within timeout of 0.2 seconds. The last error was: EvaluationError: *`;
  - `:199-202` "Wait Until Attribute Value With Ignore Exceptions Never Succeeds On A Bad Selector" expects `*No element matched*within timeout of 0.2 seconds. The last error was: EvaluationError: *`;
  - tighten `:57-59` to `*Captured element Window "Operations Console" was still valid within timeout of 0.2 seconds.`, `:83-85` to `*was 0 and did not become truthy within timeout of 0.2 seconds.`, and `:87-91` to `*was False and did not become truthy within timeout of 0.2 seconds.`

  Verify: `just test-baremetal --suite '*.WaitKeywords' tests/BareMetal` runs, these six tests fail on the old messages, and every other test of the suite passes. Judge the run with `uv run --no-sync robotcode results summary --failed`. Result: the six tests fail on the old messages, and every other test that was in the suite before passes.
- [x] 1.2 Add tests to `tests/BareMetal/wait_keywords.robot`, one per mock-verifiable scenario of the spec delta. Add `${MISSING_ROOT}` (`//control:Window[@Name="NoSuchWindow"]`) and `${MISSING_INSIDE_ROOT}` (`.//control:Button[@Name="NoSuchButton"]`) to its variables, as in `query_settings.robot`. Set roots with `Set Root … scope=TEST`. The tests:
  - `Wait Until Exists    //control:Window[broken    query_overrides={'ignore_exceptions': True}` fails with `*No element matched*within timeout of 0.2 seconds. The last error was: EvaluationError: *`;
  - `Wait Until Exists    count(//control:Window)    query_overrides={'ignore_exceptions': True}` fails with `*did not return an element*`. This one is already green and documents the rule;
  - under the missing root, `Wait Until Gone    ${MISSING_INSIDE_ROOT}    query_overrides={'timeout': 1, 'ignore_exceptions': True}` fails with `RootNotFoundError: *NoSuchWindow*within timeout of 0.2 seconds; *NoSuchButton*was not evaluated.`;
  - the same for `Wait Until Attribute Value    ${MISSING_INSIDE_ROOT}    Name    query_overrides={'timeout': 1, 'ignore_exceptions': True}`;
  - under the missing root, `Wait Until Query    count(.//control:Button)` fails with `RootNotFoundError: *NoSuchWindow*within timeout of 0.2 seconds; 'count(.//control:Button)' was not evaluated.`;
  - under the missing root, `Wait Until Query    count(//control:Window)    >    ${0}` returns a count above 0;
  - under the root `${OPS}`, `Wait Until Query    count(.//item:ListItem)` returns 4, the list items of that window, not the 8 of the desktop. This one is already green and pins that a relative path inside a function's argument needs the root (design decision 12);
  - `Wait Until Query    ${MISSING}/@Name` fails with `*matched nothing and did not become truthy within timeout of 0.2 seconds.`;
  - `Wait Until Query    count(//control:Window[broken    >    ${0}    query_overrides={'ignore_exceptions': True}` fails with `*did not satisfy the assertion within timeout of 0.2 seconds. The last error was: EvaluationError: *`;
  - `Wait Until Query    ${OPS_NAME}    >    ${5}` fails with `*not supported between*within timeout of 0.2 seconds*`;
  - `Wait Until Query    ${OPS_NAME}    matches    (    query_overrides={'ignore_exceptions': True}` fails with `*was 'Operations Console' and could not be checked within timeout of 0.2 seconds. The last error was: *unterminated subpattern*`;
  - `Wait Until Query    ${OPS_NAME}    validate    valu == 'x'    query_overrides={'ignore_exceptions': True}` fails at once with `Evaluating expression*failed: NameError: *`, not with a timeout error. This one is already green and documents the rule;
  - `Wait Until Attribute Value    ${OPS}    Name    matches    (    query_overrides={'ignore_exceptions': True}` fails with a message that starts with `Attribute 'Name' of Window "Operations Console" was 'Operations Console'` and matches `*could not be checked within timeout of 0.2 seconds. The last error was: *missing ), unterminated subpattern*`;
  - `Wait Until Attribute Value    ${OPS}    IsMaximized    validate    valu == True    query_overrides={'ignore_exceptions': True}` fails at once with `Evaluating expression*failed: NameError: *`, not with a timeout error. This one is already green.

  Name the shared polling rule in the suite's `Documentation`. Verify: the suite runs as in 1.1. The value-selector test, the two `validate` tests and the computed-expression test pass, and every other new test fails, for the reason its expectation names. Result: the value-selector test and the two `validate` tests pass within milliseconds, and the other ten new tests fail, each for the reason its expectation names. The computed-expression test came with design decision 12, after this run; it passes on the code before the change, which looks the root up for every expression. The two `matches` tests show today's defects: `Wait Until Query` lets `re.error` escape after the deadline, and `Wait Until Attribute Value` reports "No element matched" for the element it found on every attempt.
- [x] 1.3 In `tests/BareMetal/query_settings.robot`, tighten "Ignore Exceptions Keeps Retrying On Errors" (`:86-92`), the spec's scenario for the wait of a keyword that reads an element. It expects `*No element matched*within timeout of 0.2 seconds. The last error was: EvaluationError: *` and says so in its documentation. Verify: `just test-baremetal --suite '*.QuerySettings' tests/BareMetal` fails only this test. Result: only this test of the suite fails, on the old message without the last error.
- [x] 1.4 In `tests/BareMetal/library_instance_isolation.robot`, add "A Node From Another Import Is Rejected As A Wait Until Query Root": an element from `A.Query    ${OPS}    only_first=${True}` passed as `root=` to `B.Wait Until Query    count(.//item:ListItem)` fails with `*different library instance*`. Verify: `just test-baremetal --suite '*.LibraryInstanceIsolation' tests/BareMetal` fails only this test, because `B` evaluates today instead of rejecting the element. Result: only this test of the suite fails: `B` evaluated against `A`'s element, and no error occurred.

## 2. Acceptance test first

- [x] 2.1 In `tests/acceptance/egui/app_root_after_exit.robot`, add "A Root Of An Ended Application Is Not Swallowed By Ignore Exceptions". It mirrors "A Root Pinned To An Ended Application Is Looked Up Again" (`:20-31`), with a title and app id of its own:
  1. launch the app;
  2. `BM.Set Root    /app:Application[@ProcessId=${pid}]    scope=LOCAL`;
  3. resolve the root with `BM.Wait Until Exists    ./(Frame|Window)`;
  4. end the app;
  5. set `BM.Set Query Settings    {'timeout': 2}    scope=LOCAL`;
  6. expect `STARTS:RootNotFoundError: The root set by Set Root, '/app:Application[@ProcessId=${pid}]', was not found` from `BM.Wait Until Gone    ./(Frame|Window)    query_overrides={'timeout': 10, 'ignore_exceptions': True}`.

  Keep the teardown that ends the app. Update the suite's `Documentation` to name `ignore_exceptions`. Verify: `uv run --no-sync robotcode analyze code tests/acceptance/egui` reports nothing new (clear a stale cache with `robotcode analyze cache clear`). On a Linux lane (`just headless=true test-acceptance-x11 --suite '*.Egui.AppRootAfterExit'`) the test fails before the change, with `ElementStillPresentError` after about 10 s. Result: `robotcode analyze` reports nothing in this suite (its five errors elsewhere are the known `%{VAR}` false positives). On X11 the test fails before the change with `ElementStillPresentError: Element matching query './(Frame|Window)' was still present within timeout of 10.0 seconds.`, and the suite's other two tests pass.

## 3. Unit tests first

- [x] 3.1 Add `tests/PlatynUI/test_baremetal_wait_loop.py`, built like `tests/PlatynUI/test_baremetal_root_reuse.py`:
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

  Verify: `just build-native-mock`, then `uv run --no-sync pytest tests/PlatynUI/test_baremetal_wait_loop.py tests/PlatynUI/test_baremetal_root_reuse.py -v`. The existing root-reuse tests and the first test pass, and every other new test fails. Result: 6 passed, 8 failed, each on the old behavior: the root looked up only once (a target not found, still present, or a falsy result after the per-call timeout), no last error, no cause, and a second evaluation after the deadline.

- [x] 3.2 In `crates/xpath/tests/it/parser_context_dependence.rs`, add a case for every kind of context read (design decision 12) and for every place where the focus changes: dependent are an operand, a comparison, a range, `instance of`, an `if` condition, a `for`, `let` or quantifier binding, a function argument, `root()/x`, and each function that reads the context when an argument is left out, `fn:name()` included; independent are `count(//x)`, `string-join(//x/@Name, ', ')`, a predicate, a later path step, a function given every argument, `true()`, `current-date()`, a variable and a literal. In `crates/runtime/src/xpath.rs`, add `count(//x)` (independent), `count(.//x)` and `name()` (dependent). Verify: `cargo nextest run -p platynui-xpath -p platynui-runtime -E 'test(context_dependen) | test(computed_value) | test(is_context_dependent)'`: every new dependent case fails, and every independent one passes. Result: 73 run, 39 passed, 34 failed: the 32 new dependent cases of the XPath crate and the two of the runtime crate. None failed to parse.

## 4. Implementation

- [x] 4.1 Check whether `robot.utils.ErrorDetails` takes the error as its first argument at the project's floor, Robot Framework 7.0: `uv run --no-project --with 'robotframework==7.0' python -c "import inspect; from robot.utils import ErrorDetails; print(inspect.signature(ErrorDetails))"`. Record the result here. If it does not, the loop formats the error inside its `except` with `robot.utils.get_error_message()` (design.md, Risks). Verify: the result and the chosen formatting are recorded here. Result: at 7.0 the signature is `(error=None, full_traceback=True, exclude_robot_traces=True)`, as at 7.5, and `ErrorDetails(EvaluationError('x')).message` is `EvaluationError: x`. The loop formats the last error with `ErrorDetails(error).message`.
- [x] 4.2 In `src/PlatynUI/BareMetal/__init__.py`, add the shared polling helper (design decisions 1, 3, 4, 6 and 7):
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
  - `just test-baremetal --suite '*.QuerySettings' --suite '*.SelectorResolution' --suite '*.SetRootScope' --suite '*.ScopeLadder' tests/BareMetal` is green. Result: the five element-lookup tests of 3.1 pass and `test_baremetal_root_reuse.py` stays green; the four suites are green (22, 7, 5 and 18 tests), and so are the `Wait Until Exists` tests of 1.2 and the test of 1.3.
- [x] 4.3 Move `Wait Until Gone` onto the helper:
  - the root step comes out of the `try` (`:1739`);
  - a reference with neither a selector nor an element raises `NoQueryError` before the first attempt;
  - its failure builder gives "still present", "still valid" and "could not be confirmed gone", with the captured element described in the one form (`_element_text`).

  Verify: the `Wait Until Gone` tests of 1.1 and 1.2 and the `Wait Until Gone` root test of 3.1 pass. The existing `Wait Until Gone` tests of `wait_keywords.robot` and `library_instance_isolation.robot` stay green. Result: all nine `Wait Until Gone` tests of `wait_keywords.robot`, the one of `library_instance_isolation.robot` and the unit test pass.
- [x] 4.4 Move `Wait Until Query` onto the helper:
  - the expression goes through the same `needs_root` memo (`descriptor_from_query`), which sees a relative path inside a function's arguments once 4.7 is done;
  - an explicit `root` is checked with `require_own_node` before the first attempt and for validity on every attempt;
  - the loop counts `AssertionError` and `TypeError` from the check as "not yet", and treats a `TypeError` raised by the evaluation as an error;
  - its failure builder gives the truthy messages (result named, "matched nothing"), the mismatch, the comparison error with expression and timeout, and the raising attempts, as in design decision 5;
  - the evaluation after the deadline (`:1874-1885`) goes away.

  Verify: the `Wait Until Query` tests of 1.1, 1.2 and 1.4 and the pytest tests of 3.1 for the explicit root and the single evaluation pass. The existing `Wait Until Query` tests stay green. Result: all 21 `Wait Until Query` tests of `wait_keywords.robot`, the test of 1.4 and the two unit tests pass. With 4.7 the suite gained a 22nd, for an absolute expression under a missing root, and all 22 pass.
- [x] 4.5 Move `Wait Until Attribute Value` onto the helper:
  - the root step comes out of the `try` (`:1980`);
  - the attempt records how far it got: no element, the element, the value;
  - `_attribute_wait_error` gains the kinds for a raising attempt. The element was not found gives `ElementNotFoundError`. The attribute could not be read gives `AttributeNotFoundError`. The value could not be checked gives `AssertionError`, naming the value. Each ends with the last error.

  Verify: the `Wait Until Attribute Value` tests of 1.1 and 1.2 and the attribute-read test of 3.1 pass, and the existing ones stay green. Result: all 17 `Wait Until Attribute Value` tests of `wait_keywords.robot` pass, and so do all 14 unit tests and the suites `WaitKeywords`, `LibraryInstanceIsolation` and `QuerySettings` (90 tests).
- [x] 4.6 Update the documentation in `src/PlatynUI/BareMetal/__init__.py` (design decision 10):
  - "Waiting for elements" and its query-settings text (`:970-1012`): which errors are never waited out, that the root is checked on every attempt, and that a failure quotes the last swallowed error;
  - the `ignore_exceptions` row of the settings table (`:995`);
  - the subtlety paragraph of "Tuning the wait" (`:1041-1043`);
  - one sentence on the failure in each wait keyword's docstring. `Wait Until Query`'s also says that an absolute expression does not use the root and that `root` must come from the same import;
  - in `Wait Until Query`'s and `Wait Until Attribute Value`'s docstrings: a `validate` expression that raises ends the wait at once, so it has to cope with the values the element passes through, and a comparison operator waits through values it cannot compare yet (design decision 3).

  Keep the user-facing voice, with no platform internals. Verify: `uv run --no-sync python -m robot.libdoc PlatynUI.BareMetal show "Wait Until Gone"` renders, and likewise for the other three wait keywords. A generated HTML libdoc resolves the new links. `just check` passes. Result: the four keywords render; in the generated HTML libdoc the new links resolve (`When a wait gives up` six times, `One root per import` and `Waiting explicitly` twice each) and no new name stays unresolved; `just check` passes.
- [x] 4.7 In `crates/xpath/src/parser/ast.rs`, make `Expr::is_context_dependent` see every place an expression reads its context (design decision 12): recurse into every operand, condition, binding and function argument, count the standard functions that read the context when an argument is left out and `position()` and `last()`, and stop at predicates and at the later steps of a path. List every kind of expression, with no catch-all arm. Update the doc comments of the runtime's `is_context_dependent` (`crates/runtime/src/xpath.rs`) and of the binding (`packages/native/src/runtime.rs`), and the docstring of `UiNodeDescriptor.needs_root`. Verify: the tests of 3.2 pass, all tests of `platynui-xpath` and `platynui-runtime` stay green, `cargo clippy -p platynui-xpath -p platynui-runtime --all-targets -- -D warnings` is clean, and after `just build-native-mock` the mock tests of `Wait Until Query` with an absolute and a computed expression pass. Result: 73 of 73 classifier tests and all 1837 tests of the two crates pass, clippy is clean, and the six suites `WaitKeywords`, `LibraryInstanceIsolation`, `QuerySettings`, `SetRootScope`, `ScopeLadder` and `SelectorResolution` pass (121 tests), with 14 of 14 unit tests.

## 5. Bookkeeping

- [x] 5.1 Extend the Purpose paragraph of `openspec/specs/baremetal-waiting/spec.md` (outside the delta; its "two supporting rules") to name the shared polling rule. Verify: `openspec validate uniform-wait-keywords --strict` passes. Result: the paragraph names the shared polling rule before the two supporting rules; the change and `openspec validate baremetal-waiting --type spec --strict` pass.
- [x] 5.2 Add a `Status:` line saying "implemented by `uniform-wait-keywords`" to the three absorbed entries in `openspec/changes/archive/2026-09-27-logging-concept/review-findings.md` (`:721`, `:751`, `:812`). Say that the DEBUG record the two A2 entries proposed is not added (design decision 9). Verify: each of the three entries carries the line. Result: the three entries (now at `:727`, `:757` and `:818`, below their headers) end their `Status:` line with "Implemented by `uniform-wait-keywords` (2026-09-29): …", and the two A2 entries say that the DEBUG record is not added.

## 6. Verification

- [x] 6.1 Run `just check` (fmt, clippy, ruff, mypy) and verify that it is clean. Run it before the recipes below, because a plain `uv run` inside it can replace the native build, and those recipes rebuild the variant they need. Result: "All checks passed." (cargo fmt changed nothing, clippy on the workspace and ruff are clean, mypy found no issues in 143 source files and the three apps).
- [x] 6.2 Run `just test-python` and verify that it is green, the new `test_baremetal_wait_loop.py` included. Result: 902 passed, the nine tests of `test_baremetal_wait_loop.py` among them.
- [x] 6.3 Run the whole mock lane with `just test-baremetal` and verify with `uv run --no-sync robotcode results summary --failed` that it is green. The loop behind every keyword's element lookup changed, so no suite is skipped. Result: 157 of 157 tests pass.
- [x] 6.4 Run both Linux acceptance lanes in full, with `just headless=true test-acceptance-compositor` and `just headless=true test-acceptance-x11`, because every keyword's element lookup changed. Verify with `uv run --no-sync robotcode results summary --failed` that both are green, 2.1 included. Record the outcome here. Result (2026-09-29, headless): the compositor lane passed 103 of 103 tests and the X11 lane 102 of 102. The test of 2.1 passed on both, in 2.4 s and 2.5 s: the root's 2 s timeout, where it took 11 s and failed before the change.
- [x] 6.5 On a real Windows machine (Wine does not count), run `just test-acceptance-windows`. At the least run the egui suites `*.Egui.AppRootAfterExit` and `*.Egui.Wait` with the recipe's build steps and `uv run --no-sync robotcode --profile real-windows run --suite '*.Egui.AppRootAfterExit' --suite '*.Egui.Wait'`, with `PLATYNUI_TEST_APP_BIN` set. Verify that they are green: 2.1 checks that UI Automation's application node ends the wait with `RootNotFoundError`. Record the outcome here. Result (2026-09-29, Windows 11 VM, desktop session): the full `just test-acceptance-windows` passed 147 of 147 tests (Egui 66, Swing 47, QML 17, Qt 16, Win32 1), the test of 2.1 in 3.2 s. The change went onto the VM's clone as a patch on top of `daf30fa` and was reverted afterwards.
- [x] 6.6 Run `just test`, the Rust tests of the whole workspace, and verify that it is green: the classifier of 4.7 lives in the XPath crate, which every provider evaluates through. Result: 2516 of 2516 tests pass (33 skipped, as usual).

## 7. Review before the commit (2026-09-30)

- [x] 7.1 Review the change with two independent reviewers: one checks the keyword and library documentation against the code, one checks the code against the spec and the design. Result: the code review found a regression and tests that were too loose (7.2, 7.3); the documentation review found examples that cannot pass, advice that does not work, statements the code contradicts, and developer text shown to users (7.4). The maintainer found the part "When a wait gives up" too long and too complicated.
- [x] 7.2 Start the wait's clock after the first lookup of the root (design decision 2). The first version started it before, so a keyword with a short `query_overrides` timeout failed while its root was still being found. Add the unit test "a root found late leaves the target its whole timeout" and its spec scenario. Verify: the test fails against the old clock start and passes with the fix. Result: against the old start it fails with `No element matched … within timeout of 0.3 seconds.`; with the fix it passes.
- [x] 7.3 Tighten the tests the code review found too loose: anchor the value-selector test at `ResultTypeError: Query 'count(//control:Window)' did not return an element`; pin the error type of the three tests that quote a last error (`ElementStillPresentError`, and no type prefix for the two assertion errors); add a unit test for a root whose own lookup raises under the root's `ignore_exceptions`; add Rust cases for `eq`, `is`, `treat as`, `castable as` and `cast as`, and for filter predicates. Verify: the tests pass. Result: 81 of 81 classifier tests and 11 of 11 wait-loop unit tests pass.
- [x] 7.4 Rework the documentation (design decision 10): fold "When a wait gives up" into the text on `ignore_exceptions` and the section "Scope"; compare numbers with `${0}` in the examples; give `validate` examples that cope with every value; say that a root keeps the element it found; stop recommending `Query` to check that a dialog has gone; say that `Get Attribute Value` waits for its element; explain `ignore_exceptions` with an XPath example instead of provider errors; drop internal names from `Set Root` and `Wait Until Gone`; give `UiNodeDescriptor.convert` a docstring for libdoc's type documentation. Verify: libdoc renders, and no new name stays unresolved. Result: libdoc renders; the type documentation shows the user-facing text, and the developer text and its unresolved `platynui_native.UiNode.is_valid` are gone.
- [x] 7.5 Bring the rest in line: `dev-docs/architecture.md` §9.4 on `is_context_dependent`, the docstring of `tests/PlatynUI/test_baremetal_root_reuse.py`, a MODIFIED delta for `baremetal-selector-resolution` (the root is looked up on every attempt of a wait, not once per keyword), the proposal and the design. Verify: `openspec validate uniform-wait-keywords --strict` passes. Result: it passes.
- [x] 7.6 Verify again: `just check`, all Python tests, the whole mock lane, the tests of `platynui-xpath` and `platynui-runtime`, and both Linux acceptance lanes in full. Record the outcome here. The Windows lane is not rerun: the review changed timing only where a root is found late, and documentation. Result: `just check` passes ("All checks passed."); all 904 Python tests, the mock lane (157 of 157), the 1845 tests of the two crates, the compositor lane (103 of 103) and the X11 lane (102 of 102) pass.
- [x] 7.7 Make `Query` follow the same rule for the root, as the maintainer chose on 2026-09-30 (design decision 12). First add three tests to `tests/BareMetal/query_settings.robot`: under a missing root an absolute expression returns its count, and a relative one names itself in the `RootNotFoundError`; under a valid root a computed expression counts inside it. Then evaluate a relative expression against `context_node` in `query`, say so in its docstring, and add the rule and two scenarios to the `baremetal-selector-resolution` delta. Verify: the first two tests fail before the change, and all three pass after it. Result: before, the absolute expression failed with `RootNotFoundError` after 0.2 s and the relative one did not name itself, while the computed one already passed; after, the suite passes 25 of 25.
- [x] 7.8 Verify 7.7: all Python tests, the whole mock lane, and both Linux acceptance lanes in full, since the acceptance suites use `Query` under `Set Root`. Record the outcome here. Result: all 904 Python tests, the mock lane (160 of 160), the compositor lane (103 of 103) and the X11 lane (102 of 102) pass.

## 8. Commit (only when the user asks)

- [ ] 8.1 Commit in reviewable steps. Each step carries the tests it turns green, and builds, passes lint (`just check` covers the whole project) and passes its tests on its own:
  - the helper and the element lookup, with 1.3, the `Wait Until Exists` tests of 1.2, and their unit tests;
  - `Wait Until Gone`, with its tests;
  - the classifier (Rust), with the tests of 3.2;
  - `Wait Until Query`, with its tests and 1.4;
  - `Query`, with its three tests of 7.7;
  - `Wait Until Attribute Value`, with its tests;
  - the acceptance test 2.1;
  - the documentation;
  - the OpenSpec bookkeeping of section 5.

  Use Conventional Commits, subjects ≤ 72 characters, no `!`. The bodies of the behavior commits list the behavior changes of the proposal that they bring. Do not push.
