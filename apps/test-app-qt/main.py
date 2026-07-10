# /// script
# requires-python = ">=3.12"
# dependencies = ["PySide6"]
# ///
"""PlatynUI Qt test application (PySide6).

A configurable Qt application that mirrors the role of the egui test app
(`apps/test-app-egui`) for the *native-widget tier* described in
`dev-docs/testing-strategy.md` §5. Where egui provides a flat AccessKit tree,
this app exposes real native Qt controls with full patterns.

Its first purpose is to **reproduce window-bounds bugs** for Qt dialogs: it
opens several ``QDialog`` instances that are **parented to the main window**
(``QDialog(main_window)``) and shown at known positions inside it — the pattern
the affected real app uses (no ``QMdiArea`` involved). A **ground-truth geometry
readout** in the status bar shows the geometry Qt itself reports for each
window, so a test (or a human) can compare it against PlatynUI's ``@Bounds``.

The interesting question these dialogs exercise: a parented ``QDialog`` is a
top-level native window, but where Qt's AT-SPI bridge hangs it in the accessible
tree (under the main window vs. directly under the Application) decides whether
PlatynUI resolves its bounds via the window manager or by summing parent-relative
offsets up the tree.

Ground-truth caveat: Qt only knows a window's global screen position under
**X11**. Under Wayland a client cannot know its absolute position, so the readout
marks the coordinates as unavailable — on Wayland the PlatynUI compositor is the
source of truth, not this app.

Usage
-----
    # Auto-installs PySide6 into an isolated env and runs:
    uv run apps/test-app-qt/main.py

    # Custom app-id / title (X11 WM_CLASS / Wayland app_id):
    uv run apps/test-app-qt/main.py --app-id com.platynui.test.qt --title "My Qt Window"

    # Auto-close after 10s (for CI):
    uv run apps/test-app-qt/main.py --auto-close 10
"""

import argparse
import logging
import sys

from PySide6.QtCore import QPoint, Qt, QTimer
from PySide6.QtGui import QGuiApplication
from PySide6.QtWidgets import (
    QApplication,
    QDialog,
    QDialogButtonBox,
    QLabel,
    QMainWindow,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

log = logging.getLogger('platynui.test-app-qt')

#: Child dialogs opened at startup: (id, title, dx, dy, width, height).
#: dx/dy are offsets from the main window's client-area top-left, so the dialogs
#: appear *inside* the main window — matching the real app, which simply creates
#: dialogs parented to the main window rather than using a QMdiArea.
#:
#: The three dialogs use DISTINCT sizes on purpose so acceptance tests can assert
#: they are pairwise different regardless of window-manager placement.
CHILD_DIALOGS: list[tuple[str, str, int, int, int, int]] = [
    ('child-dialog-1', 'Dialog 1', 40, 60, 260, 180),
    ('child-dialog-2', 'Dialog 2', 160, 200, 300, 220),
    ('child-dialog-3', 'Dialog 3', 340, 90, 220, 160),
]


def _tag(widget: QWidget, ident: str) -> None:
    """Give *widget* a stable identity via its accessible name.

    PlatynUI surfaces Qt's ``accessibleName`` as ``@Name`` on the AT-SPI / UIA
    tree (verified against the running app); ``@Id`` is a compound object path
    (``QApplication.child-dialog-1.…``) and is not used for locating. We also set
    ``objectName`` so the widget is identifiable in Qt-side tooling/logs.
    """
    widget.setObjectName(ident)
    widget.setAccessibleName(ident)


class SampleDialog(QDialog):
    """A simple dialog, parented to the main window, with a stable identity.

    Clicking the dialog's button renames the button's ``@Name`` from
    ``<ident>-button`` to ``<ident>-button-clicked``. That gives acceptance tests
    a purely name-based, cross-platform observable that a pointer click actually
    landed *inside this dialog* — the real-world payoff of correct bounds (before
    the fix, a click at the dialog's coordinates hit the main window instead).
    """

    def __init__(self, ident: str, title: str, *, modal: bool, parent: QWidget) -> None:
        super().__init__(parent)
        self._ident = ident
        _tag(self, ident)
        self.setWindowTitle(title)
        self.setModal(modal)
        layout = QVBoxLayout(self)
        label = QLabel(f'{"Modal" if modal else "Modeless"} dialog: {ident}')
        _tag(label, f'{ident}-label')
        self._button = QPushButton('Click Me')
        _tag(self._button, f'{ident}-button')
        self._button.clicked.connect(self._on_button_clicked)
        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Ok | QDialogButtonBox.StandardButton.Cancel)
        _tag(buttons, f'{ident}-buttons')
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        layout.addWidget(label)
        layout.addWidget(self._button)
        layout.addStretch(1)
        layout.addWidget(buttons)

    def _on_button_clicked(self) -> None:
        _tag(self._button, f'{self._ident}-button-clicked')
        self._button.setText('Clicked')


class MainWindow(QMainWindow):
    """Main window that owns the child dialogs and the geometry readout."""

    def __init__(self, title: str, *, open_modal: bool = False, dialog_count: int = len(CHILD_DIALOGS)) -> None:
        super().__init__()
        _tag(self, 'main-window')
        self.setWindowTitle(title)
        self.resize(900, 640)

        self._is_x11 = QGuiApplication.platformName().lower().startswith('xcb')
        self._open_modal_on_start = open_modal
        self._dialog_count = dialog_count
        self._dialogs: list[QDialog] = []

        central = QLabel(
            'Main window. Child dialogs are parented to this window and shown\n'
            'at known positions. Compare the readout below against @Bounds.'
        )
        central.setAlignment(Qt.AlignmentFlag.AlignCenter)
        _tag(central, 'central-label')
        self.setCentralWidget(central)

        self._build_readout()

        # The main window's final geometry is only known once it is shown, so
        # spawn+position the child dialogs on the next event-loop turn.
        QTimer.singleShot(0, self._spawn_child_dialogs)

        # Refresh the ground-truth readout periodically so it tracks moves/resizes.
        self._timer = QTimer(self)
        self._timer.timeout.connect(self._refresh_readout)
        self._timer.start(250)
        self._refresh_readout()

    def _build_readout(self) -> None:
        self.readout = QLabel()
        _tag(self.readout, 'geometry-readout')
        self.readout.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        self.statusBar().addWidget(self.readout, 1)
        self.statusBar().setObjectName('main-statusbar')

    def _spawn_child_dialogs(self) -> None:
        origin = self.geometry().topLeft()  # main window client-area top-left, global
        for ident, title, dx, dy, w, h in CHILD_DIALOGS[: self._dialog_count]:
            dialog = SampleDialog(ident, title, modal=False, parent=self)
            dialog.resize(w, h)
            self._register_dialog(dialog)
            dialog.show()
            # Position inside the main window; on Wayland move() is best-effort.
            dialog.move(origin + QPoint(dx, dy))

        # A modal dialog opened at startup (via --open-modal), for the modal-bounds
        # acceptance test. Opened here rather than on demand so no pointer click is
        # needed to bring it up.
        if self._open_modal_on_start:
            modal = SampleDialog('dialog-modal', 'Modal Dialog', modal=True, parent=self)
            modal.resize(340, 200)
            self._register_dialog(modal)
            # show() (not open()) keeps setModal(True)'s ApplicationModal modality,
            # which is what surfaces as the AT-SPI modal state; open() would
            # downgrade it to WindowModal. It is non-blocking either way.
            modal.show()
            modal.move(origin + QPoint(120, 300))

    def _register_dialog(self, dialog: QDialog) -> None:
        self._dialogs.append(dialog)
        dialog.finished.connect(lambda _=0, d=dialog: self._forget_dialog(d))

    def _forget_dialog(self, dialog: QDialog) -> None:
        if dialog in self._dialogs:
            self._dialogs.remove(dialog)
        dialog.deleteLater()

    def _refresh_readout(self) -> None:
        lines = ['Ground-truth geometry (Qt):', self._line('main-window', self)]
        lines.extend(self._line(d.objectName(), d) for d in self._dialogs if d.isVisible())
        self.readout.setText('\n'.join(lines))

    def _line(self, ident: str, widget: QWidget) -> str:
        # For a top-level window, geometry() is the CLIENT rect (excludes WM
        # decorations) — the same rect PlatynUI derives on X11 from
        # translate_coordinates(client origin) + get_geometry(size).
        g = widget.geometry()
        if self._is_x11:
            pos = f'x={g.x()} y={g.y()}'
        else:
            pos = 'x=? y=? (no global coords on Wayland)'
        return f'  {ident}: {pos} w={g.width()} h={g.height()}  (client, global)'


def _init_logging(level: str) -> None:
    logging.basicConfig(
        stream=sys.stderr,
        level=getattr(logging, level.upper()),
        format='%(levelname)s %(name)s: %(message)s',
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog='platynui-test-app-qt',
        description='Qt (PySide6) test application for PlatynUI integration and functional testing',
    )
    parser.add_argument(
        '--app-id',
        default='org.platynui.test.qt',
        help='Application ID (Wayland app_id / X11 WM_CLASS).',
    )
    parser.add_argument('--title', default='PlatynUI Test App (Qt)', help='Main window title.')
    parser.add_argument('--auto-close', type=int, default=0, help='Auto-close after N seconds (0 = never).')
    parser.add_argument(
        '--open-modal',
        action='store_true',
        help='Also open a modal dialog at startup (for the modal-bounds acceptance test).',
    )
    parser.add_argument(
        '--dialogs',
        type=int,
        default=len(CHILD_DIALOGS),
        help=f'Number of child dialogs to open (0..{len(CHILD_DIALOGS)}). Use 1 to avoid overlap when '
        "clicking a dialog's widgets (e.g. on Wayland, where all windows stack at the origin).",
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
    log.info('starting test application app_id=%s title=%s', args.app_id, args.title)

    # Sets WM_CLASS on X11 and app_id on Wayland so the window is matchable.
    QApplication.setApplicationName(args.app_id)
    QApplication.setDesktopFileName(args.app_id)

    app = QApplication(sys.argv[:1])
    window = MainWindow(args.title, open_modal=args.open_modal, dialog_count=args.dialogs)
    window.show()

    if args.auto_close > 0:
        QTimer.singleShot(args.auto_close * 1000, app.quit)

    return app.exec()


if __name__ == '__main__':
    raise SystemExit(main())
