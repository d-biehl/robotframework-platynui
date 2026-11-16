# /// script
# requires-python = ">=3.12"
# dependencies = ["PyGObject>=3.52"]
# ///
"""PlatynUI GTK 4 test application (PyGObject).

The GTK 4 row of the fixture technology matrix (`dev-docs/testing-strategy.md` §5): the
blueprint's core-tier catalog (§5.1) plus the extended tier's slider, progress bar and
tab control, under the canonical names, built from GTK 4's standard widgets — none of
those deprecated in GTK 4.10. GTK 4 implements AT-SPI itself (no ATK); every catalog
name is set through ``Gtk.Accessible``'s ``LABEL`` property.

What other suites rely on beyond the catalog:

- The main window draws a ``Gtk.HeaderBar`` as its title bar (client-side decorations).
- ``dialog-modeless`` opens at startup with a size different from the main window's. It
  is a top-level of its own, not transient for the main window, so bringing the main
  window to the front is never blocked by a window manager that keeps transients above
  their parent.
- ``groupbox-basic`` holds ``radio-first`` and ``radio-second``.

Every menu, the context menu and the drop-down is a popover. Older GTK 4 releases leave a
popover's offset out of their accessible tree, so the suites tag the tests that open one
``requires:gtk-4.18``. GTK 4.18 to 4.24.1 also exposes popover menu items without names,
so the fixture names each item by its label itself.

Usage
-----
    # On the project venv (PyGObject is a Linux dev dependency, built by `just bootstrap`):
    just run-test-app-gtk4

    # Options:
    just run-test-app-gtk4 --title "My GTK 4 Window" --app-id org.example.gtk4
    just run-test-app-gtk4 --auto-close 10      # self-close for CI
    just run-test-app-gtk4 --open-modal         # open dialog-modal at startup
"""

import argparse
import sys
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
    gi.require_version('Gtk', '4.0')
    gi.require_version('Gdk', '4.0')
    gi.require_version('Graphene', '1.0')
except ValueError:
    print(
        "error: GTK 4's introspection data (the Gtk-4.0 typelib) is missing. "
        'Install it, e.g. `sudo apt install gir1.2-gtk-4.0` on Debian/Ubuntu.',
        file=sys.stderr,
    )
    sys.exit(2)
from gi.repository import Gdk, Gio, GLib, Graphene, Gtk

DEFAULT_TITLE = 'PlatynUI GTK4 TestApp'
DEFAULT_APP_ID = 'org.platynui.test.gtk4'

MAIN_SIZE = (900, 700)
MODELESS_SIZE = (420, 300)

# Tree catalog: roots and the children of every node that has some (three levels).
TREE_ROOTS = ['tree-node-a', 'tree-node-b']
TREE_CHILDREN = {
    'tree-node-a': ['tree-node-a-1', 'tree-node-a-2'],
    'tree-node-a-1': ['tree-node-a-1-i'],
}


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description='PlatynUI GTK 4 test application (blueprint fixture).')
    parser.add_argument('--title', default=DEFAULT_TITLE, help='Main window title.')
    parser.add_argument('--app-id', default=DEFAULT_APP_ID, help='GApplication id (Wayland app id).')
    parser.add_argument('--auto-close', type=int, default=0, help='Exit after N seconds (0 = never).')
    parser.add_argument('--open-modal', action='store_true', help='Open dialog-modal at startup.')
    return parser.parse_args(argv)


def name(widget: Any, ident: str) -> Any:
    """Give ``widget`` its canonical accessible name and return it.

    Buttons and drop-downs name themselves through a ``LABELLED_BY`` relation to their
    inner label, which takes precedence over the ``LABEL`` property; it is reset so the
    canonical name wins.
    """
    widget.reset_relation(Gtk.AccessibleRelation.LABELLED_BY)
    widget.update_property([Gtk.AccessibleProperty.LABEL], [ident])
    return widget


def name_menu_items(widget: Any) -> None:
    """Name every popover menu item below ``widget`` by its text.

    GTK 4.18 to 4.24.1 builds a menu item's label with the presentation role, so the item
    gets no name from its content (fixed in GTK commit fa085fe1). The text is the name the
    other releases compute themselves, so naming it is harmless there.
    """
    child = widget.get_first_child()
    while child is not None:
        if child.get_css_name() == 'modelbutton':
            name(child, child.get_property('text'))
        name_menu_items(child)
        child = child.get_next_sibling()


def name_menu_items_on_open(widget: Any) -> None:
    """Name the items of every popover at or below ``widget`` each time the popover opens.

    A name set while the popover is being built does not last for an item with an action;
    one set when the popover opens does.
    """
    if isinstance(widget, Gtk.Popover):
        widget.connect('map', name_menu_items)
    child = widget.get_first_child()
    while child is not None:
        name_menu_items_on_open(child)
        child = child.get_next_sibling()


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

    # --- menus (Gio.Menu models; each item is named by its label, see name_menu_items) ---

    def install_actions(self) -> None:
        action = Gio.SimpleAction.new('report', GLib.VariantType.new('s'))
        action.connect('activate', lambda _action, value: self.report(value.get_string()))
        self.app.add_action(action)

    @staticmethod
    def menu(items: list[str | tuple[str, list[str]]]) -> Any:
        model = Gio.Menu()
        for item in items:
            if isinstance(item, tuple):
                label, children = item
                model.append_submenu(label, Catalog.menu(list(children)))
            else:
                model.append(item, f'app.report::{item}')
        return model

    def build_menubar(self) -> Any:
        model = Gio.Menu()
        model.append_submenu('menu-file', self.menu(['menu-file-new', 'menu-file-open', 'menu-file-quit']))
        model.append_submenu(
            'menu-edit',
            self.menu(
                ['menu-edit-undo', 'menu-edit-redo', ('menu-edit-more', ['menu-edit-sub-one', 'menu-edit-sub-two'])]
            ),
        )
        model.append_submenu('menu-help', self.menu(['menu-help-about']))
        menubar = name(Gtk.PopoverMenuBar.new_from_model(model), 'main-menubar')
        name_menu_items_on_open(menubar)
        return menubar

    def attach_context_menu(self, widget: Any, host: Any) -> None:
        """Open the context menu on a secondary click on ``widget``.

        The popover hangs off ``host``, a container: a ``Gtk.Label`` does not hand popover
        children to the accessibility tree, so a menu parented to the label itself would be
        on screen but not on the bus.
        """
        model = self.menu(['ctx-cut', 'ctx-copy', 'ctx-paste', ('ctx-more', ['ctx-sub-alpha', 'ctx-sub-beta'])])
        popover = name(Gtk.PopoverMenu.new_from_model(model), 'context-menu')
        name_menu_items_on_open(popover)
        popover.set_parent(host)
        popover.set_has_arrow(False)
        gesture = Gtk.GestureClick(button=3)

        def on_pressed(_gesture: Any, _n_press: int, x: float, y: float) -> None:
            ok, point = widget.compute_point(host, Graphene.Point().init(x, y))
            hx, hy = (point.x, point.y) if ok else (x, y)
            # Boxed structs ignore constructor arguments in PyGObject: set the fields one by one.
            target = Gdk.Rectangle()
            target.x, target.y, target.width, target.height = int(hx), int(hy), 1, 1
            popover.set_pointing_to(target)
            popover.popup()

        gesture.connect('pressed', on_pressed)
        widget.add_controller(gesture)

    # --- catalog controls --------------------------------------------------------------

    def build_tree(self) -> Any:
        roots = Gio.ListStore.new(Gtk.StringObject)
        for root in TREE_ROOTS:
            roots.append(Gtk.StringObject.new(root))

        def children_of(item: Any) -> Any:
            kids = TREE_CHILDREN.get(item.get_string())
            if not kids:
                return None
            store = Gio.ListStore.new(Gtk.StringObject)
            for kid in kids:
                store.append(Gtk.StringObject.new(kid))
            return store

        model = Gtk.TreeListModel.new(roots, False, False, children_of)
        selection = Gtk.SingleSelection.new(model)
        factory = Gtk.SignalListItemFactory()

        def setup(_factory: Any, list_item: Any) -> None:
            # The expander is the row's toggle; as a generic container it does not
            # repeat the row's name as a second interactive control.
            expander = Gtk.TreeExpander(accessible_role=Gtk.AccessibleRole.GENERIC)
            expander.set_child(Gtk.Label(xalign=0))
            list_item.set_child(expander)

        def bind(_factory: Any, list_item: Any) -> None:
            row = list_item.get_item()
            expander = list_item.get_child()
            expander.set_list_row(row)
            text = row.get_item().get_string()
            expander.get_child().set_label(text)
            list_item.set_accessible_label(text)

        factory.connect('setup', setup)
        factory.connect('bind', bind)
        view = name(Gtk.ListView.new(selection, factory), 'tree-basic')

        # Double-click (row activation) toggles a row, the cross-toolkit expansion gesture.
        def on_activate(_view: Any, position: int) -> None:
            row = model.get_item(position)
            if row is not None and row.is_expandable():
                row.set_expanded(not row.get_expanded())

        view.connect('activate', on_activate)
        scroller = Gtk.ScrolledWindow(min_content_height=140, vexpand=False)
        scroller.set_child(view)
        return scroller

    def build_combo(self) -> Any:
        """A drop-down whose popup rows carry their item as accessible label.

        The default factory leaves the popup's rows unnamed; a real application names
        them the same way, through the list item's accessible label. The button shows the
        selection in a presentational label, so a closed drop-down does not keep an
        element named like the selected item on the tree.
        """
        combo = Gtk.DropDown.new_from_strings(['combo-item-1', 'combo-item-2', 'combo-item-3'])

        def factory(*, rows: bool) -> Any:
            items = Gtk.SignalListItemFactory()

            def setup(_factory: Any, list_item: Any) -> None:
                role = Gtk.AccessibleRole.LABEL if rows else Gtk.AccessibleRole.PRESENTATION
                list_item.set_child(Gtk.Label(xalign=0, accessible_role=role))

            def bind(_factory: Any, list_item: Any) -> None:
                text = list_item.get_item().get_string()
                list_item.get_child().set_label(text)
                if rows:
                    list_item.set_accessible_label(text)

            items.connect('setup', setup)
            items.connect('bind', bind)
            return items

        combo.set_factory(factory(rows=False))
        combo.set_list_factory(factory(rows=True))
        return name(combo, 'combobox-basic')

    def build_list(self) -> Any:
        listbox = name(Gtk.ListBox(), 'list-basic')
        for index in range(1, 6):
            row = Gtk.ListBoxRow()
            row.set_child(Gtk.Label(label=f'list-item-{index}', xalign=0))
            listbox.append(name(row, f'list-item-{index}'))
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
        column.append(button)
        column.append(self.status)
        column.append(self.last_action)
        column.append(name(Gtk.CheckButton(label='checkbox-basic'), 'checkbox-basic'))

        group = name(Gtk.Frame(label='groupbox-basic'), 'groupbox-basic')
        radios = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4, margin_start=8, margin_end=8)
        first = name(Gtk.CheckButton(label='radio-first', active=True), 'radio-first')
        second = name(Gtk.CheckButton(label='radio-second'), 'radio-second')
        second.set_group(first)
        radios.append(first)
        radios.append(second)
        group.set_child(radios)
        column.append(group)

        column.append(name(Gtk.Entry(), 'textfield-basic'))
        area = Gtk.ScrolledWindow(min_content_height=60)
        area.set_child(name(Gtk.TextView(), 'textarea-basic'))
        column.append(area)

        label = name(Gtk.Label(label='label-basic', xalign=0), 'label-basic')
        self.attach_context_menu(label, column)
        column.append(label)
        column.append(name(Gtk.Inscription(text='text-basic'), 'text-basic'))
        image = name(Gtk.Image.new_from_icon_name('image-x-generic'), 'image-basic')
        image.set_pixel_size(48)
        column.append(image)
        return column

    def build_right_column(self) -> Any:
        column = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, hexpand=True)
        column.append(self.build_combo())
        column.append(self.build_list())
        column.append(self.build_tree())
        slider = name(Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 1), 'slider-basic')
        slider.set_value(25)
        column.append(slider)
        progress = name(Gtk.ProgressBar(fraction=0.4), 'progress-basic')
        column.append(progress)
        column.append(self.build_tabs())
        return column

    def build_main_window(self) -> Any:
        self.install_actions()
        window = Gtk.ApplicationWindow(application=self.app, title=self.args.title)
        window.set_default_size(*MAIN_SIZE)
        window.set_titlebar(Gtk.HeaderBar())

        content = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        content.append(self.build_menubar())
        columns = Gtk.Box(spacing=24, margin_start=12, margin_end=12, margin_top=4, margin_bottom=12)
        columns.append(self.build_left_column())
        columns.append(self.build_right_column())
        content.append(columns)
        window.set_child(content)
        return window

    def build_dialog(self, ident: str, *, modal: bool) -> Any:
        dialog = Gtk.Window(title=ident)
        if modal:
            dialog.set_transient_for(self.window)
            dialog.set_modal(True)
        dialog.set_default_size(*MODELESS_SIZE)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, margin_start=12, margin_end=12, margin_top=12)
        box.append(name(Gtk.Label(label=f'{"Modal" if modal else "Modeless"} dialog', xalign=0), f'{ident}-label'))
        button = name(Gtk.Button(label='Click Me'), f'{ident}-button')
        # Reports through the main window's last-action label: only a click really
        # landing on THIS button can set its ident.
        button.connect('clicked', lambda _button: self.report(f'{ident}-button'))
        box.append(button)
        dialog.set_child(box)
        return dialog

    def present(self) -> None:
        self.window.present()
        self.modeless.present()
        if self.modal is not None:
            self.modal.present()


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
