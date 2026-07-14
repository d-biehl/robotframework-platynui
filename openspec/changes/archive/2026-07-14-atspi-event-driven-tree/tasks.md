## 1. Driving test (test-first, red)

- [x] 1.1 Add an X11 acceptance test (real AT-SPI, Qt app) that opens the test app's right-click context menu and asserts its items are findable: `BM.Query //item:MenuItem[@Name="ctx-copy"]` resolves it, and `BM.Get Element At Point` over the open item resolves it. This test FAILS today (top-down traversal misses the popup) and is the red bar this change turns green. Keep the menu open deterministically (press-and-hold right button, or a stable app trigger) and tear it down in teardown.
  - `tests/acceptance/qt/context_menu.robot` (locator uses the cross-provider `*:` wildcard, `//*:MenuItem[@Name="ctx-copy"]`; the app's `popup()` menu stays open after a plain right-click, Escape dismisses). Confirmed RED on the X11 lane: both tests fail with the item not on the accessibility tree.
- [x] 1.2 Add a companion assertion that a CLOSED context menu's items are NOT found (guards the dismiss path).

## 2. Confirm toolkit exposure (spike, real provider)

- [x] 2.1 Using `crates/provider-atspi/examples/atspi_focus_watch.rs`, record which events/roles carry the popup on each toolkit we care about: confirm Qt (`state-changed:showing` on `PopupMenu`, child-of-`Application`, `getChildren` down works), and **re-check GTK** (previously assumed broken — that predates the watcher fix) plus combo dropdowns / tooltips. Lock the popup-class role set and the event set the implementation will handle.
  - Spike results (X11 session, watcher + platynui-cli):
    - **Qt context menu** (confirmed as designed): `state-changed:showing=true` on `PopupMenu` whose `parent()` is the `Application`; `getChildren` down reaches the items; `showing=false` on dismiss; NO `children-changed` on the Application (graft required, owner = one `parent()` hop).
    - **GTK4 context menu** (re-checked; also needs the graft): `children-changed:add` on the *owning widget* (the entry, role `Text`) with the popup `Menu` as child — but the menu is NOT in the owner's `GetChildren` (an `Invalid` intermediate breaks the walk; `//*:MenuItem` finds nothing while open). Owner for `children-changed` = the event source. Removal: `children-changed:remove`.
    - **Qt combo dropdown**: the `List` under the `ComboBox` is in-tree (top-down reachable); the transient container is a generic `Panel` under the Application → `Panel` must NOT be popup-class; no graft needed.
    - **Qt tooltip**: emits no AT-SPI events at all → out of scope.
    - **Locked sets** — events: `object:state-changed` (showing/defunct) + `object:children-changed`; popup-class roles: `PopupMenu`, `Menu`, `Window`, `Dialog`, `ToolTip`. An asymmetry check at add time (skip when the owner's `GetChildren` already lists the popup) keeps in-tree popups and real windows out of the registry.

## 3. Event infrastructure in the AT-SPI provider

- [x] 3.1 Add a background event worker owning a **dedicated** `AccessibilityConnection` (separate from the synchronous query connection — a blocking call on the stream's own connection deadlocks it; see `atspi_focus_watch`'s two-connection fix). Register `object:children-changed` and `object:state-changed`. Start it lazily with the provider connection; stop it on `shutdown()`.
  - `crates/provider-atspi/src/popups.rs` (`PopupWatcher`): two dedicated connections (stream + query), started once in `connection()` via `ensure_popup_watcher()`, stopped in `shutdown()` by closing the stream's zbus connection (ends the stream, joins the thread, clears the registry).
- [x] 3.2 Implement a mutex-guarded popup registry: on a popup-class `showing=true` / `children-added`, resolve the owner via the popup's `parent()` chain and record `popup → owner`; on `showing=false` / `children-removed` / `defunct`, drop it. Apply the existing `SELF_PID` own-process filter.
  - `PopupRegistry` in popups.rs. Owner resolution per the spike: one `parent()` hop for `showing` (Qt), the event source for `children-changed` (GTK). Additions run a reachability check (skip when the owner's `GetChildren` already lists the popup) so real windows / in-tree popovers never enter the registry; capped at 32 entries with oldest-first eviction as a missed-hide-event backstop.
- [x] 3.3 Unit-test the registry + merge logic as a pure function (no live bus): given a set of recorded popups and a parent, enumeration merges them under the right owner, dedupes against `GetChildren` output, and prunes dismissed/defunct entries. Derive the cases from the spec scenarios.
  - 7 tests in `popups.rs` (`merge_appends…`, `merge_dedupes…`, `merge_prunes…`, `merge_with_empty_registry…` = the "tree unchanged" scenario, re-insert/replace, remove/clear, cap eviction).

## 4. Surface popups during enumeration

- [x] 4.1 Augment `get_nodes` ([crates/provider-atspi/src/lib.rs:124](../../../crates/provider-atspi/src/lib.rs)) to append registered popups whose owner is the node being enumerated, building each as a normal `AtspiNode` with parent wiring/keepalive (the pattern the picker already uses) so its ancestor chain and the items under it (`getChildren` down, verified) are walkable. Handle child-cache coherence for the owner node.
  - The actual per-node enumeration point is `AtspiNode::children()` (node.rs), not `get_nodes` (which only lists applications under the desktop root) — the merge lives there, plus in the hit-test's `search_subtree`. `descend_to_point` additionally searches grafted popups FIRST (`popup_at_point`): the WM reports the managed frame *beneath* an override-redirect popup, so the frame-scoped search would otherwise resolve the widget under the menu. Child lists are re-fetched per enumeration (no cross-call cache on the owner), so no extra invalidation was needed.
- [x] 4.2 Gate the whole feature behind a provider flag (default on) so it can be disabled to fall back to pure top-down traversal (rollback).
  - `providers.atspi.surface_popups` (default `true`); `false` skips the watcher and hands nodes no registry handle — the exact pre-event code path (unit-tested).

## 5. Native rebuild

- [x] 5.1 `just build-native` so the Python bindings pick up the provider change (no new keyword — findability flows through the existing `Query` / hit-test surface).

## 6. Verification

- [x] 6.1 The driving acceptance test (1.1/1.2) goes green on the X11 lane: open context-menu items are found by `Query` and by `Get Element At Point`; closed → gone. Real-provider-only (the mock has no transient popups).
- [x] 6.2 No regression: the existing `tests/acceptance/qt` (hit_test, bounds, modal) and `tests/acceptance/egui/hit_test` suites still pass, and `get_nodes` output is unchanged when no popup is open.
  - Full `real` X11 lane: 47/47 pass (all Qt + egui suites, incl. the File-menu hit-test popup path). Tree-unchanged-without-popups is covered by the merge unit test plus the untouched suites.
- [x] 6.3 Run `just check` and `just test` (workspace clippy + nextest, incl. the new registry/merge unit tests), then the X11 acceptance lane.
  - `just check` green (fmt, clippy, ruff, mypy); `just test` 2035/2035; X11 lane 47/47. The Wayland compositor lane was run additionally (CI runs both): all Qt suites incl. the new Context Menu pass there too; the one failing test (`Acceptance.Egui.Inspector Picker`) fails identically on **baseline main** in the same local nested-winit session (CI's headless compositor lane is green on main), i.e. a pre-existing local-environment issue unrelated to this change — plausibly the "PlatynUI-compositor modifier path" follow-up already noted in dev-docs/inspector.md.

## Addendum: submenu cascades (follow-up in the same change)

- [x] A.1 The Qt test app's context menu gained a cascade: `ctx-more` submenu (`ctx-sub-alpha`/`-beta`) with a nested `ctx-deep` submenu (`ctx-deep-item`) — each open level is another override-redirect QMenu window.
- [x] A.2 New acceptance tests in `tests/acceptance/qt/context_menu.robot`: open-submenu items findable by Query and resolved by hit-test, nested (2nd-level) item resolved by hit-test, and full-cascade dismissal leaves no stale items. The menu-open keyword retries the right-click (right after launch the WM may still be placing the window).
- [x] A.3 Root-cause fix the tests exposed: a grafted popup is a direct child of the `Application` and was treated as a *window surface* — its bounds resolved via `wm.resolve_window`, which PID-matches the app's **managed** main window (popups are override-redirect and unmanaged), so the popup's and every item's bounds were computed from the wrong window's origin. The original hit-test test had passed **spuriously** (move-to and hit-test shared the same wrong bounds). `AtspiNode::is_window_surface()` now excludes popup-class roles (`PopupMenu`/`Menu`/`ToolTip`); their geometry resolves via AT-SPI screen extents, and they no longer expose window patterns (so auto-activation cannot re-stack a window mid-menu-interaction). Verified: Qt spike shows the whole cascade is `getChildren`-walkable from the grafted root popup, and all 5 context-menu tests pass with physically correct coordinates.
- [x] A.4 Wayland-compositor scoping: the three submenu tests are skipped under the compositor (`Skip If XDG_SESSION_TYPE == wayland`). A Wayland client cannot know global coordinates, so Qt reports popup positions in its own client-local space; a pointer CLICK into the open menu lands offset from the visible item (≈ decoration height) and activates/dismisses the wrong entry. The two original tests (open→findable+hit-test, dismissed→gone) still pass on the compositor. Follow-up: resolve popup-surface positions from the PlatynUI compositor (it knows every xdg_popup's real position) so pointer interaction *into* popups becomes exact on Wayland.

- Emitting `ProviderEvent::TreeInvalidated` to the runtime pipeline (so the Inspector auto-refreshes on popup open/close) is a natural follow-up but not required here; this change is about tree *findability*.
- Non-AT-SPI providers are unaffected: UIA exposes popups natively top-down; the mock has none.
