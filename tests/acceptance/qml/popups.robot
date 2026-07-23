*** Settings ***
Documentation       Dual popup modes of the QML fixture. Qt Quick menus are by default IN-SCENE
...                 items (the catalog suite exercises that mode); this suite runs the same menu
...                 flows on an instance started with ``--popup-mode native`` (Qt >= 6.8
...                 popupType), where every menu is a native top-level popup window. Names are
...                 identical in both modes — the flows below prove the catalog menu items stay
...                 reachable and activatable when the popup mechanics flip.
...
...                 platform:windows — verified deviation: on Linux (X11 and the PlatynUI
...                 compositor) the native popups render but their contents never reach AT-SPI
...                 (no tree node, no hit-test), unlike Qt Widgets' native QMenu — so the
...                 tree-driven native-mode flows only work where the bridge exposes them
...                 (see apps/test-app-qml/README.md).

Resource            resources/qmlapp.resource

Suite Setup         Launch QML Test App    PlatynUI QML TestApp (Native Popups)    org.platynui.test.qml.native
...                     --popup-mode    native
Suite Teardown      Terminate QML Instance

Test Tags           real    platform:windows


*** Test Cases ***
Native Mode Menu Bar Item Activates With The Same Names
    [Documentation]    Same flow and names as the in-scene catalog test — only the popup mechanics
    ...    differ (native popup window instead of a scene item).
    BM.Pointer Click    .//*[@Name="menu-file"]
    BM.Pointer Click    .//*[@Name="menu-file-new"]
    BM.Wait Until Exists    .//*[@Name="last-action-menu-file-new"]

Native Mode Menu Bar Submenu Item Activates With The Same Names
    BM.Pointer Click    .//*[@Name="menu-edit"]
    BM.Pointer Click    .//*[@Name="menu-edit-more"]
    BM.Pointer Click    .//*[@Name="menu-edit-sub-two"]
    BM.Wait Until Exists    .//*[@Name="last-action-menu-edit-sub-two"]

Native Mode Context Menu Item Activates With The Same Names
    BM.Pointer Click    .//*[@Name="label-basic"]    button=right
    BM.Pointer Click    .//*[@Name="ctx-copy"]
    BM.Wait Until Exists    .//*[@Name="last-action-ctx-copy"]

Native Mode Context Submenu Item Activates With The Same Names
    BM.Pointer Click    .//*[@Name="label-basic"]    button=right
    BM.Pointer Click    .//*[@Name="ctx-more"]
    BM.Pointer Click    .//*[@Name="ctx-sub-alpha"]
    BM.Wait Until Exists    .//*[@Name="last-action-ctx-sub-alpha"]
