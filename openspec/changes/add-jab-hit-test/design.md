## Context

`add-jab-provider` landed the JAB provider for top-down access to Swing/AWT trees, but left `element_at_point` at the trait default (`UnsupportedOperation`). The Inspector's live picker resolves the element under the cursor via `Runtime::element_at_point`, which asks each provider in turn and returns the **first** `Ok(Some(node))` ([crates/runtime/src/runtime/input.rs](../../../crates/runtime/src/runtime/input.rs)) — there is no cross-provider z-order merge. Reference points already in the tree:

- The UIA provider's `element_at_point` ([crates/provider-windows-uia/src/provider.rs](../../../crates/provider-windows-uia/src/provider.rs)) uses `IUIAutomation::ElementFromPoint` (native z-order), scopes the result to its owning process, and wires an ancestor chain with matching RuntimeIds via `attach_ancestor_chain` so the Inspector's reveal-and-select can walk `parent()` up to the tree.
- The AT-SPI provider hit-tests by asking the `WindowManager` for the window at the point (authoritative window z-order) and then descending geometrically within that application.
- JAB offers a **native hit-test**, `getAccessibleContextAt(vmID, acParent, x, y, *ac)` (`AccessBridgeCalls.h`), returning the deepest context under a point within a parent — no geometric descent needed.
- The process-wide window-claims registry (`platynui_core::platform::window_claims`) already records which HWNDs the JAB provider owns; the UIA provider consults it during root streaming.

## Goals / Non-Goals

**Goals:**

- The Inspector picker resolves the real Swing control under the cursor (not the empty UIA shell), with a parent chain that reveals correctly in the tree.
- Correct arbitration **independent of provider registration order**: a claimed Java window resolves to its JAB node; every non-Java point still resolves through UIA exactly as today.
- Hit-testing is bounded by the JAB per-call deadline (a frozen JVM cannot hang the picker).

**Non-Goals:**

- Changing the runtime's first-hit `element_at_point` aggregation model. The arbitration is solved at the provider layer via claims, not by teaching the runtime cross-provider z-order.
- Events/push (`event_capabilities` stays `None`).
- `getAccessibleContextWithFocus`-based focus tracking, and hit-testing of nested/overlapping Java windows across processes beyond what `WindowFromPoint` + `isJavaWindow` resolve.

## Decisions

1. **JAB `element_at_point` gates on the window, then uses the native hit-test.** Resolve the top-level window under the point with `WindowFromPoint(point)` followed by walking to its root owner (`GetAncestor(GA_ROOT)`); if `isJavaWindow(hwnd)` is false, return `Err(UnsupportedOperation)` so other providers handle the point. For a Java window: `getAccessibleContextFromHWND` → `getAccessibleContextAt(vmID, root, x, y)` for the deepest context, wrapped in a `JabObject` (RAII release like every other handle). All calls run on the pump thread under `call_timeout_ms`; a timeout/degraded vmID surfaces as a provider error for that point, not a hang.

2. **Coordinates.** `WindowFromPoint` and `getAccessibleContextAt` both take desktop pixels; PlatynUI's hit-test point is already in desktop coordinates (Per-Monitor-V2). The point is passed through unchanged. The resulting node's `Bounds` follow the provider's existing DPI-calibration path, so a picked node's reported bounds stay consistent with what a top-down walk yields.

3. **Reveal-ready node.** The picked context is built as a `JabNode` scoped to `IdScope::App { pid }` (so its `RuntimeId` matches the app-grouped view the Inspector reveals into) with a parent/ancestor chain up to `app:Application`. Rather than reconstruct the enumeration-index path from a bare context handle (JAB's `indexInParent` is unreliable — see `add-jab-provider`), the node holds strong parent references built by walking `getAccessibleParentFromContext` up to the window root and mapping each level to its enumeration index during a single top-down re-walk of the owning window (bounded), so the RuntimeId path is identical to top-down traversal. If that re-walk cannot match the hit context (rare races), fall back to a parentless node scoped to the window with a best-effort id; the picker still highlights, only tree-reveal degrades.

3a. *Alternative considered:* derive the id path purely from `getAccessibleParentFromContext` + `indexInParent`. Rejected because `indexInParent` is unreliable (combo popups report `-1`, spinner editors shift), which would produce RuntimeIds that do not match top-down traversal and break reveal.

4. **UIA abstains on claimed windows (order-independent arbitration).** `provider-windows-uia::element_at_point`, when `providers.windows-uia.honor_window_claims` is true, resolves the top-level window under the point and, if it is claimed by another provider in `window_claims`, returns `Err(UnsupportedOperation)` before calling `ElementFromPoint`. So regardless of whether UIA or JAB is consulted first, UIA yields nothing for Java windows and the runtime falls through to JAB's hit. With the kill switch off, UIA resolves the shell as before (both representations remain reachable, distinguishable via `@Technology`). *Alternative considered:* rely on JAB being registered before UIA — rejected as fragile (inventory order is link-dependent).

5. **Own-process guard.** Like the other providers, JAB's hit-test returns `Ok(None)`/`UnsupportedOperation` for the host process's own windows so the picker never selects the Inspector itself.

## Risks / Trade-offs

- [The hit context cannot be matched to an enumeration path → wrong or missing RuntimeId] → single bounded top-down re-walk of the owning window to map the context via `isSameObject`; documented fallback to a window-scoped parentless node when matching fails.
- [`WindowFromPoint` returns a child HWND, not the AWT top-level] → walk to `GA_ROOT`; Swing content lives in one top-level HWND so the root owner is the Java frame.
- [Frozen JVM during a pick] → per-call deadline + degraded-vmID skip (inherited from the provider); the picker gets a prompt error, other providers stay responsive.
- [UIA abstain hides a genuinely-wanted UIA shell] → only when claims are honored (default); `providers.windows-uia.honor_window_claims=false` restores the old behavior, matching the root-streaming kill switch.

## Migration Plan

Additive: one new method on the JAB provider, one guarded early-return in the UIA provider's `element_at_point`, both behind the existing claims config. Requires `just build-native` for the Inspector/Python to see the picker. Rollback: `providers.windows-uia.honor_window_claims=false` (UIA shell picking returns) or `providers.jab.enabled=false` (JAB out entirely).

## Open Questions

- Whether to also implement `element_at_point` scoping to a specific process (the picker passes only a point today) — deferred; the window-gate already selects the right application.
- Whether the bounded re-walk depth needs tuning for very deep Swing trees — decide from the acceptance run.
