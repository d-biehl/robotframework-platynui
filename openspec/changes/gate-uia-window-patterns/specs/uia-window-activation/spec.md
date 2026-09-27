# Spec Delta

## Purpose

How the Windows UI Automation provider activates a window. Activation goes through the window manager the runtime injects, as the Java Access Bridge and Java agent backends do. It targets the top-level window of the element's native window handle. The spec also sets the precedence between that path, the UI Automation focus fallback, and the error when no window manager is available.

## ADDED Requirements

### Requirement: UIA windows are activated through the runtime's window manager

When a window element's Activatable action runs, the windows-uia provider SHALL determine the element's native window handle. If the element has none, the provider SHALL use the handle of its nearest ancestor that has one. It SHALL then decide as follows, in this order:

1. If a native window handle was found and the runtime injected a window manager, the provider SHALL activate the top-level (root) window of that handle through the window manager. The window manager brings a minimized window back in the state it was minimized from and makes it the foreground window, working around the platform's foreground lock. The provider SHALL NOT set UI Automation focus on the element.
2. If a native window handle was found but no window manager was injected, activation SHALL fail with an error stating that no window manager is available, and the foreground window SHALL NOT change.
3. If neither the element nor any ancestor has a native window handle, the provider SHALL set UI Automation focus on the element, as the only way left to bring it forward.

After a successful activation through the window manager, the action SHALL return once the window manager reports the window as active, or after a bounded wait, so that a caller reading `@IsActive` right afterwards sees the change.

#### Scenario: Activation makes the test window the foreground window

- **GIVEN** the off-screen test window of the UI Automation tests, which is not the foreground window, and a provider with the Win32 window manager injected
- **WHEN** the window's Activatable action runs
- **THEN** it SHALL return without error, and the window manager SHALL report the window as active
- **NOTE:** Real provider only. It changes the foreground window, so it is an ignored provider test that `just test-acceptance-windows` runs, not part of plain `just test`.

#### Scenario: Activation of a child window's element raises the top-level window

- **GIVEN** one of the test window's buttons, whose native window handle is a child window of the test window, and a window element that contains it
- **WHEN** the handle whose root window the provider activates is determined for the button
- **THEN** it SHALL be the test window's handle, not the button's
- **NOTE:** Real provider only, `just test` on a Windows host; it determines the handle without activating anything.

#### Scenario: Without a window manager, activation fails and changes nothing

- **GIVEN** a node for the test window created without an injected window manager
- **WHEN** its Activatable action runs
- **THEN** it SHALL fail with an error that names the missing window manager, and the foreground window SHALL be the same as before
- **NOTE:** Real provider only, `just test` on a Windows host.

#### Scenario: The route follows the native window handle

- **GIVEN** the provider's decision for a window element
- **WHEN** a native window handle is found and a window manager is injected
- **THEN** activation SHALL go through the window manager
- **WHEN** a native window handle is found and no window manager is injected
- **THEN** activation SHALL fail with the missing-window-manager error
- **WHEN** no native window handle is found in the element's chain
- **THEN** activation SHALL set UI Automation focus on the element, whether or not a window manager is injected
- **NOTE:** A unit test of the decision alone, in `just test` on a Windows host.

#### Scenario: A window minimized from the maximized state comes back maximized

- **GIVEN** an egui window that was maximized and then minimized
- **WHEN** Activate Window is called on it
- **THEN** `@IsMinimized` SHALL become False, `@IsMaximized` SHALL be True and `@IsActive` SHALL become True
- **NOTE:** Real provider only. This is the existing scenario of `window-activation`, which the Windows lane's `tests/acceptance/egui/window_activation.robot` now runs through the window-manager path.
