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
- Implementation Plan (German): `docs/architekturkonzept_runtime_umsetzungsplan.md`
- Pattern Catalogue (German – trait capabilities, coordinate rules, mappings): `docs/patterns.md`
- Provider Checklist (German/EN mix, draft): `docs/provider_checklist.md`
- Windows UIA Provider – Design (German, EN summary): `docs/provider_windows_uia_design.md`

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
- `packages/native` (`platynui_native`): Maturin/pyo3-based Python bindings (cdylib). Built and tested separately via Python tooling:
  - Build/install: `uv run maturin develop --release` (run inside `packages/native/`)
  - Tests: `uv run pytest`
  - Note: This crate is intentionally excluded from the Cargo workspace so `cargo test`/`cargo nextest` don’t try to link `-lpython3` during Rust-only test runs.

### Provider Linking (Apps vs. Tests)
- Runtime uses inventory for registration but does not auto‑link OS providers anymore.
- Applications link providers explicitly per OS:
  - CLI: links `platynui-platform-*` and `platynui-provider-*` via `cfg(target_os)` in `crates/cli/Cargo.toml` and `src/main.rs`.
  - Python extension: links providers per OS in `packages/native/Cargo.toml` and `src/lib.rs`.
- Unit tests link the mock providers inside their test modules to ensure deterministic inventory registrations:
  - Example: `const _: () = { use platynui_platform_mock as _; use platynui_provider_mock as _; };`

### CLI Quick Examples (mock-provider)

```bash
# Type text and shortcuts via the mock keyboard device
cargo run -p platynui-cli --features mock-provider -- keyboard type "<Ctrl+A>Hello"
# Der Mock-Provider protokolliert die Eingaben auf stdout, z. B.:
# mock-keyboard: start
# mock-keyboard: press Control
# …
# mock-keyboard: end

# Hold modifiers without releasing them
cargo run -p platynui-cli --features mock-provider -- keyboard press "<Shift+Ctrl+S>"

# Release a previously pressed chord
cargo run -p platynui-cli --features mock-provider -- keyboard release "<Shift+Ctrl+S>"
```

### Windows Cross‑Build (GNU target)

- Prerequisites
  - Rust target: `rustup target add x86_64-pc-windows-gnu`
  - MinGW toolchain (choose the POSIX variant if prompted):
    - Debian/Ubuntu: `sudo apt-get install mingw-w64`
    - Fedora: `sudo dnf install mingw64-gcc`
    - Arch: `sudo pacman -S mingw-w64-gcc`

- Build examples
  - Platform crate: `cargo build -p platynui-platform-windows --target x86_64-pc-windows-gnu`
  - CLI (debug): `cargo build -p platynui-cli --target x86_64-pc-windows-gnu`
  - CLI (release): `cargo build -p platynui-cli --target x86_64-pc-windows-gnu --release`
  - Artifact path: `target/x86_64-pc-windows-gnu/{debug,release}/platynui-cli.exe`

- Tips
  - Set `CARGO_TARGET_DIR=/mnt/c/...` in WSL so the `.exe` lands on the Windows filesystem.
  - If linking fails, ensure `x86_64-w64-mingw32-gcc` is in `PATH`.
  - For native Win32 builds (MSVC), see the next section.

### Windows Native Build (MSVC toolchain)

- Prerequisites (Windows host)
  - Visual Studio 2022 (or Build Tools) with workload “Desktop development with C++” (MSVC, Windows SDK).
  - Rust MSVC toolchain: `rustup toolchain install stable-x86_64-pc-windows-msvc` and `rustup default stable-x86_64-pc-windows-msvc`.
  - Use a “Developer PowerShell/Prompt for VS” so `cl.exe`/`link.exe` are on `PATH`.

- Build
  - Workspace: `cargo build --release`
  - CLI only: `cargo build -p platynui-cli --release`

- Run
  - `target\release\platynui-cli.exe info`
  - Example: `platynui-cli.exe highlight --rect 200,300,400,250 --duration-ms 1500`

- Troubleshooting
  - “link.exe not found”: Start the shell via “Developer PowerShell for VS” or install the Build Tools + Windows SDK.
  - Avoid mixing GNU/MSVC: use only the MSVC toolchain for native builds.

### Contribution Workflow (At a Glance)

- Initialize the Python tooling once via `uv sync --dev` (matching `.python-version`).
- Ensure every new crate entry in `Cargo.toml` uses the `platynui-` prefix.
- Pin dependencies to the latest stable release (`cargo search`, crates.io, or `cargo outdated`).
- Before committing, run `cargo fmt-all`, `cargo lint`, `cargo check-all`, and `cargo test-all`.
- Write unit and integration tests with `rstest` (fixtures, `#[case]`, `#[matrix]`).
- After each work batch, update `docs/architekturkonzept_runtime_umsetzungsplan.md` and tick off completed tasks.
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
