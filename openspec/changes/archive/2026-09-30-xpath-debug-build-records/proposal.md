# Proposal

## Why

Evaluating an XPath expression does nothing on the UI. Still, the XPath engine and the runtime's adapter that presents the UI tree to it write diagnostic records of their own in every build, the wheels that users install included:

- up to two debug records per evaluation, depending on the entry point (`crates/runtime/src/xpath.rs:210`, `:283`);
- two trace records for every element an evaluation wraps (`:451-462`);
- a trace record for every sequence the engine materializes (`crates/xpath/src/xdm/mod.rs:225`).

These records describe the engine, not anything the test did. A user who turns on debug or trace to report a problem gets them mixed into the records that matter, and a `Wait Until …` keyword evaluates its query up to ten times a second.

The maintainer decided on 2026-09-28 (preamble of `openspec/changes/archive/2026-09-27-logging-concept/review-findings.md`):

- these records exist in debug builds only;
- `fn:trace()` stays in every build;
- XPath failures are returned, never logged.

Until that is done, the spec's list of level meanings, `dev-docs/logging.md` and the logging checklist still use an XPath record as their model of a debug record.

## What Changes

- **Release builds carry none of XPath evaluation's own records.** The five records below are compiled only when `debug_assertions` is on:
  - the runtime adapter's debug records `xpath evaluate` and `xpath evaluate_iter`, one per call of the two entry points that log (`crates/runtime/src/xpath.rs:210`, `:283`);
  - its two trace records per wrapped element (`:451-454`, `:458-462`);
  - the engine's trace record of a materialized sequence, together with the size hint that is computed only for it (`crates/xpath/src/xdm/mod.rs:217-225`).

  The gate is `#[cfg(debug_assertions)]` at each record. tracing's compile-time level features cannot be used: Cargo unifies features across the workspace, so they would silence every crate of a release build.
- **Debug builds keep the records, at their levels and with their fields.** They follow the level setting like every other PlatynUI record: no target name of their own, no filter rule, no `RUST_LOG` exception. `crates/log-filter` does not change.
- **The five messages follow `dev-docs/logging.md` §11.** §11 asks that a record changed for any reason be brought in line with its message style, and the maintainer chose on 2026-09-30 to apply it here. The identifier-style messages become lower-case English fragments without a type or function prefix: `collecting XPath results`, `evaluating XPath expression`, `wrapping element`, `element wrapped` and `materializing sequence`.
- **`fn:trace()` is unchanged, in every build.** Its record is output that the user asked for in their own expression (`crates/xpath/src/engine/functions/diagnostics.rs:23`).
- **XPath evaluation logs no warning or error.** Its failures are returned as `EvaluateError`, and the calling keyword raises them, or keeps waiting when its query settings ignore exceptions. `fn:error()` returns an error too. That is already true. This change writes it down as a rule and pins it with a test.
- **The rule is written down in three places:**
  - `dev-docs/logging.md` §12, next to the other producer rules for native code;
  - one line in `.github/instructions/logging.instructions.md` §5;
  - a requirement of `diagnostic-logging`.
- **The XPath examples are replaced** by records that exist in every build:
  - the debug example of the spec's level list;
  - the debug paragraph of `dev-docs/logging.md:127-131`, whose "how many items it found" no record ever reported;
  - the good and the avoid example of the checklist (`:86`, `:92`);
  - the module example in a comment of `tests/PlatynUI/test_native_logging_rf.py:23`.

Not part of this change:

- Merging, removing or re-leveling the records, or changing their fields. The review entries that proposed it are superseded.
- The Inspector, the command-line tool and the Python layers. None of them logs anything about XPath evaluation.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `diagnostic-logging`:
  - MODIFIED *Each level has one meaning*: the examples for debug name a keyword action line instead of an XPath evaluation.
  - ADDED *XPath evaluation's own diagnostics exist in debug builds only*: a release build has none of them, and a debug build puts them under the normal level setting. `fn:trace()` is recorded in every build, and failures are returned, never logged.

## Impact

- **Rust crates:**
  - `platynui-runtime` (`src/xpath.rs`): four gated and reworded records, and two unit tests.
  - `platynui-xpath` (`src/xdm/mod.rs`): one gated block with a reworded record, and the import `use tracing::trace;` (`:120`), which only that record uses.
  - No `Cargo.toml` changes, no new dependency or feature. Both crates keep using `tracing` in every build, so the workspace lint `unused_crate_dependencies` stays quiet.
- **Python / Robot Framework:** no code or API change. `packages/native` links the runtime, so its release builds lose the records too. The user documentation of `native_log_level` (`src/PlatynUI/BareMetal/__init__.py:1215-1246`) names no XPath record and stays as it is.
- **Command-line tool and Inspector:** their release wheels lose the records as well. A debug build (`cargo run`, `just build-cli`, `just build-inspector`) keeps them.
- **Tests:**
  - New runtime unit tests. One checks that the records exist in a debug build and not in a release build, while `fn:trace()` is recorded in both. The other checks that a failing expression is returned and logged at no level above debug.
  - The existing `fn:trace()` tests (`packages/native/tests/test_native_logging.py:124-132`, `tests/PlatynUI/test_native_logging_rf.py:70-74`) run as before against the debug mock build, and once against a release mock build.
  - Both crates get a clippy run in release mode. Only a release build can warn about a variable or an import that a gate left behind.
- **Docs:** `dev-docs/logging.md` (§3, §12, §19) and `.github/instructions/logging.instructions.md` (§4, §5).
- **Specs:** `diagnostic-logging`, with one modified and one added requirement. Its Purpose paragraph names each requirement in one sentence. That paragraph is outside the delta and gets its new sentence by hand.
- **Native rebuild:** yes, because the runtime is linked into the extension. The only difference is what a release build logs.
- **Platforms and providers:** all of them, the mock included, because the gate sits in platform-independent crates. No acceptance lane is needed: the lanes run debug builds, whose records do not change.
- **Compatibility:** not **BREAKING**. No API changes. The debug and trace output of release builds gets shorter, and in debug builds the five records read differently. No test or tool reads their messages.
- **Coordination:**
  - The open change `xpath-document-order` edits `crates/runtime/src/xpath.rs` near these records: the identity hint of `RuntimeXdmNode` (`:517-530`), the attribute's owner link and the cycle note (`:414-425`). The edits do not conflict in meaning; whichever change lands second rebases.
  - The open change `x11-atspi-healthy-run-warnings` also adds requirements to `diagnostic-logging` and lines to the requirement map in §19 of `dev-docs/logging.md`. Neither change modifies a requirement that the other touches, so the overlap is textual.
  - The review entries for these records (`review-findings.md:37-51`, `:115-124`) already carry *superseded by the maintainer decision of 2026-09-28* and need no work.
