# Tasks

No native rebuild is needed: the change is Python, Robot Framework and documentation only (design.md, Migration Plan). The `just` recipes build the native variant they need themselves.

## 1. Tests first

- [ ] 1.1 Add the mock suite `tests/BareMetal/get_attribute_value.robot`, imported like the other mock suites (`use_mock=${True}`, `query_settings={'timeout': 0.2}`). It holds one test per `Get Attribute Value` scenario that no suite covers yet:
  - the typed value: `IsMaximized` of the Operations Console window is the boolean `False`;
  - a prefixed attribute: `native:ProcessId` is `4242`;
  - an assertion that holds returns the value;
  - an assertion that fails, fails at once: `==    Wrong Name` with `query_overrides={'timeout': 30}` inside a test whose `[Timeout]` is a few seconds;
  - a missing attribute, `ToggleState`, fails with `AttributeNotFoundError`, likewise under a short `[Timeout]`.

  The element-not-found and foreign-import scenarios stay covered by `query_settings.robot` and `library_instance_isolation.robot` once 1.2 renames them. Follow the robot-test-style skill. Verify: `just test-baremetal --suite '*.GetAttributeValue' tests/BareMetal` runs, and every test fails only because `Get Attribute Value` does not exist yet.
- [ ] 1.2 Rename every `Get Attribute` call in the suites and resources under `tests/` to `Get Attribute Value` (design decision 5):
  - calls with a library prefix too (`BM.`, `A.`, `B.`);
  - by hand, every match that is not a call: test names such as "Wait Until Query Matches Get Attribute For A Present Attribute", the `testapp.resource` documentation, and the `@assertable Get Attribute` wording in `query_settings.robot`.

  Verify: `grep -rnP "Get Attribute(?! Value)" tests` prints nothing. `uv run --no-sync robotcode analyze code tests` reports no problem other than the not-yet-existing `Get Attribute Value`; if its results look stale, run `robotcode analyze cache clear`.
- [ ] 1.3 Add `tests/PlatynUI/test_baremetal_get_attribute_alias.py` (design decision 6):
  - **libdoc:** built in-process for `PlatynUI.BareMetal`, it marks `Get Attribute` as deprecated, and its short documentation names `Get Attribute Value` and the removal before 1.0;
  - **a Robot Framework run in a subprocess:** a small mock suite written to a temporary directory calls `Get Attribute` and `Get Attribute Value`. For `Name` and `IsMaximized` it asserts equal values of the same type, and for `==    Wrong Name` the same failure message. The test then reads the run's `output.xml` with `robot.api.ExecutionResult` and finds exactly one deprecation warning naming `Get Attribute Value` among the execution errors.

  Model the subprocess use on `tests/PlatynUI/test_keyword_logging.py`. Verify: `just test-python` runs the module, and it fails only because `Get Attribute Value` and the deprecation do not exist yet.

## 2. Implementation

- [ ] 2.1 In `src/PlatynUI/BareMetal/__init__.py` (design decisions 2 to 4):
  - move the body of `get_attribute` into one private method;
  - add `get_attribute_value` with `@keyword` and `@assertable`, the same signature, and the full documentation of today's `Get Attribute`, written for the new name;
  - turn `get_attribute` into the alias: `@keyword` and `@assertable`, the same signature, calling the private method, with the documentation from decision 3. It must not call the decorated `get_attribute_value`.

  Verify: 1.1 and the pytest module from 1.3 pass, and the suites renamed in 1.2 pass again with `just test-baremetal`.

## 3. Documentation and guidance

- [ ] 3.1 Move the library documentation in `src/PlatynUI/BareMetal/__init__.py` to the new name:
  - the regular-expression example under "Finding elements";
  - "Reading and checking values", including its examples;
  - "Process attributes";
  - "Waiting explicitly";
  - "A short example";
  - the docstrings of `Wait Until Query` and `Wait Until Attribute Value`;
  - the docstring of `_split_attribute_name`.

  Verify: `grep -nP "Get Attribute(?! Value)" src/PlatynUI/BareMetal/__init__.py` finds only the alias's own lines. A libdoc generated with `--specdocformat HTML` shows no unresolved keyword or section names.
- [ ] 3.2 Move `dev-docs/testing-strategy.md` (three places) and `.claude/skills/robot-test-style/SKILL.md` (two places) to the new name. Verify: `grep -rnP "Get Attribute(?! Value)"` over both files prints nothing.
- [ ] 3.3 Add an open item to `dev-docs/planning.md` §5.3: remove the deprecated `Get Attribute` alias of `Get Attribute Value` before 1.0. Verify: the item is in §5.3.

## 4. Specs and other changes

- [ ] 4.1 Change the three WHEN lines in `openspec/changes/xpath-document-order/specs/baremetal-selector-resolution/spec.md` from `Get Attribute` to `Get Attribute Value` (design decision 7), and nothing else in that change. Verify: `openspec validate xpath-document-order --strict` passes, and a grep over that change finds no old name.
- [ ] 4.2 Grep the repository for what is left: `grep -rnP "Get Attribute(?! Value)"` over `src`, `tests`, `dev-docs`, `docs`, `.claude`, `openspec/specs` and the open changes. Verify: the only matches are the alias in `src/PlatynUI/BareMetal/__init__.py`, the out-of-scope `dev-docs/python-library-design.md` and `docs/talks`, the main specs that the MODIFIED deltas of this change replace on archive, and the `baremetal-waiting` Purpose until 4.3.
- [ ] 4.3 Change the Purpose paragraph of `openspec/specs/baremetal-waiting/spec.md` (outside the delta) from `Get Attribute` to `Get Attribute Value`. Verify: `openspec validate rename-get-attribute-value --strict` and `openspec validate baremetal-waiting --type spec --strict` pass.

## 5. Verification

- [ ] 5.1 Run `just check` and verify that it is clean. Run it before the recipes below, because a plain `uv run` can replace the native build.
- [ ] 5.2 Run `just test-python` and verify that it is green, including the alias module.
- [ ] 5.3 Run the whole mock lane with `just test-baremetal`, judge it with `uv run --no-sync robotcode results summary --failed`, and verify that it is green. Then verify that the run's execution errors, read from `results/output.xml` with `robot.api.ExecutionResult`, contain no deprecation warning, so that no suite in the repository still calls the alias.
- [ ] 5.4 Run the Linux acceptance lanes in full, `just headless=true test-acceptance-x11` and `just headless=true test-acceptance-compositor`, since the rename touched suites across them. Verify with `robotcode results` that both are green.
- [ ] 5.5 On real Windows (the Windows 11 VM counts; Wine does not), run the acceptance lane with the renamed Windows-only suites (swing, qt, win32 and egui): `just test-acceptance-windows`, whose default arguments run the whole `real-windows` profile. Verify that it is green. Keep this task open until it has run on Windows.
