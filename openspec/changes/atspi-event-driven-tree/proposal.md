# Surface transient popups (context menus, dropdowns) in the AT-SPI tree via events

## Why

The AT-SPI provider builds its tree purely by **top-down** traversal: `get_nodes(parent)` calls `Accessible.GetChildren` and recurses ([crates/provider-atspi/src/lib.rs:124](../../../crates/provider-atspi/src/lib.rs)). That misses an entire class of elements: **transient popup windows** — right-click **context menus**, combo-box **dropdowns**, **tooltips** — because at least one real toolkit exposes them *asymmetrically*.

Verified on Qt/PySide6 (X11) with our own AT-SPI event watcher and raw tree dumper (`crates/provider-atspi/examples/atspi_focus_watch.rs`, `dump_registry_tree.rs`): when a context menu opens, the `PopupMenu` accessible **is** on the bus and is a child of the `Application`, reachable via the hovered item's `parent()` chain (up) and via `getChildren` **down** from the popup — but it is **absent from the `Application`'s own `getChildren`**. A top-down walk therefore never sees it. Worse, the toolkit only creates/attaches the popup accessible while an assistive-technology client is **registered for events** (the "observer effect" — this is also why a screen reader like Orca reads the menu while our tools did not).

Consequence today: context menus and other transient popups are invisible to **XPath queries** (`BM.Query`), the **Inspector** tree, and the **live mouse picker** — even though the user can see them and a screen reader announces them. The `inspector-live-mouse-picker` change documents this as its one remaining menu limitation.

## What Changes

Make the AT-SPI provider **event-aware** so transient popups appear in the tree:

- The provider registers for structural AT-SPI events on a **dedicated connection** (separate from the one used for synchronous queries — making synchronous proxy calls on the stream's own connection deadlocks the stream; we hit and fixed exactly this in the watcher tool).
- On `object:children-changed:add` / `object:state-changed:showing` for a popup-class node (menu / popup / dialog / tooltip / combo), the provider records the popup and the owning node it belongs under (resolved via the `parent()` chain). On hide / remove / defunct it drops it.
- `get_nodes(parent)` **merges** any recorded popups whose owner is `parent` into that node's children. Each popup's own subtree enumerates normally via `getChildren` (verified reachable downward).
- The popup thus becomes part of the provider tree → findable by **XPath**, visible in the **Inspector**, and resolvable by the **hit-test / picker** (the geometric bounds search then finds the menu item, whose `Control:Bounds` are already correct).
- This is the first concrete use of the AT-SPI provider's long-planned event streaming ("Event streaming … will follow", [lib.rs:4](../../../crates/provider-atspi/src/lib.rs)); it also opens the door to emitting `ProviderEvent`s to the runtime pipeline ([tree_provider.rs:41](../../../crates/core/src/provider/tree_provider.rs), architecture §7.2), though live-refresh notification is a follow-up, not required here.

## Impact

- **Rust — `crates/provider-atspi`**: new internal AT-SPI event loop (background task/thread on its own connection), a popup registry, and merge logic in `get_nodes`. Requires a **native rebuild** (`just build-native`). Additive: absent any popup events the tree is exactly as today.
- **Providers/platforms**: AT-SPI (Linux X11; Wayland where the same bus applies). Verified on **Qt**; **GTK and other toolkits must be re-checked** with the watcher (their exposure may differ — GTK menus were previously thought broken, but that predates the watcher fix). **Windows/UIA and mock are unaffected** (UIA exposes popups natively top-down already; mock has no transient popups).
- **Python/RF**: no new keyword — findability flows through the existing `Query`/hit-test surface. New **acceptance coverage**: opening a Qt context menu and resolving `//item:MenuItem[@Name=...]` by `Query` and by `Get Element At Point`.
- **Dependent change**: unblocks the deferred "context menu" limitation in `inspector-live-mouse-picker`.
- **Not breaking**: no behavior change for callers that never open a transient popup.
