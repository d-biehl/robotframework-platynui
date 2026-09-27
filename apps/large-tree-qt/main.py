# /// script
# requires-python = ">=3.12"
# dependencies = ["PySide6"]
# ///
"""A Qt window with a large, deterministic accessibility tree.

A measurement helper, not a fixture of the test-app blueprint
(`dev-docs/testing-strategy.md` §5): it has no control catalog, only many plain
widgets under stable names. It exists so that a measurement can build large UI
snapshots against an application the repository owns, whose tree is the same on
every run. `measure_snapshot_memory.py` next to it starts it.

The main window holds ``--groups`` group boxes, ``group-<g>``, with ``--items``
widgets each, ``item-<g>-<i>``, cycling through a push button, a label and a
check box. Every widget carries its name as ``accessibleName``, which Qt exposes
through UI Automation on Windows and AT-SPI on Linux. The label
``widget-count-<n>`` names how many widgets the tree holds, groups included.

Usage
-----
    uv run python apps/large-tree-qt/main.py --groups 50 --items 20
"""

import argparse
import sys

from PySide6.QtCore import QTimer
from PySide6.QtWidgets import (
    QApplication,
    QCheckBox,
    QGridLayout,
    QGroupBox,
    QLabel,
    QMainWindow,
    QPushButton,
    QScrollArea,
    QVBoxLayout,
    QWidget,
)

#: Group boxes per row of the window.
COLUMNS = 5


def _tag(widget: QWidget, ident: str) -> None:
    """Give *widget* the stable name that locates it: its accessible name."""
    widget.setObjectName(ident)
    widget.setAccessibleName(ident)


def _item(group: int, index: int) -> QWidget:
    """The widget at *index* of *group*: a button, a label or a check box, in turn."""
    ident = f'item-{group}-{index}'
    widget: QWidget
    match index % 3:
        case 0:
            widget = QPushButton(ident)
        case 1:
            widget = QLabel(ident)
        case _:
            widget = QCheckBox(ident)
    _tag(widget, ident)
    return widget


class MainWindow(QMainWindow):
    """The window with *groups* group boxes of *items* widgets each."""

    def __init__(self, title: str, groups: int, items: int) -> None:
        super().__init__()
        _tag(self, 'large-tree-window')
        self.setWindowTitle(title)
        self.resize(1000, 700)

        content = QWidget()
        _tag(content, 'content')
        layout = QVBoxLayout(content)
        count = groups + groups * items
        summary = QLabel(f'widget-count-{count}')
        _tag(summary, f'widget-count-{count}')
        layout.addWidget(summary)

        grid = QGridLayout()
        layout.addLayout(grid)
        for group in range(groups):
            box = QGroupBox(f'group-{group}')
            _tag(box, f'group-{group}')
            box_layout = QVBoxLayout(box)
            for index in range(items):
                box_layout.addWidget(_item(group, index))
            grid.addWidget(box, group // COLUMNS, group % COLUMNS)

        scroll = QScrollArea()
        _tag(scroll, 'scroll')
        scroll.setWidgetResizable(True)
        scroll.setWidget(content)
        self.setCentralWidget(scroll)


def _positive(text: str) -> int:
    """An argparse type for a count of at least 1."""
    value = int(text)
    if value < 1:
        raise argparse.ArgumentTypeError(f'must be at least 1, got {value}')
    return value


def main(argv: list[str] | None = None) -> int:
    """Start the application and run it until it is closed or auto-closes."""
    parser = argparse.ArgumentParser(
        prog='platynui-large-tree-qt',
        description='A Qt window with a large, deterministic accessibility tree, for measurements.',
    )
    parser.add_argument('--app-id', default='org.platynui.large-tree', help='Wayland app_id / X11 WM_CLASS.')
    parser.add_argument('--title', default='PlatynUI Large Tree', help='Window title.')
    parser.add_argument('--groups', type=_positive, default=50, help='Number of group boxes.')
    parser.add_argument('--items', type=_positive, default=20, help='Number of widgets per group box.')
    parser.add_argument('--auto-close', type=int, default=0, help='Close after N seconds (0 = never).')
    args = parser.parse_args(argv)

    QApplication.setApplicationName(args.app_id)
    QApplication.setDesktopFileName(args.app_id)
    app = QApplication(sys.argv[:1])
    window = MainWindow(args.title, args.groups, args.items)
    window.show()
    if args.auto_close > 0:
        QTimer.singleShot(args.auto_close * 1000, app.quit)
    return app.exec()


if __name__ == '__main__':
    raise SystemExit(main())
