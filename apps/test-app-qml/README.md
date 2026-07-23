# PlatynUI QML test app (PySide6 / Qt Quick)

The **Qt Quick row** of the fixture technology matrix
([`dev-docs/testing-strategy.md`](../../dev-docs/testing-strategy.md) §5) and the
**first full instance of the fixture blueprint** (§5.1). Unlike the Qt Widgets
fixture ([`apps/test-app-qt`](../test-app-qt)), Qt Quick renders its own scene
graph: accessibility comes from QML `Accessible` attached properties routed
through Qt's bridge (UIA on Windows, AT-SPI on Linux), and menus/popups are by
default **in-scene items**, not native windows.

It is **Python**, not a Cargo crate (`exclude`d from the workspace in the root
`Cargo.toml`). PySide6 is a development dependency of the main project, so a
normal `uv sync` installs it; `main.py` stays thin (CLI, tree model, engine
bootstrap) and the catalog lives in `Main.qml` — the structure real QML apps
have.

## Run

```sh
# On the project venv (PySide6 is a dev dependency, installed by `uv sync`):
uv run python apps/test-app-qml/main.py

# Standalone (no project sync): PEP 723 inline metadata auto-installs PySide6:
uv run apps/test-app-qml/main.py

# Options:
uv run apps/test-app-qml/main.py --app-id com.platynui.test.qml --title "My QML Window"
uv run apps/test-app-qml/main.py --auto-close 10        # self-close for CI
uv run apps/test-app-qml/main.py --open-modal           # open the in-scene modal dialog at startup
uv run apps/test-app-qml/main.py --popup-mode native    # menus/popups as native windows (Qt >= 6.8)
uv run apps/test-app-qml/main.py --log-level info       # error|warn|info|debug
```

## Catalog

The blueprint core tier under the canonical names (§5.1): `button-basic` +
`status-label-clicks-<n>` counter, the `last-action-<ident>` report label
(`last-action-none` until the first activation), `checkbox-basic`,
`groupbox-basic` (grouping `radio-first`/`radio-second`), `textfield-basic`,
`textarea-basic` (multi-line), `label-basic`, `text-basic` (plain `Text`),
`image-basic` (`Image` from `icon.png`), `combobox-basic` (+`combo-item-1..3`),
`list-basic` (+`list-item-1..5`), `tree-basic` (three levels, `tree-node-*`),
`main-menubar` with `menu-file`, `menu-edit` (+submenu `menu-edit-more` with
`menu-edit-sub-*`), and `menu-help`, `context-menu` (+`ctx-*`, submenu
`ctx-more`), `dialog-modeless` (a **native child `Window`**) and `dialog-modal`
(an **in-scene modal `Dialog`**, opened via `--open-modal`) — each dialog with
`<ident>-label` and `<ident>-button`. Activating any menu item or dialog button
reports its ident through the always-visible last-action label; no control ever
changes its own name. The extended tier (table, slider/progress, tabs) is a
follow-up.

`--popup-mode` switches every menu between Qt Quick's default in-scene popups
(`Popup.Item`) and native popup windows (`Popup.Window`, Qt >= 6.8). Names are
identical in both modes.

**Theming:** the app pins a Controls style that follows the SYSTEM light/dark
theme (`FluentWinUI3` on Windows, `Fusion` elsewhere; an explicit
`QT_QUICK_CONTROLS_STYLE` env var wins) — without a style Qt Quick can mix
palettes (dark window, light menus). The plain child `Window` additionally sets
`color: palette.window`, since raw windows default to white regardless of style.

**Custom-controls chapter** (blueprint optional chapter, implemented):
`custom-button` is a self-drawn `Rectangle` + `TapHandler` with manually wired
`Accessible` properties (role Button, focusable, press action) updating
`custom-status-label-clicks-<n>`; the red "Hidden" rectangle (`customHidden` in
`Main.qml`) deliberately has **no** `Accessible` attachment and must be absent
from the accessibility tree — the negative case tests assert.

## Verified platform facts (Windows/UIA, PySide6 6.11)

Read from the real UIA tree of the running app (testing-strategy §7: verify
against reality before encoding). Linux/AT-SPI facts follow in the next section.

- All `Accessible.name` values surface as `@Name` exactly as set; names are the
  locator contract (`@AutomationId` stays empty).
- **Deviation — window naming:** a `QQuickWindow`'s `@Name` is its **title**
  (`Accessible` cannot attach to a `Window`), so the main window is matched via
  launch configuration (title), not by a `main-window` accessible name. The
  child windows title themselves `dialog-modeless`/`dialog-modal`, so their
  canonical names hold.
- **Deviation — modal state:** the in-scene `Dialog` surfaces as a nested
  `Window` node (`native:IsDialog` present) but exposes **no modal state** (no
  UIA WindowPattern). Presence + interactability are testable; modal-ness is
  not asserted on Windows.
- Roles differ from the Widgets fixture: `ListView`/`TreeView` surface as
  `Group` (named `list-basic`/`tree-basic`), tree rows as `ListItem` (not
  TreeItem), the closed `menu-file` menu as a `MenuBar` child. Locators
  therefore address catalog controls by `@Name` alone — names are unique
  app-wide by blueprint rule.
- Menu **items are not in the tree while their menu is closed** (unlike Qt
  Widgets, where the File menu's items are enumerable when closed): menu tests
  open the menu through real input first.
- **Deviation — popup container naming:** `Accessible` attaches only to
  Items/Actions, not to `Menu`/`Dialog`/`Popup` (attaching warns and does
  nothing). The context menu's popup therefore carries no `context-menu` name —
  the locator contract lives on its ITEMS (`ctx-*`). The modal `Dialog` gets its
  `@Name` from its **title** (`dialog-modal`), like the windows.
- `dialog-modeless` appears twice (top-level owned window and nested under the
  main window) — both nodes describe the same native window; name-based
  queries use the first match.

## Verified platform facts (Linux/AT-SPI on X11, PySide6 6.11)

Read from the real AT-SPI tree inside the isolated X11 session (Xephyr +
`scripts/platynui-robot-session.sh`). The name contract holds exactly as on
Windows: every `Accessible.name` surfaces as `@Name`, the main window's `@Name`
is its **title** (launch-configuration matching applies), and the child windows
(`dialog-modeless`, role `Frame`) carry their canonical names as titles.

- **In-scene popups are visible top-down** while open — no event-driven
  grafting: an open menu appears as a `PopupMenu` node (its items as
  `MenuItem`s) under the main window, the combo popup as an unnamed `Dialog`
  holding `MenuItem` children. Items disappear again when the popup closes,
  same dynamics as on UIA.
- **Deviation — modal state (inverse of Windows):** the in-scene `Dialog`
  surfaces with role `dialog` and its `Accessible.State` **does contain
  `Modal`**. There is still no common `@IsModal`-style attribute on the node,
  and acceptance suites do not assert `native:` attributes — so modal-ness
  remains unasserted in the shared suites; presence + interactability are the
  tested facts on both platforms.
- **Deviation — native popups are invisible to AT-SPI (X11 and our
  compositor):** with `--popup-mode native` the menus **do render** as native
  popup windows, but their contents never reach the accessibility tree: the
  app node keeps exactly its two `Frame` children, a desktop-wide query for an
  item name stays empty (verified with 20 s waits), and a hit-test at the open
  popup's position resolves the control *underneath* it in the main window.
  This differs from Qt Widgets, whose native `QMenu` does surface (the
  `tests/acceptance/qt` menu suites drive it on Linux). Since the suites drive
  exclusively through the accessibility tree, the native-mode suite
  (`tests/acceptance/qml/popups.robot`) is tagged `platform:windows`.
- **Escape closes one popup level, not the stack:** one Escape closes only the
  topmost in-scene popup — a parent menu left open consumes the next pointer
  click before it reaches the control underneath. The catalog flows avoid the
  problem structurally: activating an item closes its menu chain itself, and
  the last-action label makes the effect observable without reopening.
- **`@Id` is available but deliberately unwired:** Qt Quick's `Accessible.id`
  attached property (Qt ≥ 6.9) surfaces as the common `@Id` and is resolvable
  via `//*[@Id="…"]` (verified on AT-SPI); without it `@Id` stays empty — QML
  object ids are NOT exposed by default, despite the docs' wording. The catalog
  keeps `@Name` as its locator contract because the blueprint also targets
  technologies without any id channel (Swing/JAB, SWT, JavaFX). Wiring the
  canonical idents onto `Accessible.id` plus an id-locating test is a candidate
  follow-up change — real applications localize names, so `@Id` is the robust
  locator where a technology offers it.
- Roles differ from UIA but stay out of the locator contract: lists/trees are
  `Filler`, rows `ListItem`, the menu-bar menus `MenuBar` children of
  `main-menubar`. A tree row's inner text `Label` duplicates the row's name
  (`ListItem 'tree-node-a'` + child `Label 'tree-node-a'`); first-match
  name queries hit the interactive row. `textarea-basic` has an additional
  unnamed sibling `Text` accessible (bridge artifact of the scroll wrapper).

## Acceptance lane

Wired into the real-provider lane under
[`tests/acceptance/qml`](../../tests/acceptance/qml) (profile `real`, tag `real`):
`catalog.robot` carries the blueprint's canonical catalog test set (the reference
implementation — self-contained test bodies, instance pinned via `Set Root`,
canonical-name locators), plus QML-specific suites for popup modes, both dialog
faces, and the custom-controls chapter. The interpreter + entrypoint are handed
over via `PLATYNUI_TEST_APP_QML_PYTHON` / `PLATYNUI_TEST_APP_QML_MAIN` by
`scripts/platynui-robot-session.sh` (Linux) and the `test-acceptance-windows`
recipe (Windows); Robot Framework launches the interpreter directly so the
started PID is the app's PID (`@ProcessId` pinning).

`ruff` and strict `mypy` cover `main.py` in `just check` (own invocation in the
`mypy` recipe — the fixture apps' `main.py` scripts share a module name, which
one mypy run rejects as duplicates; see the note in `pyproject.toml`).
