# Contributing to PlatynUI

Thank you for your interest in PlatynUI! This guide summarizes our current workflow and complements the detailed concept documents in `docs/`.

## Prerequisites
- **Rust**: Use `rustc 1.90.0 (1159e78c4 2025-09-14)` or the matching toolchain release.
- **Python/uv**: Install the Python version specified in `.python-version` and run `uv sync --dev` once to pull in the development tooling.
- Have `cargo`, `uv`, `gpg` (for signed commits), and your preferred editor ready.

## Naming & Structure Conventions
- **Crate names**: Every `Cargo.toml` entry must start with the `platynui-` prefix (`platynui-runtime`, `platynui-provider-jsonrpc`, ...). Directory names may differ; the package name defines the purpose.
- **Namespaces & attributes**: Attributes use PascalCase. The default XPath namespace is `control`; additional namespaces are `item`, `app`, and `native`.
- **Rust types**: Avoid redundant prefixes such as `Platynui` inside modules—clear type names like `RuntimeRegistry` or `MockWindowSurface` are sufficient.

## Dependencies
- When adding or updating dependencies, use the latest stable release. Verify with `cargo search`, crates.io, or `cargo outdated`.
- Mention the chosen version and reasoning in your review notes (e.g., security fix, new capability).

## Code Quality & Tests
- Use the shared Cargo aliases:
  - `cargo fmt-all`
  - `cargo lint` (expands to `cargo clippy --all-targets --all-features --workspace -- -D warnings`)
  - `cargo check-all`
  - `cargo test-all`
- Author unit and integration tests with [`rstest`](https://docs.rs/rstest/latest/rstest/)—leverage fixtures, `#[case]`, `#[matrix]`, etc., to cover variations succinctly.
- Follow the repository `rustfmt.toml` and run `cargo fmt-all` before opening a pull request.

## Workflow & Planning
- `docs/umsetzungsplan.md` is treated as a living plan. After each work batch, tick off completed items, capture new findings, and adjust priorities.
- Sign every commit with GPG. If the environment cannot access the agent, pause and resolve the issue before proceeding.

## Documentation
- Record major architecture decisions and pattern updates in the respective documents under `docs/`.
- Keep README and plan aligned with the current implementation stage.
- These documents are living drafts—update them whenever assumptions change.

Thank you for contributing! Reach out via issues or discussions if questions arise.
