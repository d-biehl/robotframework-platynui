## Context

The Inspector ([apps/inspector](../../../apps/inspector)) is a standalone egui/eframe app that links the platform + provider crates directly per target ([Cargo.toml:40-54](../../../apps/inspector/Cargo.toml)). It navigates the UI tree top-down and searches by XPath. There is no way to go from a point on screen to a node: the provider contract `UiTreeProvider` is one-directional — `get_nodes(parent) → children` ([crates/core/src/provider/tree_provider.rs](../../../crates/core/src/provider/tree_provider.rs)) — and nothing maps a coordinate back to a node.

Three pieces the picker needs already exist and are load-bearing for this design:

1. **Cursor position, full stack.** `PointerDevice::position()` ([crates/core/src/platform/pointer.rs:63](../../../crates/core/src/platform/pointer.rs)) is implemented by every backend and surfaced up through `Runtime::pointer_position()` ([crates/runtime/src/runtime/input.rs:42](../../../crates/runtime/src/runtime/input.rs)), the PyO3 binding ([packages/native/src/runtime.rs:937](../../../packages/native/src/runtime.rs)), and the `Get Pointer Position` keyword ([src/PlatynUI/BareMetal/__init__.py:1620](../../../src/PlatynUI/BareMetal/__init__.py)). Windows ([platform-windows/src/pointer.rs:20](../../../crates/platform-windows/src/pointer.rs)) and X11 ([platform-linux-x11/src/pointer.rs:21](../../../crates/platform-linux-x11/src/pointer.rs)) return the real live position. On our own compositor the value comes from the control socket's `get_pointer_position` command ([platform-linux-wayland/src/input/control_socket.rs:250](../../../crates/platform-linux-wayland/src/input/control_socket.rs)) and is real. On generic Wayland the EIS/virtual-input backends only return `last_position` — a dead-reckoning shadow of the last *injected* move — or `(0,0)` ([eis.rs:238](../../../crates/platform-linux-wayland/src/input/eis.rs), [virtual_input.rs:287](../../../crates/platform-linux-wayland/src/input/virtual_input.rs)); the intended live-query escape hatch `query_compositor_pointer_position` is a stub returning `None` ([eis.rs:377](../../../crates/platform-linux-wayland/src/input/eis.rs)).

2. **Reveal-and-select.** `reveal_task` ([apps/inspector/src/viewmodel/async_tasks.rs:153](../../../apps/inspector/src/viewmodel/async_tasks.rs)) already takes a target `UiNode`, walks its `parent()` chain to collect ancestor runtime-ids, preloads caches, then expands + selects the row. `select_node` and the attribute panel are already wired ([inspector_vm.rs:337](../../../apps/inspector/src/viewmodel/inspector_vm.rs)). This is exactly the "map a node into the tree" operation the picker needs.

3. **Highlight overlay.** `highlight_bounds` / `highlight_node_task` ([inspector_vm.rs:538](../../../apps/inspector/src/viewmodel/inspector_vm.rs)) drives the platform `HighlightProvider` ([crates/core/src/platform/highlight.rs](../../../crates/core/src/platform/highlight.rs)).

So the design centers on the missing half — a hit-test capability — plus a small Inspector poll loop that ties existing pieces together.

## Goals / Non-Goals

**Goals:**
- A provider-level "element at a screen point" operation, resolved natively per provider, with an explicit "unsupported" default.
- An Inspector held-Ctrl+Alt+Shift live picker that reuses the existing reveal/select/highlight machinery and follows the cursor while held.
- Honest capability gating: the picker is only live where cursor position AND hit-test are both real; elsewhere it is greyed out.
- Correct behavior on Windows/UIA, Linux X11/AT-SPI, and the PlatynUI test compositor.

**Non-Goals:**
- Generic Wayland (KDE/GNOME/Hyprland/Sway) support — deferred; blocked on live cursor position (and, later, a global-shortcut path). No XDG `GlobalShortcuts` portal, no portal backend, no compositor pointer-position extensions in this change.
- macOS support — deferred behind the AX provider stub.
- Promoting global-shortcut / modifier reading into a shared core/platform trait — it stays inside the Inspector.
- A generic geometric hit-test fallback in the runtime — considered (below) but not required for the first cut.
- Any new Python/Robot Framework keyword — cursor position is already exposed; hit-test is not needed from RF yet.

## Decisions

### Hit-test lives on the provider contract, resolved natively, with an "unsupported" default

Add an optional hit-test to the provider surface (conceptually `element_at_point(Point) -> Result<Option<Arc<dyn UiNode>>>`) with a default implementation that reports "unsupported," so existing providers compile unchanged and callers can detect the gap. Each real provider answers with its native facility:

- **UIA** — `IUIAutomation::ElementFromPoint`. Native, single call, correct z-order and cross-process. The provider already maps bounding rectangles via `get_bounding_rect` ([provider-windows-uia/src/map.rs:95](../../../crates/provider-windows-uia/src/map.rs)); this adds the reverse direction.
- **AT-SPI** — `ComponentProxy::get_accessible_at_point(x, y, coord_type)`, available in the `atspi-proxies` crate already in use (`ComponentProxy`, `CoordType` are imported in [provider-atspi/src/node.rs](../../../crates/provider-atspi/src/node.rs)). See the AT-SPI decision below.
- **Mock** — a geometric walk of the in-memory tree (deepest node containing the point, honoring sibling stacking). Gives deterministic hit-test tests without a desktop.

**Why on the provider, not a generic runtime tree-walk:** a coordinate → node mapping is inherently provider knowledge (native APIs know window z-order and cross-process layering that a bounds-only tree walk cannot reconstruct). *Alternative considered:* a runtime-level geometric fallback (traverse the tree, read each node's bounds, return the deepest containing node). Rejected for the first cut because it cannot resolve overlapping-window z-order and would be slow over live providers; it remains a future option for providers that expose bounds but no native hit-test.

### AT-SPI: window-level z-order from the WindowManager, within-window z-order from AT-SPI layers

`get_accessible_at_point` has three sharp edges that shape the approach: the registry desktop root does not implement `Component` (no single global hit-test), it descends one level per call (one D-Bus round-trip per level), and it is reliable only with `CoordType::Window` — screen-coordinate hit-testing is toolkit-dependent (and worse on Wayland, per the README's "Wayland coordinate limits"). So the answer is resolved as **two levels of z-order from two different sources**:

1. **Which top-level window is frontmost at the point — from the WindowManager, not from AT-SPI.** This is authoritative on both supported Linux paths and needs no guessing:
   - **X11:** the EWMH stacking order (`_NET_CLIENT_LIST_STACKING`, sibling of the `_NET_CLIENT_LIST` the WM already interns at [window_manager.rs:32](../../../crates/platform-linux-x11/src/window_manager.rs)); as a cheaper hint, the same `query_pointer` call used for position/modifiers also returns the window under the pointer in `reply.child` ([pointer.rs:21](../../../crates/platform-linux-x11/src/pointer.rs)).
   - **PlatynUI compositor:** the compositor *is* the window manager and already computes `surface_under_point(state, location)` for input routing ([apps/wayland-compositor/src/input.rs:756](../../../apps/wayland-compositor/src/input.rs)); expose it as a new control command (sibling to `list_windows` / `get_pointer_position`, [control.rs:14-22](../../../apps/wayland-compositor/src/control.rs)). Z-order here is authoritative — the compositor owns the stack.
   - The window handle from the WM is then **correlated to its AT-SPI top-level Accessible** by PID + geometry — the same matching `WindowManager::resolve_window` already does node→`WindowId` ([window_manager.rs](../../../crates/platform-linux-x11/src/window_manager.rs) uses `_NET_WM_PID`; the compositor knows `app_id`/PID per surface), just used in the reverse direction.
2. **Which accessible wins within that window — from AT-SPI.** Descend via `get_accessible_at_point` in `CoordType::Window` after translating the screen point through the window's extents, terminating when no deeper child contains the point. But plain descent cannot always disambiguate **overlapping layers within one window** (a popup/menu/tooltip drawn over the client area is a *sibling* at a higher layer, not a descendant). So among candidates at the point the hit-test SHALL prefer the higher AT-SPI layer, consulting `Component.Layer` / `Component.MDIZOrder` (already surfaced as attributes in [provider-atspi/src/node.rs](../../../crates/provider-atspi/src/node.rs)). `UiNodeExt::top_level_or_self` ([crates/core/src/ui/node.rs:107](../../../crates/core/src/ui/node.rs)) and `Component.Extents.*` are the remaining building blocks.

*Alternative considered:* trust screen-coordinate `get_accessible_at_point` from each app root, or derive the frontmost *window* from AT-SPI `Layer`/`MDIZOrder` alone. Rejected — screen-coordinate descent is unreliable across toolkits and broken on Wayland, and AT-SPI has no cross-application window-stacking view; the WM/compositor is the correct source for *window*-level z-order. AT-SPI `Layer`/`MDIZOrder` is kept, but scoped to *within-window* layer disambiguation, which is what it actually models.

**Implementation note (pivot):** step 2 above shipped differently. `get_accessible_at_point` proved unreliable *even in `CoordType::Window`* — Qt's AT-SPI bridge reports inaccurate extents and AccessKit's own hit-test returns the widget *beneath* an overlay — so the within-window resolution is instead a **geometric bounds subtree search**: walk the accessible tree, read each node's toolkit-aware `Control:Bounds`, and pick the smallest-area node containing the point. Selecting the smallest containing box makes a popup/menu item win over the client area beneath it, so the explicit `Layer`/`MDIZOrder` disambiguation was not needed (the `component_layer_rank` helper was added and then removed). The window-level source (step 1) is unchanged. The search is scoped to the matched frame for managed windows and to the whole application subtree — unpruned by parent bounds — for override-redirect popups, which is what lets it reach menu items drawn outside their owning frame. This made menu-bar menu items pickable on both egui/AccessKit and Qt. Transient right-click context menus remain out of reach for this top-down search — not because the toolkit hides them: on Qt the popup *is* exposed on AT-SPI, but only event-driven (a `PopupMenu` child of the `Application`, reachable via the hovered item's `parent()` chain and via `getChildren` down from the popup, yet absent from the `Application`'s own top-down `getChildren`). Reaching it needs the AT-SPI provider to subscribe to structural events and graft the popup into the tree — tracked as the separate `atspi-event-driven-tree` change.

### The WindowManager gains a "frontmost window at point" capability

Consuming window-level z-order (above) means the picker needs to ask the WM "which window is on top at point P?". The `WindowManager` trait ([crates/core/src/platform/window_manager.rs](../../../crates/core/src/platform/window_manager.rs)) has no such method today (it exposes `resolve_window`, `bounds`, `is_active`, activate/close/min/max/restore/move/resize). Add a `window_at_point(Point) -> Option<WindowId>` (and/or a stacking-ordered enumeration) implemented on X11 (EWMH stacking / `query_pointer.child`) and the compositor (control command over `surface_under_point`); Windows and mock can default to unsupported since they do not need it. This is a genuine platform capability (unlike the Inspector-only modifier reading), so it belongs on the shared trait.

**Windows needs none of this.** UIA's built-in `IUIAutomation::ElementFromPoint` already returns the accessible at a coordinate with correct window *and* in-window z-order in a single native call — the whole two-level problem above is solved by the platform, so the Windows hit-test does not touch the `WindowManager` at all.

### Modifier reading stays in the Inspector, per platform, no hotkey crate

The picker is a **poll loop** on the egui frame/timer while active: each tick read `(position, modifiers)`; if all of Ctrl+Alt+Shift are down and the position is available, resolve and reveal. Modifier state is read per platform inside the Inspector:

- **X11** — the `query_pointer` reply already carries the modifier mask (`reply.mask` with Control/Shift/Mod1); today only `root_x/root_y` are read ([platform-linux-x11/src/pointer.rs:21](../../../crates/platform-linux-x11/src/pointer.rs)). Position and modifiers come from one call.
- **Windows** — poll `GetAsyncKeyState(VK_CONTROL/VK_MENU/VK_SHIFT)`; works globally without a window/hook/message-loop dependency.
- **Test compositor** — extend the control protocol to report `wl_keyboard` modifier state alongside `get_pointer_position`.

**Why not a crate (e.g. `global-hotkey`) and why not the core:** a held-modifier mode is a *state poll*, not a discrete registered chord — `RegisterHotKey`/`XGrabKey` register a modifier+key combination and do not model "these modifiers are currently down," so they are the wrong tool; per-platform reads are a few lines each and a better fit. And the need is Inspector-only (per the user decision), so it does not warrant a shared trait. egui cannot supply this itself: it only sees keyboard input when focused, and the whole point is hovering over another app.

*Where modifier reading physically sits:* a small `cfg(target_os)`-gated module inside `apps/inspector`. On X11 the modifier mask coincidentally rides the same query as position, but this is an Inspector-internal read; it does **not** change the shared `PointerDevice` contract.

### Cursor-position contract: unavailable, not (0,0)

Backends that cannot observe the physical pointer SHALL make `pointer_position()` return `CapabilityUnavailable` instead of `(0,0)` ([eis.rs:238](../../../crates/platform-linux-wayland/src/input/eis.rs), [virtual_input.rs:287](../../../crates/platform-linux-wayland/src/input/virtual_input.rs) today fall back to `(0,0)`). This lets the Inspector distinguish "real position" from "no idea," which is what the gating hinges on. A silent `(0,0)` would make the picker jump the selection to the screen corner — the exact misbehavior gating is meant to prevent.

### Arming toggle + configurable modifier combination

Picking is gated behind an explicit on/off **toggle** (a toolbar switch), not always-on: the Inspector reads the activation modifiers only while armed, so it never watches the keyboard behind the user's back. While armed, holding the **configured** modifier combination (default Ctrl+Alt+Shift) picks. The combination is user-configurable and persists like the Inspector's other settings. *Rationale (resolves the earlier open question):* an explicit switch is more discoverable and less surprising than silent global modifier watching, and configurability avoids clashes with OS/app chords on the user's setup. The per-tick decision therefore becomes: *armed* AND *configured combination held* AND *position available* → resolve.

### Self-exclusion: the highlight overlay must be hit-test-transparent

The picker highlights the target using the existing overlay, but that overlay is drawn *on top* of the target (override-redirect windows on X11, a layered window on Windows). Without care, the next tick's hit-test would resolve the overlay instead of the target and picking would stick to itself. The overlay MUST be excluded from hit-test — preferably rendered click/hit-through at the OS level (`WS_EX_TRANSPARENT` on Windows; input-region / override-redirect-non-input on X11), with filtering the resolved node as a fallback. Hovering the Inspector's own window must be deterministic (own controls or nothing), never a feedback loop.

### Stale-result guarding

Because AT-SPI resolution/reveal is async and can outlast a modifier release, each pick is tagged with an epoch (the same pattern the Inspector already uses for reveal/highlight/selection via `next_epoch` in [inspector_vm.rs](../../../apps/inspector/src/viewmodel/inspector_vm.rs)); a result whose epoch is stale — because picking stopped (release or disarm) — is discarded and does not move the selection.

### Gating

The Inspector computes picker availability from two probes: does `pointer_position()` return a real value (not `CapabilityUnavailable`), and does hit-test report as supported. If either fails, the toggle is greyed out and not armable, with the reason shown; the pure availability decision (e.g. "hit-test unsupported → disabled") is unit-testable with a mock reporting "unsupported."

## Risks / Trade-offs

- **Correlating the WM window to its AT-SPI application is the least-certain piece** → window-level z-order is now sourced authoritatively from the WM/compositor, but mapping that `WindowId` (HWND-analog / X11 window / compositor surface) to the right AT-SPI top-level Accessible relies on PID + geometry matching, which can be ambiguous (one PID, multiple top-levels; reparenting WMs). Mitigation: reuse the matching logic `resolve_window` already applies (`_NET_WM_PID` on X11, `app_id`/PID per surface on the compositor); verify against GTK/Qt on the acceptance lane; fall back to "topmost window whose extents contain the point" if correlation fails, and document the limitation.
- **Within-window AT-SPI layer disambiguation for overlapping popups/menus** → plain `get_accessible_at_point` descent may pick the client area under a popup rather than the popup. Mitigation: prefer the higher `Component.Layer`/`MDIZOrder` among point-containing candidates; verify with a real menu/tooltip on the acceptance lane (mock cannot exercise real AT-SPI layers).
- **AT-SPI descent latency** → one D-Bus round-trip per tree level, per picker tick, could feel sluggish on deep trees. Mitigation: the picker already runs on the async task path (like `reveal_task`); throttle ticks (only re-resolve when the cursor actually moved past a small threshold) and cache the last resolved node.
- **`pointer_position()` contract change could affect other callers** → moving from `(0,0)` to `CapabilityUnavailable`. Mitigation: audit callers (in practice only the picker cares); the `Get Pointer Position` keyword already surfaces platform errors, so RF behavior degrades to a clear error rather than a wrong coordinate.
- **Modifier polling frequency vs. responsiveness** → too slow feels laggy, too fast wastes CPU/D-Bus. Mitigation: poll on the egui repaint tick while the picker toggle is armed; request repaints only while armed.
- **`GetAsyncKeyState` reports physical key state process-wide but is a polled snapshot** → brief chords between ticks could be missed, but for a *held* mode this is a non-issue.

## Migration Plan

- **Nature:** additive at the API level (new optional provider method defaulting to "unsupported"; new Inspector UI/behavior), with **one behavioral change**: `pointer_position()` returns `CapabilityUnavailable` instead of `(0,0)` on the generic-Wayland EIS/virtual-input backends.
- **Native rebuild:** not required for the Inspector binary (it links providers directly). A rebuild of `packages/native` is only needed if the hit-test surface is later exposed to Python — it is not in this change. The `pointer_position()` contract change is within the Rust stack and reaches Python only as a clearer error from the existing keyword.
- **Rollout:** land the hit-test capability + mock implementation and its unit tests first (no user-visible change); then the WM `window_at_point` capability (X11 + compositor); then UIA (self-contained) and AT-SPI (WM window selection → correlation → in-window descent with layer disambiguation) verified on the acceptance lane; then the Inspector poll loop, gating, and UX; the compositor modifier-reporting addition last.
- **Rollback:** revert the Inspector picker commit to remove the feature entirely; the hit-test provider method is inert (unused, "unsupported" default) if the Inspector side is absent. The `pointer_position()` contract change is the only part with external reach and can be reverted independently.

## Open Questions

- How robustly can a WM `WindowId` be correlated to its AT-SPI top-level Accessible (PID + geometry) across GTK/Qt and reparenting WMs? Is the `resolve_window` matching directly reusable in reverse, and what is the fallback when a PID owns several top-levels? (Verify on the acceptance lane.)
- What is the exact shape of the new WM capability — `window_at_point(Point) -> Option<WindowId>`, or a stacking-ordered `windows()` enumeration the caller filters by extents? The former is less to implement on the compositor (it already has `surface_under_point`); the latter is more reusable but needs stacking output on X11.
- Does the compositor's control response naturally return enough to correlate (surface + `app_id` + PID + geometry), or does `window_at_point` need to return a richer record than just an id?
- ~~Explicit arming vs. always-on?~~ **Resolved:** explicit on/off toggle, with a configurable (default Ctrl+Alt+Shift) modifier combination that must be held while armed.
- Can the highlight overlay be made truly hit-test-transparent at the OS level on both X11 (input region / non-input override-redirect) and Windows (`WS_EX_TRANSPARENT`), or is filtering the resolved node the practical fallback on some backends? (The highlight already renders; this is about its input behaviour.)
- Does the test compositor's control protocol have a natural place to piggy-back modifier state on the existing `get_pointer_position` response, or should it be a sibling command?
