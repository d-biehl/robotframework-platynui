# Robot Framework PlatynUI – Inspector

A simple GUI inspector for PlatynUI. It shows the UI (accessibility) tree and attributes/patterns of the selected node and can briefly highlight the selected element’s bounds.

> [!WARNING]
> Preview release on PyPI. This package is experimental; the UI and behavior may change. Intended for evaluation, not production.

## Install

Install the pre‑release package from PyPI. After installation, the `platynui-inspector` command is available on your PATH.

```pwsh
# In a virtual environment (uv)
uv pip install --pre robotframework-platynui-inspector

# Or with pip
pip install --pre robotframework-platynui-inspector

# Or install as a user-level tool (uv tool)
uv tool install --prerelease allow robotframework-platynui-inspector
```

Supported platforms: Windows, macOS, and Linux. The bundled binary links platform providers for your OS. Note: As a pre‑release, some installers require an explicit flag (e.g., `--pre`).

## Usage

```pwsh
platynui-inspector
```

- Left: Tree view (lazy-loaded). Expand/collapse nodes; the desktop is the root.
- Right: Attribute table (namespace, name, value, type) for the selected node.
- On selection, if available, the node’s `control:Bounds` is highlighted for ~1.5s.

Tips:
- Keyboard: Up/Down selects previous/next. Left collapses or jumps to the parent. Right expands or jumps to the first child. Home/End go to first/last; PageUp/PageDown scroll by a page.
- Context menu: Right‑click a row to Refresh or Refresh subtree.
- Value types are shown in the third column (Bool, Integer, Number, String, Point, Size, Rect, Array, Object).

## Troubleshooting

- macOS: Grant Accessibility permissions to your terminal/IDE.
- Linux (X11): Ensure accessibility is enabled and AT‑SPI is running.
- Windows: UIA is typically available. If nothing appears, try running with elevated privileges.
- Highlight not visible: Some elements don’t expose `Bounds` or expose empty rectangles.

## License

Apache‑2.0. See the repository’s LICENSE file.
