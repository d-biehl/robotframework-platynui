<!-- Keep under ~2 printed pages. Authoritative quickstart for automated agents. -->
# Copilot Coding Agent Instructions

Trust this file first. Only search the repo when information is missing or demonstrably wrong.

## 1. Purpose & Summary
PlatynUI is an experimental cross‑platform UI automation toolkit for Robot Framework. Core logic (data types + XPath 2.0-ish engine) lives in Rust crates; Python provides a Robot Framework library surface and packaging (including a Rust native binary via `maturin`). Project is early stage: expect gaps, evolving APIs, sparse docs.

## 2. Tech & Tooling (Authoritative)
Languages: Rust 2024 edition (MSRV 1.90, current dev toolchain 1.93), Python >=3.10 (pinned via `.python-version`, managed by `uv`), Robot Framework >=7.0.
Primary tools: `uv` (ALWAYS for Python deps + scripts — manages its own Python), `cargo` (Rust build/test), `maturin` (packaging native), `ruff` (lint), `mypy` (optional types).
Never use `pip install`; always `uv sync` after editing any `pyproject.toml`.

## 3. Repository Layout (High Value Paths)
Root configs: `Cargo.toml`, `pyproject.toml`, `rustfmt.toml`, locks: `Cargo.lock`, `uv.lock`.
Workspace members: `crates/*`, `apps/*`, `packages/*`.

Rust crates (`crates/`):
  - `core` — shared traits/types: UiNode, UiAttribute, UiPattern, registries, platform traits
  - `xpath` — XPath engine: parser/, compiler/, evaluator, runtime, functions, xdm
  - `runtime` — provider/device orchestration, XPath pipeline, focus/window actions
  - `cli` — CLI for queries, highlight, keyboard/pointer, diagnostics
  - `server` — JSON-RPC façade (stub)
  - `link` — dynamic linking utilities for platform providers
  - `platform-{windows,linux-x11,macos,mock}` — OS device bundles, highlight/screenshot, desktop info
  - `provider-{windows-uia,atspi,macos-ax,jsonrpc,mock}` — UiTreeProvider implementations
  - `playground` — experiments, not API source

Apps (`apps/`):
  - `inspector` — GUI inspector (Slint-based)

Python packages (`packages/`):
  - `native` — Maturin-based native Python package (bindings to core/runtime)
  - `cli` — Python packaging wrapper for the CLI binary
  - `inspector` — Python packaging wrapper for the inspector binary

Python RF library: `src/PlatynUI/`.
Generated / artifacts: `target/` (Rust), `.venv/` (virtual env), `results/` (Robot output).

## 4. Bootstrap & Environment
Prereqs: rustc & cargo (MSRV 1.90+, recommend current stable), Python 3.10+ (managed by `uv` via `.python-version`), `uv` installed.
Bootstrap ALWAYS:
```bash
uv sync --dev --all-packages --all-groups --all-extras
```
Effect: creates/refreshes `.venv`, installs dev tools (ruff, mypy, maturin, robotframework) and all local packages including extras. Safe to re-run any time (fast: <0.1s warm).

## 5. Core Command Sequences
Build (Rust all targets):
```bash
cargo build --all --all-targets
```
Test (Rust):
```bash
cargo nextest run --all --no-fail-fast
```
Format (Rust):
```bash
cargo fmt --all
```
Lint (Rust — strict, treats warnings as errors):
```bash
cargo clippy --workspace --all-targets -- -D warnings
```
Lint (Python):
```bash
uv run ruff check
```
Type check (optional when adding hints):
```bash
uv run mypy .
```

ALWAYS run (in order) before submitting PR changes that touch Rust code:
1. `uv sync --dev --all-packages --all-groups --all-extras`
2. `cargo fmt --all`
3. `cargo build --all --all-targets`
4. `cargo clippy --workspace --all-targets -- -D warnings`
5. `cargo nextest run --all --no-fail-fast`
6. `uv run ruff check` (if Python touched)
7. (Optional) `uv run mypy .` (if type hints added)

## 6. Adding / Modifying Code
Rust public API: update inside the specific crate then re-export in its `lib.rs` if part of external surface. Keep error handling via existing `Error` patterns; avoid panics except truly unreachable logic.
XPath engine changes: modify relevant module in `crates/xpath/src/`; add focused tests in `crates/xpath/tests/` mirroring existing naming (e.g., `evaluator_<feature>.rs` or `parser_<aspect>.rs`).
New Rust crate: place under `crates/` and ensure workspace membership (root `Cargo.toml` uses `crates/*` glob—usually no edit required). Build to verify.
Python keyword additions: extend `src/PlatynUI/__init__.py` or introduce modules imported there; keep names snake_case and return values (avoid print side-effects). If adding a new Python package, list it in `[tool.uv.workspace].members` then `uv sync`.
Rust ↔ Python boundary changes: confine to `packages/native/`. Don’t mix binding code into core logic crates.

## 7. Style & Conventions
Rust: Edition 2024. Follow existing naming (snake_case functions, PascalCase types). Keep generics bounds consistent with existing node trait patterns. Use `cargo fix` for trivial warnings. Use `serde` for (de)serialization—do NOT add alternate JSON libs. `rustfmt.toml` sets `max_width = 120`.
Workspace lints (enforced via `Cargo.toml`): `unsafe_code = "forbid"`, `unused_must_use = "deny"`, `clippy::pedantic = "warn"`.
Python: Keep 3.10+; minimal dependencies. Use ruff to satisfy formatting/lint; avoid introducing black/isort separately (ruff already covers). Type hints optional but if added ensure mypy passes.

## 8. Testing Notes
Rust suite: ~1700 tests across all crates, runs in <15s. Prefer small, deterministic Rust tests. If adding Python tests: use `pytest` (already in dev deps), run via `uv run pytest`.

## 9. Dependency & Version Management
Python: Modify `pyproject.toml`, then ALWAYS `uv sync` (committing both the modified `pyproject.toml` and updated `uv.lock`).
Rust: Add dependencies to the crate's own `Cargo.toml`; build to regenerate `Cargo.lock`. Do NOT use `[workspace.dependencies]` for `tracing` (maturin incompatibility). Never hand‑edit lock files.
Avoid adding heavyweight dependencies without clear need; prefer standard library first.

## 10. Common Pitfalls / Remedies
Missing ruff binary: use `uv run ruff check` (works without standalone install) or `uv tool install ruff` if needed globally.
Edition mismatch or unresolved crate: ensure crate directory under `crates/` and that root workspace `Cargo.toml` pattern includes it (it does by default).
Maturin complaints: re-run `uv sync` to ensure it’s installed; invoke through `uv run` not the global path.Native Python packages are built with `uv run maturin develop -m packages/<name>/Cargo.toml --uv` (e.g., `packages/native/Cargo.toml`, `packages/cli/Cargo.toml`, `packages/inspector/Cargo.toml`). The `--uv` flag is required because the project uses uv workspaces.Stray `pip install` usage will desync environment—never do this; replace with `uv add <pkg>` (which edits pyproject + lock) then commit.

## 11. Performance Expectations
`uv sync` warm: ~<0.1s, `cargo build` dev: ~0.03–0.08s incremental, full test run: ~15s. These fast cycles justify running full sequence before every commit.

## 12. Logging & Tracing
Use the `tracing` crate for all Rust diagnostic output. Each crate specifies its own dep: `tracing = { version = "0.1", default-features = false, features = ["std"] }`. Binary crates (CLI, Inspector) add `tracing-subscriber` and are the only places that initialize a subscriber. Log levels: `error` (unexpected failures not in Result), `warn` (degraded/fallbacks), `info` (one-time lifecycle), `debug` (operational details), `trace` (hot-path per-item). Log output goes to stderr. See `.github/instructions/tracing.instructions.md` for full conventions.

## 13. Search Guidance
Only search when: a referenced symbol/path here is missing; a documented command fails; or you need to inspect an existing pattern before extending functionality. Otherwise rely on this file to minimize noisy scanning.

## 14. Submission Checklist (Agent MUST follow)
1. Update code & tests.
2. `uv sync --dev --all-packages --all-groups --all-extras`
3. `cargo fmt --all`
4. `cargo build --all --all-targets`
5. `cargo clippy --workspace --all-targets -- -D warnings`
6. `cargo nextest run --all --no-fail-fast`
7. `uv run ruff check` (if Python changed — clean output)
8. (Optional) `uv run mypy .` (if type hints added)
9. Remove accidental debug prints / unwraps.
10. Summarize changes + rationale clearly in PR.

## 15. Maintain This Document
Only adjust when toolchain versions, layout, or mandatory steps materially change. Keep concise—do not bloat beyond 2 pages.

_End. Trust this document first._
