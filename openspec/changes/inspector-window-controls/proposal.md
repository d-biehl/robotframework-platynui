## Why

On Wayland compositors that do not draw title bars, the Inspector window cannot be moved, maximized, or closed with the mouse. Under niri (with its default `prefer-no-csd`) the negotiated server-side decoration mode produces no visible decoration at all, and there is no protocol signal an app could use to detect that — winit's `is_decorated()` reports `true` whenever server-side mode was negotiated, drawn or not. Qt and GTK applications solve this client-side: they request compositor-driven interactive moves (`xdg_toplevel.move`) from empty header areas and, in GTK's case, never ask for server decorations in the first place, drawing their own window buttons instead (the libadwaita headerbar model). The Inspector should behave the same way; the whole toolkit chain (`egui::ViewportCommand::StartDrag` → winit → `xdg_toplevel.move`) already exists and is merely unused.

## What Changes

- **Wayland goes decoration-less by declaration**: on Wayland the Inspector starts with `decorations = false` — no server-side title bar, no sctk-adwaita fallback CSD (which today produces a foreign-looking bar under GNOME/Mutter). Detection of "does the compositor actually draw something" is impossible, so we adopt GTK's model: declare, don't detect.
- **The menu bar doubles as headerbar** (Wayland only): right-aligned Maximize/Restore and Close buttons in the GNOME style, with theme-following Phosphor icons and stable element IDs (`window-maximize`, `window-close`).
- **Empty menu bar area becomes a move grip** (Wayland only): press-and-drag on menu bar space not occupied by a menu or window button starts a compositor-side interactive move; double-click toggles maximized. Menu entries keep hit-test priority.
- **Windows, macOS, and X11 are unchanged**: native/WM decorations as today, no in-app window controls, no drag zone.
- Behavior change (not BREAKING): on KDE Wayland the KWin title bar disappears in favor of the in-app controls. A future escape hatch (flag/setting to force either mode, which would also serve decoration-less X11 tiling WMs) is explicitly out of scope here.

## Capabilities

### New Capabilities
- `inspector-window-controls`: the Inspector's window-management contract on decoration-less sessions — the Wayland no-decorations declaration, the menu-bar move grip with double-click maximize, and the right-aligned Maximize/Restore and Close buttons with their accessibility IDs.

### Modified Capabilities

None. `inspector-toolbar`'s header-area contract (themed panels, toolbar contents, search row) is untouched — the new buttons live inside the existing themed menu-bar panel, and the existing requirement that no header region shows the raw clear color extends to them naturally.

## Impact

- **Layer:** Rust only, entirely within `apps/inspector` (`lib.rs` viewport setup + Wayland detection, `view/toolbar.rs` menu bar, two new white-normalized Phosphor SVGs under `assets/icons/`). No new dependencies — `ViewportCommand::{StartDrag, Maximized, Close}` and `ViewportInfo::maximized` are all in egui 0.34. No native (PyO3) rebuild, no Python/Robot Framework surface change.
- **Test compositor:** no changes expected — `apps/wayland-compositor` already implements `xdg_toplevel` move/maximize/unmaximize grabs and honors client-side decoration requests, so the lane exercises the real protocol path end to end.
- **Platforms:** all Wayland compositors get the headerbar behavior (niri: gains move/maximize/close; GNOME: replaces the sctk-adwaita fallback bar; KDE: replaces the KWin title bar). Windows/macOS/X11 keep today's behavior bit for bit.
- **Tests:** unit tests for the mode decision and button state mapping; a new acceptance suite in the compositor lane (drag moves the window's bounding rectangle, maximize/restore toggles it, close removes the window from the accessibility tree); existing menu/toolbar suites guard that the drag grip steals no clicks. Look-and-feel under niri is verified manually.
- **Docs:** `dev-docs/inspector.md` (window controls, decoration policy per platform).
