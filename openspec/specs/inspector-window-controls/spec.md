# inspector-window-controls Specification

## Purpose

The Inspector's window-management contract on decoration-less sessions. On Wayland the Inspector follows GTK's headerbar model — it declares client-side decorations, draws no frame, and hosts its own window controls in the menu bar: right-aligned Maximize/Restore and Close buttons with stable element IDs, plus a move grip on the empty menu-bar space that starts a compositor-driven interactive move (double-click toggles maximized). Windows, macOS, and X11 keep native decorations and none of these controls.

## Requirements

### Requirement: Wayland sessions run without requested decorations

On Wayland sessions the Inspector SHALL start with window decorations disabled — requesting neither server-side decorations nor drawing a client-side fallback frame. On Windows, macOS, and X11 the Inspector SHALL keep requesting native decorations, and the window controls described below SHALL NOT be shown there. The mode SHALL be decided once at startup from the session type.

#### Scenario: No fallback frame on Wayland

- **WHEN** the Inspector starts on a Wayland session whose compositor does not draw server-side decorations
- **THEN** the window SHALL show no client-drawn title bar or frame
- **NOTE** This matches today's look under niri; under GNOME/Mutter it removes the sctk-adwaita fallback bar.

#### Scenario: Non-Wayland platforms are unchanged

- **WHEN** the Inspector starts on Windows, macOS, or an X11 session
- **THEN** the window SHALL carry the native/window-manager decorations as before
- **AND** the menu bar SHALL show no window buttons and no move grip

### Requirement: Menu bar hosts Maximize and Close window buttons on Wayland

On Wayland sessions the Inspector's menu bar SHALL show two right-aligned window buttons — Maximize/Restore and, rightmost, Close — with theme-following icons, tooltips, and stable element IDs `window-maximize` and `window-close`. The Maximize button SHALL reflect the window state, presenting as Restore while the window is maximized. Activating Close SHALL close the Inspector through the same path as the File menu's Exit entry.

#### Scenario: Maximize toggles the window state

- **WHEN** the user activates the element with ID `window-maximize` on a non-maximized window
- **THEN** the window SHALL become maximized
- **AND** activating it again SHALL restore the previous size

#### Scenario: Close ends the Inspector

- **WHEN** the user activates the element with ID `window-close`
- **THEN** the Inspector window SHALL close and disappear from the accessibility tree

#### Scenario: Buttons are addressable by ID

- **WHEN** an accessibility client queries the Inspector window for elements with ID `window-maximize` and `window-close` on a Wayland session
- **THEN** exactly one element SHALL match each ID

### Requirement: Empty menu-bar space moves the window

On Wayland sessions, pressing the primary pointer button on menu-bar space not occupied by a menu entry or window button and dragging SHALL start a compositor-driven interactive window move. Double-clicking the same empty space SHALL toggle the maximized state. Menu entries and window buttons SHALL keep hit-test priority — their clicks SHALL NOT be consumed by the move grip — and a plain click on empty space SHALL have no effect. The move grip SHALL NOT add an addressable element to the accessibility tree.

#### Scenario: Dragging empty menu-bar space moves the window

- **WHEN** the user presses the primary button on empty menu-bar space and drags
- **THEN** the window SHALL follow the pointer (its screen rectangle changes position)

#### Scenario: Menu clicks are not stolen

- **WHEN** the user clicks a menu entry in the menu bar
- **THEN** the menu SHALL open exactly as without the move grip

#### Scenario: Double-click maximizes

- **WHEN** the user double-clicks empty menu-bar space on a non-maximized window
- **THEN** the window SHALL become maximized
