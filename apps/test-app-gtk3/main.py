# /// script
# requires-python = ">=3.12"
# dependencies = ["PyGObject>=3.52"]
# ///
"""PlatynUI GTK 3 test application (PyGObject).

The GTK 3 row of the fixture technology matrix (`dev-docs/testing-strategy.md` §5): the
blueprint's core-tier catalog (§5.1) plus the extended tier's slider, progress bar and
tab control, under the canonical names, built from GTK 3's standard widgets. GTK 3 reaches
AT-SPI through its ATK bridge; every catalog name is set on the widget's ATK object.

What other suites rely on beyond the catalog:

- The main window draws a ``Gtk.HeaderBar`` as its title bar (client-side decorations).
- Every window's accessible name is its title. A GTK 3 window with a header bar would
  otherwise have an empty name on X11.
- ``dialog-modeless`` opens at startup with a size different from the main window's. It
  is a top-level of its own, not transient for the main window, so bringing the main
  window to the front is never blocked by a window manager that keeps transients above
  their parent.
- ``groupbox-basic`` holds ``radio-first`` and ``radio-second``.

Usage
-----
    # On the project venv (PyGObject is a Linux dev dependency, built by `just bootstrap`):
    just run-test-app-gtk3

    # Options:
    just run-test-app-gtk3 --title "My GTK 3 Window" --app-id org.example.gtk3
    just run-test-app-gtk3 --auto-close 10      # self-close for CI
    just run-test-app-gtk3 --open-modal         # open dialog-modal at startup
"""

import argparse
import sys
from collections.abc import Sequence
from typing import Any

# The prerequisites fail with what is missing and where it comes from, before any
# window exists (dev-docs/testing-strategy.md §2.6: a missing fixture prerequisite fails
# its suite with guidance).
try:
    import gi
except ImportError:
    print(
        f'error: PyGObject is not installed in {sys.executable}. '
        'On Linux, `just bootstrap` builds it into the project venv.',
        file=sys.stderr,
    )
    sys.exit(2)
try:
    gi.require_version('Gtk', '3.0')
    gi.require_version('Gdk', '3.0')
except ValueError:
    print(
        "error: GTK 3's introspection data (the Gtk-3.0 typelib) is missing. "
        'Install it, e.g. `sudo apt install gir1.2-gtk-3.0` on Debian/Ubuntu.',
        file=sys.stderr,
    )
    sys.exit(2)
from gi.repository import Gio, GLib, Gtk

DEFAULT_TITLE = 'PlatynUI GTK3 TestApp'
DEFAULT_APP_ID = 'org.platynui.test.gtk3'

MAIN_SIZE = (900, 700)
MODELESS_SIZE = (420, 300)

# Tree catalog: roots and the children of every node that has some (three levels).
TREE_ROOTS = ['tree-node-a', 'tree-node-b']
TREE_CHILDREN = {
    'tree-node-a': ['tree-node-a-1', 'tree-node-a-2'],
    'tree-node-a-1': ['tree-node-a-1-i'],
}

# Menus: a plain string is an item, a (label, items) pair a submenu.
MenuSpec = Sequence[str | tuple[str, Sequence[str]]]
MENUBAR: list[tuple[str, MenuSpec]] = [
    ('menu-file', ['menu-file-new', 'menu-file-open', 'menu-file-quit']),
    ('menu-edit', ['menu-edit-undo', 'menu-edit-redo', ('menu-edit-more', ['menu-edit-sub-one', 'menu-edit-sub-two'])]),
    ('menu-help', ['menu-help-about']),
]
CONTEXT_MENU: MenuSpec = ['ctx-cut', 'ctx-copy', 'ctx-paste', ('ctx-more', ['ctx-sub-alpha', 'ctx-sub-beta'])]


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description='PlatynUI GTK 3 test application (blueprint fixture).')
    parser.add_argument('--title', default=DEFAULT_TITLE, help='Main window title.')
    parser.add_argument('--app-id', default=DEFAULT_APP_ID, help='GApplication id (Wayland app id).')
    parser.add_argument('--auto-close', type=int, default=0, help='Exit after N seconds (0 = never).')
    parser.add_argument('--open-modal', action='store_true', help='Open dialog-modal at startup.')
    return parser.parse_args(argv)


def name(widget: Any, ident: str) -> Any:
    """Give ``widget`` its canonical accessible name through its ATK object and return it."""
    widget.get_accessible().set_name(ident)
    return widget


def labelled(widget: Any, text: str) -> Any:
    """A label naming itself by its text (status and last-action observables)."""
    widget.set_label(text)
    return name(widget, text)


class Catalog:
    """The catalog window, its dialogs and the observables they report to."""

    def __init__(self, app: Any, args: argparse.Namespace) -> None:
        self.app = app
        self.args = args
        self.clicks = 0
        self.status = Gtk.Label(xalign=0)
        labelled(self.status, 'status-label-clicks-0')
        self.last_action = Gtk.Label(xalign=0)
        labelled(self.last_action, 'last-action-none')
        self.window = self.build_main_window()
        self.modeless = self.build_dialog('dialog-modeless', modal=False)
        self.modal = self.build_dialog('dialog-modal', modal=True) if args.open_modal else None

    # --- observables -------------------------------------------------------------------

    def report(self, ident: str) -> None:
        labelled(self.last_action, f'last-action-{ident}')

    def on_button_basic(self, _button: Any) -> None:
        self.clicks += 1
        labelled(self.status, f'status-label-clicks-{self.clicks}')

    def on_radio_toggled(self, radio: Any) -> None:
        if radio.get_active():
            self.report(radio.get_label())

    # --- menus (Gtk.Menu widgets; an item's label is its canonical name) -------------------

    def menu(self, items: MenuSpec) -> Any:
        menu = Gtk.Menu()
        for item in items:
            if isinstance(item, tuple):
                label, children = item
                entry = name(Gtk.MenuItem(label=label), label)
                entry.set_submenu(self.menu(children))
            else:
                entry = name(Gtk.MenuItem(label=item), item)
                entry.connect('activate', lambda _entry, ident=item: self.report(ident))
            menu.append(entry)
        return menu

    def build_menubar(self) -> Any:
        menubar = name(Gtk.MenuBar(), 'main-menubar')
        for label, items in MENUBAR:
            entry = name(Gtk.MenuItem(label=label), label)
            entry.set_submenu(self.menu(items))
            menubar.append(entry)
        return menubar

    def attach_context_menu(self, label: Any) -> Any:
        """Wrap ``label`` in an event box that opens the context menu on a secondary click.

        A ``Gtk.Label`` has no input window of its own, so the event box receives the click.
        """
        popup = name(self.menu(CONTEXT_MENU), 'context-menu')
        popup.show_all()
        box = Gtk.EventBox()
        box.add(label)

        def on_press(_box: Any, event: Any) -> bool:
            if event.button != 3:
                return False
            popup.popup_at_pointer(event)
            return True

        box.connect('button-press-event', on_press)
        return box

    # --- catalog controls --------------------------------------------------------------

    def build_tree(self) -> Any:
        store = Gtk.TreeStore(str)

        def add(parent: Any, ident: str) -> None:
            row = store.append(parent, [ident])
            for child in TREE_CHILDREN.get(ident, []):
                add(row, child)

        for root in TREE_ROOTS:
            add(None, root)
        view = name(Gtk.TreeView(model=store, headers_visible=False), 'tree-basic')
        view.append_column(Gtk.TreeViewColumn('node', Gtk.CellRendererText(), text=0))

        # Double-click (row activation) toggles a row, the cross-toolkit expansion gesture.
        def on_row_activated(tree: Any, path: Any, _column: Any) -> None:
            if tree.row_expanded(path):
                tree.collapse_row(path)
            else:
                tree.expand_row(path, False)

        view.connect('row-activated', on_row_activated)
        scroller = Gtk.ScrolledWindow(min_content_height=140, vexpand=False)
        scroller.add(view)
        return scroller

    def build_combo(self) -> Any:
        combo = Gtk.ComboBoxText()
        for index in range(1, 4):
            combo.append_text(f'combo-item-{index}')
        return name(combo, 'combobox-basic')

    def build_list(self) -> Any:
        listbox = name(Gtk.ListBox(), 'list-basic')
        for index in range(1, 6):
            row = Gtk.ListBoxRow()
            row.add(Gtk.Label(label=f'list-item-{index}', xalign=0))
            listbox.add(name(row, f'list-item-{index}'))
        return listbox

    def build_tabs(self) -> Any:
        notebook = name(Gtk.Notebook(), 'tabs-basic')
        for ident in ('tab-one', 'tab-two'):
            notebook.append_page(Gtk.Label(label=f'{ident} content'), name(Gtk.Label(label=ident), ident))
        return notebook

    def build_left_column(self) -> Any:
        column = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        button = name(Gtk.Button(label='button-basic'), 'button-basic')
        button.connect('clicked', self.on_button_basic)
        column.pack_start(button, False, False, 0)
        column.pack_start(self.status, False, False, 0)
        column.pack_start(self.last_action, False, False, 0)
        column.pack_start(name(Gtk.CheckButton(label='checkbox-basic'), 'checkbox-basic'), False, False, 0)

        group = name(Gtk.Frame(label='groupbox-basic'), 'groupbox-basic')
        radios = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4, margin_start=8, margin_end=8)
        first = name(Gtk.RadioButton.new_with_label_from_widget(None, 'radio-first'), 'radio-first')
        second = name(Gtk.RadioButton.new_with_label_from_widget(first, 'radio-second'), 'radio-second')
        for radio in (first, second):
            radio.connect('toggled', self.on_radio_toggled)
            radios.pack_start(radio, False, False, 0)
        group.add(radios)
        column.pack_start(group, False, False, 0)

        column.pack_start(name(Gtk.Entry(), 'textfield-basic'), False, False, 0)
        area = Gtk.ScrolledWindow(min_content_height=60)
        area.add(name(Gtk.TextView(), 'textarea-basic'))
        column.pack_start(area, False, False, 0)

        label = name(Gtk.Label(label='label-basic', xalign=0), 'label-basic')
        column.pack_start(self.attach_context_menu(label), False, False, 0)
        text = name(Gtk.Label(label='text-basic', xalign=0, selectable=False), 'text-basic')
        column.pack_start(text, False, False, 0)
        image = name(Gtk.Image.new_from_icon_name('image-x-generic', Gtk.IconSize.DIALOG), 'image-basic')
        column.pack_start(image, False, False, 0)
        return column

    def build_right_column(self) -> Any:
        column = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        column.pack_start(self.build_combo(), False, False, 0)
        column.pack_start(self.build_list(), False, False, 0)
        column.pack_start(self.build_tree(), False, False, 0)
        slider = name(Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 1), 'slider-basic')
        slider.set_value(25)
        column.pack_start(slider, False, False, 0)
        column.pack_start(name(Gtk.ProgressBar(fraction=0.4), 'progress-basic'), False, False, 0)
        column.pack_start(self.build_tabs(), False, False, 0)
        return column

    def build_main_window(self) -> Any:
        window = Gtk.ApplicationWindow(application=self.app, title=self.args.title)
        window.set_default_size(*MAIN_SIZE)
        header = Gtk.HeaderBar(title=self.args.title, show_close_button=True)
        window.set_titlebar(header)
        name(window, self.args.title)

        content = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        content.pack_start(self.build_menubar(), False, False, 0)
        columns = Gtk.Box(spacing=24, margin_start=12, margin_end=12, margin_top=4, margin_bottom=12)
        columns.pack_start(self.build_left_column(), False, False, 0)
        columns.pack_start(self.build_right_column(), True, True, 0)
        content.pack_start(columns, True, True, 0)
        window.add(content)
        return window

    def build_dialog(self, ident: str, *, modal: bool) -> Any:
        dialog = Gtk.Dialog(title=ident)
        if modal:
            dialog.set_transient_for(self.window)
            dialog.set_modal(True)
        dialog.set_default_size(*MODELESS_SIZE)
        name(dialog, ident)
        box = dialog.get_content_area()
        box.set_spacing(8)
        box.set_margin_start(12)
        box.set_margin_end(12)
        box.set_margin_top(12)
        label = Gtk.Label(label=f'{"Modal" if modal else "Modeless"} dialog', xalign=0)
        box.pack_start(name(label, f'{ident}-label'), False, False, 0)
        button = name(Gtk.Button(label='Click Me'), f'{ident}-button')
        # Reports through the main window's last-action label: only a click really
        # landing on THIS button can set its ident.
        button.connect('clicked', lambda _button: self.report(f'{ident}-button'))
        box.pack_start(button, False, False, 0)
        # The dialogs stay open for the whole run: closing one only hides it.
        dialog.connect('delete-event', lambda widget, _event: widget.hide_on_delete())
        return dialog

    def present(self) -> None:
        self.window.show_all()
        self.modeless.show_all()
        if self.modal is not None:
            self.modal.show_all()


def main(argv: list[str]) -> int:
    args = parse_args(argv)

    app = Gtk.Application(application_id=args.app_id, flags=Gio.ApplicationFlags.NON_UNIQUE)
    catalogs: list[Catalog] = []

    def on_activate(application: Any) -> None:
        catalog = Catalog(application, args)
        catalogs.append(catalog)
        catalog.present()

    app.connect('activate', on_activate)
    if args.auto_close > 0:

        def quit_app() -> bool:
            app.quit()
            return False  # GLib.SOURCE_REMOVE: run once

        GLib.timeout_add_seconds(args.auto_close, quit_app)
    return int(app.run([sys.argv[0]]))


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
