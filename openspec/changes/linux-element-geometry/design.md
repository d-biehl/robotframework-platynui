# Design

## Context

See proposal.md (Why) for the failures this change fixes. The facts below were read at `a669b055`. **Verified** means read in this repository's code or measured. **Source** means read in the toolkit's upstream code on 2026-11-10, not run. **Assumed** means neither.

**How the AT-SPI provider places a node today** (verified).

- `resolve_extents` (`crates/provider-atspi/src/extents.rs:177-201`) asks the window manager for a real top-level window. For every other node, and as the substitute when the window manager fails, it calls `toolkit_extents` (`:204-237`), which works in three steps:
  1. parent's `Bounds` plus `GetExtents(PARENT)` (`node.rs:1612-1634`);
  2. otherwise `GetExtents(SCREEN)` (`node.rs:1715-1721`);
  3. for a grafted popup, the window manager's popup rectangle matched by size (`:229-234`).
- A node is a real top-level when its parent is an application-level node and it is not a transient popup (`node.rs:284-295`). The flag is cached at construction (`node.rs:1515-1521`), and the extents are memoized per node (`node.rs:1586-1588`).
- The toolkit hint is `ToolkitName` lowercased plus the major `Version` (`node.rs:587-636`), cached per bus name. The probes read `gtk3`, `gtk4`, `qt5`, `qt6` and `accesskit` (AccessKit sends no version).

**What the window managers report** (verified).

- `WindowManager::bounds(id, toolkit_hint)` (`crates/core/src/platform/window_manager.rs:116-127`). Its callers are the AT-SPI provider (`extents.rs:78`), the JAB provider on Windows (`crates/provider-java-jab/src/node.rs:1037`) and the Wayland facade (`crates/platform-linux-wayland/src/window_manager/mod.rs:51`).
- **X11:** `client_rect` is the client window's root position (`translate_coordinates`) plus its geometry size (`crates/platform-linux-x11/src/window_manager.rs:261-279`). The window manager's frame is excluded, and the hint is ignored (`:701-703`).
- **PlatynUI compositor:** `bounds` returns the buffer origin `content_x/y` plus a shadow offset applied only for `csd` windows with the hint `gtk4`, and the size `content_width/height` (`platynui_ipc.rs:93-125`, `:552-572`). Measured `list_windows` records:

  | Window | `x,y,width,height` | `content_x/y` | `content_width/height` | `geometry_x/y` | mode |
  |---|---|---|---|---|---|
  | GTK 3.24, header bar | 34,31,420,337 | 10,10 | 420,337 | 24,21 | csd |
  | GTK 4.24, header bar | 24,22,420,305 | 10,10 | 420,305 | 14,12 | csd |
  | Qt 6.11 / egui 0.37 | 2,2,656,526 / 2,2,616,546 | 10,40 | 640,480 / 600,500 | 0,0 | ssd |

  So the visible window is `content + geometry` with size `content_width/height` in both modes. `x,y,width,height` includes the compositor's frame for `ssd`. The surface origin is `content_x/y`.

**How the X11 backend finds windows** (verified).

- `find_xid_for_pid` takes only `_NET_CLIENT_LIST` (`window_manager.rs:170-239`).
- For several candidates it compares the node's `SCREEN` extents with each client rectangle (`:241-259`, read through `native:Component.Extents.Screen`, `:402-410`), then the title.
- `window_at_point` reads `_NET_CLIENT_LIST_STACKING` (`:654-677`), and override-redirect popups come from `query_tree` of the root (`:586-623`).
- `is_active` reads `_NET_ACTIVE_WINDOW` (`:734-752`), and `activate` sends it (`:759-777`).
- `check_ewmh_wm_support` (`:890ff`) only feeds a warning (`crates/platform-linux-x11/src/lib.rs:106-110`).
- Moving and resizing configure the client directly (`:846-888`).

**What the toolkits send** (source, with the measurement where one was made). `P` is the toolkit's own position for the window, `c` the element's offset inside it.

| Toolkit | `SCREEN` | `WINDOW` | `PARENT` |
|---|---|---|---|
| GTK 4 | always `(0,0)`, X11 too (`gtkatspiutils.c` main:576-581; commits cb3045bf, c2fda63b) | border box relative to the window's border box, no shadow, header bar included | in the widget parent's content box |
| GTK 3 (ATK) | X11: root coordinates, toplevel includes the WM frame (`gtkwindowaccessible.c`:377-424). Wayland: relative to the surface including the shadow | relative to its own GdkWindow toplevel; a menu counts as one | **not implemented, returns `SCREEN`** |
| Qt ≥ 6.5 | `rect()`: X11 client area; Wayland pinned to the output's top-left since 6.4 (`qwaylandwindow.cpp` 495-501) | relative to the nearest Window/Dialog role, **an MDI subwindow counts** | relative to the parent, `SCREEN` if the parent has no rect |
| Qt < 6.5, 5.15 | as above (5.15 vanilla: not pinned) | width/height inflated, y bug in vanilla 5.15 | handled as `WINDOW` |
| AccessKit 0.24 (egui 0.37) | client-area origin plus `c`. Wayland: origin `(0,0)` | relative to the client area | relative to the nearest parent with a box, `WINDOW` if it has none (`accesskit_atspi_common` `util.rs:43-56`) |
| Chromium | DIPs, frame origin plus `c`. Wayland: origin `(0,0)` | relative to the nearest frame | consistent with the parent's `SCREEN` |
| Firefox | logical px, screen | relative to the base window | **not implemented, returns `SCREEN`** |

Measured on 2026-11-10 with GTK 3.24.52, GTK 4.24.1, Qt 5.15.19, Qt 6.11.2 and AccessKit 0.24, under the PlatynUI compositor and under X11 with and without icewm, with a real click on a nested button (the egui probe has no click observable, so egui was compared by coordinates):

| Toolkit | today | window origin + `WINDOW` | window origin + `SCREEN` |
|---|---|---|---|
| GTK 4 | ✅ | ✅ | ❌ |
| GTK 3 (with and without header bar) | ❌ | ✅ | ✅ |
| Qt 6.11 MDI | ✅ | ❌ | ✅ |
| Qt 5.15 MDI | ❌ | ❌ | ✅ |
| egui 0.37, compositor | ❌ | ✅ | ✅ |

**GTK 4 popovers** (measured on 2026-11-16 with GTK 4.24.1 and the `add-gtk-test-app` fixture, on both lanes).

- A popover hangs in the tree under the widget that opened it, not under the application: a menu-bar menu under its menu-bar item, a nested submenu under the item that opens it, a drop-down list under its combo box. A context menu is one popover; its submenus slide inside it. Nested popovers come from menu bars.
- It reports the popup layer (`Component.GetLayer`). In the fixture, the popovers are the only nodes that do, besides the windows, which report the window layer. Its role is `Menu` for a popover menu and `ListBox` for a drop-down.
- Its size equals its platform popup surface:
  - **X11:** an override-redirect window of type `_NET_WM_WINDOW_TYPE_MENU` carrying the process's `_NET_WM_PID`, of exactly the node's size (165×108 and 162×82).
  - **Compositor:** the xdg_popup surface includes GTK's shadow, which the rectangle `list_popups` reports excludes. A 207×156 node had a 161×110 rectangle, a 204×128 node a 158×82 one, with an inset of `(23,21)`. The popover's first child, its content, does not match either: 158×78 against 158×82 for the nested submenu.
- Its position is wrong, in `WINDOW` and `PARENT` alike, because GTK measures it from the widget that opened it without its own placement. A nested submenu's first item was reported at `(130,58)` and drawn at `(234,144)`. First-level popovers were off by a few pixels on X11 and by about 20 under the compositor.
- Positions *inside* a popover are right. The popup's origin plus (item `WINDOW` − popover `WINDOW`) clicked both items of the nested submenu on both lanes.

The probe scripts and logs are not in the repository. Each measurement is repeated as an acceptance scenario of this change.

## Goals / Non-Goals

**Goals:**

- One rule for placing a node: its window's origin plus its position relative to the window, read in a coordinate type its toolkit implements consistently.
- The platform states what it knows about a window, and the provider decides what that means for a toolkit.
- X11 sessions without a window manager behave like managed ones wherever the X server can answer.

**Non-Goals:**

- Pixel units and scaling (`verify-display-scaling`).
- Non-EWMH reparenting window managers. Their root children are frames, so the lookup without a window manager finds nothing there, and the existing substitution path applies.
- Generic Wayland compositors without global positions.
- Toolkit bugs that make a coordinate type wrong for a subset of nodes:
  - Qt < 6.5 popups the compositor moved;
  - offscreen GTK 3 widgets.

  They are listed under Risks and stay toolkit limitations.
- The log records of the X11 window manager check and of a failed lookup (`x11-atspi-healthy-run-warnings`).

## Decisions

### 1. A node is placed relative to its window, never by summing its ancestors

A node's *reference window* is its nearest ancestor that is a real top-level window or a popup (decision 5). A real top-level keeps today's predicate (`node.rs:284-295`). Its bounds are the reference window's origin plus its own offset inside that window. The offset comes from one `GetExtents` call on the node itself, plus the reference window's own extents, which are cached on the window node. The parent-chain step (`resolve_extents_via_parent_chain`) goes.

- *Why:* every failure in the proposal comes from the chain. Toolkits that answer `PARENT` with `SCREEN` multiply positions, and a container without a box ends the chain. The window is the only reference all toolkits agree on, and it is what Orca uses (`ax_component.py`, `WINDOW` throughout) and what the AT-SPI successor drafts keep (`devel-docs/new-protocol.rst`:66-72).
- *Rejected:*
  - **Keep `PARENT` and skip nodes without a box.** That fixes egui only. GTK 3, Qt < 6.5 and Firefox stay wrong.
  - **`WINDOW` everywhere.** Qt measures from the MDI subwindow, Qt < 6.5 inflates sizes, and GTK 3 measures menu items from the popup.
  - **`SCREEN` everywhere.** GTK 4 sends none.

### 2. `SCREEN` by default, `WINDOW` where screen positions carry nothing

The offset is read from `SCREEN`, except in two cases.

- **The toolkit hint names GTK 4 or later** (`gtk` with major ≥ 4). The node is read from `WINDOW` directly, saving the `SCREEN` call.
- **A node's `SCREEN` position equals its reference window's `SCREEN` position.** The node is then read once more in `WINDOW`. If that is non-zero, it wins. This is the guard for toolkits the table does not name. It costs a second call only for nodes at their window's origin, which are rare.

Further rules:

- A rectangle with zero or negative size, or with a coordinate of `i32::MIN` (GTK 3 offscreen widgets report `G_MININT`, measured as `-2147483648`), is "no geometry". Such a node has no bounds of its own, and its descendants are unaffected (decision 1).
- `GetPosition`/`GetSize` stay unused. AccessKit does not implement them (`accesskit_unix` `interfaces/component.rs:27-76`).

*Rejected:*

- **Choose per application by calibrating one parent/child pair.** It needs a pair that happens to be offset, and caches a guess for the application's life.
- **A full toolkit table.** It drifts with every release. The table holds only the toolkit whose behavior is a stated design (GTK 4 "gave up" on screen coordinates, c2fda63b), and the guard catches the rest.

### 3. The platform says whether toolkit screen coordinates are desktop coordinates

`WindowManager` gains a property that answers whether toolkit-reported screen coordinates are desktop coordinates. X11 answers yes, the PlatynUI compositor no. The default is yes, which also covers the mock and the stub.

- **Yes:** a `SCREEN`-read node's bounds are its `SCREEN` extents, unchanged.
- **No:** they are the reference window's origin plus (node `SCREEN` − window `SCREEN`).
- A `WINDOW`-read node is placed at origin plus `WINDOW` on both.

*Why yes means "unchanged" and not "re-based too":*

- GTK 3 on X11 reports its toplevel's `SCREEN` from `gdk_window_get_frame_extents`, which includes the window manager's frame.
- Subtracting that and adding the client origin would shift every element by the title bar.
- On X11, `SCREEN` is already the answer.

*Why the subtraction on Wayland:* Qt pins every toplevel to its output's top-left (`qwaylandwindow.cpp` 495-501). That point is `(0,0)` on the lanes, but not on a second output. Subtracting the window's own `SCREEN` makes the offset correct for both.

*Rejected:*

- **Let the provider detect Wayland itself.** The provider stays free of windowing knowledge, a rule `java-provider-linux` also relies on (its design, decision 1, option b).

### 4. A window's visible geometry and its surface origin are two facts

`WindowManager::bounds(id, toolkit_hint)` becomes `geometry(id)` and returns two values:

- `bounds`: the visible window, which the top-level node reports as its `Bounds`;
- `surface_origin`: the desktop position of the window surface's `(0,0)`.

The provider picks the origin for a toolkit. GTK 4 counts from its border box, so its elements use `bounds.origin`. Every other toolkit uses `surface_origin`. The toolkit hint leaves the trait.

- **PlatynUI compositor:**
  - `bounds` is `content + geometry offset` with `content_width/height`, for every `csd` window, not only GTK 4. The existing opaque-region refinement stays (`platynui_ipc.rs:552-572`).
  - `surface_origin` is `content_x/y`.
  - `window_at_point` (`:212-240`) uses the same `bounds`.
- **X11:**
  - `surface_origin` is the client origin.
  - `bounds` is the client rectangle minus `_GTK_FRAME_EXTENTS` when the window has that property. GTK sets it for client-side shadows under a compositing window manager (source). Otherwise it is the client rectangle.
- **JAB on Windows:** it uses `geometry(id).bounds`. Its behavior is unchanged, because Win32 reports no separate surface origin and the default sets `surface_origin = bounds.origin`.
- One call instead of two keeps one `list_windows` round trip per window resolution on Wayland.

*Rejected:*

- **Keep the hint and correct inside the platform.** It splits toolkit knowledge across crates and already produced the GTK 3 mismatch: the shadow correction is GTK 4-only, but the GTK 3 position includes the shadow.

### 5. Popups and substitutes are reference windows like any other

A *popup* is a node the platform shows as a surface of its own. Two kinds are recognized:

- a grafted transient popup, by today's predicate (`node.rs:284-295`): Qt's context menus;
- a node that reports the popup layer (`Component.GetLayer`): GTK 4's popovers, which stay in the tree under the widget that opened them.

A popup is its descendants' reference window. Its geometry comes from the platform's popup list for its process, `WindowManager::popups(pid)`. The list gives each popup's visible rectangle and its surface rectangle, the two facts decision 4 gives for a window, most recently opened or topmost first.

- **PlatynUI compositor:** `list_popups` adds the popup surface's origin and size to the placed rectangle it reports today. The difference is the shadow GTK draws around a popover. Qt popups have none, so both rectangles are the same.
- **X11:** the viewable override-redirect windows of a popup type (menu, drop-down, combo, tooltip) whose `_NET_WM_PID` is the process's, topmost first. They are the windows the hit-test's popup pass already finds (`window_manager.rs:586-623`), and they are root children with and without a window manager. The window is the surface. The visible rectangle subtracts `_GTK_FRAME_EXTENTS`, as for a top-level (decision 4). A popup window without `_NET_WM_PID` cannot be attributed to a process and is not listed.

Rules:

- A popup node is matched by its own size against the surface sizes, within 2 px as today. On a tie, the first one listed wins.
- A matched popup's own `Bounds` are the visible rectangle. Its descendants are placed at the surface origin plus their offset from the popup node, read in their toolkit's coordinate type (decision 2).
- When nothing matches, the node is placed like any other node of its window, as today. This covers a toolkit whose popup layer marks no surface of its own and a platform without the query. On X11 it keeps Qt's context menus at their `SCREEN` extents when their window carries no `_NET_WM_PID`, and those are right on X11.
- **Hit-test:** a hit inside a popup wins over every hit outside it, and a hit inside a popup wins over one in the popup it was opened from. Grafted popups already win this way (`lib.rs:533-577`). The smallest box alone is not enough, because a label beneath a menu item can be smaller than the item.

When the window manager cannot answer for a top-level, the substitute (the window's `SCREEN` extents, logged once as today, `extents.rs:116-150`) is the reference window's bounds and origin. Re-basing then gives every element its raw `SCREEN` value, which is what the substitution requirement promises: a best effort, covered by the window's warning.

- *Why the surface size:* the popover node is the surface. The visible rectangle excludes GTK's shadow, and the popover's content node matches neither rectangle (Context).
- *Rejected:*
  - **Recognize popovers by role.** GTK 4 gives them `Menu` and `ListBox`, and other popovers carry other roles.
  - **Pair popovers with the platform's popups by order.** A tooltip is a popup too and shifts the pairing.
  - **Correct GTK's offsets.** For a nested popover they miss its whole placement, about 100 px measured. Nothing in the tree tells how far.

### 6. X11 decides its window mode once per runtime

`check_ewmh_wm_support` returns its finding to the bundle factory. The factory builds `X11EwmhWindowManager` in *managed* or *bare* mode (`lib.rs:106-110`). No usable EWMH window manager means bare. `x11-atspi-healthy-run-warnings` changes how that finding is logged; this change only consumes its result. Whichever change lands second adapts the factory code.

*Rejected:*

- **Detect per call by reading `_NET_CLIENT_LIST`.** It costs a property read per lookup and can flip mid-run. The specs fix the decision per runtime.

### 7. Bare mode answers from the X server

- **Candidates:** the root's children from `query_tree` that are viewable, not override-redirect, and whose `_NET_WM_PID` is the process's. Toolkits set `_NET_WM_PID` themselves, and a window without it is not a candidate.
- **`is_active`:**
  - `get_input_focus`, walked up with `query_tree` parents to the root child, compared with the window.
  - `PointerRoot`, `None` and the root itself are not active.
- **`activate`:** `map_window` if unmapped, then `configure_window(stack_mode = Above)` and `set_input_focus(window, RevertTo::Parent)`. A window mapped just now is waited for until it is viewable, bounded by the activation's existing timeout, before the focus call. That focus call fails with `BadMatch` on an unviewable window.
- **`window_at_point`:** root children from `query_tree`, top to bottom, with the same filter as the candidates, the same self-skip as `managed_window_at` (`:654-677`), and the existing popup pass (`:586-623`), which never needed a window manager.
- **`close`:** `WM_DELETE_WINDOW` as a `WM_PROTOCOLS` client message to the window when its `WM_PROTOCOLS` lists it. Otherwise an error naming the protocol.
- **`minimize`, `maximize`, `restore`:** `CapabilityUnavailable` naming the operation, with details that no window manager runs on the display.
- **`state`:** a mapped window is *normal*, not topmost.
- **`move_to`, `resize`:** unchanged.

The candidate list without a window manager is also what `java-provider-linux`'s planned window enumeration needs on such displays. That change reuses it rather than reading `_NET_CLIENT_LIST` alone.

### 8. X11 disambiguation no longer needs a screen position

When a process has several candidates, the lookup chooses in this order:

1. the exact title (`_NET_WM_NAME`, then `WM_NAME`) against the window node's name;
2. a size only one candidate has;
3. among equal sizes, the position, but only when the node's `SCREEN` position is not `(0,0)`;
4. otherwise an error naming the count.

This mirrors the Wayland matcher (`platynui_ipc.rs:326-386`). A Qt dialog whose accessible name differs from its title, the case that put geometry first (`window_manager.rs:158-169`), still resolves by size, and equal sizes still resolve by position on X11.

### 9. The lanes and fixtures

- **X11 without a window manager:**
  - `scripts/startxsession.sh` gets a `--no-window-manager` option that skips `icewm-session` (`:284`, `:295`).
  - A profile `real-x11-bare` in `robot.toml` uses it, with a recipe `just test-acceptance-x11-bare`. It is part of `just test-acceptance` on Linux and of the CI matrix, which already installs Xvfb.
  - Two tags select suites. `window-manager:required` marks tests that need minimize, maximize or restore; `real-x11-bare` excludes them. `window-manager:none` marks the suite for the bare behavior; every other profile excludes it.
- **Qt MDI:** `apps/test-app-qt` gets an MDI scenario beside its catalog, as `testing-strategy.md` §5 allows for scenario surfaces: a `--mdi` flag that puts the catalog page into a `QMdiSubWindow` offset inside the main window. It gets its own suite.
- **GTK 3 and GTK 4:** the fixtures come from the separate change `add-gtk-test-app`, which is applied first. Its GTK 3 catalog's pointer tests and its GTK 4 popover tests fail until decisions 1–5 are in, and they are committed with them. This change needs four things from the fixtures:
  - a main window with a header bar;
  - the blueprint's group with its radio buttons;
  - a modeless dialog of a different size from the main window;
  - in GTK 4, a menu bar with a nested submenu and a drop-down, the blueprint's menu and combo box.

## Risks / Trade-offs

- **[GTK 4 offsets drift by CSS border and padding]** The source suggests that `WINDOW` sums border boxes while the transform targets content boxes (research, `gtkwidget.c`:4279-4288). The measured button click landed, but a small control inside a padded parent could be off by a few pixels. → A task measures a label inside a padded button. A drift becomes a documented toolkit limitation, not a correction in PlatynUI.
- **[`_GTK_FRAME_EXTENTS` on X11 is not measured]** The lanes run icewm without compositing, so GTK draws no shadow there. → It is implemented from the GTK source, unit-tested on a recorded property, and named as unverified in the platform doc until a compositing X11 run confirms it.
- **[Qt with client-side decorations]** Qt's coordinates start inside its own decorations (qtwayland 6.8, comment at 918-923). The compositor reports `ssd` for Qt on the lanes (measured), so the surface origin is right there. Under a compositor that forces client-side decorations, Qt elements would be off by the title bar. → Out of scope (generic compositors). The Wayland doc states the assumption.
- **[Toolkit limitations stay wrong]** Qt < 6.5 popups that the compositor moved, and Java's ATK wrapper (`-1` for unowned frames). → They are listed in `platform-linux-wayland.md` as limitations. The default-plus-guard rule keeps the rest right.
- **[A popover's surface size under the compositor is inferred]** On X11 the popover node and its window are the same size (measured). Under the compositor only the visible rectangle was measured, and the surface size follows from it and GTK's shadow inset. → The compositor task asserts it on a GTK 4 popover before the provider relies on it. If it does not hold, the match falls back to today's path and the GTK 4 popover tests show it.
- **[GTK 4 before 4.18 is not measured]** Before 4.18 GTK also misses the popover's offset from its parent (GTK commit 3b68c1ea). The new rule does not need that offset, but whether those versions report the popup layer is unknown. CI is not affected, because its runner is Ubuntu 26.04 with GTK 4.22, but developers on Ubuntu 24.04 run GTK 4.14. → `add-gtk-test-app`'s `requires:gtk-4.18` tag stays until an older GTK is measured (Open Questions).
- **[Popup windows without `_NET_WM_PID` on X11]** Some toolkits leave the property off menus (comment at `window_manager.rs:585`). → They are not listed and keep the existing path. For Qt that path is correct on X11.
- **[Activation without a window manager races the map]** → It waits until the window is viewable (decision 7). The acceptance suite activates a window that was unmapped.
- **[Merge with `x11-atspi-healthy-run-warnings`]** Both changes edit `find_xid_for_pid` and the bundle factory. → The second to land rebases. That change's debug records for a failed lookup also apply to the bare candidate list.
- **[`add-avalonia-test-app` popups on X11]** A `PopupRoot` that is a top-level `Frame` but not a managed window still takes the main window's bounds when the main window is the process's only candidate. Its items are now placed by their own `SCREEN` extents on X11 (decision 3), so their clicks land even then. → That change's risk entry is updated when it is applied.
- **[Behavior change for GTK 3 windows under the compositor]** The window's `Bounds` move by the shadow inset. → Release notes (proposal).

## Migration Plan

- **Behavioral, not additive.** Element bounds change for the toolkits listed in the proposal, GTK 3 windows' own bounds change under the compositor, and X11 without a window manager gains behavior. No keyword, attribute name or configuration key changes.
- **Native rebuild:** yes. `just build-native` after the provider and platform crates change. No Python change.
- **Order:**
  1. the trait and both Linux platforms with their unit tests, including the popup list;
  2. the provider composition with its unit tests, with the GTK 3 catalog's pointer tests as its first acceptance tests;
  3. the lanes: egui compositor, Qt MDI, GTK, X11 bare.
- **Rollback:** revert the change's commits. No state, file format or setting depends on them.

## Open Questions

- **How large is the GTK 4 drift (first risk)?** Measuring it decides only whether the platform doc lists a limitation. The rule stays the same.
- **Do GTK 4 versions before 4.18 report the popup layer for popovers?** If they do, decision 5 places their popovers too, and `add-gtk-test-app` can drop `requires:gtk-4.18`. Measuring needs a GTK 4.14 environment, such as an Ubuntu 24.04 container. It changes no decision of this change.
