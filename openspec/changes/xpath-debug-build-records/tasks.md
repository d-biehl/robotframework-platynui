# Tasks

The change is native only: two Rust crates, two documents and one comment in a Python test. The runtime tests use the static `sample_tree()` of `crates/runtime/src/xpath.rs` and the capture helper `crate::test_support::logged`, so they need no provider and no platform. No scenario needs a real provider, and the acceptance lanes run debug builds, whose records do not change, so no acceptance lane runs (design, Decision 6). Release-mode runs use raw `cargo` commands, because no `just` recipe tests or lints in release mode. The tasks follow design Decision 4, which adds the requirement. If the maintainer decides against it (design, Open Questions), the §19 line of 3.2 and task 3.5 go, and the tests stay, traced to §12.

## 1. Tests first

- [ ] 1.1 In the tests module of `crates/runtime/src/xpath.rs`, add `xpath_records_exist_in_debug_builds_only` (design, Decision 6). It covers the scenarios *A release build records only the XPath trace function* and *A debug build keeps the evaluation's records under the level setting*. The test:
  - evaluates `trace(count(//Window), 'windows')` against `sample_tree()` inside `logged`;
  - asserts in every build exactly one captured line with `fn:trace`, carrying `label=windows` and `value=1`;
  - asserts that the remaining lines are empty exactly when `cfg!(debug_assertions)` is false;
  - in a debug build, also asserts that each remaining line names a target that starts with `platynui`.

  Verify: `just test-crate platynui-runtime` passes. `cargo nextest run --release -p platynui-runtime -E 'test(xpath_records_exist_in_debug_builds_only)'` fails before the gate, and its message lists the lines of the records it found.
- [ ] 1.2 In the same module, add `a_failing_expression_is_returned_not_logged` (scenario *A failing expression is returned, not logged*). It is an `rstest` with three cases, evaluated against `sample_tree()` inside `logged`:
  - an expression that does not compile, such as `//Window[`;
  - a cast that fails while the expression runs, such as `xs:integer(//Window/@Name)` (the window's name is `Main`);
  - `error()`.

  Each case asserts `Err(EvaluateError::XPath(_))`, and that `records(&log, "WARN")` and `records(&log, "ERROR")` are empty. Verify that it passes in `just test-crate platynui-runtime` and in `cargo nextest run --release -p platynui-runtime -E 'test(a_failing_expression_is_returned_not_logged)'`. It pins behavior that holds today, so it passes before the gate as well. Check that the cast case fails in the evaluation, not in compilation: its error code is `FORG0001`, not a static error.

## 2. The gate

- [ ] 2.1 In `crates/runtime/src/xpath.rs`, put `#[cfg(debug_assertions)]` on the four record statements: `:210`, `:283`, `:451-454` and `:458-462` (design, Decision 1). Keep their messages and fields. Give each gated place a one-line comment that points to `dev-docs/logging.md` §12. Verify:
  - `just clippy` passes;
  - `just test-crate platynui-runtime` passes, 1.1 included;
  - `cargo clippy --release -p platynui-runtime --all-targets -- -D warnings` passes.
- [ ] 2.2 In `crates/xpath/src/xdm/mod.rs`, put the size hint and its record (`:224-225`) into one block under `#[cfg(debug_assertions)]`. The record names `tracing::trace!` in full, and `use tracing::trace;` (`:120`) goes. Keep the function's `#[allow(clippy::cast_possible_wrap)]` and its comment (`:218-219`). Give the block the same pointer comment as in 2.1. Verify:
  - `just clippy` passes;
  - `just test-crate platynui-xpath` passes;
  - `cargo clippy --release -p platynui-runtime -p platynui-xpath --all-targets -- -D warnings` passes, with no unused variable or import;
  - the release run of 1.1 from 1.1's verify step now passes.

## 3. Documentation

- [ ] 3.1 In `dev-docs/logging.md` §12, add the fourth producer rule, and adjust the opening sentence, which counts three rules. Write it as explanatory prose, with the content that design Decision 4 lists:
  - the reason: an evaluation acts on nothing;
  - the scope: the engine and the runtime's adapter, not providers or the desktop's enumeration that run during an evaluation;
  - `#[cfg(debug_assertions)]`, and why tracing's level features would silence the whole workspace;
  - gating a record's helper variables and imports together with the record;
  - which builds carry the records: `just build-native` and `cargo` without `--release` do, the wheels and `just release=true …` do not;
  - `fn:trace()` in every build;
  - failures returned, never logged, with a pointer to §4.

  Verify by reading: the section states the rule normatively, with no status report.
- [ ] 3.2 In `dev-docs/logging.md`:
  - replace the XPath example in §3's debug paragraph (`:127-131`) with a keyword action line, such as one that names the element and the point it clicked, and take "the keyword action lines" out of the paragraph's closing list, so that they are not named twice (design, Decision 5);
  - in §19, map *XPath evaluation's own diagnostics exist in debug builds only* to §12.

  Verify: `grep -n -i xpath dev-docs/logging.md` finds only §12 and §19.
- [ ] 3.3 In `.github/instructions/logging.instructions.md`:
  - replace the good example at `:86` with a record that exists in every build, such as the `keyboard execute` record of `crates/runtime/src/keyboard.rs:35`, and the avoid example at `:92` with the same values interpolated into the message;
  - add the rule to §5 in one line: XPath evaluation's own records are compiled into debug builds only (`#[cfg(debug_assertions)]`, never tracing's level features), together with their helper variables and imports; `fn:trace()` is the exception; XPath failures are returned, never logged.

  Verify: `grep -n -i xpath .github/instructions/logging.instructions.md` finds only the new §5 line.
- [ ] 3.4 In `tests/PlatynUI/test_native_logging_rf.py:23`, change the comment's example module from `[runtime.xpath]` to `[xpath.engine.functions.diagnostics]`, the module of the `fn:trace()` record, which exists in every build. Verify with `just ruff`.
- [ ] 3.5 Extend the Purpose paragraph of `openspec/specs/diagnostic-logging/spec.md`, which is outside the delta and names each requirement in one short sentence, with a sentence for the new requirement, such as "XPath evaluation's own records stay out of release builds." Verify: `openspec validate xpath-debug-build-records --strict` passes.

## 4. Verification

- [ ] 4.1 Run `just check` (fmt, clippy, ruff, mypy) and verify that it is clean. Run it before the Python recipes below: a plain `uv run` can replace the native build, and those recipes rebuild the build they need.
- [ ] 4.2 Run `just test` and verify that the whole Rust workspace is green, with the debug half of 1.1 and with 1.2.
- [ ] 4.3 Run the release checks, and verify that both are green:
  - `cargo clippy --release -p platynui-runtime -p platynui-xpath --all-targets -- -D warnings`;
  - `cargo nextest run --release -p platynui-runtime -p platynui-xpath`, the release half of 1.1 and 1.2 included.

  Before this change, both passed, with 1788 tests (design, Context).
- [ ] 4.4 Run `just test-python` against the debug mock build, and verify that it is green, including the `fn:trace()` tests in `packages/native/tests/test_native_logging.py` and `tests/PlatynUI/test_native_logging_rf.py`.
- [ ] 4.5 Run `just test-baremetal`, then `uv run --no-sync robotcode --profile mock results log --level WARN --execution-messages`. Verify:
  - the mock lane is green (`uv run --no-sync robotcode --profile mock results summary --failed`);
  - no warning or error comes from PlatynUI.

  This is the keyword side of *A failing expression is returned, not logged*: the mock suites evaluate broken selectors on purpose, for example `tests/BareMetal/set_root_scope.robot:80`.
- [ ] 4.6 Run `just release=true test-python`, and verify that it is green against a release build of the mock extension, the `fn:trace()` tests included. This is the scenario *The XPath trace function reaches the Robot Framework log from a release build*. Afterwards, rebuild the extension you work with (`just build-native-mock` or `just build-native`), because the run leaves a release build installed.
- [ ] 4.7 Run `openspec validate xpath-debug-build-records --strict` and verify that it passes. Record the outcome of 4.1 to 4.6 here.

## 5. Commit (only when the user asks)

- [ ] 5.1 Commit in reviewable steps, each of which builds and passes lint and its own tests:
  - the gate with its tests and the test comment (groups 1 and 2, and 3.4), for example `fix(xpath): keep evaluation records out of release builds`;
  - the documentation (3.1 to 3.3, and 3.5), for example `docs(logging): compile XPath evaluation records into debug builds only`.

  Subjects are at most 72 characters, without `!`. The first commit's body says that release builds, the wheels included, no longer log XPath evaluation at debug or trace, and that `fn:trace()` is unchanged.
