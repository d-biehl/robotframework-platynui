# Repository Guidelines

## Project Structure & Module Organization
This workspace blends Rust crates and Python packages for the Robot Framework-facing PlatynUI library. The current layout is:

- Rust workspace (`cargo`):
  - `crates/core` → crate `platynui-core` (shared traits/types: UiNode, UiAttribute, UiPattern, registries, platform traits)
  - `crates/xpath` → crate `platynui-xpath` (XPath evaluator, parser helpers, benches/tests)
  - `crates/runtime` → crate `platynui-runtime` (provider/device orchestration, XPath pipeline, focus/window actions)
  - `crates/server` → crate `platynui-server` (JSON-RPC façade; currently a stub)
  - `crates/link` → crate `platynui-link` (dynamic linking utilities for platform providers)
  - `crates/platform-{windows,linux-x11,macos,mock}` → crates `platynui-platform-*` (OS device bundles, highlight/screenshot, desktop info)
  - `crates/provider-{windows-uia,atspi,macos-ax,jsonrpc,mock}` → crates `platynui-provider-*` (UiTreeProvider implementations)
  - `crates/cli` → crate `platynui-cli` (CLI for queries, highlight, keyboard/pointer, diagnostics)
  - `crates/playground` → examples and dev experiments
  - `apps/inspector` → crate `platynui-inspector` (GUI inspector, Slint-based)

- Python/Robot workspace (`uv`):
  - `src/PlatynUI` → Robot Framework library entry (keywords module scaffold)
  - `packages/native` → Maturin-based native Python package `platynui_native._native` (bindings to core/runtime)
  - `packages/cli` → Python packaging wrapper for the CLI binary
  - `packages/inspector` → Python packaging wrapper for the inspector binary

Generated artefacts such as `target/`, `.venv/`, `.vscode/`, build caches, and wheel artifacts should not be committed.

## Build, Test, and Development Commands
- Bootstrap Python tooling once (root): `uv sync --dev --all-packages --all-groups --all-extras`
- Build all Rust crates: `cargo build --workspace`
- Quick Rust tests:
  - XPath suite: `cargo nextest run -p platynui-xpath`
  - Runtime/CLI as needed: `cargo nextest run -p platynui-runtime` and `cargo nextest run -p platynui-cli`
  - Enable mocks where required: add `--features mock-provider`
- Native Python bindings (from workspace root):
  - Build native package: `uv run maturin develop -m packages/native/Cargo.toml --uv`
  - Build CLI package: `uv run maturin develop -m packages/cli/Cargo.toml --uv`
  - Build Inspector package: `uv run maturin develop -m packages/inspector/Cargo.toml --uv`
  - Run native package tests: `uv run pytest`
- Lint/format/type-check:
  - Rust format: `cargo fmt --all`
  - Rust lint (strict): `cargo clippy --workspace --all-targets -- -D warnings`
  - Python lint: `uv run ruff check .`
  - Python types: `uv run mypy .`

## Coding Style & Naming Conventions
- Rust 2024 defaults: modules/files `snake_case`, types `CamelCase`, constants `SCREAMING_SNAKE_CASE`.
- Workspace lints (enforced via root `Cargo.toml`): `unsafe_code = "forbid"`, `unused_must_use = "deny"`, `clippy::pedantic = "warn"`, `clippy::cargo = "warn"`. `rustfmt.toml` sets `max_width = 120`.
- Crate/package names in the Cargo workspace must start with `platynui-` in `Cargo.toml` (directory names may be shorter, e.g., `crates/runtime`). Exception: the Maturin-based Python native package lives outside the Cargo workspace and uses `platynui_native` to follow Python packaging conventions.
- Error handling: Prefer `thiserror` for new error enums (see `crates/runtime/src/pointer.rs`); keep consistency with existing patterns in core/xpath/runtime.
- XPath/Modeling: default namespace `control`, with additional `item`, `app`, and `native` as needed. Attributes use PascalCase (e.g., `Bounds`, `IsFocused`, `ActivationPoint`).
- Python follows PEP 8; use four-space indentation and descriptive module names. Robot Framework keywords use Title Case (e.g., `Open Application`).

## Logging & Tracing
- Use the `tracing` crate for all diagnostic output.  Each crate specifies its own dependency: `tracing = { version = "0.1", default-features = false, features = ["std"] }` (no workspace deps due to maturin incompatibility).
- Binary crates (CLI, Inspector) additionally use `tracing-subscriber = { version = "0.3", default-features = false, features = ["fmt", "env-filter"] }` and are the only crates that initialize a subscriber.
- Log levels: `error` (unexpected failures not in Result), `warn` (degraded/fallbacks/slow >200ms), `info` (one-time lifecycle), `debug` (operational details), `trace` (hot-path per-item).
- Log output goes to stderr; stdout is reserved for command output.
- Log level priority chain: `RUST_LOG` > `--log-level` CLI flag > `PLATYNUI_LOG_LEVEL` env var > default `warn`.
- See `.github/instructions/tracing.instructions.md` for the full specification including structured field conventions, per-crate instrumentation patterns, and subscriber setup reference.

## Testing Guidelines
- Rust: Place integration tests under `tests/` in each crate (e.g., `crates/xpath/tests`); use `rstest` for fixtures and parameterization. Name files `test_<feature>.rs`. Extend coverage when changing parsers/evaluators or runtime APIs.
- CLI: Keep parsing and command behavior covered (see `crates/cli/src/lib.rs` and command modules). Use the `mock-provider` feature to exercise input stacks deterministically.
- Mock Providers: Do NOT auto-register in inventory. Use factory directly in Rust tests (`MOCK_PROVIDER_FACTORY.create()`) or explicit handles in Python (`Runtime.new_with_providers([MOCK_PROVIDER])`).
- Python: `packages/native/tests` uses `pytest`. For Robot Framework, start acceptance-style suites near `src/PlatynUI` and document temporary run steps in PRs until a formal runner lands.

## Commit & Pull Request Guidelines
- Conventional Commits: `type(scope): imperative subject` (e.g., `refactor(xpath): consolidate namespace handling`).
- Keep subjects ≤ 72 characters; include context in the body and group related changes per commit.
- PRs should explain problem/solution/affected platforms, link issues, list commands you ran (tests/linters/builds), and attach logs or screenshots when UI interactions change.

## Environment & Tooling Tips
- Use the Python version pinned in `.python-version`; keep `uv` up to date for reproducible lockfiles.
- Rust toolchain: MSRV 1.90, current dev toolchain 1.93. Target current stable.
- For Windows builds from Linux, see README section on WSL2 cross-compilation. Mention any OS-specific dependencies in PRs so maintainers can reproduce.

## Project and Documentation Language
- Code, public APIs, comments, commit messages, and PR descriptions are in English.
- Concept and planning documents under `docs/` are currently authored in German (living documents). When updating or adding German docs, include a brief English summary at the top if feasible.

# Security and Privacy Considerations
Do not hardcode secrets, API keys, or personal data. Use environment variables or secure vaults. Review dependencies regularly for vulnerabilities and update them when needed. Report security issues privately to the maintainers instead of disclosing them publicly.
