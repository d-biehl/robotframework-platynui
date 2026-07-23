# /// script
# requires-python = ">=3.12"
# dependencies = ["PySide6"]
# ///
"""PlatynUI Qt Quick (QML) test application (PySide6).

The Qt Quick row of the fixture technology matrix (`dev-docs/testing-strategy.md`
§5) and the first full instance of the fixture blueprint (§5.1): the core-tier
control catalog under the canonical names, wired through QML ``Accessible``
attached properties. Unlike the Qt Widgets fixture (`apps/test-app-qt`), Qt
Quick renders its own scene graph — accessibility comes from the ``Accessible``
attachments routed through Qt's bridge (UIA on Windows, AT-SPI on Linux), and
menus/popups are by default *in-scene items*, not native windows.

The Python side stays deliberately thin — argument parsing, the tree model, and
the engine bootstrap. The catalog itself lives in ``Main.qml``, which is how
real QML applications are structured.

Usage
-----
    # On the project venv (PySide6 is a dev dependency, installed by `uv sync`):
    uv run python apps/test-app-qml/main.py

    # Standalone (no project sync): PEP 723 inline metadata auto-installs PySide6:
    uv run apps/test-app-qml/main.py

    # Options:
    uv run apps/test-app-qml/main.py --app-id com.platynui.test.qml --title "My QML Window"
    uv run apps/test-app-qml/main.py --auto-close 10          # self-close for CI
    uv run apps/test-app-qml/main.py --open-modal             # open the in-scene modal dialog at startup
    uv run apps/test-app-qml/main.py --popup-mode native      # menus/popups as native windows (Qt >= 6.8)
    uv run apps/test-app-qml/main.py --log-level info         # error|warn|info|debug
"""

import argparse
import logging
import os
import sys
from pathlib import Path

from PySide6.QtCore import QTimer, QUrl
from PySide6.QtGui import QGuiApplication, QStandardItem, QStandardItemModel
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtQuickControls2 import QQuickStyle

log = logging.getLogger('platynui.test-app-qml')

type TreeSpec = dict[str, 'TreeSpec']

#: The blueprint tree: three levels under two roots, canonical names from
#: dev-docs/testing-strategy.md §5.1. Nested dicts: name -> children.
TREE_NODES: TreeSpec = {
    'tree-node-a': {
        'tree-node-a-1': {'tree-node-a-1-i': {}},
        'tree-node-a-2': {},
    },
    'tree-node-b': {},
}


def _append_nodes(parent: QStandardItem, nodes: TreeSpec) -> None:
    for name, children in nodes.items():
        item = QStandardItem(name)
        item.setEditable(False)
        parent.appendRow(item)
        _append_nodes(item, children)


def _build_tree_model() -> QStandardItemModel:
    """Tree model for the QML ``TreeView`` (`tree-basic`), built Python-side
    because Qt Quick's TreeView needs a ``QAbstractItemModel``."""
    model = QStandardItemModel()
    _append_nodes(model.invisibleRootItem(), TREE_NODES)
    return model


def _init_logging(level: str) -> None:
    logging.basicConfig(
        stream=sys.stderr,
        level=getattr(logging, level.upper()),
        format='%(levelname)s %(name)s: %(message)s',
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog='platynui-test-app-qml',
        description='Qt Quick/QML (PySide6) test application for PlatynUI integration and functional testing',
    )
    parser.add_argument(
        '--app-id',
        default='org.platynui.test.qml',
        help='Application ID (Wayland app_id / X11 WM_CLASS).',
    )
    parser.add_argument('--title', default='PlatynUI QML TestApp', help='Main window title.')
    parser.add_argument('--auto-close', type=int, default=0, help='Auto-close after N seconds (0 = never).')
    parser.add_argument(
        '--open-modal',
        action='store_true',
        help='Also open the in-scene modal dialog (dialog-modal) at startup.',
    )
    parser.add_argument(
        '--popup-mode',
        choices=['inscene', 'native'],
        default='inscene',
        help='Menus/popups as in-scene items (Qt Quick default) or native popup windows (Qt >= 6.8 popupType).',
    )
    parser.add_argument(
        '--log-level',
        choices=['error', 'warn', 'info', 'debug'],
        default='warn',
        help='Log level.',
    )
    args = parser.parse_args(argv)

    # Python's logging has no "warn"; map it to WARNING.
    _init_logging('warning' if args.log_level == 'warn' else args.log_level)
    log.info('starting test application app_id=%s title=%s popup_mode=%s', args.app_id, args.title, args.popup_mode)

    # Sets WM_CLASS on X11 and app_id on Wayland so the window is matchable.
    QGuiApplication.setApplicationName(args.app_id)
    QGuiApplication.setDesktopFileName(args.app_id)

    # A consistent Controls style that follows the SYSTEM light/dark theme —
    # without this the default can mix palettes (dark window, light menus).
    # FluentWinUI3 tracks the Windows 11 system theme; Fusion follows the
    # system palette elsewhere. An explicit QT_QUICK_CONTROLS_STYLE env wins.
    if 'QT_QUICK_CONTROLS_STYLE' not in os.environ:
        QQuickStyle.setStyle('FluentWinUI3' if sys.platform == 'win32' else 'Fusion')

    app = QGuiApplication(sys.argv[:1])

    # Keep a reference for the app's lifetime — the engine does not own it.
    tree_model = _build_tree_model()

    engine = QQmlApplicationEngine()
    engine.rootContext().setContextProperty('treeModel', tree_model)
    engine.setInitialProperties(
        {
            'appTitle': args.title,
            'openModalOnStart': args.open_modal,
            'nativePopups': args.popup_mode == 'native',
        }
    )
    engine.load(QUrl.fromLocalFile(str(Path(__file__).resolve().parent / 'Main.qml')))
    if not engine.rootObjects():
        log.error('failed to load Main.qml')
        return 1

    if args.auto_close > 0:
        QTimer.singleShot(args.auto_close * 1000, app.quit)

    return app.exec()


if __name__ == '__main__':
    raise SystemExit(main())
