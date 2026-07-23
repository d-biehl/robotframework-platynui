// PlatynUI QML fixture — the blueprint core-tier catalog (dev-docs/testing-strategy.md §5.1)
// plus the optional custom-controls chapter. Every interactive control carries an explicit
// Accessible.name (the locator contract); action observables are name-based (counter label,
// last-action label) so no test needs screenshots and no control ever changes its name.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: root

    // Set from Python via setInitialProperties (see main.py).
    property string appTitle: "PlatynUI QML TestApp"
    property bool openModalOnStart: false
    property bool nativePopups: false

    // In-scene popups (Popup.Item) are Qt Quick's default and the hard case for
    // bounds/hit-testing; --popup-mode native flips every menu to a native
    // top-level popup window (Popup.Window, Qt >= 6.8). Names stay identical.
    readonly property int menuPopupType: nativePopups ? Popup.Window : Popup.Item

    property int clickCount: 0
    property int customClickCount: 0
    // Ident of the last activated menu item / dialog button, reported through the
    // always-visible last-action label ("none" until the first activation).
    property string lastAction: "none"

    title: appTitle
    visible: true
    width: 1080
    height: 720

    menuBar: MenuBar {
        Accessible.name: "main-menubar"

        Menu {
            title: "menu-file"
            popupType: root.menuPopupType

            // Activation reports the item's ident through the main window's
            // last-action label — observable without reopening the menu, and
            // no item ever changes its name.
            MenuItem {
                text: "menu-file-new"
                onTriggered: root.lastAction = text
            }
            MenuItem {
                text: "menu-file-open"
                onTriggered: root.lastAction = text
            }
            MenuItem {
                text: "menu-file-quit"
                onTriggered: root.lastAction = text
            }
        }

        Menu {
            title: "menu-edit"
            popupType: root.menuPopupType

            MenuItem {
                text: "menu-edit-undo"
                onTriggered: root.lastAction = text
            }
            MenuItem {
                text: "menu-edit-redo"
                onTriggered: root.lastAction = text
            }

            // Menu-bar submenu (the context menu has its own, ctx-more): both
            // cascade paths of a real menu hierarchy are part of the catalog.
            Menu {
                title: "menu-edit-more"
                popupType: root.menuPopupType

                MenuItem {
                    text: "menu-edit-sub-one"
                    onTriggered: root.lastAction = text
                }
                MenuItem {
                    text: "menu-edit-sub-two"
                    onTriggered: root.lastAction = text
                }
            }
        }

        Menu {
            title: "menu-help"
            popupType: root.menuPopupType

            MenuItem {
                text: "menu-help-about"
                onTriggered: root.lastAction = text
            }
        }
    }

    // Right-click context menu, opened from the background MouseArea below.
    // NOTE: the Menu popup itself carries no accessible name — Accessible only
    // attaches to Items/Actions, not to Menu/Popup (verified: attaching warns
    // and does nothing). The locator contract lives on the ITEMS; see README.
    Menu {
        id: contextMenu

        popupType: root.menuPopupType

        MenuItem {
            text: "ctx-cut"
            onTriggered: root.lastAction = text
        }
        MenuItem {
            text: "ctx-copy"
            onTriggered: root.lastAction = text
        }
        MenuItem {
            text: "ctx-paste"
            onTriggered: root.lastAction = text
        }

        Menu {
            title: "ctx-more"
            popupType: root.menuPopupType

            MenuItem {
                text: "ctx-sub-alpha"
                onTriggered: root.lastAction = text
            }
            MenuItem {
                text: "ctx-sub-beta"
                onTriggered: root.lastAction = text
            }
        }
    }

    // Behind all controls: right-click anywhere on empty background opens the
    // context menu (left clicks pass through to the controls on top).
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.RightButton
        onClicked: mouse => contextMenu.popup(mouse.x, mouse.y)
    }

    GridLayout {
        anchors.fill: parent
        anchors.margins: 16
        columns: 3
        columnSpacing: 24
        rowSpacing: 8

        // ── Column 1: basic controls ────────────────────────────────────────
        ColumnLayout {
            Layout.alignment: Qt.AlignTop
            spacing: 8

            Button {
                text: "Click Me"
                Accessible.name: "button-basic"
                onClicked: root.clickCount++
            }
            // Counter observable: name and text are "status-label-clicks-<n>".
            Label {
                text: "status-label-clicks-" + root.clickCount
                Accessible.name: text
            }
            // Last-action observable: name and text are "last-action-<ident>" of
            // the last activated menu item / dialog button ("last-action-none"
            // until the first activation).
            Label {
                text: "last-action-" + root.lastAction
                Accessible.name: text
            }
            CheckBox {
                text: "Check me"
                Accessible.name: "checkbox-basic"
            }
            // The radio group lives inside the catalog group box — grouping
            // controls under a titled container is realistic UI and gives the
            // catalog its container-with-children surface.
            GroupBox {
                title: "groupbox-basic"
                Accessible.name: "groupbox-basic"

                ColumnLayout {
                    spacing: 4

                    RadioButton {
                        text: "First"
                        checked: true
                        Accessible.name: "radio-first"
                    }
                    RadioButton {
                        text: "Second"
                        Accessible.name: "radio-second"
                    }
                }
            }
            TextField {
                placeholderText: "type here"
                Accessible.name: "textfield-basic"
            }
            // Multi-line editing is its own catalog control: Enter/wrapping
            // behavior differs from the single-line field in every toolkit.
            // Tall enough that several wrapped lines are visibly multi-line.
            Frame {
                Layout.preferredWidth: 260
                Layout.preferredHeight: 150
                padding: 0

                TextArea {
                    anchors.fill: parent
                    text: "multi-line text:\nline one\nline two"
                    wrapMode: TextArea.Wrap
                    Accessible.name: "textarea-basic"
                }
            }
            Label {
                text: "Static label"
                Accessible.name: "label-basic"
            }
            ComboBox {
                Layout.preferredWidth: 180
                model: ["combo-item-1", "combo-item-2", "combo-item-3"]
                Accessible.name: "combobox-basic"
            }
        }

        // ── Column 2: containers (list, tree) ───────────────────────────────
        ColumnLayout {
            Layout.alignment: Qt.AlignTop
            Layout.fillHeight: true
            spacing: 8

            Frame {
                Layout.preferredWidth: 220
                Layout.preferredHeight: 220

                ListView {
                    id: listBasic

                    anchors.fill: parent
                    clip: true
                    model: 5
                    Accessible.name: "list-basic"

                    delegate: ItemDelegate {
                        required property int index

                        width: listBasic.width
                        text: "list-item-" + (index + 1)
                        Accessible.name: text
                        highlighted: ListView.isCurrentItem
                        onClicked: listBasic.currentIndex = index
                    }
                }
            }
            Frame {
                Layout.preferredWidth: 220
                Layout.preferredHeight: 260

                TreeView {
                    id: treeBasic

                    anchors.fill: parent
                    clip: true
                    model: treeModel
                    Accessible.name: "tree-basic"

                    delegate: TreeViewDelegate {
                        Accessible.name: model.display
                    }
                }
            }
            // Plain static text (QQuickText, not a Controls Label) — the
            // second face of "static text" real QML apps use.
            Text {
                text: "Plain text element"
                color: root.palette.windowText
                Accessible.role: Accessible.StaticText
                Accessible.name: "text-basic"
            }
            Image {
                source: "icon.png"
                sourceSize.width: 64
                sourceSize.height: 64
                Accessible.role: Accessible.Graphic
                Accessible.name: "image-basic"
            }
        }

        // ── Column 3: custom-controls chapter ───────────────────────────────
        ColumnLayout {
            Layout.alignment: Qt.AlignTop
            spacing: 8

            // Self-drawn activatable control with manually wired accessibility —
            // the lower bound real QML apps exhibit (Rectangle + handler, no
            // Controls involvement).
            Rectangle {
                id: customButton

                width: 140
                height: 36
                radius: 4
                color: customTap.pressed ? "#5a5a5a" : "#3d3d3d"
                Accessible.role: Accessible.Button
                Accessible.name: "custom-button"
                Accessible.focusable: true
                Accessible.onPressAction: root.customClickCount++

                Text {
                    anchors.centerIn: parent
                    text: "Custom"
                    color: "white"
                }
                TapHandler {
                    id: customTap

                    onTapped: root.customClickCount++
                }
            }
            Text {
                text: "custom-status-label-clicks-" + root.customClickCount
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
            // Deliberately NOT exposed: no Accessible attachment, so this drawn
            // element must be absent from the accessibility tree (the negative
            // case of the custom-controls chapter; see README).
            Rectangle {
                id: customHidden

                width: 140
                height: 36
                radius: 4
                color: "#7a3d3d"

                Text {
                    anchors.centerIn: parent
                    text: "Hidden"
                    color: "white"
                }
            }
        }
    }

    // Modeless dialog as a real native child window — one of QML's two dialog
    // faces (the other is the in-scene modal Dialog below).
    Window {
        id: modelessDialog

        transientParent: root
        title: "dialog-modeless"
        width: 320
        height: 180
        // Right half of the main window, clear of the in-scene modal dialog
        // (x 240..560) and the list/tree column — an owned window floats ABOVE
        // the scene, so any overlap would swallow pointer clicks meant for
        // in-scene targets below it (frame/title bar included).
        x: root.x + 620
        y: root.y + 380
        visible: true
        // A plain Window has a white default background; follow the themed
        // palette of the ApplicationWindow (the Controls inside are styled).
        color: root.palette.window

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 12
            spacing: 8

            Label {
                text: "Modeless dialog (native child window)"
                Accessible.name: "dialog-modeless-label"
            }
            Button {
                text: "Click Me"
                Accessible.name: "dialog-modeless-button"
                // Reports through the main window's last-action label: only a
                // click really landing on THIS button can set its ident.
                onClicked: root.lastAction = Accessible.name
            }
            Item {
                Layout.fillHeight: true
            }
        }
    }

    // Modal dialog as an in-scene Qt Quick overlay (--open-modal): deliberately
    // NOT a native window. CloseOnEscape only — an outside click must not
    // dismiss it while tests interact with the desktop.
    // Its @Name comes from the TITLE (Accessible does not attach to Dialog —
    // verified: attaching warns and does nothing; the title surfaces as the
    // nested window node's name on UIA).
    Dialog {
        id: modalDialog

        title: "dialog-modal"
        modal: true
        visible: root.openModalOnStart
        closePolicy: Popup.CloseOnEscape
        x: 240
        y: 200
        width: 320
        height: 180

        ColumnLayout {
            anchors.fill: parent
            spacing: 8

            Label {
                text: "Modal dialog (in-scene overlay)"
                Accessible.name: "dialog-modal-label"
            }
            Button {
                text: "Click Me"
                Accessible.name: "dialog-modal-button"
                onClicked: root.lastAction = Accessible.name
            }
        }
    }
}
