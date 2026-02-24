# Inspector

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document covers the PlatynUI GUI Inspector. For the platform-agnostic architecture, see `docs/architecture.md`.

Binary: `platynui-inspector-rs` (package `platynui-inspector`, egui-based GUI)

## Overview

The inspector is a desktop GUI tool for exploring and debugging the PlatynUI UI tree in real time. It connects to the PlatynUI runtime, displays the full UI element hierarchy, allows XPath queries against the tree, shows element properties, and highlights selected elements on screen.

**UI framework**: [egui](https://github.com/emilk/egui) via `eframe` (pure Rust, immediate-mode GUI). Chosen for its minimal dependency footprint, no build-time code generation, and straightforward Rust-native API.

## Architecture — MVVM

The inspector follows a strict Model–ViewModel–View pattern:

```text
src/
├── main.rs              ← Entry point, wires M-VM-V together
├── lib.rs               ← Library entry point (run function)
├── model/               ← M: Data structures, PlatynUI integration
│   ├── mod.rs
│   └── tree_data.rs     ← UiNodeData (cached wrapper around UiNode)
├── viewmodel/           ← VM: Application state & logic
│   ├── mod.rs
│   ├── tree_vm.rs       ← TreeViewModel (expand/collapse/navigate)
│   └── inspector_vm.rs  ← InspectorViewModel (overall app state)
└── view/                ← V: Pure UI rendering (egui)
    ├── mod.rs
    ├── tree_view.rs     ← TreeView panel
    ├── properties.rs    ← Properties table
    └── toolbar.rs       ← Menu, search bar, results panel
```

### Model Layer (`model/`)

- **`UiNodeData`** — Cached wrapper around `Arc<dyn UiNode>`. Caches id, label, children, and `has_children` behind `Mutex` guards. Provides `display_attributes()` for the properties table and `bounds_rect()` for highlighting. `refresh()` / `refresh_recursive()` invalidate caches.
- **`SearchResultItem`** — Enum for XPath results: `Node`, `Attribute` (with owner node for tree reveal), `Value`.
- **`DisplayAttribute`** — Flat struct for properties table rows (namespace, name, value, type).

### ViewModel Layer (`viewmodel/`)

- **`TreeViewModel`** — Maintains a `HashSet<String>` of expanded node IDs and a flattened `Vec<VisibleRow>` of the currently visible tree. Supports `toggle`, `expand`, `collapse`, `reveal_node` (auto-expand ancestor chain), `refresh_row`, `refresh_subtree`.
- **`InspectorViewModel`** — Top-level app state: owns `TreeViewModel`, `Runtime`, selection/focus indices, search text, results, properties cache. Provides keyboard navigation (Up/Down/Left/Right/Home/End/PageUp/PageDown), `evaluate_xpath()` (non-blocking, spawns background thread), `poll_search()` (drains streaming results each frame), `cancel_search()`, `reveal_and_select_result()`, and auto-highlight on selection.

### View Layer (`view/`)

All view functions are pure rendering — they read state and return action enums. No mutation of ViewModel state happens inside view code.

- **`tree_view::show_tree()`** — ScrollArea with indented rows, disclosure triangles, role icons, selection/focus indicators, context menu (Refresh / Refresh Subtree). Returns `Vec<TreeAction>`.
- **`properties::show_properties()`** — `egui_extras::TableBuilder` with sortable columns (Name, Value, Type). Each cell is a read-only `TextEdit` for native text selection. Context menu: Copy Name/Value/Type/Row.
- **`toolbar::show_menu_bar()`** / `show_search_bar()` / `show_results_panel()` — Menu bar, XPath search with Enter/Button (toggles to Stop while searching), results list with click-to-reveal. Returns `Vec<ToolbarAction>` (`EvaluateXPath`, `CancelSearch`, `RevealResult`).

## Features

- **UI Tree** — Hierarchical tree with lazy child loading, expand/collapse, keyboard navigation, role icons, invalid-node strikethrough
- **Properties Panel** — Sortable table with namespace:name, value, type columns; copy via context menu and native text selection
- **XPath Search** — Non-blocking, streaming XPath evaluation with cancellation support. Results appear incrementally with a live spinner and elapsed time. Click any result to reveal the node in tree
- **Element Highlighting** — Selected elements are highlighted on screen (1.5s) via platform highlight provider
- **Always On Top** — Toggle to keep inspector above other windows
- **Context Menu** — Refresh node or subtree from tree view
- **Tracing** — `--log-level` CLI flag, `RUST_LOG` / `PLATYNUI_LOG_LEVEL` env vars

## Troubleshooting

### WSL2 / WSLg: Wayland backend crash (`Broken pipe`, `winit EventLoopError`)

Under WSLg, `winit` (the windowing library used by `eframe`) defaults to the Wayland backend. The `smithay-clipboard` crate (a transitive dependency via `eframe → egui-winit → smithay-clipboard`) opens its own Wayland connection for clipboard operations — independent of the windowing backend. WSLg's Weston compositor drops this connection, causing `Broken pipe` errors and an immediate crash:

```
Io error: Broken pipe (os error 32)
Error: winit EventLoopError: Exit Failure: 1
```

This is a known upstream issue affecting all egui/eframe applications under WSLg:
- [emilk/egui#4938](https://github.com/emilk/egui/issues/4938) — "WSL OS error: Broken pipe (os error 32)"
- [emilk/egui#3805](https://github.com/emilk/egui/issues/3805) — "`smithay-clipboard` crashes when resizing GUI with the mouse on WSL"
- [Smithay/smithay-clipboard#52](https://github.com/Smithay/smithay-clipboard/issues/52) — "Crash when running in WSL2" (panic fixed in v0.7.1, but the underlying Wayland connection drop persists)

**Workaround:** Force the X11 backend **and** unset `WAYLAND_DISPLAY` to prevent `smithay-clipboard` from connecting to the Wayland compositor independently:

```bash
WINIT_UNIX_BACKEND=x11 WAYLAND_DISPLAY= uv run platynui-inspector
```

> **Note:** Setting `WINIT_UNIX_BACKEND=x11` alone is not sufficient — `smithay-clipboard` still connects to Wayland via `WAYLAND_DISPLAY` regardless of the windowing backend. Both variables must be set.

To make this permanent, add to your shell profile (e.g. `~/.bashrc` or `~/.zshrc`):

```bash
# Force X11 backend for winit/eframe apps under WSLg
# and disable Wayland for clipboard (smithay-clipboard workaround)
if [ -n "$WSL_DISTRO_NAME" ]; then
    export WINIT_UNIX_BACKEND=x11
    export WAYLAND_DISPLAY=
fi
```
