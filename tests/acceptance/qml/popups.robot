*** Settings ***
Documentation       Dual popup modes of the QML fixture. Qt Quick menus are by default IN-SCENE
...                 items (the catalog suite exercises that mode); this suite runs the same menu
...                 flows on an instance started with ``--popup-mode native`` (Qt >= 6.8
...                 popupType), where every menu is a native top-level popup window. Names are
...                 identical in both modes — the flows below prove the catalog menu items stay
...                 reachable and activatable when the popup mechanics flip.

Resource            resources/qmlapp.resource

Suite Setup         Launch QML Catalog Instance With Native Popups
Suite Teardown      Terminate QML Instance

Test Tags           real


*** Test Cases ***
Native Mode Menu Bar Item Activates With The Same Names
    [Documentation]    Same flow and names as the in-scene catalog test — only the popup mechanics
    ...    differ (native popup window instead of a scene item).
    Activating A Menu Item Should Rename It    menu-file    menu-file-new

Native Mode Menu Bar Submenu Item Activates With The Same Names
    Activating A Menu Bar Submenu Item Should Rename It    menu-edit    menu-edit-more    menu-edit-sub-two

Native Mode Context Menu Item Activates With The Same Names
    Activating A Context Menu Item Should Rename It    ctx-copy

Native Mode Context Submenu Item Activates With The Same Names
    Activating A Context Submenu Item Should Rename It    ctx-sub-alpha
