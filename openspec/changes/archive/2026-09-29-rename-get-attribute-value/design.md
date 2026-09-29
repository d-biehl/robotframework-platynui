# Design

## Context

See proposal.md for why the keyword is renamed, and the spec deltas for the behavior. This section covers the code and the repository state the rename works on. Everything below was read in the current code unless it is marked as assumed.

- **`Get Attribute` today** ([src/PlatynUI/BareMetal/__init__.py:2645-2675](../../../src/PlatynUI/BareMetal/__init__.py#L2645-L2675)):
  - It is a method `get_attribute`, decorated with `@keyword` and `@assertable`.
  - It splits a `prefix:name` attribute name, waits for the element with `descriptor.resolve`, and returns `node.attribute(name, namespace)`.
  - `@assertable` adds the three assertion parameters and checks the value once. It returns the value, not what AssertionEngine returns ([src/PlatynUI/_assertable.py:118-123](../../../src/PlatynUI/_assertable.py#L118-L123)).
- **Keyword names and documentation** come from robotlibcore's `DynamicCore`, which `OurDynamicCore` extends ([src/PlatynUI/_our_libcore.py:10](../../../src/PlatynUI/_our_libcore.py#L10)). A method decorated with `@keyword` and no explicit name becomes the keyword named after the method, so `get_attribute` is `Get Attribute`. `@assertable` copies the method's `__doc__` onto its wrapper.
- **Deprecation in Robot Framework:** a keyword whose documentation starts with `*DEPRECATED`, with the closing `*` on the first line, is deprecated. Robot Framework then logs `Keyword '<name>' is deprecated.` followed by the rest of the short documentation, in the log, the console and the Test Execution Errors, and libdoc marks the keyword as deprecated (Robot Framework User Guide, "Deprecating keywords"). *Assumed:* the marker reaches Robot Framework through `DynamicCore` and the `@assertable` wrapper. A test in tasks 1.3 proves it.
- **Where the old name is used**, counted with `Get Attribute(?! Value)`:
  - about 260 lines in 45 suites and resources under `tests/`;
  - 22 in the library's documentation and one internal docstring;
  - 3 in `dev-docs/testing-strategy.md` and 2 in the robot-test-style skill;
  - 5 in four main specs;
  - 3 in the delta spec of the open change `xpath-document-order`;
  - the German design document of the high-level library, and a talk transcript; both are out of scope.
- **Python-level tests** of the library live in `tests/PlatynUI/test_*.py`. `test_keyword_logging.py` shows both patterns this change needs: monkeypatched element lookup, and a Robot Framework run in a subprocess ([tests/PlatynUI/test_keyword_logging.py:98-103](../../../tests/PlatynUI/test_keyword_logging.py#L98-L103)). `just test-python` builds the native package with the mock provider before pytest runs.

## Goals / Non-Goals

**Goals:**

- The reading keyword is `Get Attribute Value`, and nothing about its behavior changes.
- `Get Attribute` keeps working, with identical behavior, and says through Robot Framework's own mechanism that it is deprecated and what replaces it.
- The repository's own suites, documentation and specs use only the new name, so the repository's own runs log no deprecation warning.
- The removal before 1.0 is written down where the roadmap lives.

**Non-Goals:**

- Removing the alias. That is a later, breaking change.
- Changing what the keyword does: it still reads once and checks once.
- The planned keyword names of the high-level `PlatynUI` library (`dev-docs/python-library-design.md`).
- A Python `DeprecationWarning` for calls of `get_attribute` from Python code (see decision 4).

## Decisions

### 1. The name `Get Attribute Value`

The keyword returns a value, and the library already separates an attribute (`Query    …/@Name` returns one, with `.value` and `.owner()`) from its value. The new name also pairs with `Wait Until Attribute Value`, so that reading and waiting have matching names. "Get" is the library's verb for keywords that read something (`Get Pointer Position`, `Get Element At Point`).

*Alternative considered:* keeping `Get Attribute`, which is also the name the Browser library uses. Rejected by the user: the name says less than the keyword does.

### 2. The alias is a second keyword method over one shared implementation

`get_attribute_value` becomes the keyword. Its body moves into a private method that both keywords call. `get_attribute` keeps its signature, with `@keyword` and `@assertable`, and gets a documentation of its own. Each of the two keywords then checks its assertion through its own decorator. The alias must not call the decorated `get_attribute_value` with the assertion arguments, because that would check the assertion twice.

*Alternatives considered:*
- One method with two Robot Framework names. robotlibcore gives a method one name, and the alias needs different documentation anyway, for the deprecation marker.
- Mapping the alias inside `OurDynamicCore`. That is more machinery for the same result.

### 3. What the deprecation marker says

The first paragraph of the alias's documentation reads: `*DEPRECATED* Use `Get Attribute Value` instead; this alias will be removed before PlatynUI 1.0.` Robot Framework puts that paragraph into its warning, so the warning names the replacement and the removal. A second paragraph says that the alias behaves exactly like `Get Attribute Value`. The full documentation lives with `Get Attribute Value` only, so it cannot drift between the two.

### 4. No Python-level deprecation warning

Robot Framework's marker covers every Robot Framework user. A `warnings.warn(DeprecationWarning)` in `get_attribute` would reach the few Python callers, but in a Robot Framework run it would show up as a second warning next to Robot Framework's own. The Python method stays until the alias is removed, like the keyword.

### 5. The rename is mechanical, the review is not

Keyword calls are renamed by pattern: `Get Attribute` not followed by ` Value` becomes `Get Attribute Value`, including calls with a library prefix (`BM.`, `A.`, `B.`). Every match that is not a keyword call gets its own look before it changes:
- test names, such as "Wait Until Query Matches Get Attribute For A Present Attribute";
- suite and keyword documentation, such as the `testapp.resource` doc that says "``Get Attribute`` waits/retries for the label";
- the `@assertable Get Attribute` wording in `query_settings.robot`;
- prose in the library documentation and in `dev-docs`.

The alias's own documentation is the one place that keeps the old name.

### 6. The alias is tested in pytest, the new keyword in the mock suites

The mock suites already cover most of what `Get Attribute Value` has to do, once they use the new name:
- typed values and mismatching assertions (`window_activation.robot`, `query_settings.robot`);
- an element that never appears (`query_settings.robot`);
- an element from another import (`library_instance_isolation.robot`).

A new mock suite, `tests/BareMetal/get_attribute_value.robot`, adds the scenarios nothing covers yet:
- a prefixed attribute;
- a failing assertion that fails at once under a 30-second per-call timeout, bounded by the test's `[Timeout]`;
- a missing attribute that fails at once.

The alias is tested in `tests/PlatynUI/test_baremetal_get_attribute_alias.py`:
- libdoc, built in-process, marks `Get Attribute` as deprecated, and its short documentation names `Get Attribute Value` and the removal before 1.0;
- a small suite runs in a subprocess against the mock. It calls both keywords, compares their values and their assertion failures, and the test reads the Test Execution Errors from the run's `output.xml` for the deprecation warning.

*Alternative considered:* a mock suite for the alias. Rejected because every mock run would then carry a deprecation warning, and a warning that is always there teaches people to overlook warnings.

### 7. The open change `xpath-document-order` moves along

Its delta for `baremetal-selector-resolution` calls `Get Attribute` in three WHEN lines. If this change is archived first and that one later, the old name would come back into a main spec. The three lines change here. That touches another change's planning artifact, and nothing else of it.

### 8. The removal is tracked in the roadmap

`dev-docs/planning.md` §5.3 ("Robot Framework Integration") gets an open item: remove the deprecated `Get Attribute` alias before 1.0. The deprecation text in the alias's documentation says the same to users.

### 9. The spec deltas only rename

The MODIFIED requirements in `baremetal-waiting`, `description-attribute`, `test-app-blueprint` and `qml-test-app` are the current requirement blocks, copied whole, with only the keyword name changed. A check confirmed this when the deltas were written: restoring the old name in each delta block gives back the main spec's text exactly. The Purpose paragraph of `baremetal-waiting` also names the keyword. It lies outside the delta and is edited by hand.

## Risks / Trade-offs

- **The mechanical rename changes text that should keep the old name, or misses a call.** → Non-call matches are reviewed one by one (decision 5). `robotcode analyze` reports any leftover call of a keyword that does not exist, and the mock lane and the acceptance lanes have to stay green.
- **The deprecation marker does not survive `DynamicCore` and the `@assertable` wrapper.** This is assumed, not verified. → The libdoc test in pytest fails if Robot Framework does not see the marker.
- **Suites outside the repository keep using `Get Attribute` and now see a warning.** → That is the purpose. The warning names the replacement and the removal.
- **A diff of about 260 lines across 45 suites is hard to review.** → The rename goes into its own commit, and a grep for `Get Attribute(?! Value)` confirms what is left.
- **Other work in progress still writes `Get Attribute`.** → It keeps working, with a warning. `xpath-document-order` is updated here (decision 7).

## Migration Plan

- **Additive, with a deprecation:** a new keyword name. The old name keeps its behavior; its only new effect is the deprecation warning. Nothing is removed.
- **No native rebuild:** the change is Python, Robot Framework and documentation only.
- **Order:** keyword, alias and tests first; then the rename in suites, documentation and specs; then the roadmap entry.
- **Rollback:** revert the commits. Suites written against `Get Attribute Value` would need the old name back; nothing else depends on it.
- **Main specs on archive:** `baremetal-attribute-reading` is created from its delta, including its Purpose. The Purpose of `baremetal-waiting` is edited by hand (task 4.3).
