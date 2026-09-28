## Why

`Runtime::bring_to_front` calls `restore()` before `activate()` on every call. The goal is to bring a minimized window back, but every backend's `restore()` returns the window to its *normal* state, and that includes un-maximizing a maximized window. Because `bring_to_front` runs implicitly before every pointer and keyboard keyword while `auto_activate` is on (the default), and before the CLI pointer commands, simply clicking into a maximized application resizes it. The click point can then land on stale geometry.

The fix is to move the "bring a minimized window back" responsibility into activation itself. Doing that exposes a second gap: the `WindowManager` can act on a window but cannot report whether the window is minimized or maximized. As a result, the AT-SPI and JAB providers cannot expose `control:IsMinimized`/`IsMaximized`, and the new behavior cannot be asserted on Linux or Java windows.

## What Changes

- **Bug fix**: Activating a window (`Activatable.activate()`, `WindowManager::activate`) SHALL bring a minimized window back to the state it was minimized from, and SHALL leave the maximized state of a non-minimized window untouched. `Runtime::bring_to_front` no longer calls `restore()`; it only activates. Un-maximizing on activation was never intended behavior, so this is not a breaking change. `Restore Window` remains the explicit way to return a window to the normal state.
- Backends are aligned to that activation contract:
  - **UIA provider**: activation now brings an iconic window back before focusing it. Today it only calls `SetFocus`.
  - **X11 EWMH**: activation requests de-iconification before `_NET_ACTIVE_WINDOW`. Today it relies on the window manager doing this on its own.
  - **Wayland (PlatynUI compositor)**: `focus_window` goes through the compositor's un-minimizing activation path. The IPC backend can also resolve minimized windows, which it cannot today.
  - **Mock provider**: a window minimized from the maximized state returns maximized on activation.
  - **Win32 `WindowManager`**: already conforms.
- New `WindowManager` query for a window's state: whether it is minimized, maximized or neither, and whether it is kept on top. It has a default "capability unavailable" implementation and is implemented for Win32, X11 EWMH, the PlatynUI Wayland compositor backend, and the platform mock.
- The PlatynUI compositor reports the state that backend needs over its control socket (maximized state and matching geometry for minimized windows).
- The AT-SPI and JAB providers expose `control:IsMinimized`, `control:IsMaximized` and `control:IsTopmost` on top-level windows through that query, the same way they already expose `control:IsActive`.
- Documentation of the activation contract and the state query in `dev-docs/architecture.md` §8.5 and the platform docs. The keyword docs for `Bring To Front`/`Activate Window`, the native `Activatable.activate` docstring and the CLI `--bring-to-front` help text are updated as well.

## Capabilities

### New Capabilities

- `window-activation`: what activating a window, and bringing a node's window to the front, does to the window's minimized and maximized state. This covers direct activation, `Bring To Front`, and the implicit activation before pointer and keyboard actions.
- `window-state`: the window-state query of the `WindowManager`, and the `IsMinimized`/`IsMaximized`/`IsTopmost` attributes that window-manager-backed providers derive from it.

### Modified Capabilities

<!-- none: jab-provider's "Window capabilities via WindowManager delegation" requirement stays true as written (the patterns still delegate to the WindowManager); the new state attributes are specified once, cross-provider, in window-state. inspector-window-controls is unaffected (it specifies the Inspector's own buttons, not activation). -->

## Impact

- **Rust core**: `crates/core` gains the state type and the additive trait method (default implementation), so no implementor breaks. The runtime's `bring_to_front` in `crates/runtime` drops the `restore()` call.
- **Platform crates**:
  - `crates/platform-windows`: state query.
  - `crates/platform-linux-x11`: state query and de-iconify on activate.
  - `crates/platform-linux-wayland`: state query, resolution of minimized windows.
  - `crates/platform-mock`: state query.
  - `crates/platform-macos`: no WindowManager, so untouched.
- **Providers**:
  - `crates/provider-windows-uia`: activate brings an iconic window back.
  - `crates/provider-atspi` and `crates/provider-java-jab`: new state attributes.
  - `crates/provider-mock`: remembers the pre-minimize maximized state.
  - `crates/provider-java`: agent-backed windows inherit the WindowManager activation. Their `IsMinimized`/`IsMaximized` come from the agent and stay as they are.
- **Compositor**: `apps/wayland-compositor` gets the control-socket additions and changes to `focus_window`. This is a test fixture of this repo, so no external consumers are affected.
- **Python/RF**: no API change. Docstrings are updated in `src/PlatynUI/BareMetal` and `packages/native`. A **native rebuild** (`just build-native`) is needed before the RF mock suites see the fix.
- **Tests**:
  - Rust unit tests: runtime `bring_to_front`, mock provider, platform-mock and core state defaults.
  - RF mock suite: `tests/BareMetal/window_activation.robot`.
  - Real acceptance lane (egui app under X11 and the Wayland compositor): maximized windows stay maximized on activation, minimized windows come back, and state attributes read correctly.
  - The existing `tests/acceptance/egui/inspector_window_controls.robot` ("Maximize Button Toggles The Window State") clicks the Maximize/Restore button with auto-activation on a maximized window. It currently races against the unwanted un-maximize and must keep passing.
- **Platforms**:
  - Windows (UIA/Win32, supported) and JAB: verified only in the Windows lane. The claim that `SW_RESTORE` returns a window minimized from maximized back to maximized is an assumption to confirm there.
  - Linux X11 and the PlatynUI Wayland compositor: acceptance lanes.
  - Other Wayland compositors have no backend today and keep returning "capability unavailable".
  - macOS: unaffected.
