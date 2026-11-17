# Spec Delta

## MODIFIED Requirements

### Requirement: AT-SPI hit-test resolves window z-order and in-window z-order from distinct sources

On Linux/AT-SPI, hit-test SHALL resolve the element in two stages using the correct source for each level of z-order:

1. **Window level:** the frontmost top-level application window at the point SHALL be determined from the platform window manager's stacking order (X11 EWMH stacking, or the X server's stacking order of the root window's children when no EWMH window manager runs / the PlatynUI compositor's surface-under-point), NOT from AT-SPI, because AT-SPI has no cross-application window-stacking view. The resolved native window SHALL be correlated to its AT-SPI top-level Accessible.
2. **In-window level:** the element within the application SHALL be resolved by a geometric bounds search over the accessible tree — reading each node's toolkit-aware screen bounds and selecting the smallest-area node whose bounds contain the point (the most specific box under the cursor). The toolkit's own point hit-test (`Component.GetAccessibleAtPoint`) SHALL NOT be relied upon, being unreliable across toolkits (some report inaccurate screen extents; some return the widget *beneath* an overlay). When a managed frame maps to the selected window the search SHALL be scoped to that frame; when the selected window is an override-redirect popup with no managed frame, the search SHALL cover the whole application subtree and SHALL NOT prune by parent bounds, so a popup drawn larger than — and outside — the frame that owns it in the accessible tree is still reached. Selecting the smallest containing box decides among the nodes of one layer. A popup, which is a transient popup grafted under its application or a node reporting the AT-SPI popup layer such as a GTK 4 popover, SHALL win over that box: a hit inside a popup SHALL win over every hit outside it, and a hit inside a popup SHALL win over one in the popup it was opened from. A widget beneath a popup can be smaller than the menu item above it.

#### Scenario: The frontmost window at the point is chosen by WM stacking (real provider)

- **WHEN** two application windows overlap and hit-test is called with a point in the overlapping region
- **THEN** the element returned SHALL belong to the window the window manager reports as frontmost at that point, not to a window lower in the stack that also covers the point
- **NOTE** Verifiable only against real windows on X11 / the PlatynUI compositor, not the mock.

#### Scenario: Without a window manager the frontmost window is chosen by the X server's stacking order (real provider)

- **GIVEN** two overlapping application windows on an X11 display without a window manager
- **WHEN** hit-test is called with a point in the overlapping region
- **THEN** the element returned SHALL belong to the window the X server stacks on top
- **NOTE** Verifiable only against real windows on the X11 lane without a window manager, not the mock.

#### Scenario: Element inside the frontmost window is resolved in window coordinates (real provider)

- **WHEN** hit-test has selected the frontmost window (which exposes the AT-SPI Component interface) at the point
- **THEN** the provider SHALL translate the point into that window's coordinate system and return the deepest child at that point
- **NOTE** Verifiable only against a real AT-SPI application, not the mock.

#### Scenario: An overlapping popup wins over the client area beneath it (real provider)

- **WHEN** a popup, menu, or tooltip is displayed over a window's client area and hit-test is called with a point covered by both
- **THEN** the provider SHALL return the popup/menu element, not the client-area element beneath it
- **NOTE** Verified against real AT-SPI applications with a live open menu-bar menu (egui/AccessKit and Qt), not the mock. Transient right-click context menus are out of scope for this top-down resolver: on Qt the popup *is* exposed on AT-SPI but only event-driven (a `PopupMenu` child of the `Application`, reachable via the item's `parent()` chain and via `getChildren` down from the popup, but absent from the `Application`'s own top-down `getChildren`), so a bounds walk cannot reach it. Picking them needs event-driven tree updates (see the `atspi-event-driven-tree` change).

#### Scenario: A point inside a GTK 4 submenu resolves to its item (real provider)

- **GIVEN** the GTK 4 fixture with its `menu-edit` menu and the nested submenu `menu-edit-more` open, the submenu drawn over the window's controls
- **WHEN** hit-test is called with the center of a submenu item's `Bounds`
- **THEN** the provider SHALL return that item or one of its descendants, not a control beneath the submenu
- **NOTE** Real provider only, on both Linux lanes, with GTK 4.18 or later.

#### Scenario: A window without the Component interface does not abort the search

- **WHEN** the candidate window (or an intermediate node) does not expose the Component interface
- **THEN** hit-test SHALL skip it gracefully and continue with the next candidate rather than erroring out
- **NOTE** Verifiable only against a real AT-SPI application, not the mock.
