# Repository Guidelines

## Project Structure & Module Organization
This workspace blends Rust crates and Python packages for the Robot Framework-facing PlatynUI library. Shared models live in `crates/platynui-core`, XPath parsing logic in `crates/platynui-xpath`, and a developer playground in `crates/playground`. The native service ships from `packages/server` (Rust via Maturin), while integration layers sit in `packages/core/src/platynui_core` with keywords in `src/PlatynUI`. Leave generated artefacts such as `target/`, `.venv/`, and `.vscode/` out of version control.

## Build, Test, and Development Commands
Run `uv sync --dev` once to install Python tooling into `.venv`. Use `cargo build --workspace` to compile all Rust crates and `cargo test -p platynui-xpath` for the current unit test suite; inside `packages/server`, `uv run maturin develop --release` generates the local native server. Format Rust with `cargo fmt --all`, lint Python via `uv run ruff check .`, and run `uv run mypy src/PlatynUI packages/core/src` before pushing.

## Coding Style & Naming Conventions
Adhere to Rust 2024 defaults: module paths and files remain `snake_case`, types `CamelCase`, and constants `SCREAMING_SNAKE_CASE`; run `cargo fmt` before pushing. Keep error plumbing consistent with existing `thiserror` implementations in the XPath crate. Python code follows PEP 8 with four-space indentation, descriptive module names, and Robot Framework keywords in Title Case (e.g., `Open Application`).

## Testing Guidelines
Rust tests live under `crates/platynui-xpath/tests` and use `rstest`; add nested modules when grouping related behaviors and name files `test_<feature>.rs`. Extend coverage whenever adjusting parsers or evaluators, and run `cargo test` before submitting; new Rust crates should co-locate an integration-style `tests/` directory registered with `rstest`. For Python additions, start building acceptance suites beside the package that exercise Robot keywords and document temporary run steps in your PR until a formal runner lands.

## Commit & Pull Request Guidelines
History follows a Conventional Commit style: `type(scope): imperative subject` such as `refactor(xpath): consolidate namespace handling`. Keep subjects under 72 characters, include context in the body, and group related changes per commit. Pull requests should describe the problem, the solution, and the affected platforms, link to issues, list the commands you ran (tests, linters, builds), and attach screenshots or logs when UI interactions change.

## Environment & Tooling Tips
Use the pinned Python from `.python-version` and keep `uv` up to date so lockfiles stay reproducible. Install the VS Code settings in `.vscode/` for rust-analyzer hints, and call out any OS-specific dependencies in your PR so maintainers can reproduce issues quickly.

## Project and Documentation Language
All code, comments, and documentation should be in English to ensure accessibility for the global developer community. Use clear and concise language, avoiding idioms or colloquialisms that may not be universally understood.

# Security and Privacy Considerations
Be mindful of sensitive data: do not hardcode secrets, API keys, or personal information in the codebase. Use environment variables or secure vaults for such data. Regularly review dependencies for vulnerabilities and keep them updated. If you discover a security issue, report it privately to the maintainers rather than disclosing it publicly.
