# CLI Reference

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document covers the PlatynUI CLI tool. For the platform-agnostic architecture, see `docs/architecture.md`.

Binary: `platynui-cli-rs` (package `platynui-cli`)

## Commands

| Command | Description |
|---------|-------------|
| `list-providers` | Show registered providers (name, version, active status) |
| `info` | Desktop/platform metadata (OS, monitors, bounds) |
| `query <xpath>` | Evaluate XPath, output as table or JSON |
| `snapshot <xpath>` | Export UI subtrees as text or XML |
| `watch` | Stream provider events (text/JSON), optional XPath follow-up query |
| `highlight <xpath>` | Highlight element bounding boxes |
| `screenshot` | Capture screen to PNG (`--rect` for sub-region) |
| `focus <xpath>` | Set focus on matching elements |
| `window` | List/control windows (activate, bring-to-front, minimize, maximize, restore, close, move, resize) |
| `pointer` | Mouse control (move, click, multi-click, press, release, scroll, drag, position) |
| `keyboard` | Keyboard input (type, press, release, list) |

## Snapshot XML Model

- Element name = Role, prefix = namespace
- Fixed namespaces: `urn:platynui:control`, `urn:platynui:item`, `urn:platynui:app`, `urn:platynui:native`
- Complex values (Rect/Point/Size) as JSON strings
- Streaming writer via `quick-xml`
- Options: `--max-depth`, `--attrs default|all|list`, `--include`/`--exclude`, `--exclude-derived`, `--include-runtime-id`, `--pretty`, `--format text|xml`, `--split PREFIX`, `--output FILE`, `--no-attrs`, `--no-color`
