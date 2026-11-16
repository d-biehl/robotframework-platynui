# Tasks

## 1. Window-manager contract

- [ ] 1.1 Write the core unit tests first (`crates/core/src/platform/window_manager.rs` tests): the default `geometry` of a window manager without a separate surface origin has `surface_origin == bounds.origin`, the default for "toolkit screen coordinates are desktop coordinates" is `true`, and the default popup query stays unavailable while a popup record carries a visible and a surface rectangle. Verify they fail to compile against the old trait.
- [ ] 1.2 Replace `WindowManager::bounds(id, toolkit_hint)` with `geometry(id)` returning the visible bounds and the surface origin, add the screen-coordinates property with its default, and change `popups(pid)` to return each popup's visible and surface rectangle (design decisions 3, 4 and 5). Update the core stub and the platform mock window manager. Verify `just test-crate platynui-core` and `just test-crate platynui-platform-mock` pass.
- [ ] 1.3 Move the JAB provider (`crates/provider-java-jab/src/node.rs:1037`) and the Wayland facade (`crates/platform-linux-wayland/src/window_manager/mod.rs:51`) to `geometry(id).bounds`. Verify `just clippy-windows` and `just check` pass.

## 2. PlatynUI compositor geometry

- [ ] 2.1 Add unit tests in `platynui_ipc.rs` built from the recorded `list_windows` records in design.md (GTK 3 and GTK 4 with header bar, `csd`; Qt and egui, `ssd`). The visible bounds are `content + geometry offset` with `content_width/height` for every `csd` window. The surface origin is `content_x/y`. `window_at_point` returns the same visible bounds. Toolkit screen coordinates are not desktop coordinates. Verify the tests fail first.
- [ ] 2.2 Implement `geometry` and the property for the IPC backend: drop the GTK 4 gate from `csd_shadow_offset` and keep the opaque-region refinement. Verify `just test-crate platynui-platform-linux-wayland` passes.
- [ ] 2.3 Write the popup tests first:
  - in `apps/wayland-compositor/src/control.rs`, a listed popup carries its surface origin and size beside its placed rectangle (specs *A popup with a client-side shadow lists its surface around its placed rectangle* and *A popup without a shadow has one rectangle*);
  - in `platynui_ipc.rs`, `popups` decodes both rectangles from a recorded `list_popups` response.

  Verify they fail first.
- [ ] 2.4 Implement the surface in `list_popups`: the popup's buffer origin (its placed location minus its own window-geometry offset) and its buffer size. Decode it in `popups`. Then open the GTK 4 fixture's nested submenu under the compositor and compare the listed surface size with the popover node's size. Record the result for 5.3, because it is the assumption in design.md, Risks. Verify `just test-crate platynui-wayland-compositor` and `just test-crate platynui-platform-linux-wayland` pass.

## 3. X11 geometry and window choice

- [ ] 3.1 Add unit tests first. `_GTK_FRAME_EXTENTS` is subtracted from a client rectangle when present and ignored when absent. The candidate choice runs title, then unique size, then position only when the node's screen position is not `(0,0)`, otherwise an error naming the count. This covers the spec scenarios *Two GTK 4 windows of one process are resolved by size* (as data) and *Two indistinguishable windows fail the lookup*. Factor the choice into a pure function over candidate data so it is testable without an X server. Also write the acceptance test `tests/acceptance/gtk4/geometry.robot` *Each Window Of Two Has Its Own Bounds*: with the GTK 4 fixture's modeless dialog open, each window's `Bounds` are its own; it carries no window-manager tag, so the bare lane runs it too (spec *Two GTK 4 windows of one process are resolved by size*).
- [ ] 3.2 Implement `geometry` (client origin as surface origin, visible bounds without `_GTK_FRAME_EXTENTS`), the property (`true`) and the new candidate order (design decision 8). Remove `extract_screen_extents`'s role as the primary key. Verify `just test-crate platynui-platform-linux-x11` passes, the GTK 4 two-window test passes, and the X11 lane's egui, Qt and GTK 4 suites stay green: `just headless=true test-acceptance-x11 -X --suite '*.Egui' --suite '*.Qt' --suite '*.Gtk4'`.
- [ ] 3.3 Write unit tests first for the X11 popup list as a pure function over recorded window attributes. It takes viewable override-redirect windows of a popup type carrying the process's `_NET_WM_PID`, topmost first, and subtracts `_GTK_FRAME_EXTENTS` for the visible rectangle (specs *Popups of other processes and windows without a process are not listed* and *A popup window with frame extents has a smaller visible rectangle*). Verify they fail first.
- [ ] 3.4 Implement `popups(pid)` in `X11EwmhWindowManager` with the popup pass's filter (`window_manager.rs:586-623`), in both window modes (design decision 5). Verify `just test-crate platynui-platform-linux-x11` passes.

## 4. X11 without a window manager

- [ ] 4.1 Lane plumbing:
  - `--no-window-manager` in `scripts/startxsession.sh`, which skips `icewm-session` in both branches;
  - profile `real-x11-bare` in `robot.toml` with that wrapper, excluding `platform:wayland`, `platform:windows` and `window-manager:required`;
  - `window-manager:none` excluded from every other profile;
  - recipe `just test-acceptance-x11-bare`, added to `just test-acceptance` on Linux and to the CI acceptance matrix.

  Verify `uv run --no-sync robotcode --profile real-x11-bare discover tests` lists the egui suites, and `xprop -root _NET_SUPPORTING_WM_CHECK` in the session reports no property.
- [ ] 4.2 Write the acceptance suite `tests/acceptance/egui/without_window_manager.robot` (tags `platform:x11`, `window-manager:none`) from the `x11-window-management` scenarios: window found with X server bounds and no substitution warning, activation of an overlapped window with `@IsActive` and keyboard input, hit-test in the overlap, close, minimize failing by name, state attributes `False`, move and resize. Follow the `robot-test-style` skill. Verify it fails on the bare lane before the implementation.
- [ ] 4.3 Add unit tests first for the pure parts:
  - the mode decision from the window-manager check (spec *A runtime created after a window manager started uses it*);
  - the candidate filter over window attributes (viewable, not override-redirect, matching `_NET_WM_PID`; spec *A popup is not taken for the application's window*);
  - the focus walk (focus on a descendant, `PointerRoot`, `None`, root);
  - the `WM_PROTOCOLS` check for `WM_DELETE_WINDOW`.
- [ ] 4.4 Implement bare mode in `crates/platform-linux-x11` (design decisions 6 and 7):
  - the factory passes the check's result to `X11EwmhWindowManager`;
  - candidates come from `query_tree`;
  - `is_active` comes from the input focus;
  - `activate` maps the window, waits until it is viewable, raises it and sets the focus;
  - `window_at_point` uses the root's stacking order plus the existing popup pass;
  - `close` sends `WM_DELETE_WINDOW`;
  - minimize, maximize and restore are `CapabilityUnavailable`;
  - the state is normal.

  Verify `just test-crate platynui-platform-linux-x11` and the suite from 4.2 pass: `just headless=true test-acceptance-x11-bare -X --suite '*.Egui.WithoutWindowManager'`.
- [ ] 4.5 Run the whole bare lane: `just headless=true test-acceptance-x11-bare -X`. Tag `window-manager:required` only the tests that use minimize, maximize or restore (today `tests/acceptance/egui/window_activation.robot`). Fix every other failure in the implementation. Verify the lane passes and the managed X11 lane still passes, except the GTK 3 catalog's pointer tests, which pass only after group 5.
- [ ] 4.6 Docs:
  - `dev-docs/platform-linux.md` §3: the two modes, the candidate rules, what bare mode can and cannot do, and the non-EWMH window managers that stay out;
  - `dev-docs/testing-strategy.md` §2.6: the new tags and the bare profile;
  - `CONTRIBUTING.md`: the new recipe;
  - `AGENTS.md` only if its command list names the lane recipes.

  Verify the documented recipe runs as written.

## 5. Element placement in the AT-SPI provider

- [ ] 5.1 Write the acceptance side first.
  - Confirm that the fixtures of `add-gtk-test-app` meet design decision 9: a main window with a header bar, `groupbox-basic` with its radio buttons, and a modeless dialog of a different size.
  - Add the GTK 3 catalog's pointer tests and the GTK 4 catalog's popover tests (menu bar, submenu, context menu, drop-down), which that change keeps out of its commits. Record which of them fail by missing their target. Measured in `add-gtk-test-app` 3.5 on GTK 4.24.1 with today's provider:
    - X11: only the menu-bar submenu item misses.
    - Compositor: the menu-bar submenu item, `ctx-copy`, and `ctx-more`, which then does not open its submenu.
    - Both lanes: `menu-file-new`, the first level of `menu-edit` and the drop-down's row land by chance.
  - Add to each GTK fixture's `geometry.robot`, which GTK 4 has since 3.1 and GTK 3 gets here:
    - `radio-first` inside `groupbox-basic` clicked by locator, on both lanes (spec *A click on a nested button lands on it*);
    - on the compositor lane (tag `platform:wayland`), the main window's `Bounds` equal the visible geometry `platynui-wayland-compositor-ctl list-windows` reports for it (specs *A GTK 3 window with a header bar reports its visible frame* and *A GTK 4 window with a header bar places its elements from its visible frame*);
    - in GTK 4 only (`requires:gtk-4.18`), with the submenu `menu-edit-more` open, `BM.Get Element At Point` at the center of a submenu item's `Bounds` returns that item or a descendant (spec *A point inside a GTK 4 submenu resolves to its item*).

  Verify that the GTK 3 tests and the GTK 4 submenu tests fail on both lanes.
- [ ] 5.2 Rework the unit tests in `crates/provider-atspi/src/extents.rs` first, against a reshaped `ExtentSources` test double that answers per node: its reference window's bounds, surface origin and `SCREEN` extents, its own `SCREEN` and `WINDOW` extents, the toolkit hint and the platform's screen-coordinates property. One test per `atspi-element-geometry` scenario that names a unit test:
  - GTK 4 read in `WINDOW`;
  - GTK 3 not summed;
  - an unknown toolkit without screen positions;
  - consistent screen coordinates kept;
  - re-basing a window pinned to `(1920,0)`;
  - X11 screen extents used as they are;
  - a node below a container without geometry;
  - `G_MININT` and zero-size rectangles as no geometry.

  Also the modified *Geometry that never came from the window manager is unchanged* scenario: a control placed from the substitute's origin, with no warning of its own. Also the popup scenarios of *A popup is placed where the platform shows it* and *A popup is matched to the platform's popup by its surface*: a popup-layer node matched by its surface, equal sizes resolved to the first listed popup, and a popup without a match placed in its window. The test double then also answers the node's layer and the platform's popup list. Keep the existing substitution-warning tests. Verify the new tests fail first.
- [ ] 5.3 Implement the composition (design decisions 1, 2, 3 and 5):
  - the reference window lookup over cached parents;
  - the coordinate-type choice with the GTK 4 entry and the equal-position guard;
  - re-basing by the platform property;
  - the origin choice (`bounds.origin` for GTK 4, `surface_origin` otherwise);
  - popups as reference windows: grafted ones and nodes on the popup layer, matched by surface size against `popups(pid)`, with their own bounds from the visible rectangle and their descendants from the surface origin;
  - substitutes as reference windows;
  - in the hit-test (`lib.rs`), a hit inside a popup wins over hits outside it, and a later popup over the one it was opened from;
  - `resolve_extents_via_parent_chain` removed;
  - the toolkit hint read in the provider instead of passed to the platform.

  Verify `just test-crate platynui-provider-atspi` passes.
- [ ] 5.4 Rebuild the native extension with `just build-native` and run both Linux lanes: `just headless=true test-acceptance-compositor -X`, then `just headless=true test-acceptance-x11 -X`. Verify that:
  - the egui suites (`Interaction`, `Auto Activate`, `Hit Test`) pass on the compositor lane, which motivated the change;
  - the Qt and QML suites pass, including `context_menu.robot` (spec *Items of a grafted popup are placed relative to the popup*);
  - the GTK 3 and GTK 4 catalogs and the tests from 5.1 pass on both lanes, including the GTK 4 popover tests.

  The GTK 3 pointer tests and the GTK 4 popover tests are committed with this task's code.
- [ ] 5.5 Add an X11-only test to `tests/acceptance/qt/bounds.robot` (tag `platform:x11`): a button's `Bounds` equal its `native:Component.Extents.Screen` (spec *On X11 screen extents are the bounds*). Verify it passes with `just headless=true test-acceptance-x11 -X --suite '*.Qt.Bounds'`.
- [ ] 5.6 Measure the GTK 4 drift (design, Open Questions): a label inside a padded button, its `Bounds` against where it is drawn in a screenshot, on the GTK 4 fixture. Verify that the result is recorded for 5.7, as a measured offset or as none found.
- [ ] 5.7 Docs:
  - `dev-docs/platform-linux.md` §2: how a node is placed, with the toolkit table reduced to what each toolkit's coordinate types mean;
  - `dev-docs/platform-linux-wayland.md`:
    - replace the `GetExtents(WINDOW)`-only intent (§ on AT-SPI coordinates and the open-question item 9) with the per-toolkit rule and the surface-origin/visible-geometry split;
    - how popups are placed: the popup layer, the platform's popup list with its visible and surface rectangles, and the match by surface size;
    - list the toolkit limitations (Qt < 6.5 moved popups, Qt with client-side decorations, Java ATK wrapper, and the GTK 4 drift from 5.6) and that `_GTK_FRAME_EXTENTS` is unverified on X11;
  - `dev-docs/architecture.md` §10: the provider snapshot.

  Verify no doc still describes the parent-chain sum: `grep -rn "CoordType::Parent\|parent chain" dev-docs` finds no description of it.

## 6. Qt MDI scenario

- [ ] 6.1 Write `tests/acceptance/qt/mdi.robot` first. It starts the Qt Widgets fixture with `--mdi` and clicks a button inside a group in the MDI subwindow by locator, expecting the fixture's action observable to report it (specs *An element inside a Qt MDI subwindow is placed by screen coordinates* and *A click on a nested button lands on it*). The fixture predates the blueprint, so the suite uses its own identifiers. Verify it fails because the flag does not exist.
- [ ] 6.2 Add `--mdi` to `apps/test-app-qt`: the fixture's central page inside a `QMdiSubWindow`, offset inside the main window, as a scenario surface beside the existing ones. Document the flag in `apps/test-app-qt/README.md` and in `testing-strategy.md` §5. Verify the suite passes on both lanes: `just headless=true test-acceptance-x11 -X --suite '*.Qt.Mdi'` and the same with `test-acceptance-compositor`.

## 7. Integration

- [ ] 7.1 Run the full local gate and every Linux lane:
  - `just check`, `just test`, `just test-python`;
  - `just headless=true test-acceptance-compositor -X`;
  - `just headless=true test-acceptance-x11 -X`;
  - `just headless=true test-acceptance-x11-bare -X`.

  Verify all pass. Judge lanes with `robotcode results`, not the exit code.
- [ ] 7.2 Run the Windows lane on real Windows (`just test-acceptance-windows`), because the JAB provider moved to `geometry`. Verify it passes; a Wine run does not count.

## Workflow follow-up

- Archive after `add-gtk-test-app` task 4.5 has passed.
- `add-gtk-test-app` keeps its GTK 4 popover tests out of its commits while they fail, as it does with the GTK 3 pointer tests. Its reason for `requires:gtk-4.18` becomes the unmeasured popup layer of older GTK 4 (design.md, Risks and Open Questions).
- When `add-avalonia-test-app` is applied, update its risk entry on X11 popups (design.md, Risks).
