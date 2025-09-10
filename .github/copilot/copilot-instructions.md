# GitHub Copilot Instructions

## Priority Guidelines
When generating code for this repository:
1. Version Compatibility: Detect and respect actual versions in project files. Rust edition 2024 (toolchain observed: rustc 1.89.0 / cargo 1.89.0). Python requires >=3.10 (dev env observed 3.13.7). `uv` (>=0.8.15) is the Python package/dependency orchestrator. Do NOT use Python features requiring >3.13 specifics unless codebase already uses them. Keep compatibility with declared minimum (3.10) unless file already uses newer constructs.
2. Context First: Use this file and `.github/copilot-instructions.md` before performing broad searches.
3. Pattern Fidelity: Mirror existing naming, module exposure (e.g., `pub use` re‑exports in Rust, simple flat functions in Python package root), error handling, and evaluation design in the XPath engine.
4. Architectural Consistency: Mixed monolithic workspace: Rust crates + Python workspace. Keep clear boundaries: Rust core logic stays in crates; Python provides Robot Framework surface / packaging.
5. Quality Focus: Maintainability + Testability (no performance/security specific patterns yet). Keep code small, cohesive, and explicit. Avoid premature abstractions.

## Technology Version Detection
- Rust: Use features available in current stable (1.89) and 2024 edition; avoid nightly-only APIs.
- Python: Target `>=3.10`. Use type hints sparingly (currently minimal). Avoid adding heavy dependencies; declare any new dependency in the appropriate `pyproject.toml` then run `uv sync`.
- Robot Framework: Declared dependency `robotframework>=7.0.0`; only rely on APIs available since 7.0.
- Build/Packaging: `maturin` for `packages/server`; `uv_build` backend for pure Python modules.

## Codebase Patterns
Rust:
- Module exposure: central `lib.rs` re-exports key types and functions (`pub use ...`). Follow same style when adding new public API items.
- Evaluator style: Uses explicit VM struct with `run` loop, pattern matching on opcode enum. Extend by adding new `OpCode` arms and helper methods; keep error creation via `Error::...` patterns.
- Error handling: Returns `Result<_, Error>`; use existing constructors (`Error::not_implemented`, `Error::dynamic_err`). Avoid panics for controllable errors.
- Naming: snake_case modules, PascalCase types, concise function names (`evaluate`, `evaluate_expr`). Boolean helpers prefixed logically (`compare_value`, etc.).
- Generics: Constraint pattern `N: 'static + Send + Sync + XdmNode + Clone` repeated—maintain consistency if introducing functions over node types.
Python:
- Minimal surface now (`dummy_keyword`). Add Robot Framework keywords as plain functions or methods; keep naming descriptive and snake_case.
- Keep prints only for provisional behavior; prefer returning values when finalizing APIs.

## Pattern Resolution Rules
When conflicting patterns emerge:
1. Prefer code in `crates/platynui-xpath/src` over any staging content under `trash/`.
2. Prefer re-export pattern for public API exposure rather than exposing deep internal paths in consumer code.
3. Avoid introducing alternative parsing/eval frameworks; extend existing VM model.

## Build & Execution Commands (Authoritative)
Always use these sequences; search only if they fail.
```bash
# Sync / install Python workspace deps
uv sync

# Build Rust (all targets)
cargo build --all --all-targets

# Run Rust tests
cargo test --all --no-fail-fast

# Lint Python (ruff installed as tool or via uv run)
ruff check
# or
uv run ruff check

# Type check (when type hints added)
uv run mypy src

# Package server wheel (only if needed for distribution)
uv run maturin build -m packages/server/pyproject.toml --no-sdist
```
Never invoke `pip install` directly. Always regenerate lock impacts via `uv sync` after editing any `pyproject.toml`.

## Testing Guidance
Current Rust tests are effectively empty scaffold. When adding tests:
- Place new evaluator tests under `crates/platynui-xpath/tests/` with descriptive filenames (`evaluator_<feature>.rs`).
- Use `rstest` (already a dev-dependency) for parameterized cases if patterns emerge.
- Keep tests focused: compile expression, build minimal `DynamicContext`, call `evaluate_expr`, assert on sequence content.
Python tests absent—if adding, prefer `tests/` root or package-specific `tests/` directory and run via `uv run pytest` (add pytest to dev dependency group first). Only add once a real API exists.

## Documentation & Comments
- Rust: Code is sparsely documented; when adding complex logic provide brief `///` doc comments and inline clarifying comments directly above non-obvious match arms or algorithms.
- Python: Add docstrings for new keyword functions describing Robot Framework usage (Arguments, Returns). Keep concise.

## Error Handling & Logging
- Rust: Use existing `Error` type; if new categories required, extend centrally rather than ad-hoc strings scattered.
- Avoid adding logging framework prematurely; prefer returning rich errors.

## Dependency Management Rules
- Python: Update `[project]` or appropriate extras; keep minimal. Run `uv sync` then commit `pyproject.toml` + `uv.lock`.
- Rust: Add dependencies inside specific crate `Cargo.toml`. Only expose via re-export if part of public API contract.

## Security & Performance
- No explicit security model yet; do not introduce unsafe Rust blocks unless strictly necessary and documented.
- Performance: Favor clarity first. Micro-optimizations only if a demonstrated hotspot.

## Introducing New Features
Follow this micro-process:
1. Identify target crate or package (core logic typically Rust).
2. Add minimal API + tests (Rust) or keyword (Python).
3. Run full validation sequence (sync, build, test, lint).
4. Update re-exports in `lib.rs` if new public items.
5. If cross-language artifact needed, add via `maturin` project not ad-hoc build scripts.

## Versioning & Releases
- Workspace versions currently `0.1.0` (pre-stable). Breaking changes acceptable but prefer additive changes; document rationale in PR description.

## Prohibited / Avoid
- Introducing alternate Python dependency managers (`pipenv`, `poetry`).
- Using nightly Rust features or unstable compiler flags.
- Committing generated build artifacts beyond lock files and logs intended for review.
- Adding verbose logging / println! left in production paths (tests OK).

## Adding an OpCode (Concrete Example Steps)
1. Define new variant in opcode enum (search for `enum OpCode`).
2. Extend compiler to emit new opcode.
3. Handle variant in `Vm::run` match (group logically near related ops).
4. Add helper methods if complex (impl block below `run`).
5. Add tests demonstrating semantics.

## Consistency Mandate
When unsure, replicate closest existing pattern precisely. Do NOT invent architecture layers, dependency injection frameworks, or alternative execution engines.

## Search Triggers
Only search if:
- A symbol referenced here cannot be found.
- A build command fails with novel error.
- You need to confirm existing pattern before extending.
Otherwise rely on these instructions to minimize workspace scanning overhead.

## Final Checklist Before Submitting Changes
1. `uv sync`
2. `cargo build --all --all-targets`
3. `cargo test --all`
4. `ruff check`
5. (Optional) `uv run mypy src`
6. Confirm no new warnings (or justify). Apply `cargo fix` if trivial.

Adhere strictly to these patterns to maximize acceptance probability.
