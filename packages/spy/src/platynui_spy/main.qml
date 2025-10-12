import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQml.Models

ApplicationWindow {
    width: 900
    height: 560
    visible: true
    title: "PlatynUI Spy"

    SplitView {
        anchors.fill: parent
        orientation: Qt.Horizontal

        Frame {
            SplitView.preferredWidth: 360
            SplitView.minimumWidth: 220
            padding: 0

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                Label {
                    id: treeLabel
                    text: "Baum"
                    font.bold: true
                    padding: 8
                    Layout.fillWidth: true
                    Layout.preferredHeight: 40
                }

                TreeView {
                    id: tree
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    model: TreeModel
                    clip: true
                    resizableColumns: false

                    ScrollBar.vertical: ScrollBar {
                        policy: ScrollBar.AsNeeded
                        implicitWidth: 12
                    }
                    ScrollBar.horizontal: ScrollBar {
                        policy: ScrollBar.AsNeeded
                        implicitHeight: 12
                    }

                    selectionModel: ItemSelectionModel {
                        model: TreeModel
                    }

                    delegate: TreeViewDelegate {}

                    Connections {
                        target: tree.selectionModel
                        function onCurrentChanged(current, previous) {
                            if (!current.valid) {
                                Backend.selectTreeNode(null);
                                return;
                            }
                            var treeNode = tree.model.data(current, Qt.UserRole + 3);
                            Backend.selectTreeNode(treeNode);
                        }
                    }
                }
            }
        }

        Frame {
            SplitView.fillWidth: true
            SplitView.minimumWidth: 260
            padding: 0

            ColumnLayout {
                anchors.fill: parent
                spacing: 0

                Label {
                    id: attributeLabel
                    text: "Attribute"
                    font.bold: true
                    padding: 8
                    Layout.fillWidth: true
                    Layout.preferredHeight: 40
                }

                HorizontalHeaderView {
                    id: attributeHeader
                    Layout.fillWidth: true
                    Layout.preferredHeight: 30
                    syncView: attrTable
                    model: ["Name", "Value"]
                    delegate: Label {
                        verticalAlignment: Text.AlignVCenter
                        horizontalAlignment: Text.AlignLeft
                        padding: 4
                        font.bold: true
                        text: modelData
                    }
                }

                TableView {
                    id: attrTable
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    resizableColumns: true
                    model: AttributesModel
                    clip: true

                    ScrollBar.vertical: ScrollBar {
                        policy: ScrollBar.AsNeeded
                        implicitWidth: 12
                    }
                    ScrollBar.horizontal: ScrollBar {
                        policy: ScrollBar.AsNeeded
                        implicitHeight: 12
                    }

                    selectionModel: ItemSelectionModel {
                        model: AttributesModel
                    }

                    delegate: TableViewDelegate {}
                    columnSpacing: 2
                }
            }
        }
    }
}
