# PlatynUI GTK 3 test app (PyGObject)

The **GTK 3 row** of the fixture technology matrix
([`dev-docs/testing-strategy.md`](../../dev-docs/testing-strategy.md) §5), an instance of
the fixture blueprint (§5.1). It is the catalog built the way an ordinary GTK 3
application is built: standard widgets, `Gtk.MenuBar` and `Gtk.Menu` menus, `Gtk.Dialog`
dialogs. GTK 3 reaches AT-SPI through its ATK bridge, so the tree PlatynUI reads is the one
every GTK 3 application exposes.

It is **Python**, not a Cargo crate (`exclude`d from the workspace in the root
`Cargo.toml`), and runs on Linux only: GTK 3 exposes no accessibility tree on Windows or
macOS. PlatynUI tests GTK on Linux, on X11 and natively on Wayland.

## Prerequisites

- **PyGObject** in the project venv. It is a Linux dev dependency, and PyPI has no Linux
  wheels for it, so `just bootstrap` builds it from source. That needs a C compiler,
  `pkg-config`, and the girepository-2.0 and cairo headers; `just bootstrap` checks them
  first and names what is missing. On Debian/Ubuntu:
  `sudo apt install libgirepository-2.0-dev libcairo2-dev pkg-config gcc`.
  girepository-2.0 comes with GLib 2.80, so a distribution with an older GLib cannot build
  it.
- **GTK 3 and its introspection data** at run time, on Debian/Ubuntu
  `sudo apt install libgtk-3-0 gir1.2-gtk-3.0`.

A missing prerequisite stops the app before any window opens, with one line naming what is
missing and where it comes from, and exit code 2:

```text
error: PyGObject is not installed in /…/.venv/bin/python. On Linux, `just bootstrap` builds it into the project venv.
error: GTK 3's introspection data (the Gtk-3.0 typelib) is missing. Install it, e.g. `sudo apt install gir1.2-gtk-3.0` on Debian/Ubuntu.
```

The acceptance suite's launcher waits for the main window, and when the process exits
first, it fails the suite with this log. A missing prerequisite is a failure, never a skip.

## Run

```sh
# On the project venv (PyGObject comes with `just bootstrap`):
just run-test-app-gtk3

# Options:
just run-test-app-gtk3 --title "My GTK 3 Window" --app-id org.example.gtk3
just run-test-app-gtk3 --auto-close 10      # exit after 10 seconds
just run-test-app-gtk3 --open-modal         # open dialog-modal at startup
just run-test-app-gtk3 --help

# Standalone, without the project venv: uv reads the script's PEP 723 header and
# builds PyGObject into an environment of its own (the build dependencies above apply):
uv run apps/test-app-gtk3/main.py
```

An unknown argument prints the usage and exits with code 2.

## Catalog

The blueprint's core tier under the canonical names (§5.1), plus the extended tier's
slider, progress bar and tab control. The table is not implemented.

| Control | Canonical names | GTK 3 widget |
|---|---|---|
| main window | matched by its launch title (`--title`, default `PlatynUI GTK3 TestApp`) | `Gtk.ApplicationWindow` with a `Gtk.HeaderBar` as its title bar, 900×700 |
| button, status label | `button-basic`, `status-label-clicks-<n>` | `Gtk.Button`, `Gtk.Label` |
| last-action label | `last-action-none`, then `last-action-<ident>` | `Gtk.Label` |
| checkbox | `checkbox-basic` | `Gtk.CheckButton` |
| group with radios | `groupbox-basic` with `radio-first`, `radio-second` | `Gtk.Frame`, `Gtk.RadioButton`s |
| text field, text area | `textfield-basic`, `textarea-basic` | `Gtk.Entry`, `Gtk.TextView` |
| label, plain text, image | `label-basic`, `text-basic`, `image-basic` | `Gtk.Label`, a non-selectable `Gtk.Label`, `Gtk.Image` |
| combo box | `combobox-basic`, items `combo-item-1`…`-3` | `Gtk.ComboBoxText` |
| list | `list-basic`, `list-item-1`…`-5` | `Gtk.ListBox` |
| tree | `tree-basic`, `tree-node-a`, `tree-node-a-1`, `tree-node-a-1-i`, … | `Gtk.TreeView` over a `Gtk.TreeStore` |
| menu bar | `main-menubar`; `menu-file`, `menu-edit` (submenu `menu-edit-more`), `menu-help`, each with its items | `Gtk.MenuBar` with `Gtk.Menu`s |
| context menu | `context-menu`; `ctx-cut`, `ctx-copy`, `ctx-paste`, submenu `ctx-more` | `Gtk.Menu`, opened by a secondary click on `label-basic` |
| dialogs | `dialog-modeless` (420×300, open at startup), `dialog-modal` (`--open-modal`); each with `<ident>-label`, `<ident>-button` | `Gtk.Dialog`; the modal one is `modal` and transient for the main window |
| slider, progress, tabs | `slider-basic`, `progress-basic`, `tabs-basic` with `tab-one`, `tab-two` | `Gtk.Scale`, `Gtk.ProgressBar`, `Gtk.Notebook` |

Every name is set on the widget's ATK object (`get_accessible().set_name`), windows
included. A `Gtk.Label` has no input window of its own, so `label-basic` sits in a
`Gtk.EventBox`, which receives the secondary click.

Activating a menu item or a dialog button, or selecting a radio button, reports its ident
through the last-action label; no control changes its own name. A double-click on a tree row expands or collapses it, the
expansion gesture the catalog uses for every toolkit.

Beyond the catalog, other suites rely on three things:

- The main window draws a header bar, so under the PlatynUI compositor it has client-side
  decorations and its title is part of its own tree.
- `dialog-modeless` has a different size from the main window. On X11 the window lookup
  tells two windows of one process apart by size.
- `dialog-modeless` is a top-level window of its own, not transient for the main window, so
  a window manager that keeps transients above their parent never blocks bringing the
  main window to the front. Closing it only hides it.

## Verified facts (GTK 3.24.52, ATK/AT-SPI, X11 and the PlatynUI compositor)

Read from the real tree in both lane sessions. Roles stay out of the locator contract; the
suites address controls by `@Name` alone, through the named container they belong to.

- **Window names:** a GTK 3 window with a header bar has an empty accessible name on X11,
  so the fixture sets every window's name to its title. The main window is matched by its
  launch title, and the dialogs are titled with their canonical names. The main window is
  a `Frame` node, the dialogs are `Dialog` nodes. The application node is named after the
  program, `main.py`.
- **Menu bar:** each menu-bar entry is a `Menu` node named after its entry, and its items
  are on the tree while the menu is closed, at positions far off screen. The submenu
  `menu-edit-more` is a `Menu` below `menu-edit`, with its items below it.
- **Context menu:** it is on the tree only while open, in an unnamed popup `Window` below
  the application node. `context-menu` is the menu in it, and the submenu `ctx-more` is a
  `Menu` below `context-menu`.
- **Combo box:** its items are on the tree while the list is closed, below an unnamed
  `Menu` below `combobox-basic`. Whether the list is open shows only in the native AT-SPI
  state (`Showing`), not in any common attribute, and the combo box's name stays
  `combobox-basic`, so no common attribute shows the selection either. The catalog's
  *Opening The Combo Box Reveals Its Items* is therefore a documented skip.
- **Tree:** `tree-basic` is a tree table with one cell per visible row, plus a column
  header cell `node`. A collapsed node's children are not on the tree until it expands.
- **Duplicated names:** the frame's label repeats `groupbox-basic`, and under the
  compositor a dialog's own title bar repeats the dialog's name. A name query's first
  match is the group or the dialog itself, which comes first in document order.
- **Header bar:** it is a panel below the main window with the title label and the window
  buttons, whose names follow the session's locale.
- **Image:** `image-basic` surfaces with the icon role.

**Geometry.** GTK 3 reports screen coordinates where AT-SPI asks for coordinates relative
to the parent, so a sum over an element's ancestors counts each of their positions again:
added up that way, `radio-first` on X11 lands at a `y` of 931 inside a window 758 px high.
The screen extents themselves are right on X11. Under the compositor they count from the
window's surface, and a menu's items report no usable screen position at all. PlatynUI
therefore places a GTK 3 element relative to its window, read in screen coordinates, and a
menu and its items from the popup window the platform lists for the menu
([`dev-docs/platform-linux.md`](../../dev-docs/platform-linux.md) §2, *Element geometry*).

## Acceptance lane

Wired into the real-provider lanes under
[`tests/acceptance/gtk3`](../../tests/acceptance/gtk3), tagged `acceptance`, `real` and
`platform:linux`: both Linux lanes run it, the Windows lane does not.

- `catalog.robot`: the blueprint's canonical catalog test set, plus the window shape other
  suites rely on (the header bar under the compositor, the dialog's size, the radios inside
  the group) and the fast failure of an unknown argument.
- `modal.robot`: `dialog-modal`, on an instance of its own started with `--open-modal`.

`scripts/platynui-robot-session.sh` hands the interpreter and entrypoint over as
`PLATYNUI_TEST_APP_GTK3_PYTHON` / `PLATYNUI_TEST_APP_GTK3_MAIN`, unsets `NO_AT_BRIDGE`,
which would keep GTK 3's ATK bridge from loading, and leaves `GDK_BACKEND` alone, so GTK
picks X11 or Wayland from the session. Robot Framework starts the interpreter directly, so
the started PID is the app's and the suites pin it with `@ProcessId`.

```sh
just headless=true test-acceptance-x11 --suite '*.Gtk3'
just headless=true test-acceptance-compositor --suite '*.Gtk3'
```

`ruff` and strict `mypy` cover `main.py` in `just check`, in an invocation of its own: the
fixture apps' `main.py` scripts share a module name, which one mypy run rejects as
duplicates. `gi` is declared untyped in `pyproject.toml`.
