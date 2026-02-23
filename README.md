# Robot Framework PlatynUI

Cross-platform UI automation for Robot Framework. Early alpha stage.

> [!WARNING]
> Preview quality. APIs and behavior may change. Use for evaluation only.

## Quick install (preview)

Install the pre-release packages from PyPI (explicit flags required):

```sh
# CLI
uv pip install --pre robotframework-platynui-cli
pip install --pre robotframework-platynui-cli
uv tool install --prerelease allow robotframework-platynui-cli

# Inspector GUI
uv pip install --pre robotframework-platynui-inspector
pip install --pre robotframework-platynui-inspector
uv tool install --prerelease allow robotframework-platynui-inspector
```

> **Note:** The Robot Framework library package (`robotframework-PlatynUI`) is not yet published on PyPI. For local development, see `CONTRIBUTING.md`.

Try it:

```sh
platynui-cli list-providers
platynui-cli info --format json
platynui-cli keyboard list | head -n 20   # show first key names
platynui-cli keyboard type "Hello <Ctrl+A>\u00A7"   # mixed text + chord + unicode
platynui-cli snapshot "//control:Window" --pretty              # human-readable tree on console
platynui-cli snapshot "//control:Window" --format xml --output windows.xml  # export as XML
platynui-inspector
```

## What is PlatynUI?

PlatynUI is a Robot Framework library and toolset to inspect, query, and control native desktop UIs across Windows, Linux, and macOS. It ships with:

- A CLI for XPath queries, highlighting, screenshots, keyboard/pointer input
- A GUI inspector to explore the UI tree and attributes
- Python bindings to integrate with Robot Framework test suites

Why PlatynUI?
- Consistent, cross-platform API surface
- Works with native accessibility stacks
- XPath-like queries to find elements

## Vision and direction

This repository is a ground‑up rewrite of the original project (see https://github.com/imbus/robotframework-PlatynUI), keeping the vision but modernizing the architecture and tooling.

We’re building PlatynUI to be:

- Robot Framework‑first: a clean keyword library with simple Python packaging and installation.
- Cross‑platform at the core: shared logic in Rust for performance, determinism, and safety; Python exposes the library to Robot Framework.
- Query‑centric: an XPath 2.0‑inspired language tailored for desktop UIs with a streaming evaluator and predictable document‑order semantics.
- Uniformly modeled: a single UI model with namespaces (control, item, app, native), typed attributes, and discoverable patterns (e.g., Focusable, WindowSurface, TextContent).
- Provider‑based: native OS providers (Windows UIA, Linux AT‑SPI, macOS AX) plus fast mock providers for tests. External (out-of-process) providers are on the roadmap.
- Tooled for productivity: a CLI for diagnostics/automation and a GUI Inspector for exploring the tree and crafting queries.
- Reliability‑oriented: pointer/keyboard profiles, motion/acceleration and timing controls, highlighting and screenshots for feedback, and typed errors to reduce flakiness.
- Extensible: hook points for custom providers and functions; as we leave preview, public APIs will stabilize.

Expect differences to the original project’s API and keywords during the preview phase—capabilities converge, but names and behaviors may change as the new core matures.

## Platform support

| Component | Windows | Linux (X11) | macOS |
|-----------|---------|-------------|-------|
| UI tree provider | ✅ UIA | ✅ AT-SPI2 | ❌ stub |
| Pointer | ✅ SendInput | ✅ XTest | ❌ stub |
| Keyboard | ✅ SendInput | ❌ planned | ❌ stub |
| Screenshot | ✅ GDI | ✅ XGetImage | ❌ stub |
| Highlight | ✅ Layered window | ✅ Override-redirect | ❌ stub |
| Window management | ✅ Win32 | ⚠️ partial (EWMH) | ❌ stub |
| Inspector | ✅ | ✅ | ❌ |

Wayland support is deferred; X11/XWayland is used on Linux. See `docs/planning.md` §4.4 for details.

## Package docs

- CLI: `packages/cli/README.md`
- Inspector: `packages/inspector/README.md`
- Native Python bindings: `packages/native/README.md`

## Documentation

- Architecture: `docs/architecture.md`
- Planning & Roadmap: `docs/planning.md`
- Windows Platform: `docs/platform-windows.md`
- Linux Platform: `docs/platform-linux.md`
- Python Bindings: `docs/python-bindings.md`
- CLI Reference: `docs/cli.md`
- Inspector: `docs/inspector.md`
- Logging & Tracing: `.github/instructions/tracing.instructions.md`

## Contributing

Contributions are welcome. Please see `CONTRIBUTING.md` for guidelines. Development notes and deeper build instructions live in the repository docs and package READMEs.

## License

Apache-2.0. See `LICENSE` in this repository.
