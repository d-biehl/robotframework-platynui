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

The Python native package (`packages/native`) is a Cargo workspace member (the root `Cargo.toml` has `members = ["crates/*", "apps/*", "packages/*"]`), so the workspace-wide gates — `just check` (`clippy --workspace`) and `just test` (`nextest --workspace`) — cover it; it is only *named* `platynui_native` (underscore) to follow Python conventions rather than the crates' `platynui-` prefix. Platform/provider status (which OS is real, stub, or experimental) is in the README's platform-support table — consult it before promising behavior.

## Task Routing

- Rust crates and apps:
	- Owning paths: `crates/`, `apps/`.
- Python / Robot Framework:
	- Owning paths: `src/PlatynUI`, `packages/`.
	- The Rust/Python boundary lives in `packages/native`; see [`dev-docs/python-bindings.md`](dev-docs/python-bindings.md).
	- Robot Framework surface state, incl. the `PlatynUI` vs `PlatynUI.BareMetal` situation: [`dev-docs/python-library-design.md`](dev-docs/python-library-design.md), [`dev-docs/python-migration-status.md`](dev-docs/python-migration-status.md).
- Docs:
	- Owning paths: `dev-docs/` (developer & design docs), `docs/` (user-facing documentation), root Markdown files.

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

- [`dev-docs/architecture.md`](dev-docs/architecture.md) — overall system design
- [`dev-docs/error-handling.md`](dev-docs/error-handling.md) — error type conventions
- [`dev-docs/testing-strategy.md`](dev-docs/testing-strategy.md) — test layout, mock-provider usage
- [`dev-docs/platform-linux.md`](dev-docs/platform-linux.md), [`dev-docs/platform-linux-wayland.md`](dev-docs/platform-linux-wayland.md), [`dev-docs/platform-windows.md`](dev-docs/platform-windows.md) — platform specifics
- [`dev-docs/cli.md`](dev-docs/cli.md), [`dev-docs/inspector.md`](dev-docs/inspector.md), [`dev-docs/keyboard-input.md`](dev-docs/keyboard-input.md), [`dev-docs/pointer-input.md`](dev-docs/pointer-input.md) — component-level designs

When a design doc and the code disagree, the code is reality but the doc usually documents the *intent* — flag the divergence rather than silently picking one.

## Commits and Pull Requests

- **Conventional Commits** are required (`type(scope): subject`, ≤72 chars).
- Only commit when the user explicitly asks.
- Keep changes focused. No unrelated refactors or formatting noise in the same PR.

## Agent Notes

- Code, public APIs, comments, commit messages, and PR descriptions are English — even when the user writes German. Developer and design docs live under `dev-docs/` and are English; a few still-German living documents there (`python-library-design.md`, `python-migration-status.md`, `plan-waylandCompositor.md`, `eis-libei.md`) are slated for English translation when they migrate to OpenSpec. When editing a still-German doc, add a brief English summary at the top if feasible.
- Make small, focused changes and avoid unrelated refactors.
- Update [CONTRIBUTING.md](CONTRIBUTING.md) when contributor rules change; update this file when the orientation, task routing, or common commands change.
