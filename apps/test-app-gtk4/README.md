# PlatynUI GTK 4 test app (PyGObject)

The **GTK 4 row** of the fixture technology matrix
([`dev-docs/testing-strategy.md`](../../dev-docs/testing-strategy.md) §5), an instance of
the fixture blueprint (§5.1). It is the catalog built the way an ordinary GTK 4
application is built: standard widgets only, none of those GTK 4.10 deprecated, menus from
`Gio.Menu` models. GTK 4 implements AT-SPI itself, without ATK, so the tree PlatynUI reads
is GTK's own.

It is **Python**, not a Cargo crate (`exclude`d from the workspace in the root
`Cargo.toml`), and runs on Linux only. GTK 3 exposes no accessibility tree on Windows or
macOS, and GTK 4 only through its AccessKit backend, which GTK builds without by default.
PlatynUI tests GTK on Linux, on X11 and natively on Wayland.

## Prerequisites

- **PyGObject** in the project venv. It is a Linux dev dependency, and PyPI has no Linux
  wheels for it, so `just bootstrap` builds it from source. That needs a C compiler,
  `pkg-config`, and the girepository-2.0 and cairo headers; `just bootstrap` checks them
  first and names what is missing. On Debian/Ubuntu:
  `sudo apt install libgirepository-2.0-dev libcairo2-dev pkg-config gcc`.
  girepository-2.0 comes with GLib 2.80, so a distribution with an older GLib cannot build
  it.
- **GTK 4.18 or later and its introspection data** at run time, on Debian/Ubuntu
  `sudo apt install libgtk-4-1 gir1.2-gtk-4.0`.

A missing prerequisite stops the app before any window opens, with one line naming what is
missing and where it comes from, and exit code 2:

```text
error: PyGObject is not installed in /…/.venv/bin/python. On Linux, `just bootstrap` builds it into the project venv.
error: GTK 4's introspection data (the Gtk-4.0 typelib) is missing. Install it, e.g. `sudo apt install gir1.2-gtk-4.0` on Debian/Ubuntu.
```

The acceptance suite's launcher waits for the main window, and when the process exits
first, it fails the suite with this log. A missing prerequisite is a failure, never a skip.

## Run

```sh
# On the project venv (PyGObject comes with `just bootstrap`):
just run-test-app-gtk4

# Options:
just run-test-app-gtk4 --title "My GTK 4 Window" --app-id org.example.gtk4
just run-test-app-gtk4 --auto-close 10      # exit after 10 seconds
just run-test-app-gtk4 --open-modal         # open dialog-modal at startup
just run-test-app-gtk4 --help

# Standalone, without the project venv: uv reads the script's PEP 723 header and
# builds PyGObject into an environment of its own (the build dependencies above apply):
uv run apps/test-app-gtk4/main.py
```

An unknown argument prints the usage and exits with code 2.

## Catalog

The blueprint's core tier under the canonical names (§5.1), plus the extended tier's
slider, progress bar and tab control. The table is not implemented.

| Control | Canonical names | GTK 4 widget |
|---|---|---|
| main window | matched by its launch title (`--title`, default `PlatynUI GTK4 TestApp`) | `Gtk.ApplicationWindow` with a `Gtk.HeaderBar` as its title bar, 900×700 |
| button, status label | `button-basic`, `status-label-clicks-<n>` | `Gtk.Button`, `Gtk.Label` |
| last-action label | `last-action-none`, then `last-action-<ident>` | `Gtk.Label` |
| checkbox | `checkbox-basic` | `Gtk.CheckButton` |
| group with radios | `groupbox-basic` with `radio-first`, `radio-second` | `Gtk.Frame`, grouped `Gtk.CheckButton`s |
| text field, text area | `textfield-basic`, `textarea-basic` | `Gtk.Entry`, `Gtk.TextView` |
| label, plain text, image | `label-basic`, `text-basic`, `image-basic` | `Gtk.Label`, `Gtk.Inscription`, `Gtk.Image` |
| combo box | `combobox-basic`, rows `combo-item-1`…`-3` | `Gtk.DropDown` over a string list |
| list | `list-basic`, `list-item-1`…`-5` | `Gtk.ListBox` |
| tree | `tree-basic`, `tree-node-a`, `tree-node-a-1`, `tree-node-a-1-i`, … | `Gtk.ListView` over a `Gtk.TreeListModel`, rows with a `Gtk.TreeExpander` |
| menu bar | `main-menubar`; `menu-file`, `menu-edit` (submenu `menu-edit-more`), `menu-help`, each with its items | `Gtk.PopoverMenuBar` from a `Gio.Menu` |
| context menu | `context-menu`; `ctx-cut`, `ctx-copy`, `ctx-paste`, submenu `ctx-more` | `Gtk.PopoverMenu` from a `Gio.Menu`, opened by a secondary click on `label-basic` |
| dialogs | `dialog-modeless` (420×300, open at startup), `dialog-modal` (`--open-modal`); each with `<ident>-label`, `<ident>-button` | `Gtk.Window`; the modal one is `modal` and transient for the main window |
| slider, progress, tabs | `slider-basic`, `progress-basic`, `tabs-basic` with `tab-one`, `tab-two` | `Gtk.Scale`, `Gtk.ProgressBar`, `Gtk.Notebook` |

Every name is set through `Gtk.Accessible`'s `LABEL` property. A button or a drop-down
names itself through a labelled-by relation to its inner label, which wins over `LABEL`,
so the fixture resets that relation first.

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
  main window to the front.

## Verified facts (GTK 4.24.1, AT-SPI, X11 and the PlatynUI compositor)

Read from the real tree in both lane sessions. Roles stay out of the locator contract; the
suites address controls by `@Name` alone.

- **Window names:** GTK 4 names every window by its title, so the main window is matched by
  its launch title and the dialogs are titled with their canonical names. Both are `Frame`
  nodes. The application node is named after the program, `main.py`.
- **Version on the tree:** the application node reports GTK's version
  (`native:Application.Version`).
- **Duplicated names:** a button's, check box's or radio button's inner label carries the
  control's name too, and the drop-down's inner toggle button repeats `combobox-basic`.
  A name query's first match is the control itself, which comes first in document order.
- **Plain text:** `Gtk.Inscription`, GTK 4's text widget without label semantics, still
  surfaces with the label role, like `label-basic`. The tree does not tell the two apart.
- **Tree:** `tree-basic` is a list of list items, one per visible row. A collapsed node's
  children are not on the tree until it expands.
- **Header bar:** it is a grouping below the main window with the title label and the
  window buttons, whose names follow the session's locale.
- **Tabs:** GTK names the notebook's page containers itself, in the session's locale (in a
  German session `Seite 1`, `Seite 2`). The tab labels carry the canonical names.

### Popovers

Every menu, submenu and the drop-down's list is a popover. A popover's items are on the
tree only while it is open. There is no window node for a popover; it hangs below the
widget that opened it:

- a menu-bar menu below its menu-bar item, a nested submenu below the item that opens it;
- the drop-down's rows below `combobox-basic`;
- the context menu below the column that holds `label-basic`. A `Gtk.Label` does not hand
  popover children to the accessibility tree, so the menu is parented to the label's
  container. Its submenu slides into the same popover: the first page leaves the tree, and
  the back item at the top carries the submenu's name, `ctx-more`.

Locators therefore go through the named menu, as the user opens it:
`.//*[@Name="menu-edit"]//*[@Name="menu-edit-more"]//*[@Name="menu-edit-sub-one"]`,
`.//*[@Name="context-menu"]//*[@Name="ctx-copy"]`,
`.//*[@Name="combobox-basic"]//*[@Name="combo-item-2"]`.

**Menu item names.** From GTK 4.18 to 4.24.1 a popover menu item has no accessible name.
Its label is built with the presentation role, and GTK's label code skips the name for
that role, so nothing below the item carries its text either. GTK fixed it in commit
`fa085fe1` (MR !10510, GTK 4.25.0) and backported it as `361abc89` for 4.24.2.

The fixture names every item by its label itself, each time its popover opens: an item is
a model button (CSS name `modelbutton`) with its label as its `text`, and a `LABEL` set
when the popover maps lasts on every item. On a GTK with the fix this changes nothing.
**Real applications on GTK 4.18 to 4.24.1 expose their menu items unnamed**, and PlatynUI
reports them that way; the fixture hides the defect only so the catalog stays name-based.
The model button's CSS name and `text` property are not public API: if a GTK release
changes them, the items lose their names again and the menu tests fail at their locators.

The drop-down's rows are named by the fixture's list factory, through each row's
accessible label, as a real application names them. Its button shows the selection in a
presentational label, so a closed drop-down keeps no element named like the selected
item.

**Placement.** GTK 4 reports a popover's position measured from the widget that opened it,
without the popover's own placement: a nested submenu item was reported about 100 px from
where it was drawn. PlatynUI recognizes a popover by the popup layer GTK 4 reports for it
and places it, and its items, from the popup the platform lists for it
([`dev-docs/platform-linux.md`](../../dev-docs/platform-linux.md) §2, *Element geometry*).
A child of a padded widget, such as a button's inner label, still reports its parent's
origin: GTK 4 sums CSS border boxes while it draws in content boxes
([`dev-docs/platform-linux-wayland.md`](../../dev-docs/platform-linux-wayland.md) §3).

**After a submenu activation (Wayland).** When an item of a nested menu-bar submenu is
activated with the pointer, GTK opens the next menu-bar menu by itself, here `menu-help`. GDK
logs `Tried to map a grabbing popup with a non-top most parent`, and the Wayland protocol log
shows no input between the activation and the new popover. Escape does not close it. The
next click outside it only closes it, and GTK 4.24.1 then ignores every further click in the
application. This was seen with GTK's default settings, on GTK 4.22.4 and 4.24.1, and not with
a desktop's own theme and font settings, nor on X11. The catalog's submenu test therefore runs
on an instance of its own.

**PyGObject:** `Gdk.Rectangle(x=…, y=…)` ignores its arguments, so the context menu's
target rectangle is filled field by field.

## Acceptance lane

Wired into the real-provider lanes under
[`tests/acceptance/gtk4`](../../tests/acceptance/gtk4), tagged `acceptance`, `real` and
`platform:linux`: both Linux lanes run it, the Windows lane does not.

- `catalog.robot`: the blueprint's canonical catalog test set, plus the window shape other
  suites rely on (the header bar under the compositor, the dialog's size, the radios inside
  the group) and the fast failure of an unknown argument. Its menu-bar submenu test launches
  an instance of its own and terminates it afterwards (see *After a submenu activation*).
- `modal.robot`: `dialog-modal`, on an instance of its own started with `--open-modal`.

`scripts/platynui-robot-session.sh` hands the interpreter and entrypoint over as
`PLATYNUI_TEST_APP_GTK4_PYTHON` / `PLATYNUI_TEST_APP_GTK4_MAIN`, sets `GTK_A11Y=atspi` so a
host-wide `GTK_A11Y=none` cannot hide the app, and leaves `GDK_BACKEND` alone, so GTK picks
X11 or Wayland from the session. Robot Framework starts the interpreter directly, so the
started PID is the app's and the suites pin it with `@ProcessId`.

```sh
just headless=true test-acceptance-x11 --suite '*.Gtk4'
just headless=true test-acceptance-compositor --suite '*.Gtk4'
```

`ruff` and strict `mypy` cover `main.py` in `just check`, in an invocation of its own: the
fixture apps' `main.py` scripts share a module name, which one mypy run rejects as
duplicates. `gi` is declared untyped in `pyproject.toml`.
