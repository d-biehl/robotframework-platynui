# Streaming Evaluator Progress Log

## Phase 1 – Infrastructure Foundations
- Introduced the `SequenceCursor` abstraction and refactored `XdmSequenceStream` to wrap cursor-based pipelines, including eager adapters.
- Added `VmHandle` to reuse virtual machine state across streaming operations and updated axis evaluation to rely on cursor cloning.
- Instrumented `materialize` calls with `tracing` and added cursor-focused unit tests.
- Commands: `cargo fmt`, `cargo clippy -p platynui-xpath --all-targets -- -D warnings`, `cargo test -p platynui-xpath`.

## Phase 2 – Streaming Predicates & Path Expressions
- Implemented cursor-based predicate filtering that preserves `position()`/`last()` semantics without materialising sequences and reused it across axis steps and explicit predicate opcodes.
- Added a streaming path-step cursor to flatten sub-expression results lazily, relying on the new VM handle infrastructure.
- Expanded streaming tests with predicate and path coverage to guard positional behaviour.
- Commands: `cargo fmt`, `cargo clippy -p platynui-xpath --all-targets -- -D warnings`, `cargo test -p platynui-xpath`.

## Phase 3 – Set Operations & Deduplication
- Added a document-order cursor that defers sorting/deduplication until first consumption and integrated it into doc-order opcodes.
- Refactored union/intersect/except to wrap streaming cursors, reusing VM handles while keeping node-only validations.
- Extended streaming tests to cover set operators (including large unions) ensuring parity with eager evaluation.
- Commands: `cargo fmt`, `cargo clippy -p platynui-xpath --all-targets -- -D warnings`, `cargo test -p platynui-xpath`.

## Phase 4 – Control-Flow Opcodes
- Replaced eager `ForLoop`/`QuantLoop` execution with cursor-based implementations that stream results and respect position/last semantics.
- Avoided redundant materialisation for `let` bindings by storing streaming values directly; ensured VM snapshots carry cancellation metadata.
- Added cooperative cancellation support via the dynamic context and exercised it (plus quantifier behaviour) in new streaming tests.
- Commands: `cargo fmt`, `cargo clippy -p platynui-xpath --all-targets -- -D warnings`, `cargo test -p platynui-xpath`.
