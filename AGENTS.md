# Repository Guidelines

## Project Structure & Module Organization
- `src/PlatynUI/` — Python Robot Framework library (entry module `PlatynUI`).
- `tests/*.robot` — Robot Framework suites.
- `crates/platynui-core` — shared Rust utilities and types.
- `crates/platynui-xpath` — XPath parser (Pest grammar + tests).
- `packages/server` — Rust binary exposed to Python via Maturin.
- `packages/core` — Python core package scaffold.
- `target/` — Rust build artifacts (ignored).

## Build, Test, and Development Commands
- Environment (Python + dev tools): `uv venv && uv sync --group dev`
- Python format/lint: `uv run ruff format . && uv run ruff check .`
- Type check (Python): `uv run mypy src`
- Robot tests: `uv run robot tests`
- Rust build/test: `cargo build` and `cargo test -p platynui-xpath`
- Run server locally: `cargo run -p platynui-server`
- Build Python wheels: root `uv build`; server `maturin build -m packages/server/pyproject.toml`

## Coding Style & Naming Conventions
- Python: 4-space indent, type hints for public APIs, modules and functions `snake_case`, classes `CapWords`. Keep files under `src/PlatynUI`.
- Rust: format with `cargo fmt`, lint with `cargo clippy`. Use `snake_case` for modules/functions, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for consts.
- Robot: keyword names in Title Case; test/suite names descriptive (e.g., `tests/xpath_parsing.robot`).

### Language for Comments & Docs
- Use English for all code comments and developer-facing documentation.
- Prefer clear, concise phrasing and keep comments close to the code they explain.
- If external specifications or plans are in another language, mirror key developer notes in English within the code where relevant.

## Testing Guidelines
- Robot Framework suites live in `tests/`; group by feature and keep tests deterministic (avoid sleeps). Run with `uv run robot tests`.
- Rust: unit tests inline and in `crates/*/tests`. Prefer small, focused cases for grammar and parsing.
- Rust tests use `rstest` exclusively. Write tests with `#[rstest]`, define reusable `#[fixture]`s, and use `#[case]`/`#[values]` for parameterized inputs; avoid plain `#[test]` unless strictly necessary.
- Add tests with fixes/features; ensure they fail before the change and pass after.

## Commit & Pull Request Guidelines
- Use Conventional Commits: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`. Include scope when helpful (e.g., `feat(xpath): normalize quotes`).
- PRs: clear description, linked issues, rationale, and how to test; include logs or screenshots when behavior changes. Keep changes minimal and focused.

## Security & Configuration Tips
- Toolchains: Python 3.10+; latest stable Rust. Use `uv`-managed venvs; don’t commit secrets. Optional: run `cargo audit` if available.
- CI/local: run lint, type-check, tests for both Python and Rust before opening a PR.
