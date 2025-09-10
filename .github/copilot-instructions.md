<!-- Keep under ~2 printed pages. Authoritative quickstart for automated agents. -->
# Copilot Coding Agent Instructions

Trust this file first. Only search the repo when information is missing or demonstrably wrong.

## 1. Purpose & Summary
PlatynUI is an experimental cross‑platform UI automation toolkit for Robot Framework. Core logic (data types + XPath 2.0-ish engine) lives in Rust crates; Python provides a Robot Framework library surface and packaging (including a Rust server binary via `maturin`). Project is early stage: expect gaps, evolving APIs, sparse docs.

## 2. Tech & Tooling (Authoritative)
Languages: Rust 2024 edition (tested rustc/cargo 1.89.0), Python >=3.10 (dev seen 3.13.7), Robot Framework >=7.0.
Primary tools: `uv` (ALWAYS for Python deps + scripts), `cargo` (Rust build/test), `maturin` (packaging server), `ruff` (lint), `mypy` (optional types).
Never use `pip install`; always `uv sync` after editing any `pyproject.toml`.

## 3. Repository Layout (High Value Paths)
Root configs: `Cargo.toml`, `pyproject.toml`, locks: `Cargo.lock`, `uv.lock`.
Python RF library: `src/PlatynUI/__init__.py`.
Rust crates:
  - Core types: `crates/platynui-core/src/`
  - XPath engine: `crates/platynui-xpath/src/` (parser/, compiler/, evaluator.rs, runtime.rs, functions.rs, xdm.rs)
  - Playground: `crates/playground/` (experiments, not API source)
Python packages workspace: `packages/core/` (placeholder), `packages/server/` (Rust binary packaged).
Generated / artifacts: `target/` (Rust), `.venv/` (virtual env), `results/` (Robot output if tests added later).
Tests: Numerous Rust tests under `crates/platynui-xpath/tests` and unit tests inside `platynui-core`. `trash/` contains legacy / staging tests—ignore for new work unless migrating.

## 4. Bootstrap & Environment
Prereqs: rustc & cargo (1.89+), Python 3.10+, `uv >=0.8.15` installed.
Bootstrap ALWAYS:
```bash
uv sync
```
Effect: creates/refreshes `.venv`, installs dev tools (ruff, mypy, maturin, robotframework) and local packages. Safe to re-run any time (fast: <0.1s warm).

## 5. Core Command Sequences
Build (Rust all targets):
```bash
cargo build --all --all-targets
```
Test (Rust):
```bash
cargo test --all --no-fail-fast
```
Lint (Python):
```bash
uv run ruff check
```
Type check (optional when adding hints):
```bash
uv run mypy src
```
Package server wheel (only if distribution explicitly needed):
```bash
uv run maturin build -m packages/server/pyproject.toml --no-sdist
```
Robot Framework test run placeholder (only if a `tests/` RF suite appears):
```bash
uv run robot -d results tests
```
ALWAYS run (in order) before submitting PR changes that touch code:
1. `uv sync`
2. `cargo build --all --all-targets`
3. `cargo test --all --no-fail-fast`
4. `uv run ruff check`
5. (Optional) `uv run mypy src`

## 6. Adding / Modifying Code
Rust public API: update inside the specific crate then re-export in its `lib.rs` if part of external surface. Keep error handling via existing `Error` patterns; avoid panics except truly unreachable logic.
XPath engine changes: modify relevant module in `crates/platynui-xpath/src/`; add focused tests in `crates/platynui-xpath/tests/` mirroring existing naming (e.g., `evaluator_<feature>.rs` or `parser_<aspect>.rs`).
New Rust crate: place under `crates/` and ensure workspace membership (root `Cargo.toml` already glob-includes; usually no edit required). Build to verify.
Python keyword additions: extend `src/PlatynUI/__init__.py` or introduce modules imported there; keep names snake_case and return values (avoid print side-effects). If adding a new Python package, list it in `[tool.uv.workspace].members` then `uv sync`.
Rust ↔ Python boundary changes: confine to `packages/server/`. Don’t mix binding code into core logic crates.

## 7. Style & Conventions
Rust: Edition 2024. Follow existing naming (snake_case functions, PascalCase types). Keep generics bounds consistent with existing node trait patterns. Use `cargo fix` for trivial warnings (e.g., unused parentheses). Use `serde` for (de)serialization—do NOT add alternate JSON libs.
Python: Keep 3.10+; minimal dependencies. Use ruff to satisfy formatting/lint; avoid introducing black/isort separately (ruff already covers). Type hints optional but if added ensure mypy passes.

## 8. Testing Notes
Current state (validated 2025-09-10): Rust suite passes quickly (<2s) with many parser/evaluator tests + 31 core type tests. Zero Python tests. Prefer small, deterministic Rust tests. If adding Python tests later: add `pytest` to dev deps, create `tests/` directory, run via `uv run pytest` (document if you do so).

## 9. Dependency & Version Management
Python: Modify `pyproject.toml`, then ALWAYS `uv sync` (committing both the modified `pyproject.toml` and updated `uv.lock`).
Rust: Add dependencies to the crate’s own `Cargo.toml`; build to regenerate `Cargo.lock`. Never hand‑edit lock files.
Avoid adding heavyweight dependencies without clear need; prefer standard library first.

## 10. Common Pitfalls / Remedies
Missing ruff binary: use `uv run ruff check` (works without standalone install) or `uv tool install ruff` if needed globally.
Edition mismatch or unresolved crate: ensure crate directory under `crates/` and that root workspace `Cargo.toml` pattern includes it (it does by default).
Maturin complaints: re-run `uv sync` to ensure it’s installed; invoke through `uv run` not the global path.
Stray `pip install` usage will desync environment—never do this; replace with `uv add <pkg>` (which edits pyproject + lock) then commit.

## 11. Performance Expectations
`uv sync` warm: ~<0.1s, `cargo build` dev: ~0.03–0.08s incremental, full test run: ~2s. These fast cycles justify running full sequence before every commit.

## 12. Search Guidance
Only search when: a referenced symbol/path here is missing; a documented command fails; or you need to inspect an existing pattern before extending functionality. Otherwise rely on this file to minimize noisy scanning.

## 13. Submission Checklist (Agent MUST follow)
1. Update code & tests.
2. `uv sync`
3. `cargo build --all --all-targets`
4. `cargo test --all --no-fail-fast`
5. `uv run ruff check` (clean output)
6. (Optional) `uv run mypy src` (if hints added)
7. Remove accidental debug prints / unwraps.
8. Summarize changes + rationale clearly in PR.

## 14. Maintain This Document
Only adjust when toolchain versions, layout, or mandatory steps materially change. Keep concise—do not bloat beyond 2 pages.

_End. Trust this document first._
