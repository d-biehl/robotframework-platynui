# Streaming Evaluator Plan – Codex Agent Instructions

## Overview for the Codex Agent

You are an autonomous Codex-based assistant tasked with transforming the XPath evaluator in `crates/platynui-xpath/` into a fully streaming execution engine. Your mission is to eliminate avoidable materialisation of intermediate sequences while maintaining full XPath semantics (`position()`, `last()`, document order, error handling, etc.).

### Required Skills

- Mastery of Rust (ownership, lifetimes, traits, generics, iterators, smart pointers).
- Experience implementing lazy/streaming evaluation patterns.
- Familiarity with Rust testing frameworks (`cargo test`, `rstest`).
- Ability to profile and benchmark Rust code.
- Comfortable with tooling (`cargo fmt`, `cargo clippy`).
- Capable of reading/modifying Cargo workspace layouts and writing documentation.

Follow the phases sequentially. After each phase:
1. Run `cargo fmt`, `cargo clippy -p platynui-xpath --all-targets -- -D warnings`, and `cargo test -p platynui-xpath` successfully.
2. Add/adjust tests covering both streaming and eager paths introduced in that phase.
3. Leave the repository in a releasable state.

### Global Constraints

- Implement **all** phases in this roadmap; do not stop early.
- Evaluate existing tests; extend them using `rstest` (fixtures, matrices) as needed—no property-testing frameworks are required (see `AGENTS.md` for guidance).
- No interim release deadlines; deliver when the full plan is complete.
- Invasive refactorings are acceptable; no external teams depend on this crate yet.
- Use Criterion (`cargo bench`) for profiling/benchmarking.

## Repository Layout
- Evaluator: `crates/platynui-xpath/src/engine/evaluator.rs`
- Runtime context: `crates/platynui-xpath/src/engine/runtime.rs`
- Sequence abstractions: `crates/platynui-xpath/src/xdm/mod.rs`
- Streaming tests: `crates/platynui-xpath/tests/evaluator_streaming.rs`
- Integration tests: `crates/platynui-xpath/tests/`

## Phase 1 – Infrastructure Foundations
1. **Sequence Cursor Traits**
   - Define a `SequenceCursor` trait supporting cloning and `size_hint` metadata.
   - Update `XdmSequenceStream` to wrap cursors, providing adapters for eager sequences.
2. **VM Snapshot Enhancements**
   - Finalize `VmSnapshot` with ownership of frames, locals, and context items.
   - Introduce `VmHandle` helpers for resuming evaluation with minimal allocation.
3. **Testing & Instrumentation**
    - Create unit tests for new cursors and add tracing to detect materialisation.

## Phase 2 – Streaming Predicates & Path Expressions
1. Implement lazy predicate evaluation tracking `position()`/`last()` without full materialisation.
2. Refactor `PathExprStep` to compose chained cursors.
3. Expand testing for nested predicates and positional filters; profile memory usage to confirm gains.

## Phase 3 – Set Operations & Deduplication
1. Build a document-order cursor supporting lazy ordering (heap/on-demand sorting).
2. Streamline `DocOrderDistinct`, `Union`, `Intersect`, and `Except` to operate over streaming inputs while maintaining deduplication semantics.
3. Add benchmarks and regression tests covering large datasets and verifying ordering/dedup under streaming conditions.

## Phase 4 – Control-Flow Opcodes
1. Replace `ForLoop`/`QuantLoop` with cursor-based implementations yielding each iteration lazily.
2. Streamline `Let` bindings so they reuse streaming values without materialisation; ensure scope frames remain correct.
3. Implement cooperative cancellation, audit lazy error propagation, and add tests for cancellation/error paths.

## Phase 5 – Optimisation & Polish
1. Profile the fully streaming evaluator under representative workloads; identify remaining hotspots (CPU, memory, allocations).
2. Apply targeted optimisations (e.g., avoiding redundant clones, shrinking buffers, micro-optimising hot cursor paths).
3. Double-check documentation, code comments, and examples; clean up any temporary instrumentation added during earlier phases.

## Phase 6 – Documentation & Migration
1. Document the streaming architecture, cursor APIs, and best practices for future opcodes.
2. Prepare migration notes/adapters for downstream consumers relying on eager behaviour.
3. Run full regression and benchmark suites;