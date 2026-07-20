## Why

The Inspector's live picker cannot reach a `JTable`, its cells, or its column headers: hovering them resolves to a **parentless fallback node**, so the tree reveal never moves. Verified against the Swing fixture (pointer physically hovering, as the Inspector does): a pick over a data cell answers `Label 'r2c1'` with RuntimeId `…/hit/0x…` and an empty ancestor chain; the header answers `Label 'col-1'` the same way — while a pick over a plain button resolves with a full reveal chain.

Root cause: with the mouse over the window, the JDK's native hit-test (`getAccessibleContextAt` → `EventQueueMonitor`/`AccessibleJTable.getAccessibleAt`) returns **virtual accessible wrappers** (`AccessibleJTableCell`, `AccessibleJTableHeaderEntry`). The reveal mapping in `descend_to_hit` ([crates/provider-jab/src/node.rs](../../../crates/provider-jab/src/node.rs)) matches the hit's ancestor chain against enumeration children via `isSameObject` — which **cannot** match virtual wrappers: JTable cell children all alias the shared cell-renderer component (see `jab-interface-attributes` design 1a), and `JTableHeader` entries are freshly allocated on every `getAccessibleChild(i)` call, so no two lookups are ever the same object. The current documented fallback (a parentless window-scoped node) makes the Inspector reveal a no-op.

## What Changes

- `descend_to_hit` gains a **per-level index fallback**: when `isSameObject` matching fails at a level, the target wrapper's own `indexInParent` is used as the enumeration index, guarded to the level's child count. Virtual wrappers (`AccessibleJTableCell`, `AccessibleJTableHeaderEntry`) carry a constructor-set, correct `indexInParent`, so picked cells and header entries resolve to their real tree nodes (tree RuntimeId, walkable chain up to `app:Application`).
- When no index can be recovered either, the hit resolves to the **deepest matched ancestor** (at minimum the claimed window) instead of the parentless fallback node — the picker then lands on the `Table`/container rather than nowhere. The parentless best-effort node remains only for hits whose parent chain never reaches the window.
- Live Rust test with a real pointer hover (the native hit-test only answers after the JVM has seen a mouse event) plus a Swing acceptance scenario; `dev-docs/platform-windows.md` hit-testing note updated.

Not breaking: hit results for regular components are unchanged; only the failure path of the reveal mapping improves.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `jab-hit-test`: the "Reveal-ready hit result" requirement gains the index-recovery fallback for virtual wrapper children and the deepest-matched-ancestor fallback; the parentless node is demoted to a last resort.

## Impact

- **Modified crate**: `crates/provider-jab` — `node.rs` (`descend_to_hit`, `hit_test_node`).
- **Tests**: `crates/provider-jab/tests/live_fixture.rs` (hover-based pick over cell/header), `tests/acceptance/swing/picker.robot` (table pick scenario).
- **Docs**: `dev-docs/platform-windows.md` (hit-testing paragraph).
- **Depends on**: `add-jab-hit-test` (archived — hit-test + reveal chain), `add-jab-interface-attributes` (active — documents the renderer aliasing this works around). **Platform scope**: Windows only. No config changes, no BREAKING changes.
