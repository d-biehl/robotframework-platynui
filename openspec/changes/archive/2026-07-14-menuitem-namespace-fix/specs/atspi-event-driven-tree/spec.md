# atspi-event-driven-tree Specification (delta)

## MODIFIED Requirements

### Requirement: The AT-SPI provider surfaces transient popups in its tree

The AT-SPI provider SHALL make transient popup windows — right-click context menus, combo-box dropdowns, and tooltips — appear in the UI tree as descendants of the application (or window) they belong to, even when the toolkit does not list them in that ancestor's own `Accessible.GetChildren`. It SHALL do so by observing structural accessibility events (`object:children-changed`, `object:state-changed:showing`) and, for a popup-class node, recording it under the owner resolved from its `parent()` chain, then merging it into that owner's children during tree enumeration. When the popup is dismissed (hidden, removed, or defunct) the provider SHALL stop surfacing it.

The result is that a surfaced popup and its items are reachable by every top-down consumer — XPath queries, the Inspector tree, and the point hit-test — with the same node identity and parent chain as any other node.

#### Scenario: An open context menu's items are found by XPath

- **WHEN** a real AT-SPI application (e.g. Qt) has an open right-click context menu whose items exist only under a transient popup
- **THEN** an XPath query for a menu item (e.g. `//MenuItem[@Name="ctx-copy"]`) SHALL resolve it
- **NOTE** Verifiable only against a real toolkit that exposes popups event-driven (observed on Qt), not the mock.

#### Scenario: The point hit-test resolves an open context-menu item

- **WHEN** a context menu is open and the cursor is over one of its items
- **THEN** hit-test at that point SHALL return the menu item (not the widget beneath the popup)
- **NOTE** Verifiable only against a real toolkit, not the mock.

#### Scenario: Cascaded submenu items are reachable through the grafted popup

- **WHEN** an open context menu has cascading submenus (each open level its own transient popup window)
- **THEN** submenu items on every cascade level SHALL resolve by XPath through the grafted root popup, and the hit-test over an open submenu item SHALL return that item with physically correct screen bounds
- **NOTE** Exact on X11 via toolkit screen extents; under the PlatynUI compositor the popup rectangles come from the compositor's popup-geometry query (see `compositor-popup-geometry`), so the pointer-driven submenu scenarios run there as well. Generic Wayland (no PlatynUI control socket) remains best-effort.

#### Scenario: A dismissed popup is no longer in the tree

- **WHEN** a previously open context menu is closed
- **THEN** subsequent XPath queries and enumeration SHALL NOT return its items, and no stale/defunct popup node SHALL remain grafted

#### Scenario: No transient popup means the tree is unchanged

- **WHEN** no transient popup is open
- **THEN** tree enumeration SHALL return exactly what top-down `GetChildren` traversal returns (the feature is purely additive)

#### Scenario: The provider's own process popups are not surfaced

- **WHEN** the popup belongs to the PlatynUI process itself (e.g. the Inspector's own menus)
- **THEN** it SHALL NOT be surfaced, consistent with the existing own-process filtering
