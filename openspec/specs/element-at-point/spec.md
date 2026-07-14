# element-at-point Specification

## Purpose

The UI tree is otherwise strictly top-down (`get_nodes(parent) → children`); nothing maps a screen coordinate back to a node. This capability is the reverse lookup: given a point in desktop coordinates, resolve the topmost UI element the user perceives under it — respecting window and layer z-order, excluding hidden nodes and the host process's own UI — with an explicit "unsupported" signal where a platform cannot answer. It is resolved per provider (Windows/UIA natively, Linux/AT-SPI via window-manager stacking plus a geometric bounds search, mock geometrically for tests) and drives the Inspector's live mouse picker and the `Get Element At Point` keyword.

## Requirements

### Requirement: Providers can resolve the element at a screen point

A UI tree provider SHALL offer an optional hit-test operation that, given a point in desktop (screen) coordinates, returns the **topmost** UI node at that point — the deepest node the user perceives as being under the cursor, **respecting window and layer z-order (occlusion)**, not merely the deepest node whose bounds geometrically contain the point. When two nodes both cover the point (overlapping siblings, or a popup over a client area), the visually frontmost one SHALL win. The operation SHALL return nothing if no node is at that point. The returned node SHALL be equivalent — same runtime identity and same parent chain — to the node that top-down traversal would reach, so that the result can be fed into ancestor-walking consumers (tree reveal) without special handling.

#### Scenario: A point over a control returns that control

- **WHEN** hit-test is called with a point that lies within a visible control's bounds and within no deeper descendant
- **THEN** the provider SHALL return that control's node

#### Scenario: The deepest node wins

- **WHEN** hit-test is called with a point that lies within a control that itself contains a smaller descendant control at that point
- **THEN** the provider SHALL return the deepest (innermost) node containing the point, not an ancestor

#### Scenario: A point over nothing returns no node

- **WHEN** hit-test is called with a point that lies outside every node the provider exposes
- **THEN** the provider SHALL return nothing (an empty result), not an error and not an arbitrary node

#### Scenario: The returned node has a usable parent chain

- **WHEN** hit-test returns a node
- **THEN** walking that node's parent chain SHALL reach the same ancestors as top-down traversal, and the node's runtime identity SHALL match the one produced by top-down traversal for the same element

### Requirement: Hit-test does not return hidden nodes

Hit-test SHALL NOT return a node that is not actually shown to the user, even when the node's bounds geometrically contain the point. A node is excluded when it reports `Control:IsVisible` — or, where the provider surfaces it, `Control:IsInView` — as explicitly false; a node that does not report the attribute at all SHALL be treated as visible (so a provider that does not model visibility is unaffected). This matters because the accessibility tree routinely retains laid-out but hidden nodes with stale bounds (a closed menu's items, a non-current stacked page, a collapsed panel's contents), and such a node SHALL NOT be picked in preference to the visible node the user actually sees at the point. When a hidden node contains the point, the resolver SHALL fall through to the visible node beneath or around it.

> `IsInView` is included deliberately even though its precise, per-context meaning is still being defined: gating on an *explicit* false keeps hit-test correct once `IsInView` diverges from `IsVisible` (e.g. to mean "within the scrolled viewport"), without depending on that definition now.

#### Scenario: A hidden node containing the point is skipped

- **WHEN** hit-test is called with a point inside a node whose bounds contain it but which reports `Control:IsVisible` (or `Control:IsInView`) as false
- **THEN** the provider SHALL NOT return that node, and SHALL instead return the visible node beneath/around the point (or nothing if none)

#### Scenario: A hidden topmost sibling falls through to the visible one beneath

- **WHEN** two siblings overlap at the point and the frontmost (topmost) one is hidden
- **THEN** the provider SHALL return the visible sibling beneath it, not the hidden frontmost one

### Requirement: Hit-test uses the same coordinate space as the reported cursor position

The point passed to hit-test SHALL be interpreted in the same desktop coordinate space that the platform's cursor-position reporting (`pointer_position()`) produces, so that a point read from the cursor resolves to the element actually under the cursor. This space is the multi-monitor desktop coordinate system (which may include negative coordinates for monitors left of / above the primary), and providers SHALL account for display scaling (HiDPI) consistently with how bounds and cursor position are reported, rather than assuming primary-monitor, unscaled pixels.

#### Scenario: A point read from the cursor resolves to the element under the cursor

- **WHEN** the cursor rests over a known control and hit-test is called with exactly the point that cursor-position reporting returns
- **THEN** hit-test SHALL return that control (not a neighbouring or offset element)
- **NOTE** Verifiable against a real provider; on HiDPI / multi-monitor setups this is where scaling/offset mismatches surface.

#### Scenario: A point on a secondary monitor resolves correctly

- **WHEN** hit-test is called with a point located on a non-primary monitor (including one at negative desktop coordinates)
- **THEN** hit-test SHALL resolve the element on that monitor, not misinterpret the coordinates as primary-monitor-relative
- **NOTE** Verifiable only on a real multi-monitor setup, not the mock.

### Requirement: Hit-test reports "unsupported" rather than guessing

A provider that cannot resolve an element at a point on the current platform SHALL report the operation as unsupported through a distinct, detectable signal (a default "not supported" result), so callers can disable point-based features instead of receiving a misleading node or a fabricated coordinate result.

#### Scenario: A provider without hit-test support is detectable

- **WHEN** hit-test is invoked on a provider or platform that does not implement it (e.g. the macOS AX stub)
- **THEN** the caller SHALL receive an explicit "unsupported" signal distinguishable from "supported but nothing at this point"

### Requirement: Hit-test excludes the host process's own UI

Hit-test SHALL never resolve an element that belongs to the process hosting the runtime, consistent with top-down tree enumeration (which already excludes the own process). Where the window manager can see the stack (X11, the compositor), an own-process window at the point SHALL be skipped so the window **behind** it is resolved; otherwise resolving a point over own-process UI SHALL return nothing rather than that UI. This prevents a point-based consumer (the Inspector live picker) from selecting its own window or overlay.

#### Scenario: A point over the host process's own window is skipped

- **WHEN** hit-test is called with a point over a window belonging to the process hosting the runtime
- **THEN** it SHALL NOT return that process's own element — it resolves the window behind it (where the stack is known) or nothing
- **NOTE** Verifiable only against a real provider with an own-process window on screen, not the mock.

### Requirement: Windows UIA hit-test uses the native element-from-point facility

On Windows, hit-test SHALL be resolved through the UI Automation `ElementFromPoint` facility, so that window z-order, layering, and cross-process boundaries are handled by the platform rather than reconstructed.

#### Scenario: Overlapping windows resolve to the topmost (real provider)

- **WHEN** two application windows overlap and hit-test is called with a point in the overlapping region
- **THEN** the node returned SHALL belong to the window that is visually on top at that point
- **NOTE** Verifiable only against the real Windows UIA provider, not the mock.

### Requirement: AT-SPI hit-test resolves window z-order and in-window z-order from distinct sources

On Linux/AT-SPI, hit-test SHALL resolve the element in two stages using the correct source for each level of z-order:

1. **Window level:** the frontmost top-level application window at the point SHALL be determined from the platform window manager's stacking order (X11 EWMH stacking / the PlatynUI compositor's surface-under-point), NOT from AT-SPI, because AT-SPI has no cross-application window-stacking view. The resolved native window SHALL be correlated to its AT-SPI top-level Accessible.
2. **In-window level:** the element within the application SHALL be resolved by a geometric bounds search over the accessible tree — reading each node's toolkit-aware screen bounds and selecting the smallest-area node whose bounds contain the point (the most specific box under the cursor). The toolkit's own point hit-test (`Component.GetAccessibleAtPoint`) SHALL NOT be relied upon, being unreliable across toolkits (some report inaccurate screen extents; some return the widget *beneath* an overlay). When a managed frame maps to the selected window the search SHALL be scoped to that frame; when the selected window is an override-redirect popup with no managed frame, the search SHALL cover the whole application subtree and SHALL NOT prune by parent bounds, so a popup drawn larger than — and outside — the frame that owns it in the accessible tree is still reached. Selecting the smallest containing box makes an overlapping popup/menu win over the client area beneath it without consulting an explicit layer order.

#### Scenario: The frontmost window at the point is chosen by WM stacking (real provider)

- **WHEN** two application windows overlap and hit-test is called with a point in the overlapping region
- **THEN** the element returned SHALL belong to the window the window manager reports as frontmost at that point, not to a window lower in the stack that also covers the point
- **NOTE** Verifiable only against real windows on X11 / the PlatynUI compositor, not the mock.

#### Scenario: Element inside the frontmost window is resolved in window coordinates (real provider)

- **WHEN** hit-test has selected the frontmost window (which exposes the AT-SPI Component interface) at the point
- **THEN** the provider SHALL translate the point into that window's coordinate system and return the deepest child at that point
- **NOTE** Verifiable only against a real AT-SPI application, not the mock.

#### Scenario: An overlapping popup wins over the client area beneath it (real provider)

- **WHEN** a popup, menu, or tooltip is displayed over a window's client area and hit-test is called with a point covered by both
- **THEN** the provider SHALL return the popup/menu element (the smaller, more specific box), not the client-area element beneath it
- **NOTE** Verified against real AT-SPI applications with a live open menu-bar menu (egui/AccessKit and Qt), not the mock. Transient right-click context menus are out of scope for this top-down resolver: on Qt the popup *is* exposed on AT-SPI but only event-driven (a `PopupMenu` child of the `Application`, reachable via the item's `parent()` chain and via `getChildren` down from the popup, but absent from the `Application`'s own top-down `getChildren`), so a bounds walk cannot reach it. Picking them needs event-driven tree updates (see the `atspi-event-driven-tree` change).

#### Scenario: A window without the Component interface does not abort the search

- **WHEN** the candidate window (or an intermediate node) does not expose the Component interface
- **THEN** hit-test SHALL skip it gracefully and continue with the next candidate rather than erroring out
- **NOTE** Verifiable only against a real AT-SPI application, not the mock.

### Requirement: The mock provider resolves hit-test geometrically

The mock provider SHALL implement hit-test by walking its in-memory tree and returning the deepest node whose bounds contain the point, honoring declared stacking order among siblings and skipping hidden nodes (`@visible="false"` ⇒ `Control:IsVisible` false), so that hit-test behavior (deepest-node selection, misses, z-order among overlapping mock nodes, and the hidden-node exclusion) can be exercised deterministically in unit and CI tests without a real desktop.

#### Scenario: Mock deepest-node selection is deterministic

- **WHEN** the mock tree has a child node fully inside a parent node and hit-test is called with a point inside the child
- **THEN** the mock SHALL return the child node

#### Scenario: Mock miss returns nothing

- **WHEN** hit-test is called with a point outside all mock node bounds
- **THEN** the mock SHALL return an empty result

#### Scenario: Mock resolves overlapping siblings by stacking order

- **WHEN** two sibling mock nodes overlap and hit-test is called with a point inside the overlapping region
- **THEN** the mock SHALL return the sibling declared on top (higher stacking order), not the one beneath it, so that z-order selection is covered by a deterministic test without a real desktop

#### Scenario: Mock skips a hidden node at the point

- **WHEN** a mock node whose bounds contain the point is marked hidden (`@visible="false"`)
- **THEN** the mock SHALL NOT return it, resolving instead to the visible node beneath/around the point — so the hidden-node exclusion is covered without a real desktop
