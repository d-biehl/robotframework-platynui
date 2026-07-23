*** Settings ***
Documentation       Blueprint catalog acceptance for the QML fixture (the canonical test set of
...                 dev-docs/testing-strategy.md §5.1). The Suite Setup launches the fixture and
...                 pins it as the query root, so every locator below is relative and addresses
...                 catalog controls by their canonical @Name only (roles differ across bridges,
...                 names are pairwise-unique app-wide by blueprint rule).
...
...                 Menu and combo items exist on the tree only while their popup is OPEN
...                 (verified Qt Quick reality on UIA and AT-SPI), and collapsed tree children
...                 appear only after expansion. Activations report through the always-visible
...                 ``last-action-<ident>`` label. The action keywords' built-in waiting covers
...                 popup timing.
...
...                 Test order matters for the click-counter test (the counter is cumulative per
...                 instance), so it runs before anything else clicks button-basic.

Resource            resources/qmlapp.resource

Suite Setup         Launch QML Test App
Suite Teardown      Terminate QML Instance

Test Tags           real


*** Variables ***
# Core-tier controls that must be enumerable on an idle (untouched) fixture instance.
# Deliberately absent: menu/context-menu/combo ITEMS (popups exist only while open),
# collapsed tree children (appear after expansion), dialog-modal (own instance via
# --open-modal, see modal.robot), and the main window (matched via launch configuration —
# a QQuickWindow derives its name from its title, so "main-window" is not a resolvable name).
@{CATALOG_CORE_IDLE}    button-basic    status-label-clicks-0    last-action-none
...                     checkbox-basic
...                     groupbox-basic    radio-first    radio-second    textfield-basic
...                     textarea-basic    label-basic    text-basic    image-basic
...                     combobox-basic    list-basic    list-item-1    list-item-2
...                     list-item-3    list-item-4    list-item-5    tree-basic
...                     tree-node-a    tree-node-b    main-menubar    menu-file
...                     menu-edit    menu-help
...                     dialog-modeless    dialog-modeless-label    dialog-modeless-button


*** Test Cases ***
Core Catalog Controls Are Enumerable
    [Documentation]    Every idle-state core-tier control resolves under its canonical @Name.
    FOR    ${name}    IN    @{CATALOG_CORE_IDLE}
        BM.Wait Until Exists    .//*[@Name="${name}"]
    END

Clicking The Catalog Button Updates The Click Counter
    BM.Wait Until Exists    .//*[@Name="status-label-clicks-0"]
    BM.Pointer Click    .//*[@Name="button-basic"]
    BM.Wait Until Exists    .//*[@Name="status-label-clicks-1"]
    BM.Pointer Click    .//*[@Name="button-basic"]
    BM.Wait Until Exists    .//*[@Name="status-label-clicks-2"]

Activating A Menu Bar Item Updates The Last Action
    BM.Pointer Click    .//*[@Name="menu-file"]
    BM.Pointer Click    .//*[@Name="menu-file-new"]
    BM.Wait Until Exists    .//*[@Name="last-action-menu-file-new"]

Activating A Menu Bar Submenu Item Updates The Last Action
    BM.Pointer Click    .//*[@Name="menu-edit"]
    BM.Pointer Click    .//*[@Name="menu-edit-more"]
    BM.Pointer Click    .//*[@Name="menu-edit-sub-one"]
    BM.Wait Until Exists    .//*[@Name="last-action-menu-edit-sub-one"]

Activating A Context Menu Item Updates The Last Action
    BM.Pointer Click    .//*[@Name="label-basic"]    button=right
    BM.Pointer Click    .//*[@Name="ctx-copy"]
    BM.Wait Until Exists    .//*[@Name="last-action-ctx-copy"]

Activating A Context Submenu Item Updates The Last Action
    BM.Pointer Click    .//*[@Name="label-basic"]    button=right
    BM.Pointer Click    .//*[@Name="ctx-more"]
    BM.Pointer Click    .//*[@Name="ctx-sub-alpha"]
    BM.Wait Until Exists    .//*[@Name="last-action-ctx-sub-alpha"]

Opening The Combo Box Reveals Its Items
    [Documentation]    Combo items exist only while the popup is open; selecting one closes it.
    BM.Wait Until Gone    .//*[@Name="combo-item-2"]
    BM.Pointer Click    .//*[@Name="combobox-basic"]
    BM.Wait Until Exists    .//*[@Name="combo-item-2"]
    BM.Pointer Click    .//*[@Name="combo-item-2"]
    BM.Wait Until Gone    .//*[@Name="combo-item-2"]

Expanding Tree Nodes Reveals Descendants
    [Documentation]    Double-click expands a collapsed row (the cross-toolkit expansion gesture),
    ...    revealing child and grandchild — proving all three catalog tree levels.
    BM.Wait Until Gone    .//*[@Name="tree-node-a-1"]
    BM.Pointer Multi Click    .//*[@Name="tree-node-a"]
    BM.Wait Until Exists    .//*[@Name="tree-node-a-1"]
    BM.Wait Until Gone    .//*[@Name="tree-node-a-1-i"]
    BM.Pointer Multi Click    .//*[@Name="tree-node-a-1"]
    BM.Wait Until Exists    .//*[@Name="tree-node-a-1-i"]

Clicking The Modeless Dialog Button Lands Inside The Dialog
    [Documentation]    ``last-action-dialog-modeless-button`` only appears if the pointer action
    ...    really landed inside the dialog — the bounds-correctness observable of the blueprint.
    BM.Wait Until Gone    .//*[@Name="last-action-dialog-modeless-button"]
    BM.Pointer Click    .//*[@Name="dialog-modeless-button"]
    BM.Wait Until Exists    .//*[@Name="last-action-dialog-modeless-button"]
