## 1. Driving test (test-first, red)

- [ ] 1.1 Add an X11 acceptance test (real AT-SPI, Qt app) that opens the test app's right-click context menu and asserts its items are findable: `BM.Query //item:MenuItem[@Name="ctx-copy"]` resolves it, and `BM.Get Element At Point` over the open item resolves it. This test FAILS today (top-down traversal misses the popup) and is the red bar this change turns green. Keep the menu open deterministically (press-and-hold right button, or a stable app trigger) and tear it down in teardown.
- [ ] 1.2 Add a companion assertion that a CLOSED context menu's items are NOT found (guards the dismiss path).

## 2. Confirm toolkit exposure (spike, real provider)

- [ ] 2.1 Using `crates/provider-atspi/examples/atspi_focus_watch.rs`, record which events/roles carry the popup on each toolkit we care about: confirm Qt (`state-changed:showing` on `PopupMenu`, child-of-`Application`, `getChildren` down works), and **re-check GTK** (previously assumed broken — that predates the watcher fix) plus combo dropdowns / tooltips. Lock the popup-class role set and the event set the implementation will handle.

## 3. Event infrastructure in the AT-SPI provider

- [ ] 3.1 Add a background event worker owning a **dedicated** `AccessibilityConnection` (separate from the synchronous query connection — a blocking call on the stream's own connection deadlocks it; see `atspi_focus_watch`'s two-connection fix). Register `object:children-changed` and `object:state-changed`. Start it lazily with the provider connection; stop it on `shutdown()`.
- [ ] 3.2 Implement a mutex-guarded popup registry: on a popup-class `showing=true` / `children-added`, resolve the owner via the popup's `parent()` chain and record `popup → owner`; on `showing=false` / `children-removed` / `defunct`, drop it. Apply the existing `SELF_PID` own-process filter.
- [ ] 3.3 Unit-test the registry + merge logic as a pure function (no live bus): given a set of recorded popups and a parent, enumeration merges them under the right owner, dedupes against `GetChildren` output, and prunes dismissed/defunct entries. Derive the cases from the spec scenarios.

## 4. Surface popups during enumeration

- [ ] 4.1 Augment `get_nodes` ([crates/provider-atspi/src/lib.rs:124](../../../crates/provider-atspi/src/lib.rs)) to append registered popups whose owner is the node being enumerated, building each as a normal `AtspiNode` with parent wiring/keepalive (the pattern the picker already uses) so its ancestor chain and the items under it (`getChildren` down, verified) are walkable. Handle child-cache coherence for the owner node.
- [ ] 4.2 Gate the whole feature behind a provider flag (default on) so it can be disabled to fall back to pure top-down traversal (rollback).

## 5. Native rebuild

- [ ] 5.1 `just build-native` so the Python bindings pick up the provider change (no new keyword — findability flows through the existing `Query` / hit-test surface).

## 6. Verification

- [ ] 6.1 The driving acceptance test (1.1/1.2) goes green on the X11 lane: open context-menu items are found by `Query` and by `Get Element At Point`; closed → gone. Real-provider-only (the mock has no transient popups).
- [ ] 6.2 No regression: the existing `tests/acceptance/qt` (hit_test, bounds, modal) and `tests/acceptance/egui/hit_test` suites still pass, and `get_nodes` output is unchanged when no popup is open.
- [ ] 6.3 Run `just check` and `just test` (workspace clippy + nextest, incl. the new registry/merge unit tests), then the X11 acceptance lane.

## Notes / out of scope

- Emitting `ProviderEvent::TreeInvalidated` to the runtime pipeline (so the Inspector auto-refreshes on popup open/close) is a natural follow-up but not required here; this change is about tree *findability*.
- Non-AT-SPI providers are unaffected: UIA exposes popups natively top-down; the mock has none.
