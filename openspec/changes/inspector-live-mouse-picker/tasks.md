## 1. Hit-test capability in the core + mock (test-first)

- [x] 1.1 Add mock-provider hit-test tests derived from the `element-at-point` spec: deepest-node-at-point, sibling z-order, a miss returns nothing, and the "unsupported" default is detectable on a provider that does not implement it.
- [x] 1.2 Add the optional hit-test method to the provider contract (`element_at_point(Point) -> Result<Option<Arc<dyn UiNode>>>`) with a default implementation that reports "unsupported"; confirm all existing providers still compile unchanged.
- [x] 1.3 Implement the mock provider's geometric hit-test (walk the in-memory tree, topmost/deepest node containing the point, honoring sibling stacking) so 1.1 passes; add a per-node stacking notion to the mock model if none exists, so the overlapping-sibling scenario is expressible.
- [x] 1.4 Expose hit-test on the runtime surface (a `Runtime` method returning the resolved node), so the Inspector can call it without reaching into providers directly.

## 2. Cursor-position "unavailable" contract

- [x] 2.1 Add/adjust tests so `pointer_position()` returns `CapabilityUnavailable` (not `(0,0)`) on the generic-Wayland EIS and virtual-input backends when no real position is known.
- [x] 2.2 Change the EIS and virtual-input backends ([eis.rs](../../../crates/platform-linux-wayland/src/input/eis.rs), [virtual_input.rs](../../../crates/platform-linux-wayland/src/input/virtual_input.rs)) to surface `CapabilityUnavailable` instead of the `(0,0)` fallback; leave the control-socket (test compositor), X11, and Windows real-position paths unchanged.
- [x] 2.3 Audit callers of `pointer_position()` / `Runtime::pointer_position()` for reliance on the old `(0,0)` fallback (expected: only the new picker); confirm the `Get Pointer Position` keyword degrades to a clear error rather than a wrong coordinate.

## 3. Real-provider hit-test: Windows UIA (self-contained)

- [x] 3.1 Implement UIA hit-test via the built-in `IUIAutomation::ElementFromPoint`, returning a node whose parent chain and runtime identity match top-down traversal. No `WindowManager` involvement — the API resolves window and in-window z-order natively.
- [ ] 3.2 Verify on the acceptance lane (real UIA): a point over a control resolves to it; overlapping windows resolve to the topmost; a point over the desktop background resolves to nothing.

## 4. Window-at-point capability (WindowManager) for the AT-SPI path

- [x] 4.1 Decide the capability shape (open question): `window_at_point(Point) -> Option<WindowId>` vs. a stacking-ordered `windows()` enumeration the caller filters by extents; add it to the `WindowManager` trait ([window_manager.rs](../../../crates/core/src/platform/window_manager.rs)) with an "unsupported" default so Windows/mock are unaffected.
- [x] 4.2 Implement it on X11 via EWMH stacking (`_NET_CLIENT_LIST_STACKING`, alongside the existing `_NET_CLIENT_LIST` at [window_manager.rs:32](../../../crates/platform-linux-x11/src/window_manager.rs)); optionally use `query_pointer.child` ([pointer.rs:21](../../../crates/platform-linux-x11/src/pointer.rs)) as a fast hint.
- [x] 4.3 Implement it on the PlatynUI compositor: add a control command (sibling to `list_windows`/`get_pointer_position`, [control.rs:14-22](../../../apps/wayland-compositor/src/control.rs)) backed by `surface_under_point` ([input.rs:756](../../../apps/wayland-compositor/src/input.rs)), returning enough to correlate (surface + `app_id`/PID + geometry); consume it in the Wayland platform's `WindowManager`.

## 5. Real-provider hit-test: Linux AT-SPI

- [x] 5.1 Select the frontmost top-level window at the point via the new `window_at_point` capability (task 4), then correlate that `WindowId` to its AT-SPI top-level Accessible by PID + geometry (reusing `resolve_window`'s matching in reverse); fall back to "topmost window whose extents contain the point" if correlation fails.
- [x] 5.2 Resolve the element within the application by a **geometric bounds subtree search** (not the native `get_accessible_at_point`): walk the accessible tree reading each node's toolkit-aware `Control:Bounds` and pick the smallest-area node whose bounds contain the point. The native hit-test was abandoned because it proved unreliable across toolkits — Qt reports bad screen extents, and AccessKit returns the widget *beneath* an overlay. Scope: when a managed frame maps to the hit window, confine the search to that frame (multi-window correctness); otherwise (override-redirect popup with no managed frame) search the whole application subtree, unpruned by parent bounds, so a menu drawn outside its owning frame is still reached. A node budget guards against pathological trees.
- [x] 5.3 In-window layer disambiguation superseded by the smallest-area rule: a popup/menu item's bounds are strictly smaller than the client area beneath it, so the most-specific (smallest) containing box already wins without querying `Component.Layer`/`MDIZOrder`. (The `component_layer_rank` helper added for this was removed as unused.)
- [x] 5.4 Verified on the X11 acceptance lane (real AT-SPI): frontmost-window selection incl. a multi-top-level child dialog resolving the dialog (not the main window); element inside the window resolves; **menu-bar menu items are picked on both egui/AccessKit and Qt** (the geometric search reaches them where the native hit-test could not). See [tests/acceptance/egui/hit_test.robot](../../../tests/acceptance/egui/hit_test.robot) and [tests/acceptance/qt/hit_test.robot](../../../tests/acceptance/qt/hit_test.robot).
- [x] 5.5 Exclude hidden nodes from the result: a node whose bounds contain the point but which reports `Control:IsVisible` (or `Control:IsInView`) explicitly false SHALL NOT be picked (the accessibility tree keeps laid-out but hidden nodes with stale bounds — e.g. a closed menu's items report `IsVisible=false` with bounds `{0,0,145,26}`). Gate the candidate selection in the AT-SPI search and mirror it in the mock's `hit_test_node`; both fall through to the visible node. Covered by mock unit tests (`hit_test_skips_hidden_node`, `hit_test_hidden_topmost_falls_through_to_visible_beneath`).

## 6. Inspector picker: core loop + gating (pure, unit-testable)

- [x] 6.1 Add unit tests for the picker decision logic from the `inspector-live-mouse-picker` spec: armed + configured-combination-held + available position → resolve+reveal; disarmed → no effect; partial/other combination → inactive; unavailable position → tick skipped without moving selection; re-resolving the same element is idempotent; a miss leaves selection unchanged; a stale (post-release/disarm) result is discarded.
- [x] 6.2 Implement the picker state module (armed toggle state, configured-combination match, per-tick decision, cursor-move threshold, last-resolved cache, epoch tag for stale-result guarding) as a pure module so 6.1 can test it without egui.
- [x] 6.3 Compute picker availability from the two probes (real `pointer_position()` and hit-test "supported"); expose it for gating (disabled ⇒ not armable).
- [x] 6.4 Wire the picker into the Inspector poll: on each armed tick, read position + modifiers, call hit-test, and drive the existing `reveal_task`/`select_node` and `highlight_bounds` paths ([async_tasks.rs](../../../apps/inspector/src/viewmodel/async_tasks.rs), [inspector_vm.rs](../../../apps/inspector/src/viewmodel/inspector_vm.rs)), tagging results with an epoch (as reveal/highlight already do) so results arriving after release/disarm are dropped.
- [x] 6.5 Highlight overlay is hit-test-transparent so the picker never resolves its own overlay. **Windows:** the overlay was already created with `WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_NOACTIVATE` ([platform-windows/src/highlight.rs](../../../crates/platform-windows/src/highlight.rs)) — OS-level click-through, so `ElementFromPoint` passes through. **X11:** the overlay windows are now given an empty SHAPE **input** region ([platform-linux-x11/src/highlight.rs](../../../crates/platform-linux-x11/src/highlight.rs)) so clicks pass through (parity with Windows); the bounding/clip shape is untouched, so the outline still draws (verified by the "Highlight Does Not Error" acceptance test). Independently, the picker already cannot resolve the overlay: it is an override-redirect window absent from `_NET_CLIENT_LIST_STACKING` and carries no `_NET_WM_WINDOW_TYPE`, so neither `managed_window_at` nor the type-filtered `popup_window_at` sees it — the "filtering as fallback" the task called for, plus the self-PID skip. No feedback loop.

## 7. Inspector picker: per-platform modifier reading + configuration

- [x] 7.1 X11: read the modifier mask (Control/Shift/Mod1) from the `query_pointer` reply ([platform-linux-x11/src/pointer.rs](../../../crates/platform-linux-x11/src/pointer.rs) already fetches it) inside an Inspector-internal, `cfg`-gated module.
- [x] 7.2 Windows: poll `GetAsyncKeyState(VK_CONTROL/VK_MENU/VK_SHIFT)` in the same Inspector-internal module.
- [ ] 7.3 Test compositor: extend the Wayland control protocol to report `wl_keyboard` modifier state (piggy-backed on `get_pointer_position` or a sibling command) in [apps/wayland-compositor](../../../apps/wayland-compositor) + [platform-linux-wayland control path](../../../crates/platform-linux-wayland/src/input/control_socket.rs); read it in the Inspector module.
- [ ] 7.4 Represent the activation combination as configurable data (default Ctrl+Alt+Shift), evaluated by the pure decision logic (task 6.2), and persist it with the Inspector's other settings; add tests for "changed combination activates / former no longer activates / partial does not".

## 8. Inspector picker: UX and discoverability

- [x] 8.1 Add the toolbar on/off toggle that arms/disarms picking, shows the configured activation gesture (e.g. "hold Ctrl+Alt+Shift"), and reflects disarmed / armed / actively-picking state.
- [x] 8.2 Grey out / disable the toggle (not armable) where picker availability is false, with a discoverable reason (unsupported cursor position or hit-test).
- [x] 8.3 Add the status hint text (armed prompt, "picking…", or unavailable reason).
- [ ] 8.4 Add UI to configure the activation combination (bind to the configurable data from task 7.4).

## 9. Docs

- [x] 9.1 Update [dev-docs/inspector.md](../../../dev-docs/inspector.md) with the picker feature and its per-platform gating.
- [x] 9.2 Update the README platform-support table (picker coverage: Windows/UIA, X11/AT-SPI, test compositor full; generic Wayland + macOS deferred), noting the deferred generic-Wayland path (cursor-position extensions / GlobalShortcuts portal) as future work.

## 10. Verification

- [x] 10.1 Run `just check` and `just test` (workspace clippy + nextest), including the new mock hit-test, `window_at_point`, and picker-logic unit tests.
- [ ] 10.2 Run the real-provider acceptance checks in the acceptance lane (egui app in the compositor/X session): UIA and AT-SPI hit-test (incl. overlapping windows and an overlapping popup), coordinate-space consistency (a point read from the cursor resolves to the element under it; sanity-check HiDPI / multi-monitor if available), the highlight overlay not being self-picked, and the live picker following the cursor + gating; on macOS/generic-Wayland confirm the picker is correctly greyed out.

## Notes

- Added a `Get Element At Point` BareMetal keyword + `PyRuntime.element_at_point` binding (beyond the proposal's "no RF keyword" note) as the acceptance-lane hook; verified via `tests/acceptance/egui/hit_test.robot` and `tests/acceptance/qt/hit_test.robot` (X11 lane, 6/6 pass: button, follows-cursor, and open menu-bar menu item on egui; main window, child dialog, and open menu-bar menu item on Qt).
- The AT-SPI in-window resolution pivoted from the designed native `get_accessible_at_point` descent to a geometric smallest-area bounds subtree search (see task 5.2) after the native path proved unreliable on Qt (bad screen extents) and egui/AccessKit (returns the widget beneath an overlay). The switch also made menu-bar menu items pickable on both toolkits.
- **Known limitation — transient context menus:** not pickable by this top-down resolver, but NOT because the toolkit hides them. On Qt the popup *is* exposed on AT-SPI, event-driven: while an AT client is registered for events, a `PopupMenu` appears as a child of the `Application`, reachable via the hovered item's `parent()` chain and via `getChildren` down from the popup — but absent from the `Application`'s own top-down `getChildren`, so the geometric bounds walk misses it (a screen reader following children-changed / state-changed:showing events finds it). Verified in the X11 lane with an AT-SPI event watcher (`crates/provider-atspi/examples/atspi_focus_watch.rs`). Picking context menus needs event-driven tree updates in the provider, tracked as the separate `atspi-event-driven-tree` change; the Qt test app keeps a realistic right-click context menu as the fixture.
- Deferred (first cut ships without): 3.2 Windows acceptance (real Windows), 7.3 compositor modifier reading, 7.4/8.4 configurable-combination UI + persistence (default Ctrl+Alt+Shift hardcoded), full interactive picker acceptance suite.
- 10.1 `just check`: passes fmt+clippy(workspace)+ruff+mypy for all touched files; one pre-existing unrelated mypy nit in tests/PlatynUI/test_settings.py.
