# Contributing to PlatynUI

Thanks for helping build PlatynUI! This guide describes how to get set up, our coding standards, and the quality gates we expect before merging.

## 1) Prerequisites

- Rust: Stable toolchain (repo developed with rustc/cargo ~1.90). Install via rustup.
- Python: 3.10+ and uv ≥ 0.8.15. Do not use pip directly in this repo.
- Tools: cargo, uv, and (recommended) GPG for signed commits.

Bootstrap once:

```pwsh
uv sync
```

This creates `.venv` and installs dev tools (ruff, mypy, maturin, robotframework) and local packages.

## 2) Project layout (quick orientation)

- Rust workspace in `crates/*` and `apps/*` (core, xpath, runtime, providers/platforms, cli, inspector).
- Python packages in `packages/*` (native bindings, CLI, inspector) and RF library entry in `src/PlatynUI`.

## 3) Branching, commits, and PRs

- Use Conventional Commits: `type(scope): subject` (e.g., `feat(runtime): add window resize action`).
- Keep subjects ≤ 72 chars; describe “what/why” in the body; link issues/PRs.
- Sign commits when possible (`git config commit.gpgsign true`).
- Small, focused PRs with clear rationale and “how to verify” notes.

## 4) Dev workflow (green-before-merge)

Run these in the repo root before pushing:

```pwsh
uv sync --dev --all-packages --all-extras
cargo build --all --all-targets
cargo test --all --no-fail-fast
uv run ruff check
# Optional when you add hints
uv run mypy .
```

Targets that change public behavior should include/update tests.

## 5) Coding standards

Rust:
- Edition 2024; follow existing naming (snake_case functions, PascalCase types).
- Prefer typed errors (thiserror) in library crates; avoid panics in normal flows.
- Keep JSON/serde usage consistent; do not add alternate JSON libs.
- Use `rstest` for fixtures/parametrization; keep tests small and deterministic.

Python:
- 3.10+; keep dependencies minimal. Lint with ruff; optional typing with mypy.
- Robot Framework keywords: Title Case (e.g., `Open Application`). Avoid `print`; return values instead.

CLI/Inspector (apps):
- Cross‑platform providers are linked via the `platynui_link_providers!` macro and Cargo target cfgs; follow the existing pattern.

## 6) Dependencies

- Rust: add to the crate’s `Cargo.toml`; build to update `Cargo.lock`.
- Python: edit `pyproject.toml`, then always run `uv sync`. Commit both the `pyproject.toml` and updated `uv.lock`.
- Prefer small, widely‑used, stable libraries. Justify heavyweight deps in the PR.

## 7) Testing guidance

Rust:
- Unit tests live alongside code; integration tests under `tests/` per crate.
- Use the mock provider/platform for deterministic tests. For manual runs, enable with `--features mock-provider` (some crates enable it via dev‑deps automatically).

Python:
- Currently no Python test suite by default. If adding tests, prefer `pytest` under the relevant package and run via `uv run pytest`.

End‑to‑end / acceptance:
- When Robot suites are added, run via `uv run robot -d results tests` (document in PRs until a formal runner lands).

## 8) Adding or changing public APIs

- Rust public APIs: update crate modules and re‑export in `lib.rs` if part of external surface. Keep breaking changes minimal and documented in the PR.
- XPath engine changes: add targeted tests under `crates/platynui-xpath/tests/` following existing naming (e.g., `evaluator_*.rs`, `parser_*.rs`).
- Python RF library: extend `src/PlatynUI/__init__.py` or new modules imported there. Keep keyword names stable; document changes in README.

## 9) Packaging and release (preview)

- Pre‑release wheels for CLI/Inspector may be published to PyPI. End‑users should install with pre‑release flags:

```pwsh
uv pip install --pre robotframework-platynui-cli
uv pip install --pre robotframework-platynui-inspector
uv tool install --prerelease allow robotframework-platynui-cli
uv tool install --prerelease allow robotframework-platynui-inspector
```

- Only package when explicitly needed; prefer source builds during development.

## 10) Documentation

- Keep README files accurate and concise. Link to package READMEs for CLI/Inspector details.
- Architecture and plans live under `docs/` (some in German). Update relevant docs with any non‑trivial design change and add a brief English summary when possible.

## 11) Security & privacy

- Do not commit secrets or personal data. Use environment variables or secure stores.
- Review dependencies for vulnerabilities; note relevant CVEs or fixes in PR descriptions when upgrading.

---

Questions? Open an issue or start a discussion. Thank you for contributing to PlatynUI!
