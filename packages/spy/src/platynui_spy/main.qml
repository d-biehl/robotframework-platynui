import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQml.Models

ApplicationWindow {
    width: 900
    height: 560
    visible: true
    title: "Tree + Attribute-Ansicht (PySide6 + QtQuick)"

    SplitView {
        anchors.fill: parent

        Frame {
            SplitView.preferredWidth: 360
            SplitView.minimumWidth: 220
            padding: 0

            ColumnLayout {
                anchors.fill: parent
                spacing: 6

                Label {
                    text: "Baum"
                    font.bold: true
                    padding: 8
                }

                TreeView {
                    id: tree
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    model: TreeModel

                    selectionModel: ItemSelectionModel {
                        model: TreeModel
                    }

                    delegate: TreeViewDelegate {}

                    Connections {
                        target: tree.selectionModel
                        function onCurrentChanged(current, previous) {
                            if (current.valid) {
                                var path = tree.rowPath(current);
                                Backend.selectPath(path);
                            }
                        }
                    }

                    function rowPath(idx) {
                        var path = [];
                        var cur = idx;
                        while (cur.valid) {
                            path.unshift(cur.row);
                            cur = cur.parent;
                        }
                        return path;
                    }
                }
            }
        }

        Frame {
            padding: 0
            ColumnLayout {
                anchors.fill: parent
                spacing: 6

                Label {
                    text: "Attribute"
                    font.bold: true
                    padding: 8
                }

                HorizontalHeaderView {
                    id: attributeHeader
                    Layout.fillWidth: true
                    syncView: attrTable
                    model: ["Name", "Value"]
                }

                TableView {
                    id: attrTable
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    model: AttributesModel
                    clip: true

                    selectionModel: ItemSelectionModel {
                        model: AttributesModel
                    }

                    delegate: TableViewDelegate {}

                    columnWidthProvider: function (column) {
                        if (column === 0)
                            return attrTable.width * 0.45;
                        if (column === 1)
                            return attrTable.width * 0.55;
                        return 100;
                    }
                }
            }
        }
    }
}
