# Contributing to PlatynUI

Thanks for helping build PlatynUI! This guide describes how to get set up, our coding standards, and the quality gates we expect before merging.

## 1) Prerequisites

- Rust: Stable toolchain via rustup; Rust 1.95 or newer.
- Python: 3.12+ and uv >= 0.11.7. Do not use pip directly in this repo.
- [just](https://github.com/casey/just): Task runner for common dev workflows. Install with `cargo install just`, `brew install just`, or your system package manager (`pacman -S just`, `apt install just`, etc.).
- [git-cliff](https://git-cliff.org): Changelog generator. Install with `cargo install git-cliff`, `cargo binstall git-cliff`, or `brew install git-cliff`.
- Tools: cargo, uv, and (recommended) GPG for signed commits.

Run commands from the repository root. `just` is the primary entry point for local development tasks; it wraps the expected `uv`, `cargo`, and `maturin` commands so everyone runs the same workflow.

Bootstrap once:

```bash
just bootstrap
```

This creates `.venv` and installs dev tools (ruff, mypy, maturin, robotframework) and dependency packages.

## 2) Project layout (quick orientation)

- Rust workspace in `crates/*` and `apps/*` (core, xpath, runtime, providers/platforms, cli, inspector).
- Python packages in `packages/*` (native bindings, CLI, inspector) and RF library entry in `src/PlatynUI`.
- Developer, design, and planning docs live under `dev-docs/`; `docs/` is reserved for user-facing documentation; some crates keep component-local `docs/` directories.
- Generated artifacts such as `target/`, `.venv/`, `dist/`, `results/`, wheel files, and build caches should not be committed.

## 3) Contribution scope and expectations

PlatynUI is still preview-stage software. Good contributions keep the moving parts understandable:

- Keep changes focused on one problem or one coherent feature.
- Prefer small, reviewable PRs over broad refactors.
- Preserve existing public behavior unless the PR explicitly changes it.
- Update tests and docs when behavior, commands, packaging, or platform support changes.
- Avoid drive-by cleanups in unrelated files; save them for separate PRs.
- Call out platform assumptions in the PR, especially for Windows, Linux X11, Linux Wayland, and macOS work.

If you are unsure whether a design belongs in the current architecture, open an issue or draft PR first. Early discussion is cheaper than a large rewrite late in review.

## 4) Branching, commits, and PRs

- Use Conventional Commits: `type(scope): subject` (e.g., `feat(runtime): add window resize action`).
- Keep subjects ≤ 72 chars; describe “what/why” in the body; link issues/PRs.
- Sign commits when possible (`git config commit.gpgsign true`).
- Small, focused PRs with clear rationale and “how to verify” notes.

PR descriptions should include:

- What changed and why.
- User-visible behavior changes, if any.
- Platforms affected or tested.
- Commands run, preferably using `just` recipes.
- Known gaps, skipped checks, or follow-up work.

## 5) Dev workflow with `just`

Run `just` without arguments to list all available recipes. The `justfile` is the source of truth, and this section documents the contributor-facing workflow.

Use recipes first, and drop down to raw `cargo`, `uv`, or `maturin` commands only for targeted debugging or when a recipe does not exist yet. If a raw command becomes part of the normal workflow, add a `just` recipe and update the docs.

Common recipes:

| Goal | Recipe | Notes |
|---|---|---|
| List workflows | `just` | Shows recipes from the `justfile`. |
| Bootstrap dependencies | `just bootstrap` | Refreshes the local `uv` environment. |
| Format, lint, and type-check | `just check` | Runs Rust formatting, clippy, ruff, and mypy. |
| Rust tests | `just test` | Runs the Rust workspace test suite via nextest. |
| One Rust crate | `just test-crate platynui-xpath` | Replace the package name as needed. |
| Python tests | `just test-python` | Builds the native package with `mock-provider`, then runs pytest. |
| BareMetal RF (mock) tests | `just test-baremetal` | Robot suites under `tests/BareMetal` against the built-in mock tree; builds `mock-provider`, no display needed. |
| Rust and Python tests | `just test-all` | Runs `just test` and `just test-python`. |
| Acceptance tests | `just test-acceptance` | Real-provider suites under `tests/acceptance` (egui app today; non-mock build). Linux: compositor + X11. See §8 for backends, headless, and CI. |
| Full local gate | `just pre-commit` | Runs bootstrap, checks, Rust tests, and Python tests. |
| Cross-target gate | `just pre-commit-cross` | Linux-only; adds Windows and macOS ARM cargo check/clippy passes. |
| Install git hooks | `just hooks-install` | Installs `pre-commit`, `commit-msg`, and `pre-push` hooks. |
| Install push gate | `just hooks-install-push` | Alias for `just hooks-install`; the push gate is standard. |
| Enable Linux cross-target push gate | `just hooks-cross-enable` | Opts in to cross-target checks before every push on Linux. |
| Disable Linux cross-target push gate | `just hooks-cross-disable` | Turns the optional Linux cross-target push gate off again. |
| Native mock build | `just build-native-mock` | Needed before Python/RF work that uses `Runtime.new_with_mock()`. |
| CLI or Inspector build | `just build-cli`, `just build-inspector` | Builds local binary Python packages with maturin. |
| Clean local artifacts | `just clean` | Removes build/test artifacts while keeping `.venv` and tool caches. |

Additional build and packaging recipes:

| Goal | Recipe | Notes |
|---|---|---|
| Rust workspace build | `just build` | Builds all Rust crates and targets. |
| Native package build | `just build-native` | Builds the native Python package with maturin and uv workspace support. |
| Native package with feature | `just build-native mock-provider` | Passes optional Cargo features through to maturin. |
| Native wheel | `just build-native-wheel` | Builds a release wheel into `dist/`. |
| CLI wheel | `just build-cli-wheel` | Builds a release wheel for `platynui-cli`. |
| Inspector wheel | `just build-inspector-wheel` | Builds a release wheel for `platynui-inspector`. |
| Robot Framework wheel | `just build-platynui-wheel` | Builds the pure Python Robot Framework package wheel. |
| All local Python packages | `just build-all-python` | Builds native, CLI, and Inspector packages for local development. |
| All release wheels | `just build-all-wheels` | Builds every wheel into `dist/`. |
| Rust API docs | `just doc` | Builds Rust API documentation without dependencies. |

The debug-default build recipes — `build`, `build-native`, `build-cli`, `build-inspector`, and `build-native-mock` — honor a `release` variable: pass `release=true` to compile in release mode instead of debug, e.g. `just release=true build-all-python` (which inherits the flag through its dependencies). Because the acceptance lane also depends on `build-native`, `just release=true test-acceptance` runs those suites against an optimized native module. The `*-wheel` recipes are always release builds and ignore the flag.

Git hook recipes:

| Goal | Recipe | Notes |
|---|---|---|
| Install standard hooks | `just hooks-install` | Installs `pre-commit`, `commit-msg`, and `pre-push` hooks from `.pre-commit-config.yaml`. |
| Install push gate | `just hooks-install-push` | Alias for `just hooks-install`; kept as an explicit push-gate command. |
| Run hooks manually | `just hooks-run` | Runs the `pre-commit` stage hooks against all files. |
| Run push hook manually | `just hooks-run-push` | Runs the `pre-push` gate without pushing. |
| Run cross-target hook manually | `just hooks-run-cross` | Linux-only; runs the optional cross-target checks directly. |
| Enable cross-target push checks | `just hooks-cross-enable` | Linux opt-in; makes pre-push run Windows and macOS ARM checks too. |
| Disable cross-target push checks | `just hooks-cross-disable` | Removes the local opt-in flag. |
| Update hook revisions | `just hooks-update` | Updates remote hook revisions in `.pre-commit-config.yaml`. |
| Remove hooks | `just hooks-uninstall` | Removes installed hooks managed by `pre-commit`. |

Linux desktop integration recipes:

| Goal | Recipe | Notes |
|---|---|---|
| Install desktop files | `just install-desktop` | Installs `.desktop` files and icons under `$XDG_DATA_HOME` or `~/.local/share`. |
| Remove desktop files | `just uninstall-desktop` | Removes the locally installed desktop files and icons. |
| Refresh icon cache | `just update-icon-cache` | Refreshes the GTK icon cache after install/uninstall. |

The desktop application IDs are `org.platynui.compositor`, `org.platynui.inspector`, and `org.platynui.test.egui`.

Linux cross-target recipes:

| Goal | Recipe | Notes |
|---|---|---|
| Check Windows crates | `just check-windows` | Cargo-checks Windows-relevant crates from Linux. |
| Clippy Windows crates | `just clippy-windows` | Runs clippy for Windows-relevant crates from Linux. |
| Check macOS ARM crates | `just check-macos-arm` | Cargo-checks macOS ARM-relevant crates from Linux. |
| Clippy macOS ARM crates | `just clippy-macos-arm` | Runs clippy for macOS ARM-relevant crates from Linux. |

The default cross targets can be overridden with `PLATYNUI_WINDOWS_TARGET` and `PLATYNUI_MACOS_ARM_TARGET`.

### Git hooks with `pre-commit`

This repository uses [pre-commit](https://pre-commit.com/) as the Git hook runner. It is installed through the `uv` development environment; no global installation is required after `just bootstrap`.

Install the hooks with:

```bash
just hooks-install
```

The commit-time hook set is intentionally quick:

- file hygiene checks from `pre-commit-hooks` (`check-yaml`, `check-toml`, trailing whitespace, final newline, large files)
- `just fmt-check` for Rust formatting when Rust files changed
- `just ruff` for Python linting when Python files changed
- `just mypy` for Python type checks when Python files changed
- `conventional-pre-commit` in the `commit-msg` hook for Conventional Commit messages

Some file hygiene hooks can update files automatically. If that happens, review the changes, stage them, and commit again.

The full project gate is heavier, so it runs at Git `pre-push` instead of Git `pre-commit`. That hook runs `just pre-commit`, which includes bootstrap, checks, Rust tests, and Python tests. You can run the same gate manually with `just pre-commit` or `just hooks-run-push`.

On Linux, contributors with the cross-target toolchain installed can opt in to an additional pre-push gate:

```bash
just hooks-cross-enable
```

That writes a local Git config flag and makes every push run the Windows and macOS ARM cross-target checks after the normal pre-push gate. The opt-in is local to your checkout and is not committed. Disable it again with `just hooks-cross-disable`, or run the cross-target checks manually with `just hooks-run-cross`.

To run these recipes on Linux, install the Rust targets and host tools first:

```bash
rustup target add x86_64-pc-windows-gnu
rustup target add aarch64-apple-darwin

# Debian/Ubuntu
sudo apt install gcc-mingw-w64-x86-64 llvm

# Arch Linux
sudo pacman -S mingw-w64-gcc llvm

# Fedora
sudo dnf install mingw64-gcc llvm
```

Windows cross checks require `x86_64-w64-mingw32-gcc` and `llvm-rc` on `PATH`. macOS ARM checks currently type-check/clippy the relevant crates only; they require the Rust target but not a full Apple SDK. The `just` recipes validate these prerequisites and print the missing command or package when something is not installed.

These Linux cross-target recipes are early compatibility checks, not release builds. Real platform binaries, wheels, installers, and release candidates must still be built and verified on the appropriate target platform or a dedicated native builder: Windows on Windows, macOS on macOS, and Linux on Linux.

Before pushing non-trivial changes, run:

```bash
just pre-commit
```

For quick iteration, run the smallest recipe that covers the touched area. Examples:

```bash
just test-crate platynui-xpath
just ruff
just test-python
```

Targets that change public behavior should include/update tests.

Recommended verification by change type:

| Change type | Recommended local checks |
|---|---|
| Documentation only | Check links and examples touched by the change; run `just` if documented commands changed. |
| Rust library/runtime change | `just check` and `just test-crate <package>`; use `just test` for shared behavior. |
| XPath parser/evaluator change | `just test-crate platynui-xpath` plus targeted tests under `crates/xpath/tests/`. |
| Python or Robot Framework change | `just test-python`; also run `just ruff` and `just mypy` when editing typed Python. |
| Native Python binding change | `just build-native-mock` and `just test-python`; add Rust tests if the Rust API changed. |
| CLI or Inspector packaging change | `just build-cli` or `just build-inspector`; run relevant CLI/Inspector checks manually if behavior changed. |
| Platform/provider change | `just test` plus platform-specific manual verification when possible. Use `just pre-commit-cross` on Linux for cross-target checks. |
| Dependency change | Run the relevant build/test recipe and commit the changed lockfile (`Cargo.lock` or `uv.lock`). |

## 6) Coding standards

Rust:
- Edition 2024; follow existing naming (snake_case functions, PascalCase types).
- Prefer typed errors (thiserror) in library crates; avoid panics in normal flows.
- Error handling conventions are documented in [dev-docs/error-handling.md](dev-docs/error-handling.md).
- Keep JSON/serde usage consistent; do not add alternate JSON libs.
- Use `rstest` for fixtures/parametrization; keep tests small and deterministic.
- Keep public APIs documented through clear names and focused docs rather than broad comments.
- Re-export public surface from the relevant crate `lib.rs` when it is meant for external use.
- Use `tracing` for diagnostics; stdout is reserved for command output in binaries.
- Unsafe code is denied by default. If unavoidable for FFI or shared memory, keep it narrow and document the safety invariant.

Python:
- 3.12+; keep dependencies minimal. Lint with ruff; optional typing with mypy.
- Robot Framework keywords: Title Case (e.g., `Open Application`). Avoid `print`; return values instead.
- Use `uv` for environment and package workflows. Do not use `pip install` to mutate the repo environment.
- Keep the high-level Robot Framework API stable where possible; prefer additive changes during preview unless a breaking change is intentional.

CLI/Inspector (apps):
- Cross‑platform providers are linked via the `platynui_link_providers!` macro and Cargo target cfgs; follow the existing pattern.
- Keep stdout machine-readable when a command promises structured output; send logs and diagnostics to stderr.
- For UI or terminal output changes, include before/after notes or screenshots in the PR when useful.

## 7) Dependencies

- Rust: add to the crate’s `Cargo.toml`; build to update `Cargo.lock`.
- Python: edit `pyproject.toml`, then always run `uv sync`. Commit both the `pyproject.toml` and updated `uv.lock`.
- Prefer small, widely‑used, stable libraries. Justify heavyweight deps in the PR.
- Before adding a Rust dependency, check whether the standard library already provides the needed functionality for the workspace's Rust version.
- Keep `tracing` as a per-crate dependency rather than a workspace dependency because of maturin compatibility constraints.
- Avoid adding dependencies only for tests if a small local fixture is clearer and cheaper.

## 8) Testing guidance

The test layering and the mock vs. real-provider lanes — what goes where, and why — are described in [dev-docs/testing-strategy.md](dev-docs/testing-strategy.md). This section is the operational complement: what to run, and how the build duality affects your local loop.

Rust:
- Unit tests live alongside code; integration tests under `tests/` per crate.
- Use the mock provider/platform for deterministic tests. For manual runs, enable with `--features mock-provider` (some crates enable it via dev‑deps automatically).
- Prefer targeted tests close to the changed behavior, then broaden to workspace tests for shared contracts.
- Use `cargo nextest` through `just` recipes for normal local runs.

Python / RF mock lane:
- Python tests live under `tests/PlatynUI` and `packages/native/tests`; the deterministic RF mock suites live under `tests/BareMetal`. Run them with `just test-python` (pytest) and `just test-baremetal` (RF mock) — both build the `mock-provider` native package first.
- Tests that call `Runtime.new_with_mock()` (or import the library with `use_mock`) need that `mock-provider` build, or they error.

End‑to‑end / acceptance:
- The real lane needs the **non-mock** native build (a `mock-provider` build would resolve the mock tree, not the real desktop); the `just test-acceptance*` recipes handle the build and an isolated session. They are **not** part of `just pre-commit` — run them separately:

  | Command | Scope |
  |---|---|
  | `just test-acceptance` | This OS — Linux runs both backends (compositor, then X11); Windows runs on the real desktop. |
  | `just test-acceptance-compositor` | Linux — under the PlatynUI Wayland compositor. |
  | `just test-acceptance-x11` | Linux — under an isolated X11/Xephyr session. |
  | `just test-acceptance-windows` | Windows — on the native desktop (UIA), no isolated session. |

  Extra arguments pass through to robotcode and replace the default command entirely. The default is the lane profile matching the session — `real-wayland` / `real-x11` (chosen by the session script) or `real-windows` — which excludes suites tagged for other platforms (`platform:*`, see `robot.toml`); e.g. `just test-acceptance-compositor --profile real-wayland run --suite "Auto Activate"`.
- **Headless / CI.** `headless=true` runs the Linux backends with no visible window (default under `CI`); it needs a GPU render node or Mesa software GL so egui can draw.
- UI automation is platform-sensitive: include OS/session details (compositor vs X11, headless or not) when reporting failures or adding manual verification notes.

## 9) Adding or changing public APIs

- Rust public APIs: update crate modules and re‑export in `lib.rs` if part of external surface. Keep breaking changes minimal and documented in the PR.
- XPath engine changes: add targeted tests under `crates/xpath/tests/` following existing naming (e.g., `evaluator_*.rs`, `parser_*.rs`).
- Python RF library: extend `src/PlatynUI/__init__.py` or new modules imported there. Keep keyword names stable; document changes in README.
- Rust/Python boundary changes belong in `packages/native`; keep binding code out of core logic crates.
- CLI behavior changes should update help text, examples, and tests where practical.
- Platform-provider changes should state which backends are implemented, stubbed, or intentionally unsupported.

## 10) Packaging and release (preview)

- Pre‑release wheels for CLI/Inspector may be published to PyPI. End‑users should install with pre‑release flags:

```pwsh
uv pip install --pre platynui-cli
uv pip install --pre platynui-inspector
uv tool install --prerelease allow platynui-cli
uv tool install --prerelease allow platynui-inspector
```

- Only package when explicitly needed; prefer source builds during development.
- Local package builds should use the `just build-*` recipes or `uv run maturin ... --uv` commands shown in package READMEs.
- Release and changelog automation is maintainer-owned unless explicitly coordinated in an issue or PR.

## 11) Documentation

- Keep README files accurate and concise. Link to package READMEs for CLI/Inspector details.
- Architecture, design, and planning docs live under `dev-docs/` (alongside component-local `docs/` directories); `docs/` is where user-facing documentation will live. Update relevant docs with any non-trivial design change and add a brief English summary when possible.
- Public README files should orient users; keep deep implementation notes in `dev-docs/` or crate-specific docs.
- When documenting commands, prefer `just` recipes for contributor workflows and package commands for end-user workflows.
- Keep docs in English unless updating an existing German planning document.

## 12) Security & privacy

- Do not commit secrets or personal data. Use environment variables or secure stores.
- Review dependencies for vulnerabilities; note relevant CVEs or fixes in PR descriptions when upgrading.
- Be careful with screenshots, UI tree dumps, logs, and Robot output; they can contain window titles, paths, hostnames, or user data.

## 13) Troubleshooting contribution setup

- If Python tools are missing, run `just bootstrap` again.
- If mock Python tests fail because mock providers are unavailable, run `just build-native-mock` before retrying.
- If Linux accessibility trees are empty, make sure AT-SPI is enabled and running for X11/XWayland sessions.
- If Linux cross-target recipes fail, install the missing Rust target or system compiler named in the recipe error.
- If a `just` recipe is too broad for investigation, run the equivalent raw command temporarily and capture the result in the PR notes.

---

Questions? Open an issue or start a discussion. Thank you for contributing to PlatynUI!
