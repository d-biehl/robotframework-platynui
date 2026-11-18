# Spec Delta

## MODIFIED Requirements

### Requirement: The compositor exposes global geometry for transient popup surfaces

The PlatynUI Wayland compositor SHALL expose the currently mapped xdg_popup surfaces over its control socket, each with its **global** (compositor-space, logical) placed rectangle, the global origin and size of its surface, and the parent toplevel's window id and process id. The placed rectangle SHALL reflect the popup's actual placed position (root toplevel location plus the popup chain's relative offsets). The surface SHALL be the popup's whole buffer, which includes a shadow the client draws around the placed rectangle. The listing SHALL cover every cascade level of nested popups. Dismissed or unmapped popups SHALL NOT be reported.

#### Scenario: An open context menu is listed with its real position

- **WHEN** an application under the compositor has an open context menu (an xdg_popup)
- **THEN** the popup listing SHALL contain one entry whose rectangle matches where the menu is actually drawn on screen, carrying the parent window's id and pid

#### Scenario: Nested cascade levels are all listed

- **WHEN** a context menu with an open submenu (and an open sub-submenu) is showing
- **THEN** the popup listing SHALL contain one entry per open cascade level, each with its own global rectangle

#### Scenario: A popup with a client-side shadow lists its surface around its placed rectangle

- **GIVEN** a popup whose client set its window geometry inset by `(23,21)` inside a 204×128 buffer, placed at `(265,179)` with size 158×82
- **WHEN** the popups are listed
- **THEN** its entry SHALL carry the placed rectangle `(265,179,158,82)` and the surface origin `(242,158)` with size 204×128
- **NOTE** Compositor unit test; measured GTK 4 submenu geometry.

#### Scenario: A popup without a shadow has one rectangle

- **WHEN** a popup whose client sets no window geometry inset, such as a Qt context menu, is listed
- **THEN** its surface SHALL coincide with its placed rectangle

#### Scenario: No open popups yields an empty listing

- **WHEN** no xdg_popup is mapped
- **THEN** the popup listing SHALL be empty and the command SHALL still succeed

### Requirement: Popup geometry reaches the AT-SPI provider through the platform window manager

The platform `WindowManager` abstraction SHALL offer a popup-geometry query: for a given process, each popup's global visible rectangle and surface rectangle, most recently opened or topmost first, with a conservative default. The PlatynUI-compositor backend SHALL implement it through the control socket, the visible rectangle being the placed one. The X11 backend SHALL implement it from the X server's popup windows, as `x11-window-management` specifies. The AT-SPI provider SHALL use this query for every popup node, grafted under its application or reporting the popup layer, matching provider popups to listed popups by process and surface size, so that the popup gets its visible rectangle as bounds and every item inside it gets physically correct global bounds. Backends that do not implement the query (Windows, mock) SHALL be unaffected, and the provider SHALL keep its existing extents path there and for a popup that matches nothing.

#### Scenario: Clicking into an open menu lands on the intended item under the compositor

- **WHEN** a test running under the PlatynUI compositor clicks a context-menu entry that opens a submenu, then a nested submenu, and hit-tests an item
- **THEN** each pointer action SHALL land on the visually intended element (the submenu opens, the nested item resolves), i.e. the three previously Wayland-skipped submenu scenarios of the Qt context-menu acceptance suite SHALL pass without skips

#### Scenario: On X11 popup bounds are the toolkit's screen extents

- **WHEN** the Qt context-menu suite runs on the X11 lane
- **THEN** a Qt popup's bounds SHALL equal the toolkit's screen extents, whether they come from its popup window or, for a window without `_NET_WM_PID`, from the toolkit
- **AND** the suite SHALL pass

#### Scenario: GTK 4 popovers place their items on the X11 lane

- **WHEN** the GTK 4 fixture's menu, submenu and drop-down tests run on the X11 lane
- **THEN** every pointer action SHALL land on the intended item
- **NOTE** Real provider only, with GTK 4.18 or later. GTK 4 reports popover positions measured from the widget that opened them; the popup window gives the real one.
