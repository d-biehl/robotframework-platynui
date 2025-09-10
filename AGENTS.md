# AGENTS.md

Authoritative, agent‑focused handbook for working on PlatynUI. Optimized for fully automated coding agents (and helpful for humans). Complements `README.md` (human overview) and `.github/copilot-instructions.md` (quickstart). If instructions ever conflict: `.github/copilot-instructions.md` > this file > README.

## 1. Project Overview
PlatynUI is an experimental cross‑platform UI automation toolkit for Robot Framework providing:
- Rust core for data models + an XPath 2.0–ish evaluation engine
- A Python Robot Framework library surface (keywords) plus a packaged Rust server binary (maturin)
- Goal: consistent UI element identification / interaction across Linux (X11 / AT‑SPI2), Windows (UIA), macOS (Accessibility API)

Status: Early stage (expect missing features / evolving APIs). Primary active domains: XPath engine correctness & core type system.

## 2. Architecture Snapshot
Layers (bottom → top):
1. `crates/platynui-core` – Core data types, internal traits, shared utilities.
2. `crates/platynui-xpath` – Parser, compiler, evaluator/runtime, XPath/XDM types & standard functions.
3. (Future) UI backend crates (none yet public here).
4. `packages/server` – Rust binary exposed to Python (maturin packaged, bindings = `bin`).
5. `src/PlatynUI` – Robot Framework library (Python) exposing keywords.

Supporting areas:
- `crates/playground` – Experiments (NOT production API).
- Tests: Rust unit + integration tests inside crates (esp. `platynui-xpath/tests/`). Legacy / staging tests in `trash/` (ignore unless migrating).

## 3. Toolchain & Versions
- Rust edition: 2024 (cargo ≥1.89). All crates share workspace root `Cargo.toml`.
- Python: ≥3.10 (dev often uses 3.13). Package management: `uv` ONLY.
- Robot Framework: ≥7.0.
- Lint: `ruff`; Types (optional): `mypy`.
- Packaging (server): `maturin` (invoked via `uv run`).
- No `pip install`—ever. Use `uv add` & `uv sync`.

## 4. Environment Bootstrap
Prerequisites installed globally: Rust toolchain, `uv` (>=0.8.15), Python >=3.10.

Run once (safe to repeat):
```bash
uv sync
```
Creates / refreshes managed `.venv`, installs dev tools and local packages.

## 5. Core Development Commands (Rust + Python)
Build everything (dev artifacts):
```bash
cargo build --all --all-targets
```
Run full Rust test suite (fast, < ~2s typical):
```bash
cargo test --all --no-fail-fast
```
Python lint:
```bash
uv run ruff check
```
Optional type check (only if you added hints):
```bash
uv run mypy src
```
Package server wheel (on demand):
```bash
uv run maturin build -m packages/server/pyproject.toml --no-sdist
```
(Artifacts emitted under `target/wheels/` by maturin.)

Robot Framework test execution placeholder (if / when `tests/` RF suites are added):
```bash
uv run robot -d results tests
```

## 6. Standard Contribution Sequence
Always perform (in order) before pushing or opening a PR that touches code:
```bash
uv sync
cargo build --all --all-targets
cargo test --all --no-fail-fast
uv run ruff check
# Optional:
uv run mypy src
```
Ensure zero warnings you introduced; remove stray debug `println!`, `dbg!`, `unwrap()` where avoidable.

## 7. Adding / Modifying Rust Code
- Public API: define in crate module files and re-export via that crate's `lib.rs` if intended for external usage.
- New crate: create under `crates/<name>` → cargo will include via workspace glob (no root `Cargo.toml` edit normally). Then:
  ```bash
  cargo build -p <name>
  cargo test -p <name>
  ```
- Tests: prefer small, descriptive files using existing naming patterns e.g. `evaluator_<aspect>.rs`, `compiler_<topic>.rs` in `crates/platynui-xpath/tests/`.
- Add dependencies: edit that crate's `Cargo.toml`, then rebuild. Avoid heavy crates unless essential; prefer std + existing deps.
- Error handling: use existing error enums / results; avoid `panic!` except unreachable conditions.

### XPath Engine Changes
Edit appropriate module under `crates/platynui-xpath/src/`:
- Grammar / parsing: `parser/`
- Compilation passes: `compiler/`
- Evaluation: `evaluator.rs`, `runtime.rs`, function library: `functions.rs`
Add or adapt tests alongside others to cover new syntax, semantics, edge cases (temporal, numeric, collation, namespace, set operations, etc.).

## 8. Python Layer Changes
- Keywords live (currently) in `src/PlatynUI/__init__.py`. Add functions (snake_case) returning values (avoid prints). Robot Framework docstrings helpful but optional for now.
- New Python package: create under `packages/` and add its path to `[tool.uv.workspace].members` in root `pyproject.toml`, then:
  ```bash
  uv sync
  ```
- Adding a Python dependency: use `uv add <package>` (optionally with `--group dev`), commit updated `pyproject.toml` + `uv.lock`.

## 9. Server (Rust ↔ Python Boundary)
- All packaging / FFI boundaries isolated in `packages/server`.
- Do not introduce binding or Python-specific logic inside core logic crates.
- To adjust packaging metadata edit `packages/server/pyproject.toml` (maturin section). Rebuild wheel only if external distribution required.

## 10. Code Style & Conventions
Rust:
- Edition 2024 idioms; snake_case for functions/modules, PascalCase for types.
- Use `cargo fmt` only if repository later adds a rustfmt config (currently rely on existing style; keep diffs minimal).
- Prefer explicit error propagation (`?`), avoid unnecessary clones.
- Serialization: `serde` (single canonical choice).

Python:
- Minimal dependencies; keep imports explicit.
- Run `uv run ruff check` (ruff handles lint + formatting). Fix all reported issues before commit.
- Optional typing accepted; if added ensure `uv run mypy src` passes.

General:
- Avoid speculative abstractions; prefer incremental, well-tested additions.
- Keep functions short & focused.

## 11. Testing Strategy
Rust tests are authoritative for correctness now.
- Location: `crates/*/tests` + inline `#[cfg(test)]` modules.
- Naming: Group by concern (`evaluator_*.rs`, `compiler_*.rs`, function domain files). Mirror existing style for consistency.
- Add at least one failing test first when fixing a bug; then implement fix until green.
- Edge cases to consider: empty sequences, namespaces, temporal arithmetic & casting, numeric precision, collation & ordering, node identity vs deep equality, variable scoping.

Potential future Python tests (none yet): if introduced, add `pytest` to `[dependency-groups.dev]` then run via:
```bash
uv run pytest -q
```
Document any new test layout here if added.

## 12. Performance Notes
- Full Rust test pass targeted under ~2s incremental.
- Expect incremental `cargo build` well below 0.1s for small edits.
- Keep performance-sensitive XPath evaluation code allocation-light; prefer iterators & borrowing.

## 13. Dependency Management
Python:
- Add runtime dep: `uv add <pkg>` (updates lock). Dev dep: `uv add --group dev <pkg>`.
- After manual pyproject edits: `uv sync`.
Rust:
- Add crate dep in specific crate's `Cargo.toml`; run `cargo build` to update `Cargo.lock`.
- Do not hand-edit lock files.

## 14. Security & Secrets
- No secrets should be committed; repo currently has no secret management tooling.
- Do not embed credentials in tests or examples.
- When adding external process interaction later (e.g., OS automation backends) ensure proper sandboxing & input validation.

## 15. Pull Request Guidelines
Before opening / updating a PR:
1. Run the full sequence (bootstrap + build + test + lint (+ optional mypy)).
2. Ensure new tests added for bug fixes or new features.
3. Keep commits logically scoped; message format: concise imperative ("Add X", "Fix Y crash in Z").
4. PR title: `<area>: <short description>` (example: `xpath: add substring-after edge cases`).
5. Describe rationale + any tradeoffs (esp. performance / API changes).
6. Confirm no stray debug prints.

## 16. Debugging & Troubleshooting
Common issues:
- Missing ruff / maturin: rerun `uv sync`.
- Unresolved crate path: ensure placed under `crates/` and workspace build again.
- Test discovery gap: confirm filename pattern matches existing style (e.g., `evaluator_*.rs`).
- `maturin` build errors about metadata: ensure `[tool.maturin] bindings = "bin"` retained.
- Python import problem: verify `.venv` active (or always use `uv run <cmd>` for isolation).

Logging / Inspection:
- Prefer targeted `dbg!(...)` during dev; remove before commit.
- Add focused unit tests instead of permanent verbose logging.

## 17. Extending the XPath Engine (Quick Checklist)
1. Add or modify grammar (parser) if syntax change.
2. Update compiler stage(s) for new AST node(s).
3. Implement evaluation logic in runtime / evaluator.
4. Add function(s) in `functions.rs` if standard function being added.
5. Write positive tests + edge / error tests.
6. Verify no regressions (`cargo test --all`).

## 18. Release (Future Guidance Placeholder)
No formal release pipeline yet. When preparing a release:
- Bump versions in workspace (Rust & Python) consistently.
- Build server wheel: `uv run maturin build -m packages/server/pyproject.toml --no-sdist`.
- Tag commit and publish (future automation TBD).

## 19. Quick Reference (Copy/Paste)
Bootstrap:
```bash
uv sync
```
Build + Test + Lint (full cycle):
```bash
uv sync && cargo build --all --all-targets && cargo test --all --no-fail-fast && uv run ruff check
```
Add Python dep:
```bash
uv add <package>
```
Add dev Python dep:
```bash
uv add --group dev <package>
```
Run specific crate tests:
```bash
cargo test -p platynui-xpath -- --nocapture
```
Run mypy (optional):
```bash
uv run mypy src
```
Package server wheel:
```bash
uv run maturin build -m packages/server/pyproject.toml --no-sdist
```

## 20. Maintenance of This Document
Update when:
- New crates / layers introduced
- Test strategy changes (e.g., Python test suite added)
- Toolchain version bumps impact commands
- Release process formalized

Keep concise, actionable, and consistent with `.github/copilot-instructions.md` (do not duplicate entire content; link or reference instead).

---
If something here fails: re-check `.github/copilot-instructions.md` then adapt. When in doubt favor minimal, well-tested changes.
