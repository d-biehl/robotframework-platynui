# Proposal

## Why

On Linux, an element's `Bounds` are often wrong, and a click by locator then lands beside the element and still reports success. The AT-SPI provider places each element by adding its parent-relative extents (`GetExtents(PARENT)`) to its parent's bounds, up to the window (`crates/provider-atspi/src/extents.rs:204-237`, `node.rs:1612-1634`). That holds only for toolkits that implement `PARENT` and whose tree has geometry on every level. Measured on 2026-11-10 with probe windows and real clicks, under the PlatynUI compositor and under X11:

- **GTK 3.24, Qt 5.15 and Firefox** report their screen values under `PARENT`, so the sum counts every ancestor's position again. A GTK3 button landed at `(268,535)` instead of `(85,169)`.
- **egui 0.37** (committed in `a669b055`) adds container nodes without extents. The chain stops there, and the fallback to `GetExtents(SCREEN)` is window-relative on Wayland. The compositor acceptance lane has been red since then: every click misses.
- **GTK4** reports `(0,0)` for `SCREEN` on every platform. The fallback therefore puts its elements in the screen corner, and the X11 window lookup, which compares `SCREEN` positions, cannot tell its windows apart.
- **GTK3 with a header bar on Wayland**: the window's own `Bounds` start at the buffer origin, including the shadow, but have the size without it.
- **GTK 4 popovers** (menus, submenus, drop-down lists) are measured from the widget that opened them, without their own placement. Measured on 2026-11-16 with GTK 4.24.1: a submenu item reported at `(130,58)` was drawn at `(234,144)`, so its click missed. The platform knows where each popup is: X11 shows it as a window of its own, and the PlatynUI compositor lists it.

No single AT-SPI coordinate type is right for every toolkit, and the newer toolkits no longer send screen coordinates at all, because Wayland has no global coordinate space. PlatynUI has to place elements the way assistive technologies on Wayland already do: from the window's own coordinate system, with the window's position taken from the platform.

The X11 backend also needs a window manager for everything window-related, because it finds windows only through `_NET_CLIENT_LIST` (`crates/platform-linux-x11/src/window_manager.rs:170-239`). In an X11 session without one, which is common with `xvfb-run` in CI, measured on the same day: no window is found, window bounds fall back to toolkit values, `IsActive` is always `False`, the hit-test finds nothing and every activation fails. The X server itself can answer most of these questions.

## What Changes

- **An element's bounds are its window's coordinate origin plus its position relative to that window.** The provider no longer adds parent-relative extents.
  - Each toolkit is read in the coordinate type it implements consistently. `SCREEN` is the default: GTK3, Qt, AccessKit, Chromium and Firefox. `WINDOW` is used for toolkits that send no screen position: GTK4, and any toolkit whose screen positions turn out to carry none.
  - On X11, `SCREEN` values are desktop coordinates and are used as they are. On Wayland they are relative to the window, so they are re-based on the window's origin, which the platform reports.
  - A container without extents no longer breaks the placement of its descendants.
- **Popups are placed where the platform shows them.**
  - A node that reports the popup layer, such as a GTK 4 popover, is the reference window of its contents, like Qt's grafted context menus.
  - Its position comes from the platform's popup list, matched by process and size. The compositor's popup listing gains each popup's surface, which includes GTK's shadow, and the X11 backend lists the process's popup windows.
  - In the hit-test, a hit inside a popup wins over the widgets beneath it.
- **The window manager reports a window's visible geometry and the origin of its surface as two facts.** The window's own `Bounds` are its visible geometry. Elements inside it are placed from the surface origin their toolkit counts from. The toolkit hint goes from `WindowManager::bounds` into the AT-SPI provider, which owns the toolkit knowledge.
- **The X11 window lookup no longer depends on toolkit screen positions.** Several windows of one process are told apart by size and title, as on Wayland.
- **X11 works without a window manager.**
  - Top-level windows are found among the root window's children by process ID.
  - `IsActive` comes from the input focus.
  - Activation raises the window and gives it the focus.
  - The hit-test takes the stacking order from the X server.
  - Closing asks the window through `WM_DELETE_WINDOW`.
  - Minimize, maximize and restore report the capability as unavailable, and the window state reads *normal*.
  - Moving and resizing work, as they already do once the window is found.
- **Fixtures and lanes.**
  - The Qt Widgets fixture gets an MDI scenario.
  - GTK3 and GTK4 are covered by the blueprint fixtures of the separate change `add-gtk-test-app`, which is applied first. Its GTK 3 catalog's pointer tests and its GTK 4 popover tests are this change's first failing tests and are committed with the provider rework.
  - An X11 lane profile runs without a window manager.
- **Docs:** `dev-docs/platform-linux.md`, `dev-docs/platform-linux-wayland.md`, `dev-docs/architecture.md` §10, `dev-docs/testing-strategy.md`, `CONTRIBUTING.md`.
- **Deliberately unchanged:**
  - Pixel units and display scaling (AccessKit reports physical pixels, Qt and Chromium device-independent ones) belong to `verify-display-scaling`.
  - Generic Wayland compositors without global window positions stay as they are.
  - How a missing X11 window manager is reported in the log belongs to `x11-atspi-healthy-run-warnings`.

Behavior changes that users see, for the release notes:

- Clicks and hit-tests by locator land on the element in GTK3, Qt 5 and Qt before 6.5, and egui 0.37 apps, including Qt MDI subwindows. Firefox follows the same rule but has no fixture.
- Menus, submenus and drop-down lists of GTK 4 apps (4.18 and later) are clicked where they are drawn, and the hit-test finds their items rather than the widgets beneath them.
- On X11, a Qt context menu whose window names its process takes its position from that window. The values do not change, because Qt's screen coordinates are already right there.
- A GTK3 window with a header bar under the PlatynUI compositor reports its visible frame as `Bounds`: the position moves by its shadow inset.
- In an X11 session without a window manager, windows are found, `IsActive` is real, activation and the hit-test work, and minimize, maximize and restore fail by name instead of doing nothing.

Nothing here is breaking at the keyword, attribute or configuration level: `Bounds` keeps its meaning (desktop coordinates); its values become correct. The `WindowManager` trait changes inside the Rust workspace only.

## Capabilities

### New Capabilities

- `atspi-element-geometry`: how the AT-SPI provider places a node on the desktop. It covers the window as the reference, which coordinate type a toolkit is read in, re-basing on the window's origin, containers without extents, a window's own bounds as its visible geometry, and popups placed where the platform shows them.
- `x11-window-management`: how the X11 backend finds an application's windows and popups and operates on its windows, with and without an EWMH window manager.

### Modified Capabilities

- `wayland-compositor-detection`: *A substituted window geometry is visible in the log*. A node inside a window is placed relative to its window, no longer relative to its ancestors. When the window manager cannot answer, the window's substituted bounds are that origin.
- `element-at-point`: *AT-SPI hit-test resolves window z-order and in-window z-order from distinct sources*. Without an EWMH window manager, the window level comes from the X server's stacking order of top-level windows. A hit inside a popup wins over the widgets beneath it.
- `compositor-popup-geometry`: both requirements. The compositor's popup listing carries each popup's surface beside its placed rectangle. The X11 backend answers the popup query too, and the provider uses it for every popup node, grafted or reporting the popup layer, matched by surface size.
- `acceptance-lane-selection`: *Lane profiles select by excluding foreign platforms* and *Lane profiles establish their session* gain the `real-x11-bare` profile. A new requirement declares the `window-manager:required` and `window-manager:none` tags. `add-gtk-test-app` changes the first of these requirements too; the change archived second merges both deltas.

## Impact

- **Rust:**
  - `crates/core`: `platform/window_manager.rs`. `bounds` loses its toolkit hint, and a query for the window's surface origin is added, with a default for window managers that have no separate origin. `popups` returns each popup's visible and surface rectangle instead of one rectangle.
  - `crates/provider-atspi`:
    - `src/extents.rs`: the composition, the coordinate-type choice and its unit tests;
    - `src/node.rs`: the extents sources (`resolve_extents_via_parent_chain` goes), the window reference of a node, popups recognized by their layer, and the toolkit name already read for the hint (`resolve_toolkit_name`, `:587-636`);
    - `src/lib.rs`: the hit-test descends by the new bounds and lets a hit inside a popup win.
  - `crates/platform-linux-wayland`: `src/window_manager/platynui_ipc.rs`. Visible geometry and surface origin come from `list_windows`, the GTK4-only shadow correction (`csd_shadow_offset`, `:552-572`) becomes the general geometry offset, and `popups` decodes the popup surface.
  - `apps/wayland-compositor`: `src/control.rs`. `list_popups` adds each popup's surface origin and size.
  - `crates/platform-linux-x11`: `src/window_manager.rs` and `src/lib.rs`, for the lookup without `_NET_CLIENT_LIST`, size-based disambiguation, focus, activation, stacking and close without a window manager, the popup list, and the EWMH check deciding the mode.
  - `crates/platform-mock` and the core stub window manager follow the trait change.
  - No change to the runtime, the XPath layer, the Windows or macOS crates.
- **Python / Robot Framework:** no library change. `packages/native` needs a rebuild (`just build-native`) because the providers change.
- **Fixtures and tests:**
  - `apps/test-app-qt` gets an MDI scenario.
  - The GTK3 and GTK4 acceptance scenarios run against the fixtures of `add-gtk-test-app`, which is applied before this change, including the GTK 4 menu bar, its nested submenu and the drop-down.
  - New acceptance suites for element geometry on both Linux lanes.
  - An X11 lane profile without a window manager, with its own `just` recipe and a tag for suites that need one.
  - The egui compositor lane turns green again with its existing suites.
- **Platforms:** Linux only. On X11 and under the PlatynUI compositor, behavior changes. Windows (UIA) and macOS are untouched. On generic Wayland compositors, which have no window manager backend, behavior is unchanged.
- **Related open changes:**
  - `x11-atspi-healthy-run-warnings` edits the same X11 lookup's logging;
  - `java-provider-linux` plans a window enumeration on X11 that the lookup without a window manager feeds;
  - `add-avalonia-test-app` relies on the X11 fallback to toolkit extents;
  - `verify-display-scaling` owns units.
