## Context

`add-jab-hit-test` (archived) resolves a picked context to a reveal-ready node by walking the hit's `getAccessibleParentFromContext` chain up to the claimed window and re-descending top-down, matching each level to an enumeration index via `isSameObject` (`descend_to_hit` in [crates/provider-java-jab/src/node.rs](../../../crates/provider-java-jab/src/node.rs)). `indexInParent` was deliberately not trusted (combo popups report `-1`, spinner editors report shifted indices — spike findings from `add-jab-provider`).

`add-jab-interface-attributes` then uncovered how the JDK bridge represents JTable content: cell children all alias the one shared cell-renderer component, and `JTableHeader` entries are freshly allocated wrapper objects per `getAccessibleChild(i)` call. Both break `isSameObject` matching **structurally**: for cells every enumerated child is the same renderer (and never the hit's `AccessibleJTableCell` wrapper); for header entries no two lookups ever compare equal. `descend_to_hit` therefore always fails inside tables/headers, and the documented fallback (parentless window-scoped node) leaves the Inspector's ancestor-walking reveal with nothing to do — verified empirically with a hovering pointer: cell and header picks return nodes with empty ancestor chains and `…/hit/0x…` ids.

## Goals / Non-Goals

**Goals:**

- A pick over a JTable cell or a header entry resolves to the corresponding **tree** node (tree RuntimeId, walkable chain to `app:Application`), so the Inspector reveal works.
- When even that fails, the pick degrades to the deepest matched ancestor (at minimum the window) — never to a node the reveal cannot place.

**Non-Goals:**

- No change to hit results for regular components (the `isSameObject` path stays primary).
- No synthesis of cell bounds/visibility (JDK-8 JAB has no cell-rectangle API; documented in `dev-docs/platform-windows.md`).
- No general rehabilitation of `indexInParent` for tree traversal or RuntimeIds — it remains untrusted there.

## Decisions

1. **Per-level index fallback, only after `isSameObject` matching failed.** When no enumeration child of the current level matches the chain target, read the target's own `getAccessibleContextInfo().indexInParent`; if `0 <= index < children_count` of the current level, accept it as the enumeration index and continue the descent with the child fetched at that index. Virtual wrappers set `indexInParent` correctly in their constructors (`AccessibleJTableCell`, `AccessibleJTableHeaderEntry`), which is exactly the population `isSameObject` cannot handle. The known unreliable cases are filtered by the range guard (`-1` from combo popups) or were already failing before (a shifted index selects a sibling — a wrong-but-adjacent selection beats no reveal at all, and the primary `isSameObject` path is unaffected). *Alternative considered:* verifying the index-fetched child against the target via `isSameObject` — pointless, that comparison is the thing that structurally fails here.

2. **Deepest matched ancestor instead of the parentless fallback.** When the index fallback cannot recover a level either, `hit_test_node` returns the node built so far (the last successfully matched level — for a table pick that is at least the `Table` node, in the worst case the window). The parentless `hit_fallback_node` remains only for hits whose parent chain never reaches the window root (`reached_window == false`), where no anchored ancestor exists at all.

3. **Verification needs a real hover.** The JDK's native hit-test answers only after the target JVM has observed a mouse event (`EventQueueMonitor.currentMousePosition`), so the live test must physically move the pointer over the target (e.g. `SetCursorPos`) before calling `element_at_point`; the acceptance scenario uses the runtime's pointer-move plus `Get Element At Point`. Without the hover, the geometric fallback path masks the wrapper problem entirely.

## Risks / Trade-offs

- [A lying `indexInParent` resolves a neighbouring sibling] → bounded blast radius (picker highlights/reveals a sibling); occurs only where `isSameObject` already failed, i.e. where the status quo is a dead reveal.
- [Extra bridge calls on the failure path (one `getAccessibleContextInfo` per unmatched level)] → constant per level, pump-thread + deadline as always.

## Migration Plan

Additive behavior change inside the provider's hit-test failure path; no config, no API change. Rollback: revert the crate change. Requires `just build-native` for the Inspector/Python consumers.

## Open Questions

- Whether tree widgets (`JTree` — `AccessibleJTreeNode` wrappers, same freshly-allocated pattern as header entries) should get a fixture stage and scenario in this change or a follow-up — leaning follow-up; the mechanism is identical.
