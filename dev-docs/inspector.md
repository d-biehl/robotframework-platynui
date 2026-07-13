# Inspector

<!-- This is a living document. For version history see CHANGELOG.md and git log. -->

This document covers the PlatynUI GUI Inspector. For the platform-agnostic architecture, see `dev-docs/architecture.md`. For planned UX and workflow improvements, see `dev-docs/inspector-improvements.md`.

Binary: `platynui-inspector-rs` (package `platynui-inspector`, egui-based GUI)

## Overview

The inspector is a desktop GUI tool for exploring and debugging the PlatynUI UI tree in real time. It connects to the PlatynUI runtime, displays the full UI element hierarchy, allows XPath queries against the tree, shows element attributes, and highlights selected elements on screen.

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
│   ├── inspector_vm.rs  ← InspectorViewModel (overall app state)
│   └── async_tasks.rs   ← Background task helpers
└── view/                ← V: Pure UI rendering (egui)
    ├── mod.rs
    ├── tree_view.rs     ← TreeView widget
    ├── attributes.rs    ← Attributes table
    ├── toolbar.rs       ← Menu bar, search bar
    ├── results_panel.rs ← XPath results list
    ├── status_bar.rs    ← Status bar
    └── about_dialog.rs  ← About dialog
```

### Model Layer (`model/`)

- **`UiNodeData`** — Cached wrapper around `Arc<dyn UiNode>`. Caches id, label, children, and `has_children` behind `Mutex` guards. Provides `display_attributes()` for the attributes table and `bounds_rect()` for highlighting. `refresh()` / `refresh_recursive()` invalidate caches.
- **`SearchResultItem`** — Enum for XPath results: `Node`, `Attribute` (with owner node for tree reveal), `Value`.
- **`DisplayAttribute`** — Flat struct for attributes table rows (namespace, name, value, type).

### ViewModel Layer (`viewmodel/`)

- **`TreeViewModel`** — Maintains a `HashSet<String>` of expanded node IDs and a flattened `Vec<VisibleRow>` of the currently visible tree. Supports `toggle`, `expand`, `collapse`, `reveal_node_cached` (auto-expand ancestor chain from cached data), `refresh_row`, `refresh_subtree`.
- **`InspectorViewModel`** — Top-level app state: owns `TreeViewModel`, `Runtime`, selection/focus indices, search text, results, attributes cache. Provides keyboard navigation (Up/Down/Left/Right/Home/End/PageUp/PageDown), `evaluate_xpath()` (non-blocking, spawns background thread), `poll_search()` (drains streaming results each frame), `cancel_search()`, `reveal_and_select_result()`, and auto-highlight on selection.

### View Layer (`view/`)

All view functions are pure rendering — they read state and return action enums. No mutation of ViewModel state happens inside view code.

- **`tree_view::TreeView`** — A generic tree widget built via `TreeView::new(&rows).selected(...).focused(...).show(ui)`: ScrollArea with indented rows, disclosure triangles, role icons, selection/focus indicators, context menu (Refresh / Refresh Subtree). `show()` returns a `TreeResponse` struct (`selected`, `toggled`, `navigate`, `page_size`).
- **`attributes::show_attributes()`** — `egui_extras::TableBuilder` with sortable columns (Name, Value, Type), a grouped/ungrouped namespace toggle, and a text filter over `namespace:name`, value, and type. Each cell is a read-only `TextEdit` for native text selection. In grouped mode, the same resizable table stays in place and inserts collapsible namespace group rows into the body, closer to a classic list-view layout. Context menu: Copy Name/Value/Type/Row, Pin Attribute, Unpin Attribute. Pinned attributes stay above the normal sorted rows.
- **`toolbar::show_menu_bar()`** / `show_search_bar()` — Menu bar and XPath search with Enter/Button (toggles to Stop while searching).
- **`results_panel::show_results_panel()`** — Keyboard-navigable results list with explicit reveal, highlight, and copy actions.

## Features

- **UI Tree** — Hierarchical tree with lazy child loading, expand/collapse, keyboard navigation, role icons, invalid-node strikethrough
- **Attributes Panel** — Sortable table with grouped and ungrouped namespace views, collapsible namespace groups in grouped mode, a quick text filter, copy via context menu and native text selection, and per-attribute pinning to keep common rows at the top
- **XPath Search** — Non-blocking, streaming XPath evaluation with cancellation support. Results appear incrementally with a live spinner and elapsed time. The Inspector shows the first 5000 results by default; override with `--search-result-limit` or `PLATYNUI_INSPECTOR_SEARCH_RESULT_LIMIT` (`unlimited` disables the guard). Single-click focuses a result, `Enter` or double-click reveals it in the tree, and a context menu exposes highlight and copy actions
- **Live Mouse Picker** — A toolbar toggle ("Pick Element") arms live picking: while armed, holding **Ctrl+Alt+Shift** continuously resolves the element under the mouse cursor (across other applications), selects it in the tree, and highlights it — the Accessibility-Insights-style workflow. It is gated on platform support (a live cursor position *and* hit-test): available on **Windows/UIA** and **Linux X11/AT-SPI**, greyed out where unsupported. Hit-test resolves window z-order from the platform (UIA `ElementFromPoint`; on X11 the window manager's EWMH stacking plus AT-SPI in-window descent). Global modifier state is read per platform inside the Inspector (X11 `XQueryPointer` mask, Windows `GetAsyncKeyState`). On AT-SPI the in-window resolution is a geometric subtree search: after the window manager selects the application at the point, the resolver walks that application's accessible tree and picks the smallest-area node whose toolkit-aware `Control:Bounds` contain the point (the most specific box under the cursor wins over the content behind it). Nodes that are not actually shown are excluded — a candidate reporting `Control:IsVisible` (or, when surfaced, `Control:IsInView`) explicitly false is skipped, because the accessibility tree keeps laid-out but hidden nodes with stale bounds (a closed menu's items, for instance, report `IsVisible=false` yet still carry bounds), and those must not be picked over the visible element the user sees. It deliberately reads each node's bounds rather than calling AccessKit/Qt's own `Component.GetAccessibleAtPoint`, which proved unreliable across toolkits (Qt reports bad screen extents; AccessKit returns the widget *beneath* an overlay). Picking a live element that post-dates the cached tree (opened menus, combo popups, expanded tree rows) refreshes the ancestor path so it is still revealed. Not yet supported: generic Wayland (no live cursor position for unfocused clients) and macOS (AX provider stub). Multi-top-level applications are handled: when a managed frame maps to the hit window, the search is scoped to that frame, so hovering a child dialog resolves that dialog, not the main window. On X11 the hit-test is also popup-aware — override-redirect windows carrying a menu/popup/combo/tooltip `_NET_WM_WINDOW_TYPE` are resolved above the window they popped from (filtering by window type avoids mistaking backing-store/panel/pointer override-redirect windows for popups). Because such a popup has no managed frame, the search falls back to the whole application subtree and — crucially, without pruning by parent bounds — reaches menu items even though the popup is drawn larger than, and outside, the frame that owns it in the AT-SPI tree.

Menu items **are** picked by hover on both **egui/AccessKit** and **Qt** menu-bar menus (the geometric search finds them where the old native hit-test could not). Known limitation: **transient right-click context menus** are not yet pickable. Not because the toolkit hides them — on Qt the popup *is* exposed on AT-SPI, but only *event-driven*: while an AT client is registered for events, the context `PopupMenu` appears as a child of the `Application`, reachable via the hovered item's `parent()` chain and via `getChildren` down from the popup, yet it is absent from the `Application`'s own top-down `getChildren`. The picker's geometric bounds search walks top-down, so it misses the popup (a screen reader following children-changed / state-changed:showing events finds it). Picking context menus needs an event-driven tree update in the AT-SPI provider (subscribe to structural events, graft the popup under its parent so it becomes XPath-visible) — tracked as the `atspi-event-driven-tree` change. The highlight overlay is hit-test-transparent so the picker never resolves it (Windows `WS_EX_TRANSPARENT`; X11 empty SHAPE input region), and on X11 it is additionally invisible to the window-at-point lookup (untyped override-redirect window). Other follow-ups: configurable activation combination and the PlatynUI-compositor modifier path.
- **Element Highlighting** — Selected elements are highlighted on screen (1.5s) via platform highlight provider
- **Always On Top** — Toggle to keep inspector above other windows
- **Context Menu** — Refresh node or subtree from tree view
- **Tracing** — `--log-level` CLI flag, `RUST_LOG` / `PLATYNUI_LOG_LEVEL` env vars

## Troubleshooting

### Windows: WGPU backend selection and startup latency

The Inspector uses `eframe` with the `wgpu` renderer by default. On Windows, it tries Vulkan first and DX12 next. OpenGL is not tried automatically because it can make startup noticeably slower on some systems while Windows UI Automation is being queried.

The practical defaults are:

- Unset `WGPU_BACKEND`: use Vulkan/DX12 only, with the adapter selector preferring Vulkan before DX12.
- `WGPU_BACKEND=vulkan`: force Vulkan.
- `WGPU_BACKEND=dx12`: force DX12.
- `WGPU_BACKEND=gl`: opt into the GL path for diagnostics or compatibility testing.

For implementation work, do not work around this with pre-window UIA root traversal or shell-specific heuristics such as stopping at `Program Manager`. The Inspector keeps provider traversal behind the first rendered frames and uses renderer/backend configuration to avoid the OpenGL-specific startup stall.

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
