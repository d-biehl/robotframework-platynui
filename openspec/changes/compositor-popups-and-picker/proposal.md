# Exact popup interaction and Inspector picker support under the PlatynUI Wayland compositor

## Why

Two Wayland-compositor gaps were left as explicit follow-ups by `atspi-event-driven-tree` (archived 2026-07-14) and `inspector-live-mouse-picker`:

1. **Popup geometry.** A Wayland client cannot know its global position, so AT-SPI screen extents for transient popups (context menus) come back client-local — a grafted popup and every menu item under it resolves to wrong global bounds, and a pointer click *into* an open menu lands offset (typically by the decoration height) and dismisses it. Three submenu acceptance tests in `tests/acceptance/qt/context_menu.robot` are skipped on Wayland for exactly this reason. The PlatynUI compositor *knows* every popup's real position (it places them), but exposes neither popup geometry nor any popup listing over its control socket — `list_windows`/`window_at_point` iterate only mapped toplevels.
2. **Inspector live picker.** The picker's modifier reader ([apps/inspector/src/modifiers.rs](../../../apps/inspector/src/modifiers.rs)) has only X11 (`XQueryPointer`) and Windows (`GetAsyncKeyState`) paths; under the compositor there is no way to observe the user's held Ctrl+Alt+Shift, so the picker is greyed out — or worse, a leaked host `DISPLAY` lets the X11 reader bind to the *host* X server and silently observe the wrong seat. `Acceptance.Egui.Inspector Picker` fails on the local compositor lane today. The pointer half already works (`get_pointer_position` over the control socket); only the modifier half is missing.

Both problems have the same shape: state the compositor already tracks server-side but does not expose over its control protocol.

## What Changes

- **Compositor control protocol** (`apps/wayland-compositor`): two new JSON commands —
  - `list_popups`: enumerate currently mapped xdg_popup surfaces with their **global** rectangles (computable today as `root window location + get_popup_toplevel_coords + popup geometry offset`), plus the parent toplevel's `window_id`/`pid`. Popups live in smithay's `PopupManager`, separate from the `Space`, and are not covered by any existing command.
  - `get_modifiers`: return the seat keyboard's current XKB modifier state (ctrl/alt/shift/logo) — smithay tracks it, nothing exposes it.
- **WindowManager trait** (`crates/core`): a popup-geometry query (e.g. `popups(pid)` returning global rects) with a conservative default (empty / capability unavailable) so X11, Windows, and mock backends are untouched.
- **Wayland platform backend** (`crates/platform-linux-wayland`): implement the popup query over the control socket IPC.
- **AT-SPI provider** (`crates/provider-atspi`): when resolving bounds for a grafted popup-class node (which is deliberately *not* a window surface), consult the window manager's popup geometry — matched by PID and size — before falling back to AT-SPI screen extents. X11 keeps its current (working) extents path.
- **Inspector** (`apps/inspector`): a compositor modifier reader that polls `get_modifiers` over the control socket; reader selection prefers the compositor path when running in a Wayland session with a PlatynUI control socket, instead of blindly trying X11 first (fixes the host-`DISPLAY`-leak misbinding).
- **Acceptance tests**: remove the Wayland `Skip If` from the three submenu tests in `tests/acceptance/qt/context_menu.robot` (they become the red bar for the popup half); `Acceptance.Egui.Inspector Picker` is the red bar for the picker half.
- Not breaking: all protocol additions are new commands; trait additions have defaults; X11/Windows behavior is unchanged.

## Capabilities

### New Capabilities
- `compositor-popup-geometry`: the PlatynUI compositor exposes global geometry for transient popup surfaces, and the platform/provider stack uses it so popup interaction on Wayland is exact.
- `compositor-modifier-state`: the compositor exposes the seat's current keyboard modifier state, and the Inspector's live picker consumes it so picking works under the compositor.

### Modified Capabilities
- `atspi-event-driven-tree`: the "Cascaded submenu items are reachable through the grafted popup" scenario loses its Wayland caveat (pointer interaction into popups becomes exact there; the skip note no longer applies).

## Impact

- **Rust**: `apps/wayland-compositor` (control.rs dispatch + popup enumeration helper, ipc_tests), `crates/core` (WindowManager trait + default), `crates/platform-linux-wayland` (platynui_ipc backend, control_ipc), `crates/provider-atspi` (popup bounds resolution in node.rs), `apps/inspector` (modifiers.rs reader + selection).
- **Python/RF**: no keyword changes; findability/interaction flows through the existing surface. Needs a **native rebuild** (`just build-native`) for the provider/platform changes.
- **Platforms/providers**: PlatynUI compositor (Wayland) gains exact popup interaction + picker support; X11 and Windows unaffected (X11 popup extents already correct; Windows/UIA exposes popups natively). Generic Wayland (no PlatynUI control socket) stays unsupported for both — unchanged.
- **Tests**: compositor `ipc_tests` for both commands; provider unit tests for popup-rect matching; the three unskipped submenu tests + the Inspector picker test on the compositor lane are the acceptance gates; X11 lane must stay green (50/50).
- **Open investigation** (scoped into tasks): why CI's *headless* compositor lane reports green for the picker suite while the local lane fails — suspected `DISPLAY` leakage into the local compositor session (host-X modifier reader binds, then never sees compositor-injected modifiers); the reader-selection fix plus session-script isolation should make local and CI agree.
