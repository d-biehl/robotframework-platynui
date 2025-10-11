import os
from PySide6.QtCore import QObject, Slot, QUrl, Signal
from PySide6.QtGui import QGuiApplication
from PySide6.QtQml import QQmlApplicationEngine

from .models import TreeModel, AttributesModel, TreeNode

import platynui_native as pn


class VirtualUiNode:
    """Dummy UiNode für Virtual Root in der UI."""

    @property
    def namespace(self) -> str:
        return "virtual"

    @property
    def role(self) -> str:
        return "root"

    @property
    def name(self) -> str:
        return "Root"

    def attributes(self) -> list[object]:
        return []

    def children(self) -> list[object]:
        return []


class Backend(QObject):
    # Signal um QML über Auswahl-Änderungen zu informieren
    selectedPathChanged = Signal(list)

    def __init__(self, tree_model: TreeModel, attrs_model: AttributesModel) -> None:
        super().__init__()
        self.tree_model = tree_model
        self.attrs_model = attrs_model
        self._selected_path: list[int] = []

    @property
    def selectedPath(self) -> list[int]:
        """Aktuell ausgewählter Pfad."""
        return self._selected_path

    @Slot(list)
    def selectPath(self, row_path: list[int]) -> None:
        """
        row_path: z.B. [2, 0, 1] bedeutet:
        root -> child(2) -> child(0) -> child(1)
        """
        self._selected_path = row_path
        self.selectedPathChanged.emit(row_path)

        node = self.tree_model.node_from_row_path(row_path)
        if node is None:
            self.attrs_model.clear_attrs()
            return
        self.attrs_model.set_attrs_dict(node.attrs)


def run_app(mock: bool=True) -> None:
    app = QGuiApplication([])

    if mock:
        platforms = pn.PlatformOverrides()
        platforms.desktop_info = pn.MOCK_PLATFORM
        platforms.highlight = pn.MOCK_HIGHLIGHT_PROVIDER
        platforms.screenshot = pn.MOCK_SCREENSHOT_PROVIDER
        platforms.pointer = pn.MOCK_POINTER_DEVICE
        platforms.keyboard = pn.MOCK_KEYBOARD_DEVICE

        runtime = pn.Runtime.new_with_providers_and_platforms(
            [pn.MOCK_PROVIDER], platforms
        )
    else:
        runtime = pn.Runtime()

    try:
        # Erstelle den eigentlichen Root-Node
        actual_root = TreeNode(runtime.desktop_node())

        # Erstelle einen Virtual Root nur für die UI, damit der echte Root sichtbar wird
        virtual_root = TreeNode(VirtualUiNode())  # type: ignore
        virtual_root._children = [actual_root]
        actual_root.parent = virtual_root

        tree_model = TreeModel(virtual_root)
        attrs_model = AttributesModel()

        # Backend verbindet Tree-Auswahl mit Attribut-Tabelle
        backend = Backend(tree_model, attrs_model)

        # Modelle und Backend nach QML exportieren (als Singletons)
        engine = QQmlApplicationEngine()
        engine.rootContext().setContextProperty("TreeModel", tree_model)
        engine.rootContext().setContextProperty("AttributesModel", attrs_model)
        engine.rootContext().setContextProperty("Backend", backend)
        qml_file = os.path.join(os.path.dirname(__file__), "main.qml")
        engine.load(QUrl.fromLocalFile(qml_file))
        if not engine.rootObjects():
            raise SystemExit(1)

        app.exec()
    finally:
        runtime.shutdown()


def main() -> None:
    """Entry point for the spy application."""
    run_app()


if __name__ == "__main__":
    main()
