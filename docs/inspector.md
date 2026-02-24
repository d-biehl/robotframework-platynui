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
- **`InspectorViewModel`** — Top-level app state: owns `TreeViewModel`, `Runtime`, selection/focus indices, search text, results, properties cache. Provides keyboard navigation (Up/Down/Left/Right/Home/End/PageUp/PageDown), `evaluate_xpath()`, `reveal_and_select_result()`, and auto-highlight on selection.

### View Layer (`view/`)

All view functions are pure rendering — they read state and return action enums. No mutation of ViewModel state happens inside view code.

- **`tree_view::show_tree()`** — ScrollArea with indented rows, disclosure triangles, role icons, selection/focus indicators, context menu (Refresh / Refresh Subtree). Returns `Vec<TreeAction>`.
- **`properties::show_properties()`** — `egui_extras::TableBuilder` with sortable columns (Name, Value, Type). Each cell is a read-only `TextEdit` for native text selection. Context menu: Copy Name/Value/Type/Row.
- **`toolbar::show_menu_bar()`** / `show_search_bar()` / `show_results_panel()` — Menu bar, XPath search with Enter/Button, results list with click-to-reveal. Returns `Vec<ToolbarAction>`.

## Features

- **UI Tree** — Hierarchical tree with lazy child loading, expand/collapse, keyboard navigation, role icons, invalid-node strikethrough
- **Properties Panel** — Sortable table with namespace:name, value, type columns; copy via context menu and native text selection
- **XPath Search** — Expression evaluation with results panel; click to reveal node in tree
- **Element Highlighting** — Selected elements are highlighted on screen (1.5s) via platform highlight provider
- **Always On Top** — Toggle to keep inspector above other windows
- **Context Menu** — Refresh node or subtree from tree view
- **Tracing** — `--log-level` CLI flag, `RUST_LOG` / `PLATYNUI_LOG_LEVEL` env vars
