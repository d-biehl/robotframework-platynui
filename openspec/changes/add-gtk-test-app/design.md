# Design

## Context

See proposal.md (Why). The facts below were read at `cda12175`. **Verified** means read in this repository or measured. **Source** means read in upstream code or documentation and not run. **Assumed** means neither.

**How the Python fixtures are wired today** (verified). The Qt Widgets and Qt Quick fixtures are the model:

- **Code:** a single `main.py` with PEP 723 inline metadata (`apps/test-app-qml/main.py:1-4`), excluded from the Cargo workspace (`Cargo.toml:7-13`).
- **Interpreter:** they run on the project venv's interpreter. `scripts/platynui-robot-session.sh:97-108` hands interpreter and entrypoint over as `PLATYNUI_TEST_APP_QT*_PYTHON`/`_MAIN`.
- **Launch:** the launcher resource starts the interpreter directly, never through `uv run`, pins the instance with `Set Root /app:Application[@ProcessId=…]` and waits for the main window by its launch title (`tests/acceptance/qml/resources/qmlapp.resource`).
- **Type check:** `just mypy` checks each fixture in its own invocation, because they all share the module name `main` (`justfile:203-208`, `pyproject.toml` comment at the `files` list).
- **Selection:** the Windows lane excludes `platform:x11` and `platform:wayland` only (`robot.toml:72-75`). A Linux-only suite without a platform tag would run there.

**What the probes showed** (verified, 2026-11-10). GTK 3.24.52 and GTK 4.24.1 probe windows, run on the system's PyGObject 3.58, registered on the accessibility bus in both lane sessions with no extra environment:

- Under the PlatynUI compositor both use client-side decorations with a header bar. GTK 3's geometry offset was `24,21`, GTK 4's `14,12`.
- Under Xvfb with icewm, which does not composite, GTK draws the header bar without a shadow.
- GTK 4 windows carried their title as their accessible name in every run.
- A GTK 3 window with a header bar on X11 had an **empty** name, and the same window without a header bar was named by its title.
- GTK 4 reports `Application.ToolkitName` `GTK` and `Version` `4.24.1`, so the version is readable from the tree.

**PyGObject** (source: pygobject.gnome.org, getting started, changelog 3.51.0):

- PyPI has no binary wheels for Linux. `pip`/`uv` build it with `gcc`, `pkg-config`, `libgirepository-2.0-dev` and `libcairo2-dev`; `pycairo` comes along as a dependency.
- Since 3.51 it uses girepository-2.0 and needs GLib ≥ 2.80.
- Ubuntu 24.04 ships GLib 2.80 (assumed from the package archive). Older distributions, such as Ubuntu 22.04 and Debian 12, cannot build current PyGObject.

**GTK 4 deprecations in 4.10** (source: GTK 4 release notes): `GtkTreeView`, `GtkComboBox`/`GtkComboBoxText`, `GtkDialog`, `GtkTreeStore`/`GtkListStore` and others. Their replacements are list models with `GtkListView`/`GtkColumnView` and `GtkTreeListModel`, `GtkDropDown`, and plain `GtkWindow`s with `modal` and `transient-for`.

**GTK 4 popovers** (source, `linux-element-geometry` research):

- Before 4.18, a popover's position relative to its parent widget is missing from its accessible bounds (GTK commit 3b68c1ea, MR !8181).
- GTK 4 menu bars, context menus (`GtkPopoverMenu`) and `GtkDropDown` all open popovers.
- `ubuntu-latest` is the CI runner (`.github/workflows/ci.yml:257`) and is Ubuntu 24.04 with GTK 4.14 (assumed from the runner image's package versions; task 6.1 records the real one).

## Goals / Non-Goals

**Goals:**

- Two catalogs that a GTK developer would recognize as ordinary GTK 3 and GTK 4 code, so their trees are the trees users automate.
- A fixture that fails because of the environment says which package is missing.

**Non-Goals:**

- The extended tier's table, the custom-controls chapter, and libadwaita widgets.
- GTK on Windows or macOS. Neither GTK 3 nor GTK 4 exposes an accessibility tree to UIA or AX.
- GTK 4's AccessKit backend (`GTK_A11Y=accesskit`), which is off by default.
- Any PlatynUI provider change. Defects the fixtures reveal go to the provider changes that own them, `linux-element-geometry` first.

## Decisions

### 1. Two fixtures, each written against its own GTK version

`apps/test-app-gtk3/main.py` and `apps/test-app-gtk4/main.py` stand alone, with the same structure and no shared module. GTK 3 and GTK 4 differ in exactly the widgets the catalog needs: menus, trees, combo boxes, dialogs and event handling. Shared code would end in a version switch on every control.

*Rejected:*

- **One script with a `--gtk` switch.** The branches would outnumber the shared lines.
- **A shared helper module.** The fixture directories are not importable packages, like the Qt ones (decision 6).

### 2. PyGObject joins the dev group on Linux

`pyproject.toml` gets `PyGObject>=3.52; sys_platform == 'linux'` in the dev group. `uv sync` builds it once, and uv's cache keeps the built wheel. Each `main.py` carries a PEP 723 header for standalone runs.

`just bootstrap` checks `pkg-config --exists girepository-2.0 cairo` on Linux before syncing. When that fails, it stops with the apt package names rather than failing inside a meson log.

*Rejected:*

- **The system interpreter's `python3-gi`.** It is a second interpreter beside the venv.
- **A separate, non-default dependency group.** A plain `uv run`, which `just check` uses, removes packages of groups it does not sync, so every lane run would rebuild PyGObject.
- **Rust with gtk-rs.** It puts GTK headers into every `cargo` build of the workspace, and `gtk3-rs` is unmaintained.

### 3. The widgets per catalog control

| Blueprint control | GTK 3 | GTK 4 |
|---|---|---|
| main window | `Gtk.ApplicationWindow` with a `Gtk.HeaderBar` titlebar | same |
| button, status label, last-action label | `Gtk.Button`, `Gtk.Label` | same |
| checkbox | `Gtk.CheckButton` | `Gtk.CheckButton` |
| group with radios | `Gtk.Frame` labelled `groupbox-basic`, `Gtk.RadioButton`s | `Gtk.Frame`, grouped `Gtk.CheckButton`s |
| text field / area | `Gtk.Entry` / `Gtk.TextView` | same |
| label, plain text, image | `Gtk.Label`, a non-selectable `Gtk.Label`, `Gtk.Image` | `Gtk.Label`, `Gtk.Inscription`, `Gtk.Picture` |
| combo box | `Gtk.ComboBoxText` | `Gtk.DropDown` over a `Gtk.StringList` |
| list | `Gtk.ListBox` | `Gtk.ListBox` |
| tree | `Gtk.TreeView` over a `Gtk.TreeStore` | `Gtk.ListView` over a `Gtk.TreeListModel` with `Gtk.TreeExpander` |
| menu bar, context menu | `Gtk.MenuBar`, `Gtk.Menu` widgets | `Gtk.PopoverMenuBar`, `Gtk.PopoverMenu` from a `Gio.Menu` |
| modeless / modal dialog | `Gtk.Dialog` | `Gtk.Window` with `transient-for`, `modal` for the modal one |
| slider, progress, tabs | `Gtk.Scale`, `Gtk.ProgressBar`, `Gtk.Notebook` | same |

- **Names, GTK 3:** set through the widget's ATK object (`get_accessible().set_name`), for windows too. That covers the empty name a header bar produced (Context).
- **Names, GTK 4:** set through `Gtk.Accessible.update_property` with `LABEL`.
- **Menu items, GTK 4:** a `Gio.Menu` item gets its accessible name from its label. Its label is therefore the canonical name, and its action reports the last action.
- **GTK 4 tree fallback:** the tree's expanded state and the names of its rows are the least certain part. If the real tree does not expose them, `Gtk.TreeView` is the documented fallback (spec, *One blueprint fixture per GTK major version*).
- **Sizes:** the main window opens at 900×700 and `dialog-modeless` at 420×300.

*Why `Gtk.Inscription` for GTK 4's plain text* (source, GTK 4.8+): it is GTK 4's text widget without label semantics, which is what the blueprint's "plain static text" distinguishes from a label. This is to be verified against the real tree. A plain `Gtk.Label` is the fallback.

### 4. Windows are matched by title

Both fixtures follow the blueprint's window-naming fallback, as the QML and Avalonia fixtures do. The launcher matches the main window by its launch title, and dialogs are titled with their canonical names. GTK 3 additionally sets each window's accessible name to its title, so the name never depends on the header bar.

### 5. A missing prerequisite fails with its package

Each `main.py` imports `gi` and calls `gi.require_version("Gtk", "<n>.0")` before anything else.

- An `ImportError` names PyGObject and `just bootstrap`.
- A `ValueError` from `require_version` names the typelib package: `gir1.2-gtk-3.0` or `gir1.2-gtk-4.0`.

Either prints one line and exits with code 2. The launcher waits for the main window. When the process has exited first, it fails the suite with the fixture's log, so the message reaches the report. That is the failure, not a skip, that `acceptance-lane-selection` requires for runtime prerequisites.

### 6. Typing

`[[tool.mypy.overrides]]` declares `gi` and `gi.*` untyped (`ignore_missing_imports`). `just mypy` checks each GTK fixture in its own invocation.

*Rejected:*

- **`pygobject-stubs`.** It generates stubs for one GTK version per installation, and the venv serves both.

### 7. Selection: `platform:linux` and `requires:gtk-4.18`

- **`platform:linux`:**
  - It joins the tag vocabulary.
  - `real-windows` excludes it, and both Linux profiles keep it.
  - Both GTK suite trees carry it suite-wide in `__init__.robot`.
- **`requires:gtk-4.18`:**
  - It marks every GTK 4 test that opens a popover: the menu bar, submenu, context-menu, context-submenu and combo-box tests of the catalog.
  - The GTK 4 launcher resource gets a test setup, `Require GTK Version`. It reads `native:Application.Version` from the pinned application node and compares it with the version in the test's `requires:gtk-*` tag. A lower version fails with the installed version, the required one and `--exclude requires:gtk-4.18`.
  - CI passes `--exclude requires:gtk-4.18` to the Linux lane recipes while the runner image is older. A comment in `ci.yml` says when to drop the flag.

*Rejected:*

- **A documented skip.** `acceptance-lane-selection` allows a skip only for a limitation that holds on every machine of a lane. This one depends on the installed GTK.
- **Leaving the tests out until CI has a newer GTK.** Local runs on current distributions would lose coverage that works there.

### 8. Session and lane wiring

- `platynui-robot-session.sh` exports `PLATYNUI_TEST_APP_GTK3_PYTHON`, `PLATYNUI_TEST_APP_GTK3_MAIN` and their GTK 4 counterparts. The interpreter is the venv's, as for Qt.
- It sets `GTK_A11Y=atspi` so a host-wide `GTK_A11Y=none` cannot hide the GTK 4 fixture. It also unsets `NO_AT_BRIDGE` for GTK 3.
- It does not set `GDK_BACKEND`. The X11 session has only `DISPLAY` and the compositor session only `WAYLAND_DISPLAY`, so each GTK picks its native backend.
- `justfile` gets `run-test-app-gtk3` and `run-test-app-gtk4`.
- CI installs the PyGObject build dependencies in every Linux job that syncs the environment. The acceptance job also installs `libgtk-3-0`, `libgtk-4-1`, `gir1.2-gtk-3.0` and `gir1.2-gtk-4.0`.

## Risks / Trade-offs

- **[A heavier Linux dev environment]** Every Linux contributor needs four more apt packages, and a first `uv sync` takes longer while PyGObject builds. → `just bootstrap` names the packages (decision 2), and `CONTRIBUTING.md` lists them. uv's wheel cache makes later syncs fast.
- **[Older distributions cannot build current PyGObject]** GLib < 2.80 fails the build. → Documented as unsupported for the development environment. Pinning PyGObject < 3.51 would instead require girepository-1.0 on current systems.
- **[GTK 4 list-model widgets expose less than the old ones]** The tree's expansion state and the drop-down's items may not surface as the blueprint needs. → Each is verified against the real tree before its test is written. A gap uses the deprecated widget or a name-based observable, and the README documents the choice (spec).
- **[The GTK 3 catalog's pointer tests need `linux-element-geometry`]** → This change is applied after that change's groups 1–5 (proposal, Depends on). Its GTK 4 work can start earlier.
- **[CI exercises no GTK 4 popover]** While the runner's GTK is older than 4.18, menus, context menus and the drop-down run only locally. → The `ci.yml` comment and `testing-strategy.md` §2.6 say so. Moving the acceptance job to a newer runner image ends it.
- **[GTK 3's empty window name returns elsewhere]** Dialogs with header bars could show the same effect. → Decision 4 names every GTK 3 window explicitly, and the catalog's dialog test finds dialogs by name.

## Migration Plan

- **Additive:** new fixtures, suites, tags and CI steps. No PlatynUI behavior changes. The one contributor-facing change is the PyGObject build dependencies on Linux.
- **Native rebuild:** none.
- **Order:**
  1. the environment (dependency, bootstrap check, CI packages);
  2. the GTK 4 fixture and its suites;
  3. the GTK 3 fixture and its suites, once `linux-element-geometry` groups 1–5 have landed;
  4. the docs.
- **Rollback:** revert the change's commits. No state depends on them. The dev dependency goes away with the next `uv sync`.
