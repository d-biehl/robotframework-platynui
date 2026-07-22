## Context

On Wayland, window decorations are negotiated per toplevel via `xdg-decoration`: the compositor and client agree on server-side (SSD) or client-side (CSD) drawing. The Inspector (eframe/winit, decorations on by default) today ends up in three different states — all verified in source or live:

- **niri** (default `prefer-no-csd`): negotiates ServerSide but draws only a focus ring — no title bar, no way to move/maximize/close with the mouse.
- **GNOME/Mutter** (no `xdg-decoration` support): winit draws its sctk-adwaita fallback CSD — a foreign-looking bar, but functional.
- **KDE/KWin**: real SSD title bar.

The app cannot tell these apart: winit's `is_decorated()` on Wayland returns `true` unconditionally when ServerSide was negotiated (winit-0.30.13 `src/platform_impl/linux/wayland/window/state.rs:588` — "Server side decorations." → `true`), and egui does not surface decoration state in `ViewportInfo` at all. There is no protocol signal for "the compositor actually draws something".

Qt and GTK stay movable in the niri case because moving is not a decoration feature: the client asks the compositor for an interactive move (`xdg_toplevel::move` with the press serial) and the compositor drives the drag. GTK additionally never requests SSD — it always declares CSD and draws its own header controls (the libadwaita model). The full chain exists unused in our stack: `egui::ViewportCommand::StartDrag` → winit `drag_window()` → `xdg_toplevel.move` (and `_NET_WM_MOVERESIZE` on X11, native equivalents on Windows/macOS). egui 0.34 also provides `ViewportCommand::{Maximized, Close, Decorations, BeginResize}` and `ViewportInfo::maximized`; it does **not** expose winit's `show_window_menu` (no right-click window menu possible).

The test compositor is ready on all three axes — `apps/wayland-compositor` implements the `xdg_toplevel` move grab ([`handlers/xdg_shell.rs:128`](../../../apps/wayland-compositor/src/handlers/xdg_shell.rs) → `grabs::handle_move_request`), maximize/unmaximize requests, and honors client-side decoration requests (`handlers/decoration.rs::request_mode`, including the position adjustment on the SSD→CSD transition). The lane can therefore exercise the real protocol path.

Existing Inspector building blocks to reuse: the menu bar renders in `egui::Panel::top("menu_bar")` (`view/toolbar.rs::show_menu_bar`); the toolbar's `toolbar_button`/`ToolbarButtonSpec` pattern provides theme-tinted white-normalized Phosphor SVG icons plus stable AccessKit `author_id`s (surfaced as `@Id` to the acceptance suites); `File → Exit` already sends `ViewportCommand::Close`.

## Goals / Non-Goals

**Goals:**

- Move, maximize/restore, and close the Inspector with the mouse on Wayland sessions, regardless of whether the compositor draws decorations.
- GNOME-style presentation: window buttons right-aligned in the menu bar, empty menu-bar space as move grip, double-click maximizes.
- Stable accessibility IDs for the new buttons so the compositor-lane suites can drive them.
- Windows, macOS, and X11 behavior unchanged bit for bit.

**Non-Goals:**

- No minimize button — niri and other scrollable/tiling compositors have no minimize concept; revisit if a floating-Wayland user asks.
- No right-click window menu (egui 0.34 lacks `ShowWindowMenu`).
- No client-side resize edges (`BeginResize` hot zones) — see Risks for the GNOME-floating trade-off.
- No escape hatch to force either chrome mode (native decorations on Wayland, headerbar on X11 tiling WMs); explicitly deferred.
- No visual window frame, shadows, or rounded corners of our own.

## Decisions

### D1: Declare, don't detect — Wayland always runs decoration-less

Since "does the compositor draw decorations?" is undetectable (see Context), the Inspector adopts GTK's stance: on Wayland it declares CSD and brings its own controls; everywhere else it keeps native decorations. A two-value chrome mode is decided **once at startup**:

- Wayland session (winit will pick the Wayland backend — same rule winit itself uses: `WAYLAND_DISPLAY` is set) → `ViewportBuilder::with_decorations(false)`, headerbar controls active.
- Otherwise (Windows, macOS, X11) → decorations on, controls inactive; nothing else changes.

The decision is a pure function of the environment (unit-testable) and is passed into the app/view layer as a bool — no runtime re-negotiation, no `is_decorated()` polling.

Alternatives rejected: runtime detection (impossible, winit lies under niri); keeping SSD where offered and adding controls only elsewhere (same detection problem — under niri SSD is "offered" and invisible); a per-compositor allowlist (fragile, wrong by design).

### D2: Window buttons are `toolbar_button`s right-aligned in the menu bar

In headerbar mode, `show_menu_bar` gains a right-to-left region (same pattern as the toolbar's always-on-top pin) with, from the right: **Close** (`author_id = "window-close"`, Phosphor `x`) and **Maximize/Restore** (`author_id = "window-maximize"`, Phosphor `corners-out`, swapping to `corners-in` when maximized) — GNOME's order and glyph language. Both reuse `ToolbarButtonSpec`/`toolbar_button`, inheriting theme-tinted icons, tooltips, and the AccessKit wiring for free. Maximized state comes from `ctx.input(|i| i.viewport().maximized)`; the buttons send `ViewportCommand::Close` and `ViewportCommand::Maximized(!maximized)`. Two new white-normalized SVGs join `assets/icons/` under the existing MIT license note.

### D3: The move grip is a full-width interact registered before the menu content

Inside the `menu_bar` panel (headerbar mode only), a `ui.interact(panel_rect, id, Sense::click_and_drag())` is registered **before** `egui::MenuBar` and the window buttons render — egui's hit-test prefers later widgets, so menus and buttons keep priority and only genuinely empty space acts as grip. `drag_started_by(Primary)` sends `ViewportCommand::StartDrag`; `double_clicked()` toggles `Maximized`. The interact carries no label and no AccessKit node customization, so the accessibility tree gains no addressable element (guarded by the existing toolbar/menu suites staying green).

One subtlety: `StartDrag` must be sent on `drag_started_by`, not on press — a plain click on empty space must stay a no-op (and remain available for the double-click), matching GTK/Qt feel.

### D4: Testing posture

- **Unit:** the chrome-mode decision as a pure function of environment values; the maximize-button spec swap (icon/label vs. maximized state).
- **Acceptance (compositor lane, new suite `inspector_window_controls.robot`):** the compositor implements the real grabs, so behavior is observable through the accessibility tree — drag on empty menu-bar space moves the window's screen rectangle; activating `@Id="window-maximize"` grows the rectangle and toggles back on the second activation; activating `@Id="window-close"` removes the Inspector window from the tree (last test in the suite; teardown must tolerate the app already being gone). Locators use `@Id` per the provider-independent locator rule. Existing menu/toolbar suites double as the regression guard that the grip steals no clicks.
- **Manual (niri):** look-and-feel only — button placement, drag feel, double-click.

The pointer path for the drag test uses the BareMetal pointer keywords against coordinates derived from the menu-bar element's rectangle (empty area = right of the last menu, left of the window buttons).

## Risks / Trade-offs

- [KDE Wayland loses the KWin title bar] → intended consequence of declare-don't-detect; the in-app controls replace it. The deferred escape hatch would restore choice if anyone objects.
- [GNOME floating: no pointer resize edges] → with decorations off, winit creates no frame and thus no client resize borders; edge-resize by mouse is unavailable on compositors that rely on CSD for it (GNOME). Super+drag/keybindings still work everywhere; niri/tiling compositors manage resize themselves. Mitigation if it bites: a follow-up adding `ViewportCommand::BeginResize` hot zones along the window edges.
- [No drop shadow / visual frame on GNOME] → GTK apps draw their own shadows, egui does not; the window renders as a plain rectangle. Cosmetic, accepted for a dev tool.
- [Drag grip could shadow menu clicks if hit-test assumptions break] → covered continuously by the existing menu suites in the lane; the interact-before-content ordering is the documented egui pattern (`custom_window_frame` example).
- [`WAYLAND_DISPLAY` set but winit forced to X11] → winit 0.30 has no env override for backend selection (build features only, both enabled); the mismatch cannot occur in practice.
- [Close button vs. unsaved state] → the Inspector persists settings on graceful close via eframe; `ViewportCommand::Close` goes through the same path as `File → Exit` and the WM close button today.

## Migration Plan

Behavioral change confined to the `platynui-inspector` binary; no native (PyO3) rebuild, no Python/RF surface change, no settings-schema change. Wayland users see the new chrome immediately; Windows/macOS/X11 builds are byte-for-byte behaviorally identical. Rollback = revert the commits.

## Open Questions

- Minimize button on floating Wayland desktops (GNOME/KDE) — deliberately left out; add later behind the same headerbar mode if requested.
- The deferred chrome-mode escape hatch (`--window-chrome native|headerbar` or a persisted setting) — would also serve decoration-less X11 tiling WMs; out of scope here.
