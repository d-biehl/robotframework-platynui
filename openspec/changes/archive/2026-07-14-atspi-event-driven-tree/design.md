# Design — event-driven surfacing of transient popups (AT-SPI)

## What was verified (real Qt/PySide6 on X11)

All of the following was observed directly with two diagnostic tools kept in the repo — `crates/provider-atspi/examples/atspi_focus_watch.rs` (registers for AT-SPI events; prints each with its `parent()` chain and, for popup-class nodes, a `getChildren` dump downward) and `crates/provider-atspi/examples/dump_registry_tree.rs` (cold top-down `GetChildren` walk from the registry root):

1. **A context menu opening emits `object:state-changed:showing (enabled=true)` on a `PopupMenu`**, whose `parent()` chain is `PopupMenu → Application` (sibling of the main-window `Frame`). Hovering an item additionally emits `object:state-changed:focused` / `focus` on the `MenuItem`.
2. **`getChildren` downward from that `PopupMenu` returns its items** (`ctx-cut`, `ctx-copy`, `ctx-paste`) — the popup's own subtree is fully enumerable top-down.
3. **The `Application`'s own `getChildren` does NOT list the `PopupMenu`.** The asymmetry is exactly one link: parent→child (down from app) is missing, while child→parent (`parent()`) and down-from-popup work. This is why the top-down `get_nodes` walk misses it.
4. **Exposure is lazy / observer-gated:** with no AT client registered for events, the popup is absent even from the event path; the cold dump never shows it. Registering for events (as a screen reader does) is what makes the toolkit expose it. This matches the earlier mystery of "Orca reads it but our picker doesn't".
5. **Menu-item `Control:Bounds` are correct** (established in `inspector-live-mouse-picker`: menu-bar items resolved by bounds), so once a popup is in the tree the existing geometric hit-test resolves its items with no extra work.

Not yet verified (assumptions to test during implementation): whether **GTK** and other toolkits expose transient popups the same way (they may use `window:create`, a different role, or a different attachment point); the exact set of hide/teardown events; behavior of nested submenus.

## The mechanism

`get_nodes` today ([crates/provider-atspi/src/lib.rs:124](../../../crates/provider-atspi/src/lib.rs)) is a pure function of `Accessible.GetChildren`. We keep that as the baseline and **augment** it with popups discovered out-of-band via events:

- **Event connection (dedicated).** A background worker owns its own `AccessibilityConnection` and registers for the structural events (`object:children-changed`, `object:state-changed`). It must NOT be the connection used for the provider's synchronous `GetChildren`/property reads: making a blocking proxy call on the very connection whose event stream is being awaited deadlocks the stream and no events arrive. We hit this exact bug in `atspi_focus_watch` and fixed it by using a second connection — the provider must do the same.
- **Popup registry.** A shared, mutex-guarded map: popup object-ref → owner object-ref (the accessible under which it should appear, from the popup's `parent()`), plus its role. On a popup-class `showing=true` / `children-added`, resolve the owner and insert. On `showing=false` / `children-removed` / `state:defunct`, remove. "Popup-class" = role in {PopupMenu, Menu, Window, Dialog, ToolTip, ComboBox-dropdown} (final set TBD per toolkit).
- **Merge in `get_nodes`.** When enumerating a node's children, append the registered popups whose owner is that node, deduped against whatever `GetChildren` already returned. Each popup node is a normal `AtspiNode`; its own children come from `GetChildren` (verified). Parent wiring/keepalive follows the pattern the picker already uses so the grafted node's ancestor chain stays walkable.
- **A grafted popup is NOT a window surface.** It hangs directly under the `Application` like a real top-level, but it is an override-redirect window the WM does not manage — resolving it through the window manager PID-matches the app's *managed* window, which poisons the whole subtree's bounds (computed from the wrong window's origin) and exposes window patterns whose activation re-stacks a window mid-interaction. Popup-class roles are excluded from window-surface treatment; their geometry comes from AT-SPI screen extents. (Found via the submenu acceptance tests; the single-level hit-test had passed spuriously because move-to and hit-test shared the same wrong bounds.)

**Alternative considered — resolve popups on demand inside the hit-test only** (consult the focused/active accessible when the bounds search misses). Rejected as the primary design: it would fix the picker but NOT XPath/Inspector findability, which is the actual goal ("wir wollen das Element über den XPath finden"). The registry approach makes the popup a first-class tree node for every consumer.

**Alternative considered — keep polling top-down and hope.** Rejected: the asymmetry (point 3) is structural; no amount of top-down `GetChildren` will ever return the popup.

## Threading / lifecycle

The provider is otherwise synchronous (`block_on_timeout` over D-Bus). This adds one long-lived background worker per provider instance for the event stream, writing into the shared registry; `get_nodes` (called on the caller's thread) reads it. The worker starts with the provider's connection and stops on `shutdown()`. This is the provider's first use of the planned event streaming ([lib.rs:4](../../../crates/provider-atspi/src/lib.rs)); emitting `ProviderEvent::TreeInvalidated` to the runtime pipeline ([tree_provider.rs:41](../../../crates/core/src/provider/tree_provider.rs), architecture §7.2) so the Inspector auto-refreshes is a natural but **separate** follow-up.

## Open questions (resolve during implementation)

- Owner resolution: how many `parent()` hops, and how to map the owner object-ref onto the *cached* `AtspiNode` a caller is enumerating (identity by bus-name+path).
- Cache coherence: nodes cache children; a graft must invalidate/augment the owner's cached child list.
- Registry lifetime vs. rapidly opening/closing popups; missing hide events (fall back to pruning defunct/`showing=false` on next read).
- Which roles/events per toolkit — verify GTK with the watcher before widening beyond Qt.
- Self-filtering still applies (never surface our own process's popups), consistent with the existing `SELF_PID` skip.

## Migration Plan

- **Additive, behavioral only when a popup is open.** With no transient popup present, `get_nodes` returns exactly today's result; existing suites are unaffected.
- **Needs a native rebuild** (`just build-native`) — Rust provider change behind the Python bindings.
- **Rollback:** gate the event worker behind a provider flag (default on); disabling it reverts to pure top-down traversal. No data migration, no persisted state.
- **Verification** is real-provider-only (the mock has no transient popups): the X11 acceptance lane opening a Qt context menu.
