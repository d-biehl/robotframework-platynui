## ADDED Requirements

### Requirement: The compositor exposes global geometry for transient popup surfaces

The PlatynUI Wayland compositor SHALL expose the currently mapped xdg_popup surfaces over its control socket, each with its **global** (compositor-space, logical) rectangle and the parent toplevel's window id and process id. The geometry SHALL reflect the popup's actual placed position (root toplevel location plus the popup chain's relative offsets), and the listing SHALL cover every cascade level of nested popups. Dismissed or unmapped popups SHALL NOT be reported.

#### Scenario: An open context menu is listed with its real position

- **WHEN** an application under the compositor has an open context menu (an xdg_popup)
- **THEN** the popup listing SHALL contain one entry whose rectangle matches where the menu is actually drawn on screen, carrying the parent window's id and pid

#### Scenario: Nested cascade levels are all listed

- **WHEN** a context menu with an open submenu (and an open sub-submenu) is showing
- **THEN** the popup listing SHALL contain one entry per open cascade level, each with its own global rectangle

#### Scenario: No open popups yields an empty listing

- **WHEN** no xdg_popup is mapped
- **THEN** the popup listing SHALL be empty and the command SHALL still succeed

### Requirement: Popup geometry reaches the AT-SPI provider through the platform window manager

The platform `WindowManager` abstraction SHALL offer a popup-geometry query (global rectangles of the popups belonging to a given process) with a conservative default, implemented by the PlatynUI-compositor backend via the control socket. The AT-SPI provider SHALL use this query when resolving the bounds of a grafted popup-class node whose toolkit-reported extents are not trustworthy (Wayland), matching provider popups to compositor rectangles by process and size, so that the popup — and, through parent-relative accumulation, every item inside it — gets physically correct global bounds. Backends that do not implement the query (X11, Windows, mock) SHALL be unaffected, and the provider SHALL keep its existing extents path there.

#### Scenario: Clicking into an open menu lands on the intended item under the compositor

- **WHEN** a test running under the PlatynUI compositor clicks a context-menu entry that opens a submenu, then a nested submenu, and hit-tests an item
- **THEN** each pointer action SHALL land on the visually intended element (the submenu opens, the nested item resolves), i.e. the three previously Wayland-skipped submenu scenarios of the Qt context-menu acceptance suite SHALL pass without skips

#### Scenario: X11 behavior is unchanged

- **WHEN** the same suites run on the X11 lane
- **THEN** popup bounds SHALL resolve exactly as before (toolkit screen extents) and all previously passing tests SHALL keep passing
