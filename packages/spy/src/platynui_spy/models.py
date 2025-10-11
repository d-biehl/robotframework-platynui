from typing import Any
from PySide6.QtCore import (
    QAbstractItemModel,
    QModelIndex,
    QPersistentModelIndex,
    Qt,
    QByteArray,
)

from platynui_native import UiNode


# --------- Baumdaten ---------
class TreeNode:
    def __init__(self, ui_node: UiNode, parent: "TreeNode | None" = None) -> None:
        self.ui_node = ui_node
        self.parent = parent
        self._children: list["TreeNode"] | None = None
        self._attrs: dict[str, Any] = {}

    @property
    def name(self) -> str:
        return f"{self.ui_node.namespace}:{self.ui_node.role} '{self.ui_node.name}'"

    @property
    def attrs(self) -> dict[str, Any]:
        """Attribute des UI-Knotens als Dictionary."""
        attrs: dict[str, Any] = {}
        for attr in self.ui_node.attributes():
            attrs[f"{attr.namespace}:{attr.name}"] = attr.value()

        return attrs

    @property
    def children(self) -> list["TreeNode"]:
        """Lazy-Loading der Kinder."""
        if self._children is None:
            self._children = []
            # UiNode hat immer eine children() Methode
            try:
                for child_ui_node in self.ui_node.children():
                    child_tree_node = TreeNode(child_ui_node, parent=self)
                    self._children.append(child_tree_node)
            except Exception:
                # Falls ein Fehler auftritt, leere Liste zurückgeben
                pass
        return self._children


# --------- TreeModel (hierarchisch) ----------
class TreeModel(QAbstractItemModel):
    NameRole = Qt.ItemDataRole.UserRole + 1
    AttrsRole = Qt.ItemDataRole.UserRole + 2

    def __init__(self, root: TreeNode) -> None:
        super().__init__()
        self._root = root

    # QAbstractItemModel Pflichtmethoden
    def index(
        self,
        row: int,
        column: int,
        parent: QModelIndex | QPersistentModelIndex = QModelIndex(),
    ) -> QModelIndex:
        if not self.hasIndex(row, column, parent):
            return QModelIndex()
        parent_node = parent.internalPointer() if parent.isValid() else self._root
        if 0 <= row < len(parent_node.children):
            return self.createIndex(row, column, parent_node.children[row])
        return QModelIndex()

    def parent(  # type: ignore[override]
        self, child_index: QModelIndex | QPersistentModelIndex, /
    ) -> QModelIndex:
        if not child_index.isValid():
            return QModelIndex()
        node = child_index.internalPointer()
        if not node or node.parent is None:
            return QModelIndex()
        parent_node = node.parent
        grand = parent_node.parent or self._root
        if grand is self._root:
            row = self._root.children.index(parent_node) if parent_node in self._root.children else 0
        else:
            row = grand.children.index(parent_node) if parent_node in grand.children else 0
        return self.createIndex(row, 0, parent_node)

    def rowCount(
        self, parent: QModelIndex | QPersistentModelIndex = QModelIndex()
    ) -> int:
        node = parent.internalPointer() if parent.isValid() else self._root
        return len(node.children)

    def columnCount(
        self, parent: QModelIndex | QPersistentModelIndex = QModelIndex()
    ) -> int:
        return 1  # wir zeigen nur den Namen

    def data(
        self,
        index: QModelIndex | QPersistentModelIndex,
        role: int = Qt.ItemDataRole.DisplayRole,
    ) -> Any:
        if not index.isValid():
            return None
        node: TreeNode = index.internalPointer()
        if role in (Qt.ItemDataRole.DisplayRole, self.NameRole):
            return node.name
        if role == self.AttrsRole:
            return node.attrs
        return None

    def flags(self, index: QModelIndex | QPersistentModelIndex) -> Qt.ItemFlag:
        if not index.isValid():
            return Qt.ItemFlag.NoItemFlags
        return Qt.ItemFlag.ItemIsEnabled | Qt.ItemFlag.ItemIsSelectable

    def hasChildren(
        self, parent: QModelIndex | QPersistentModelIndex = QModelIndex()
    ) -> bool:
        """Gibt an, ob ein Index Kinder hat - wichtig für TreeView Expansion."""
        node = parent.internalPointer() if parent.isValid() else self._root
        return len(node.children) > 0

    def roleNames(self) -> dict[int, QByteArray]:
        roles = super().roleNames()
        roles[Qt.ItemDataRole.DisplayRole] = QByteArray(b"display")
        roles[self.NameRole] = QByteArray(b"name")
        roles[self.AttrsRole] = QByteArray(b"attrs")
        return roles

    # Hilfsfunktion für Backend: Node über Row-Pfad finden
    def node_from_row_path(self, row_path: list[int]) -> TreeNode | None:
        node: TreeNode = self._root
        try:
            for r in row_path:
                node = node.children[int(r)]
            return node
        except Exception:
            return None


# --------- flache Attribut-Tabelle (Name/Value) als 2-Spalten-Modell ----------
class AttributesModel(QAbstractItemModel):
    NameRole = Qt.ItemDataRole.UserRole + 1
    ValueRole = Qt.ItemDataRole.UserRole + 2

    def __init__(self) -> None:
        super().__init__()
        self._items: list[tuple[str, Any]] = []  # list[tuple(name, value)]

    def rowCount(
        self, parent: QModelIndex | QPersistentModelIndex = QModelIndex()
    ) -> int:
        return 0 if parent.isValid() else len(self._items)

    def columnCount(
        self, parent: QModelIndex | QPersistentModelIndex = QModelIndex()
    ) -> int:
        return 0 if parent.isValid() else 2  # Name + Value Spalten

    def data(
        self,
        index: QModelIndex | QPersistentModelIndex,
        role: int = Qt.ItemDataRole.DisplayRole,
    ) -> Any:
        if not index.isValid() or index.row() >= len(self._items):
            return None

        name, value = self._items[index.row()]

        if role == Qt.ItemDataRole.DisplayRole:
            if index.column() == 0:
                return name
            elif index.column() == 1:
                return str(value)
        elif role == self.NameRole:
            return name
        elif role == self.ValueRole:
            return str(value)

        return None

    def index(self, row: int, column: int, parent: QModelIndex | QPersistentModelIndex = QModelIndex()) -> QModelIndex:
        if parent.isValid() or row < 0 or row >= len(self._items) or column < 0 or column >= 2:
            return QModelIndex()
        return self.createIndex(row, column)

    def parent(self, child: QModelIndex) -> QModelIndex:  # type: ignore[override]
        return QModelIndex()  # Flat table, no hierarchy

    def roleNames(self) -> dict[int, QByteArray]:
        roles = super().roleNames()
        roles[Qt.ItemDataRole.DisplayRole] = QByteArray(b"display")
        roles[self.NameRole] = QByteArray(b"name")
        roles[self.ValueRole] = QByteArray(b"value")
        return roles

    # API fürs Backend
    def clear_attrs(self) -> None:
        self.beginResetModel()
        self._items = []
        self.endResetModel()

    def set_attrs_dict(self, d: dict[str, Any]) -> None:
        self.beginResetModel()
        self._items = list(d.items())
        self.endResetModel()
