# Repository Guidelines

This file is a short orientation layer for automated contributors. [README.md](README.md) is the project overview and [CONTRIBUTING.md](CONTRIBUTING.md) is the full contributor workflow — both are the source of truth and are not repeated here. Read them before non-trivial work. The points below are things that are easy to get wrong from skimming the repo alone.

## Quick Orientation

PlatynUI is a cross-platform UI automation toolkit for Robot Framework, built on a Rust core with Python bindings. Two stacked workspaces share this repo.

- Rust workspace (`cargo`, crate names prefixed `platynui-`):
	- `crates/core`, `crates/xpath`, `crates/runtime`, `crates/link`
	- `crates/platform-{windows,linux-x11,linux,macos,mock}`
	- `crates/provider-{windows-uia,java,java-jab,atspi,macos-ax,mock}` (`provider-java` = the single registered Java provider, a router over toolkit backends; `provider-java-jab` = its Java Access Bridge backend for Swing/AWT on Windows — a library crate, not a provider)
	- `crates/java-agent` — JVM attach transport, handshake discovery and RPC client for the in-JVM Java agent; provider-neutral, depends on no other PlatynUI crate
	- `crates/cli`, `crates/xkb-util`, `crates/playground`
	- `apps/inspector`, `apps/wayland-compositor`, `apps/wayland-compositor-ctl`, `apps/test-app-egui`, `apps/eis-test-client`
- Java workspace (Gradle, self-contained per project):
	- `java/agent` — the agent PlatynUI loads **into** a target JVM (a *product*; the Java *fixtures* live under `apps/`)
- Python/Robot workspace (`uv`):
	- `src/PlatynUI` — Robot Framework library entry
	- `packages/native` — Maturin bindings (`platynui_native._native`)
	- `packages/cli`, `packages/inspector` — Python wrappers around the Rust binaries
	- `packages/provider-java` — pure-data wheel carrying the Java agent JAR (no Rust; excluded from the Cargo workspace)

The Python native package (`packages/native`) is a Cargo workspace member (the root `Cargo.toml` has `members = ["crates/*", "apps/*", "packages/*"]`), so the workspace-wide gates — `just check` (`clippy --workspace`) and `just test` (`nextest --workspace`) — cover it; it is only *named* `platynui_native` (underscore) to follow Python conventions rather than the crates' `platynui-` prefix. `packages/provider-java` is the exception: it holds no Rust and is `exclude`d from the Cargo workspace. Platform/provider status (which OS is real, stub, or experimental) is in the README's platform-support table — consult it before promising behavior.

## Task Routing

- Rust crates and apps:
	- Owning paths: `crates/`, `apps/`.
- Java agent (the artifact loaded into a target JVM):
	- Owning paths: `java/agent` (product), `crates/java-agent` (attach transport + RPC client), `packages/provider-java` (delivery wheel).
	- The three carry **one version** and must move together; `scripts/update-git-versions.py` syncs `java/agent/gradle.properties` with the rest. Provider↔agent versions are compared for exact equality at connect time, because an agent cannot be unloaded from a JVM.
	- A toolkit adapter is split across two of them: the tree reader is in `java/agent` (`Swing*.java`), the mapping onto `UiNode` is in `crates/provider-java/src/agent`. Change one and you usually change the other.
	- **Rebuilding the JAR is not delivering it**: `just build-java-agent` writes `java/agent/build/libs`, but the installed package serves the copy staged under `packages/provider-java` until `just build-provider-java` restages it — and the exact-version handshake cannot notice, because both sides still report the same dev version. Use `just install-provider-java`.
	- Injection paths, JEP 451 facts and the delivery story: [`dev-docs/java-toolkits.md`](dev-docs/java-toolkits.md), [`java/agent/README.md`](java/agent/README.md).
- Python / Robot Framework:
	- Owning paths: `src/PlatynUI`, `packages/`.
	- The Rust/Python boundary lives in `packages/native`; see [`dev-docs/python-bindings.md`](dev-docs/python-bindings.md).
	- Robot Framework surface state, incl. the `PlatynUI` vs `PlatynUI.BareMetal` situation: [`dev-docs/python-library-design.md`](dev-docs/python-library-design.md), [`dev-docs/python-migration-status.md`](dev-docs/python-migration-status.md).
	- Writing/reviewing Robot Framework suites: follow the `robot-test-style` skill (`.claude/skills/robot-test-style/SKILL.md`) — the authoring checklist for [`dev-docs/testing-strategy.md`](dev-docs/testing-strategy.md) §2.5/§2.6.
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
- `just build-java-agent` / `just test-java-agent` / `just test-java-agent-live`
	- Java agent: build the JAR, its JUnit tests, and the live attach checks against a real JVM. `just build-native` stays JDK-free on purpose — a missing JAR is a runtime diagnostic, not a build failure.

Heavy recipes (`just pre-commit-cross`, the `build-*-wheel` recipes, `just build-all-wheels`, `just test-provider-java-delivery`) take minutes and are not part of the normal verification loop. Do **not** run them unless the user asks or the change clearly warrants it.

## Design Docs

Don't guess conventions — the design docs are authoritative. Consult them before editing the corresponding area:

- [`dev-docs/architecture.md`](dev-docs/architecture.md) — overall system design
- [`dev-docs/error-handling.md`](dev-docs/error-handling.md) — error type conventions
- [`dev-docs/testing-strategy.md`](dev-docs/testing-strategy.md) — test layout, mock-provider usage
- [`dev-docs/platform-linux.md`](dev-docs/platform-linux.md), [`dev-docs/platform-linux-wayland.md`](dev-docs/platform-linux-wayland.md), [`dev-docs/platform-windows.md`](dev-docs/platform-windows.md) — platform specifics
- [`dev-docs/java-toolkits.md`](dev-docs/java-toolkits.md) — Java UI toolkit (Swing/SWT/JavaFX) detection and accessibility coverage across platforms
- [`dev-docs/provider-plugins.md`](dev-docs/provider-plugins.md) — constraints and option space for distributable providers (notes, not a decision)
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
