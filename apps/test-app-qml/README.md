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
`status-label-clicks-<n>` counter, `checkbox-basic`, `groupbox-basic` (grouping
`radio-first`/`radio-second`), `textfield-basic`, `textarea-basic` (multi-line),
`label-basic`, `text-basic` (plain `Text`), `image-basic` (`Image` from
`icon.png`), `combobox-basic` (+`combo-item-1..3`), `list-basic`
(+`list-item-1..5`), `tree-basic` (three levels, `tree-node-*`), `main-menubar`
with `menu-file`, `menu-edit` (+submenu `menu-edit-more` with
`menu-edit-sub-*`), and `menu-help` (items renaming to `<ident>-activated`),
`context-menu` (+`ctx-*`, submenu `ctx-more`), `dialog-modeless` (a **native
child `Window`**) and `dialog-modal` (an **in-scene modal `Dialog`**, opened via
`--open-modal`) — each dialog with `<ident>-label` and `<ident>-button` renaming
to `<ident>-button-clicked` on click. The extended tier (table, slider/progress,
tabs) is a follow-up.

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
against reality before encoding). Linux/AT-SPI verification is still pending.

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

## Acceptance lane

Wired into the real-provider lane under
[`tests/acceptance/qml`](../../tests/acceptance/qml) (profile `real`, tag `real`):
`catalog.robot` is the thin onboarding suite over the shared catalog keywords in
[`tests/acceptance/resources`](../../tests/acceptance/resources) (the blueprint's
reference implementation), plus QML-specific suites for popup modes, both dialog
faces, and the custom-controls chapter. The interpreter + entrypoint are handed
over via `PLATYNUI_TEST_APP_QML_PYTHON` / `PLATYNUI_TEST_APP_QML_MAIN` by
`scripts/platynui-robot-session.sh` (Linux) and the `test-acceptance-windows`
recipe (Windows); Robot Framework launches the interpreter directly so the
started PID is the app's PID (`@ProcessId` pinning).

`ruff` and strict `mypy` cover `main.py` in `just check` (own invocation in the
`mypy` recipe — the fixture apps' `main.py` scripts share a module name, which
one mypy run rejects as duplicates; see the note in `pyproject.toml`).
