# Design

## Context

See proposal.md for the motivation and the spec delta for the required behavior. Everything below was read in the working tree at `7180d20b`, where the cited code, docs and tests had no local changes. *Assumed* marks what was not checked; *inferred* marks conclusions drawn from reading code without running it.

**The records (verified).** These are all the tracing calls in the two places that `grep` finds:

| Location | Level | Message (before Decision 7) | How often |
|---|---|---|---|
| `crates/runtime/src/xpath.rs:210` | debug | `xpath evaluate` | once per `evaluate()` |
| `crates/runtime/src/xpath.rs:283` | debug | `xpath evaluate_iter` | once per `evaluate_iter()`, which `evaluate()` also calls |
| `crates/runtime/src/xpath.rs:451-454` | trace | `RuntimeXdmNode::element: resolving namespace/role` | once per wrapped element |
| `crates/runtime/src/xpath.rs:458-462` | trace | `RuntimeXdmNode::element: resolved` | once per wrapped element |
| `crates/xpath/src/xdm/mod.rs:225` | trace | `xdm_sequence_stream_materialize` | once per materialized sequence |
| `crates/xpath/src/engine/functions/diagnostics.rs:23` | debug | `fn:trace` | once per call of `fn:trace()`; **stays** |

Details the gate depends on:

- `EvaluationStream::new` (`xpath.rs:251-272`) logs nothing. Python's `evaluate_iter` reaches it through `Runtime::evaluate_iter_owned_runtime_cached` (`packages/native/src/runtime.rs:980`).
- `RuntimeXdmNode::element` (`xpath.rs:449-464`) uses `runtime_id` and `role` after its records as well, so gating the records leaves nothing unused. The same holds for `xpath` and `options` in `evaluate` and `evaluate_iter`.
- In `XdmSequenceStream::materialize` (`xdm/mod.rs:213-227`), the size hint `let hint` (`:224`) exists only for the record. So does the function's `#[allow(clippy::cast_possible_wrap)]` with its comment (`:218-219`), which excuses the `v as i64` inside the record. `use tracing::trace;` (`:120`) has no other use in the file.
- Both crates keep using `tracing` in every build: `platynui-xpath` through `fn:trace()` (`diagnostics.rs:4`, `:23`), and `platynui-runtime` through its other modules (for example `keyboard.rs:35`, `pointer.rs:384`, `runtime/desktop.rs:145-160`). The workspace lint `unused_crate_dependencies` (`Cargo.toml:41`) therefore stays quiet in a release build.

**Failures are returned already (verified).** Neither place has a `warn!` or an `error!` (`grep` over `crates/xpath/src` and `crates/runtime/src/xpath.rs`).

- `EvaluateError` (`xpath.rs:151-159`) carries every failure of an evaluation:
  - a compile error (`:261`, `:293`);
  - a failure to start the stream (`:269`, `:301`);
  - an error while items are produced (`:377`);
  - a node resolver that fails or cannot find the context node (`:314-316`).
- `fn:error()` returns `Err` for every arity (`crates/xpath/src/engine/functions/common.rs:639-669`, called from `diagnostics.rs:6-13`).
- A constructor such as `xs:integer()` returns `FORG0001` for a string that does not parse (`crates/xpath/src/engine/functions/constructors.rs:36`).

**Records that run during an evaluation but are not its own (verified).** While a query reads the tree, the desktop's enumeration reports a provider that fails to list its top-level elements: an error once per episode, and debug records while the episode lasts (`crates/runtime/src/runtime/desktop.rs:141-160`). That is the enumeration's report of a swallowed provider failure (`diagnostic-logging`, *Warnings and errors state their consequence and are reported once per episode*). Provider records work the same way. The gate does not touch any of them.

**Builds (verified).**

- The workspace has no `[profile]` section (`Cargo.toml`), and `.cargo/config.toml` sets only the target directory and aliases. `debug_assertions` is therefore on in the dev profile and off in the release profile, as Cargo's defaults have it.
- Release builds:
  - the wheels: `maturin build --release` in the `just build-*-wheel` recipes (`justfile:87`) and in CI (`.github/workflows/ci.yml:514`, `:524`, `:534`, and `:617-624` through those recipes);
  - every `just release=true …` build (`justfile:55`);
  - `cargo … --release`, tests included: the baseline test run below reports the `release` profile.
- Debug builds:
  - `just build-native` and `just build-native-mock` (`justfile:83`, `:150`);
  - `cargo build`, `cargo run` and `cargo test`;
  - the acceptance lanes, which build with `just build-native` (`justfile:363-368`).
- The only existing use of `debug_assertions` is the Inspector's `windows_subsystem` switch (`apps/inspector/src/main.rs:4`, `packages/inspector/src/main.rs:1`).

**Release baseline (verified on 2026-09-29, before this change, with a separate target directory).**

- `cargo clippy --release -p platynui-runtime -p platynui-xpath --all-targets -- -D warnings` passes.
- `cargo nextest run --release -p platynui-runtime -p platynui-xpath` passes all 1788 tests in 3.5 s.

A release check that fails after this change therefore points at this change.

**Tests that read the records (verified).**

- No test reads any of the five records. `grep` for their messages finds only the records themselves and the checklist's examples.
- These tests read `fn:trace()`:
  - `packages/native/tests/test_native_logging.py:124-132` and `:159-179`;
  - `tests/PlatynUI/test_native_logging_rf.py:70-74`, `:93-121`, with its fixture `tests/PlatynUI/robot/native_logging.robot:15-27`.

  Each picks its record by `fn:trace` in the message.
- The handler test (`test_native_logging.py:159-179`) calls back into the runtime on the first native record it receives. In a debug build that record is the adapter's first debug record (`collecting XPath results`, `xpath evaluate` before Decision 7); in a release build it is the `fn:trace()` record. Its assertions count only the `fn:trace` records, so they hold in both builds (inferred).

**Capturing records in a unit test (verified).**

- `crate::test_support::logged` (`crates/runtime/src/test_support.rs:39-63`) runs a closure under a thread-scoped `fmt` subscriber at TRACE. It returns every record as one line: level, target, message and fields. `records` (`:67-69`) filters those lines by level.
- The pointer tests use it (`pointer.rs:1083-1092`).
- The XPath tests have a small static tree whose nodes log nothing, `sample_tree()` (`xpath.rs:1691-1712`): a desktop with one window named `Main`.

**The review record.** The five review entries for these records are superseded (`openspec/changes/archive/2026-09-27-logging-concept/review-findings.md:37-51`, `:115-124`). The priority-B entry at `review-findings.md:927` names "the XPath-exclusion filter" as its occasion to touch `crates/log-filter`. This change adds no filter, so that entry waits for another occasion.

## Goals / Non-Goals

**Goals:**

- A release build *contains* none of the five records. It is not enough that it does not emit them.
- A debug build keeps them, at their levels and with their fields. Only their messages change, to the style of `dev-docs/logging.md` §11 (Decision 7).
- The rule is written down where contributors and reviewers look: the concept, the checklist and the spec.
- Tests prove the behavior in both builds, including the release build, which CI does not test.

**Non-Goals:**

- Merging or re-leveling the records, changing their fields, or adding new ones. A superseded review entry proposed an item count and a duration per evaluation. Only the messages change (Decision 7).
- Any change to the level knob, to the filter builder or to the Python bridge.
- A standing release-mode check in CI or in a `just` recipe. The maintainer decided against it for now (see Open Questions).
- Changing the level or the form of `fn:trace()`'s record.

## Decisions

### 1. `#[cfg(debug_assertions)]` at each record

- **In `crates/runtime/src/xpath.rs`:** each of the four record statements gets `#[cfg(debug_assertions)]`.
- **In `crates/xpath/src/xdm/mod.rs`:** the attribute goes on a block that holds the size hint and the record together. The record names `tracing::trace!` in full, and the import at `:120` goes. A release build then has neither an unused variable nor an unused import. The function's `#[allow(clippy::cast_possible_wrap)]` can stay where it is: it still excuses only the cast in the record, and an allowance that finds nothing to allow does not warn.
- **A pointer at each gate:** each gated place gets a short comment that points to the rule in `dev-docs/logging.md` §12, so that a reader who wonders why the record is gated finds the reason.

`debug_assertions` is the switch between Cargo's dev and release profiles, and no profile of the workspace overrides it (Context).

*Alternatives rejected:*

- **tracing's compile-time level features (`max_level_*`, `release_max_level_*`).** They are features of the `tracing` crate, and Cargo unifies features across a build. Enabling one for the XPath crates would set the ceiling for every crate that links `tracing` in the same build: the providers, the platforms, the runtime's other modules, and third-party crates. A release wheel would lose every debug record, which contradicts *The level setting means the same everywhere*.
- **`if cfg!(debug_assertions) { … }`.** The record would still be compiled into release builds, and removing it would be up to the optimizer. The record's callsite and message may stay in the binary (assumed, not checked). This form would keep `hint` in use, which would spare the block in `materialize`. But the requirement is that a release build contains none of the records, and only `#[cfg]` guarantees that by construction.
- **A crate-local macro that expands to nothing in release builds.** The maintainer allowed it, but it does not pay off here:
  - it would be defined twice, because the two crates share no crate of their own for it. The runtime depends on the engine, and a `#[macro_export]` from `platynui-xpath` would put it into that crate's public API;
  - five records do not justify two macros;
  - a macro that drops its arguments does not remove `hint`, which is computed before the record.

  The plain attribute says what happens, where it happens.
- **A target, a filter rule or a `RUST_LOG` exception for XPath records.** Rejected by the maintainer. In a debug build these records are not treated specially, and a release build does not contain them at all.
- **Removing the records.** The maintainer keeps them for work on the engine, and that work happens in debug builds.

### 2. What the gate covers

The gate covers everything that XPath evaluation records about itself: the five records of the table in Context, and any record that the engine (`platynui-xpath`) or the runtime's adapter (`platynui_runtime::xpath`) adds later.

It does not cover:

- `fn:trace()` (`diagnostics.rs:23`), whose record is output the user asked for;
- records that run during an evaluation but belong to another layer: the desktop's enumeration (`runtime/desktop.rs:141-160`) and the providers' records;
- Python and Robot Framework code, which records nothing about an evaluation. `Query`, the `Wait Until …` keywords and the `Get …` keywords write no keyword action line (`dev-docs/logging.md` §15).

### 3. XPath evaluation logs no warning or error

This is what the code does today (Context). The change writes it down as a rule in three places:

- the new requirement;
- `dev-docs/logging.md` §12, with a reference to log-or-return (§4);
- the checklist.

A unit test pins it with three failing expressions:

- one that does not compile;
- one that fails while it runs, on a cast of a value from the tree;
- `error()`.

The test asserts the returned `EvaluateError::XPath`, and that the capture holds no `WARN` or `ERROR` line.

For returned failures, §4 already says everything. The rule adds that XPath evaluation has no other reason to warn. It acts on nothing, so nothing it does can leave the system in a wrong state that only a warning or an error could report.

### 4. The rule becomes a requirement, and `dev-docs/logging.md` §12 explains it

The working notes of the triage of 2026-09-28, which are not part of the repository, expected no normative spec change beyond the replaced example. This design adds a requirement anyway, for three reasons:

- **The spec is where PlatynUI promises what its diagnostics contain.** This change alters what a release build shows at debug and trace. Two of its parts are promises to users: `fn:trace()` is recorded in every build, and an XPath failure is raised, not logged.
- **The tests need scenarios to trace to.** That is the project's test-first rule for specs, and the rule yields four checkable scenarios.
- **The level meanings alone would invite the records back.** A later change that follows *Each level has one meaning* could, in good faith, add an XPath record to release builds, as the superseded review entry at `review-findings.md:37-41` did with its item count and duration.

**Where the rule is explained.** The explanation stays in the concept. §12 gets it as a fourth producer rule, with:

- the reason: an evaluation acts on nothing;
- the scope of Decision 2;
- the mechanism, and why tracing's level features cannot do it;
- the advice to gate a record's helper variables and imports together with it;
- the exception for `fn:trace()`;
- the failure rule, pointing to §4.

§19 maps the new requirement to §12. The checklist states the rule in one line in §5, because agents and reviewers apply the checklist and may not follow its link to the concept. That was the reason for keeping the rules in the checklist in the first place (archived `logging-concept` `design.md:147`).

*Alternative:* the rule only in `dev-docs/logging.md` and the checklist. The only spec change would then be the replaced example, and the tests would trace to §12 instead of to scenarios. The maintainer kept the requirement on 2026-09-30 (see Open Questions). The alternative would have dropped the ADDED requirement, its line in §19 and its sentence in the spec's Purpose paragraph, and nothing else.

### 5. The replaced examples

- **The spec's level list** (`openspec/specs/diagnostic-logging/spec.md:16`). The keyword action line, the replacement the maintainer suggested, takes the XPath evaluation's place among the examples of what an operation did. It leaves the end of the list, so that it is not named twice.
- **`dev-docs/logging.md:127-131`.** The same replacement in prose, for example "a keyword action line that names the element and the point it clicked". The paragraph's closing sentence names the keyword action lines already, so they leave that sentence.
- **The checklist** (`.github/instructions/logging.instructions.md:84-93`). The good example becomes a record that exists in every build, such as `tracing::debug!(mode = ?mode, segments = sequence.segments().len(), "keyboard execute");` (`crates/runtime/src/keyboard.rs:35`). The avoid example interpolates the same values into the message. The block's other examples stay.
- **The test comment** (`tests/PlatynUI/test_native_logging_rf.py:23`). `[runtime.xpath]` becomes `[xpath.engine.functions.diagnostics]`: the module of the `fn:trace()` record, which this test file reads and which exists in every build. `packages/native/tests/test_native_logging.py:130` asserts the same module as a logger name.

### 6. One test for both builds, plus release checks by hand

**A test whose expectation follows the build.** One unit test in the tests module of `crates/runtime/src/xpath.rs` evaluates `trace(count(//Window), 'windows')` against `sample_tree()` inside `logged`. It splits the captured lines into the `fn:trace` record and all others, and checks:

- in every build: exactly one `fn:trace` line, with `label=windows` and `value=1`;
- in a debug build (`cfg!(debug_assertions)`): the other lines are not empty, and each names a target that starts with `platynui`, which the level knob reaches (§12's first rule);
- in a release build: there are no other lines.

`just test` runs the debug half on every CI push. The release half runs with `cargo nextest run --release -p platynui-runtime -p platynui-xpath`, a task of this change. The baseline shows that this run is clean and takes seconds.

*Alternatives rejected:*

- **A test gated with `#[cfg(not(debug_assertions))]`.** It would never run in `just test` or in CI, so a capture that broke would let it pass unnoticed. With both halves in one test, the debug half shows on every run that the capture sees these records, and that makes the release half's "none" meaningful.
- **Searching the release binary for the messages** (`strings` on the extension). The result depends on the platform and the linker, and says nothing about a record whose message is short or shared.
- **A Python test for release builds.** Python cannot tell a debug build of the extension from a release build without a new API for that purpose. Instead, the existing `fn:trace()` tests run once against a release build (`just release=true test-python`), which covers the end-to-end scenario *The XPath trace function reaches the Robot Framework log from a release build*.

**The failure test** of Decision 3 has the same expectation in both builds.

**Release clippy.** `cargo clippy --release -p platynui-runtime -p platynui-xpath --all-targets -- -D warnings` is the only check that sees a gate leave a variable or an import unused. `just clippy` builds in debug mode (`justfile:174-175`), and the CI wheel builds do not deny warnings (`ci.yml:508-535`, `:617-624`).

No `just` recipe runs tests or clippy for these crates in release mode, so the tasks use the raw commands, as `xdm-snapshot-release` did for its release check (archived `tasks.md:86`).

**No acceptance test.** Nothing here depends on a provider or a platform, and the lanes run debug builds.

### 7. The messages follow §11

`dev-docs/logging.md` §11 asks that a record changed for any reason be brought in line with its message style. The maintainer chose on 2026-09-30 to apply that to the gated records. Their levels, targets and fields stay; only the messages change:

| Record | Before | After |
|---|---|---|
| `evaluate()` | `xpath evaluate` | `collecting XPath results` |
| `evaluate_iter()` | `xpath evaluate_iter` | `evaluating XPath expression` |
| `RuntimeXdmNode::element`, before the provider calls | `RuntimeXdmNode::element: resolving namespace/role` | `wrapping element` |
| `RuntimeXdmNode::element`, after them | `RuntimeXdmNode::element: resolved` | `element wrapped` |
| `XdmSequenceStream::materialize` | `xdm_sequence_stream_materialize` | `materializing sequence` |

- The new messages are lower-case English fragments without a type, function or module prefix. The target (`platynui_runtime::xpath`, `platynui_xpath::xdm`) already names the module, and XPath keeps its case as a proper name.
- `evaluate()` calls `evaluate_iter()`, so a collecting call still writes both debug records. Merging them stays a non-goal.
- The fields (`xpath`, `cached`, `runtime_id`, `role`, `lower`, `upper`) do not carry a fact of §11's field table under another name, so they stay.
- No test reads the messages (Context), and the example in §12 uses the new message of `evaluate_iter()`.

## Risks / Trade-offs

- **[A release-only warning slips in later]** A later edit might add a gated record with a helper variable, or remove the last ungated use of an import. That warns only in a release build, and no gate compiles a release build with `-D warnings`. → §12 and the checklist say to gate the whole block. The maintainer decided against a standing release check for now.
- **[The release half of the test runs only by hand]** CI runs `just test` in debug mode. → The debug half runs on every push and keeps the capture honest. The release half is part of this change's verification.
- **[A developer does not find the records in their own build]** This happens when the installed extension is a release build: after `just release=true …`, or when `uv sync` rebuilt it from source. That `uv sync` makes a release build is assumed, not checked: maturin's PEP 517 backend passes no profile (`.venv/lib/python3.12/site-packages/maturin/__init__.py:90-137`), and `maturin pep517 build-wheel` is assumed to default to release. → §12 says which builds carry the records, and `just build-native` gives a debug build.
- **[Bug reports from users carry no XPath internals]** A user's report at `native_log_level=debug` no longer shows the evaluation's records. → Robot Framework records each keyword's arguments, the query included, and the keyword action lines and failures carry the rest. A maintainer who needs the engine's view reproduces with a debug build.
- **[Merge with `xpath-document-order`]** Both changes edit `crates/runtime/src/xpath.rs`:
  - this change at `:210`, `:283`, `:449-464` and in the tests module;
  - that change at the equality and identity of `RuntimeXdmNode` (`:517-530`), the attribute's owner, and the cycle note (`:414-425`).

  → Only textual conflicts are expected, and whichever change lands second rebases and repeats this change's release checks. Any record that `xpath-document-order` adds to the adapter or to the engine falls under the rule and is gated too.
- **[Merge with `x11-atspi-healthy-run-warnings`]** That open change adds requirements to `diagnostic-logging` and lines to §19 of `dev-docs/logging.md`, as this change does. It modifies none of the requirements this change modifies. → Only textual conflicts are expected, in §19.

## Migration Plan

- **Behavioral**, in what release builds log. No API changes, and nothing for users to migrate.
- **Native rebuild:** yes. The runtime is linked into `packages/native`, and also into the command-line tool and the Inspector.
- **Sequence:**
  1. The tests. The test of Decision 6 fails in a release run until the gate exists; the rest passes today.
  2. The gate.
  3. The docs.
  4. Verification in both builds.
- **Rollback:** revert the commits. The gate, the tests and the docs depend on no other change. Reverting brings the records back into release builds, and the XPath examples back into the docs.
- **Main spec Purpose:** the Purpose paragraph of `openspec/specs/diagnostic-logging/spec.md` names each requirement in one sentence. It is outside the delta and gets the new requirement's sentence by hand (task 3.5).

## Open Questions

None. The maintainer answered the three questions of this design on 2026-09-30:

- **The ADDED requirement (Decision 4) stays.** The rule is a requirement of `diagnostic-logging`, with its line in §19 and its sentence in the spec's Purpose paragraph, and the tests trace to its scenarios.
- **No standing release check for now.** The release-mode clippy and test runs of the two crates stay a one-time part of this change's verification, not a `just` recipe or a CI step.
- **The messages follow §11 (Decision 7).** The gate touches the records anyway, so their messages are brought in line with the style, as §11 asks of every changed record.
