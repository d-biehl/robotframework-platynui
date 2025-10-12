import os
from PySide6.QtCore import QObject, Slot, QUrl, QTimer
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
    def __init__(self, tree_model: TreeModel, attrs_model: AttributesModel) -> None:
        super().__init__()
        self.tree_model = tree_model
        self.attrs_model = attrs_model
        self._selected_node: TreeNode | None = None
        self._pending_node: TreeNode | None = None
        self._load_timer = QTimer(self)
        self._load_timer.setSingleShot(True)
        self._load_timer.timeout.connect(self._load_attributes)

    @Slot(object)
    def selectTreeNode(self, tree_node: object) -> None:
        """Update the attribute table whenever the tree selection changes."""
        if isinstance(tree_node, TreeNode):
            self._selected_node = tree_node
            needs_deferred = self.attrs_model.set_tree_node(tree_node)
            if needs_deferred:
                self._pending_node = tree_node
                self._load_timer.start(75)
            else:
                self._pending_node = None
            return
        self._selected_node = None
        self.attrs_model.clear_attrs()
        self._pending_node = None
        self._load_timer.stop()

    def _load_attributes(self) -> None:
        node = self._pending_node
        if node is None or node is not self._selected_node:
            return
        attrs = node.compute_attrs()
        node.cache_attrs(attrs)
        if node is self._selected_node:
            self.attrs_model.set_attrs_dict(attrs)
        self._pending_node = None


def run_app(mock: bool=False) -> None:
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
