# Repository Guidelines

This file is a short orientation layer for automated contributors. [README.md](README.md) is the project overview and [CONTRIBUTING.md](CONTRIBUTING.md) is the full contributor workflow — both are the source of truth and are not repeated here. Read them before non-trivial work. The points below are things that are easy to get wrong from skimming the repo alone.

## Quick Orientation

PlatynUI is a cross-platform UI automation toolkit for Robot Framework, built on a Rust core with Python bindings. Two stacked workspaces share this repo.

- Rust workspace (`cargo`, crate names prefixed `platynui-`):
	- `crates/core`, `crates/xpath`, `crates/runtime`, `crates/link`
	- `crates/platform-{windows,linux-x11,linux,macos,mock}`
	- `crates/provider-{windows-uia,atspi,macos-ax,mock}`
	- `crates/cli`, `crates/xkb-util`, `crates/playground`
	- `apps/inspector`, `apps/wayland-compositor`, `apps/wayland-compositor-ctl`, `apps/test-app-egui`, `apps/eis-test-client`
- Python/Robot workspace (`uv`):
	- `src/PlatynUI` — Robot Framework library entry
	- `packages/native` — Maturin bindings (`platynui_native._native`)
	- `packages/cli`, `packages/inspector` — Python wrappers around the Rust binaries

The Python native package lives outside the Cargo workspace and uses `platynui_native` to follow Python conventions. Platform/provider status (which OS is real, stub, or experimental) is in the README's platform-support table — consult it before promising behavior.

## Task Routing

- Rust crates and apps:
	- Owning paths: `crates/`, `apps/`.
- Python / Robot Framework:
	- Owning paths: `src/PlatynUI`, `packages/`.
	- The Rust/Python boundary lives in `packages/native`; see [`docs/python-bindings.md`](docs/python-bindings.md).
	- Robot Framework surface state, incl. the `PlatynUI` vs `PlatynUI.BareMetal` situation: [`docs/python-library-design.md`](docs/python-library-design.md), [`docs/python-migration-status.md`](docs/python-migration-status.md).
- Docs:
	- Owning paths: `docs/`, root Markdown files.

## Common Commands

`just` is the canonical entry point. CONTRIBUTING.md lists every recipe; do not invent equivalent raw `cargo`/`uv`/`maturin` invocations when a recipe exists.

- `just check`
	- fmt, clippy, ruff, mypy.
- `just test` / `just test-crate <pkg>`
	- Rust tests via nextest.
- `just test-python`
	- Builds native with `mock-provider`, then pytest.
- `just pre-commit`
	- Full local gate before pushing non-trivial changes.

Heavy recipes (`just pre-commit-cross`, the `build-*-wheel` recipes, `just build-all-wheels`) take minutes and are not part of the normal verification loop. Do **not** run them unless the user asks or the change clearly warrants it.

## Design Docs

Don't guess conventions — the design docs are authoritative. Consult them before editing the corresponding area:

- [`docs/architecture.md`](docs/architecture.md) — overall system design
- [`docs/error-handling.md`](docs/error-handling.md) — error type conventions
- [`docs/testing-strategy.md`](docs/testing-strategy.md) — test layout, mock-provider usage
- [`docs/platform-linux.md`](docs/platform-linux.md), [`docs/platform-linux-wayland.md`](docs/platform-linux-wayland.md), [`docs/platform-windows.md`](docs/platform-windows.md) — platform specifics
- [`docs/cli.md`](docs/cli.md), [`docs/inspector.md`](docs/inspector.md), [`docs/keyboard-input.md`](docs/keyboard-input.md), [`docs/pointer-input.md`](docs/pointer-input.md) — component-level designs

When a design doc and the code disagree, the code is reality but the doc usually documents the *intent* — flag the divergence rather than silently picking one.

## Commits and Pull Requests

- **Conventional Commits** are required (`type(scope): subject`, ≤72 chars).
- Only commit when the user explicitly asks.
- Keep changes focused. No unrelated refactors or formatting noise in the same PR.

## Agent Notes

- Code, public APIs, comments, commit messages, and PR descriptions are English — even when the user writes German. The design docs under `docs/` (architecture, error-handling, platform-*, cli, inspector, keyboard-input, pointer-input, python-bindings, testing-strategy) are English; only specific planning docs (`python-library-design.md`, `python-migration-status.md`, and the `plan-*.md` files) are German living documents. When editing a German doc, add a brief English summary at the top if feasible.
- Make small, focused changes and avoid unrelated refactors.
- Update [CONTRIBUTING.md](CONTRIBUTING.md) when contributor rules change; update this file when the orientation, task routing, or common commands change.
