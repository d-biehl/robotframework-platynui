# robotframework-PlatynUI

## Disclaimer

This project is still under development and should not be used productively **yet**.

At the current state expect:

- bugs
- missing features
- missing documentation

Feel free to contribute, create issues, provide documentation or test the implementation.

## Project Description

PlatynUI is a library for Robot Framework, providing a cross-platform solution for UI test automation. Its main goal is to make it easier for testers and developers to identify, interact with, and verify various UI elements.

We aim to provide a Robot Framework-first library.

### Documentation

- Architecture & Runtime Concept (German): `docs/architekturkonzept_runtime.md`
- Implementation Plan (German): `docs/umsetzungsplan.md`
- Pattern Catalogue (German – trait capabilities, coordinate rules, mappings): `docs/patterns.md`

All concept documents are living drafts and evolve alongside the implementation.

### Workspace Layout

- `crates/core`: Shared datatypes (UiNode, attribute keys, pattern primitives).
- `crates/xpath`: XPath evaluator and parser helpers tailored for PlatynUI.
- `crates/runtime` (`platynui-runtime`): Orchestrates providers, devices, and the XPath pipeline.
- `crates/server` (`platynui-server`): JSON-RPC façade that exposes the runtime.
- `crates/platform-*` (`platynui-platform-*`): Platform-level device drivers and window control APIs (Windows, Linux/X11, macOS, mock).
- `crates/provider-*` (`platynui-provider-*`): UiTreeProvider implementations (UIAutomation, AT-SPI, macOS AX, JSON-RPC, mock).
- `crates/cli` (`platynui-cli`): Command-line utility for XPath queries, highlighting, keyboard/pointer interactions, and diagnostics.
- `apps/inspector` (`platynui-inspector`): Planned GUI to explore the UI tree and craft XPath expressions.

### CLI Quick Examples (mock-provider)

```bash
# Type text and shortcuts via the mock keyboard device
cargo run -p platynui-cli --features mock-provider -- keyboard type "<Ctrl+A>Hello"
# Der Mock-Provider protokolliert die Eingaben auf stdout, z. B.:
# [mock-keyboard] StartInput
# [mock-keyboard] Press Ctrl
# …
# [mock-keyboard] EndInput

# Hold modifiers without releasing them
cargo run -p platynui-cli --features mock-provider -- keyboard press "<Shift+Ctrl+S>"

# Release a previously pressed chord
cargo run -p platynui-cli --features mock-provider -- keyboard release "<Shift+Ctrl+S>"
```

### Contribution Workflow (At a Glance)

- Initialize the Python tooling once via `uv sync --dev` (matching `.python-version`).
- Ensure every new crate entry in `Cargo.toml` uses the `platynui-` prefix.
- Pin dependencies to the latest stable release (`cargo search`, crates.io, or `cargo outdated`).
- Before committing, run `cargo fmt-all`, `cargo lint`, `cargo check-all`, and `cargo test-all`.
- Write unit and integration tests with `rstest` (fixtures, `#[case]`, `#[matrix]`).
- After each work batch, update `docs/umsetzungsplan.md` and tick off completed tasks.
- See `CONTRIBUTING.md` for the full contributor guide.

### Why PlatynUI?

- Cross-platform capability with consistent API across Windows, Linux, and MacOS
- Direct access to native UI elements
- Simplified element identification
- Builtin ui spy tool

## Testable Frameworks

- **Linux**
  - X11
  - AT-SPI2
- **Windows**
  - Microsoft UI Automation (UIA)
- **MacOS**
  - Accessibility API

> Roadmap focus: Windows and Linux/X11 implementations are prioritized in the current development cycle; macOS support will follow once both are stable.

Extendable for any other ui technologies.
