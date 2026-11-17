# Spec Delta

## Purpose

Defines how the X11 backend finds an application's top-level windows and popups and operates on its windows, both when an EWMH window manager runs on the display and when none does, so that window geometry, popup geometry, activation and hit-testing do not silently depend on one.

## ADDED Requirements

### Requirement: The window mode is decided when the runtime connects

The X11 backend SHALL decide once per runtime, when the runtime connects to the display, whether an EWMH window manager runs there. With one, windows SHALL be found and operated through it. Without one, they SHALL be found and operated through the X server directly. A runtime created later SHALL decide again.

#### Scenario: A runtime on a display without a window manager uses the X server

- **GIVEN** an X11 display on which no window manager runs
- **WHEN** a runtime is created for it and a window's `@IsActive` is read
- **THEN** the read SHALL come from the X server's input focus, not from a window manager property
- **NOTE** Real provider only, on the X11 lane without a window manager.

#### Scenario: A runtime created after a window manager started uses it

- **GIVEN** a display on which a window manager was started after an earlier runtime had connected without one
- **WHEN** a new runtime is created for the display
- **THEN** the new runtime SHALL find and operate windows through the window manager
- **NOTE** Unit test of the decision with an answered window-manager check; the lanes start their window manager before the suite.

### Requirement: An application's windows are found with or without a window manager

With a window manager, an application's windows SHALL be the managed client windows whose process ID is the application's. Without one, they SHALL be the mapped children of the root window that are not override-redirect and whose process ID is the application's. A window that cannot be found SHALL fail the lookup with an error naming the process ID.

#### Scenario: A window is found without a window manager

- **GIVEN** the egui fixture on an X11 display without a window manager, its window moved away from the screen origin
- **WHEN** the window's `Bounds` are read
- **THEN** they SHALL be the window's geometry from the X server
- **AND** no substitution warning SHALL be logged for the window
- **NOTE** Real provider only, X11 lane without a window manager.

#### Scenario: A popup is not taken for the application's window

- **GIVEN** an application without a window manager that has one top-level window and an open override-redirect menu
- **WHEN** the top-level window is looked up
- **THEN** the lookup SHALL return the top-level window, not the menu
- **NOTE** Real provider only (Qt Widgets fixture's menu, X11 lane without a window manager).

#### Scenario: A process without a window fails by name

- **GIVEN** an application node whose process has no top-level window on the display
- **WHEN** its window is looked up
- **THEN** the lookup SHALL fail with an error that names the process ID
- **NOTE** Real provider only.

### Requirement: An application's popups are listed from the X server

The X11 backend SHALL answer the popup query for a process with the viewable override-redirect children of the root window that carry a popup window type (menu, drop-down menu, popup menu, combo, tooltip) and the process's `_NET_WM_PID`, topmost first, with and without a window manager. A popup window without `_NET_WM_PID` SHALL NOT be listed.

#### Scenario: An open GTK 4 submenu is listed above its menu

- **GIVEN** the GTK 4 fixture with its `menu-edit` menu and the nested submenu `menu-edit-more` open
- **WHEN** an item of the submenu is clicked by locator
- **THEN** the item SHALL report the click, which needs the submenu's window to be listed first
- **NOTE** Real provider only, on the X11 lane with and without a window manager, with GTK 4.18 or later. The filter and the order are also unit-tested over recorded window attributes.

#### Scenario: Popups of other processes and windows without a process are not listed

- **GIVEN** a root window with a popup window of another process, a popup window without `_NET_WM_PID`, an override-redirect window of no popup type, and one popup window of the process
- **WHEN** the process's popups are listed
- **THEN** the listing SHALL contain only the process's popup window
- **NOTE** Unit test over recorded window attributes.

### Requirement: An X11 popup's surface is its window

A listed popup's surface SHALL be its window's rectangle. Its visible rectangle SHALL be that rectangle without the `_GTK_FRAME_EXTENTS` insets when the window has the property, and the whole rectangle otherwise.

#### Scenario: A popup window with frame extents has a smaller visible rectangle

- **GIVEN** a popup window at `(200,100)` of size 210×130 whose `_GTK_FRAME_EXTENTS` are 23 left, 23 right, 21 top and 25 bottom
- **WHEN** its process's popups are listed
- **THEN** its surface SHALL be `(200,100,210,130)` and its visible rectangle `(223,121,164,84)`
- **NOTE** Unit test over a recorded property; the lanes run no compositing window manager, so GTK sets no frame extents there.

### Requirement: Windows of one process are told apart without screen positions

When a process has several windows, the lookup SHALL choose by window title first, then by a size that only one candidate has. The node's screen position SHALL only decide between candidates of equal size where its toolkit reports one. When none of these singles out one window, the lookup SHALL fail rather than pick one.

#### Scenario: Two GTK 4 windows of one process are resolved by size

- **GIVEN** a GTK 4 application with two windows of different sizes, and whose toolkit reports `(0,0)` as every screen position
- **WHEN** each window's `Bounds` are read
- **THEN** each SHALL be that window's own geometry
- **NOTE** Real provider only (the GTK4 fixture's main window and its modeless dialog), on the X11 lane with and without a window manager.

#### Scenario: Two indistinguishable windows fail the lookup

- **GIVEN** a process with two windows of equal title and size, whose toolkit reports no screen position
- **WHEN** one of them is looked up
- **THEN** the lookup SHALL fail with an error that names the number of candidates, instead of returning either window
- **NOTE** Unit test of the candidate choice.

### Requirement: Without a window manager, activation raises the window and focuses it

Activating a window without a window manager SHALL map it if it is unmapped, raise it above the other top-level windows and give it the input focus. The override-redirect popups SHALL stay above it, so that activating a window keeps its own open menu usable. It SHALL NOT move or resize the window.

#### Scenario: A background window comes to the front

- **GIVEN** two overlapping egui fixture windows on an X11 display without a window manager, the second one on top
- **WHEN** the first window is activated
- **THEN** its `@IsActive` SHALL be `True` and the second window's SHALL be `False`
- **AND** the element at a point in the overlapping area SHALL belong to the first window
- **AND** keyboard input SHALL reach the first window
- **NOTE** Real provider only, X11 lane without a window manager.

#### Scenario: Auto-activation before a click works without a window manager

- **GIVEN** a background window on an X11 display without a window manager
- **WHEN** `Pointer Click` is called on a button inside it
- **THEN** the window SHALL come to the front first and the button SHALL report the click
- **NOTE** Real provider only; the egui auto-activate suite on the X11 lane without a window manager covers it.

### Requirement: Without a window manager, a window is active while it holds the input focus

Without a window manager, a window SHALL be active while the X server's input focus is the window itself or one of its descendants. No window SHALL be active while the focus is the root window, `PointerRoot` or none.

#### Scenario: Focus inside a window makes it active

- **GIVEN** two windows on a display without a window manager, with the input focus on a child window of the first
- **WHEN** both windows' `@IsActive` are read
- **THEN** the first SHALL be `True` and the second `False`
- **NOTE** Real provider only, X11 lane without a window manager; the descendant walk is also unit-tested.

#### Scenario: No focused window means no active window

- **GIVEN** a display without a window manager whose input focus is `PointerRoot`
- **WHEN** a window's `@IsActive` is read
- **THEN** it SHALL be `False`
- **NOTE** Unit test.

### Requirement: Without a window manager, closing asks the window

Closing a window without a window manager SHALL send it the `WM_DELETE_WINDOW` request when the window lists that protocol. A window that does not list it SHALL fail the close with an error naming the missing protocol. The backend SHALL NOT destroy the window or disconnect its client.

#### Scenario: A window that supports the delete protocol closes

- **GIVEN** the egui fixture on an X11 display without a window manager
- **WHEN** `Close Window` is called on its window
- **THEN** the window SHALL disappear from the tree
- **NOTE** Real provider only, X11 lane without a window manager.

#### Scenario: A window without the delete protocol fails by name

- **GIVEN** a window without a window manager that does not list `WM_DELETE_WINDOW` in its protocols
- **WHEN** it is closed
- **THEN** the close SHALL fail with an error naming `WM_DELETE_WINDOW`, and the window SHALL stay open
- **NOTE** Unit test of the protocol check.

### Requirement: Without a window manager, its own operations are unavailable

Minimizing, maximizing and restoring a window without a window manager SHALL fail with a capability-unavailable error that names the operation and says that no window manager runs. A mapped window's state SHALL read *normal* and not topmost. Moving and resizing SHALL work, because the X server performs them.

#### Scenario: Minimize fails by name

- **GIVEN** the egui fixture on an X11 display without a window manager
- **WHEN** `Minimize Window` is called on its window
- **THEN** the keyword SHALL fail with an error that names the minimize capability and the missing window manager
- **AND** the window SHALL stay visible
- **NOTE** Real provider only, X11 lane without a window manager.

#### Scenario: The state of a window without a window manager is normal

- **GIVEN** a mapped window on a display without a window manager
- **WHEN** its `@IsMinimized`, `@IsMaximized` and `@IsTopmost` are read
- **THEN** each SHALL be `False`
- **NOTE** Real provider only, X11 lane without a window manager.

#### Scenario: Move and resize work without a window manager

- **GIVEN** the egui fixture on an X11 display without a window manager
- **WHEN** its window is moved to `(200,150)` and resized to `640x480`
- **THEN** its `Bounds` SHALL be `(200,150,640,480)`
- **NOTE** Real provider only, X11 lane without a window manager.
