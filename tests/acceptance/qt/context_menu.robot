*** Settings ***
Documentation       Real acceptance for transient-popup findability, driven by the Qt test app's
...                 right-click context menu. The menu is a transient ``QMenu`` popup: Qt exposes it
...                 on AT-SPI only *event-driven* — the popup accessible hangs under the Application
...                 via ``parent()``, but the Application's own ``GetChildren`` never lists it, so a
...                 pure top-down tree walk misses it (the ``atspi-event-driven-tree`` change). An
...                 OPEN menu's items must be findable by XPath query and by the point hit-test; a
...                 DISMISSED menu's items must be gone again.
...
...                 The app opens the menu non-blocking (``popup()`` in ``contextMenuEvent``), so it
...                 stays open after the right-click until explicitly dismissed (Escape). Popup
...                 registration is event-driven and therefore asynchronous — every visibility
...                 assertion polls instead of reading once.

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Test Teardown       Teardown Context Menu Test
Test Tags           real


*** Variables ***
# How long the event-driven popup surfacing may take to show up in / vanish from the tree.
${POPUP_SETTLE_TIMEOUT}     5s


*** Test Cases ***
Open Context Menu Items Are Findable
    [Documentation]    Right-click opens the context menu; its items must resolve by XPath query
    ...    (``//*:MenuItem[@Name="ctx-copy"]``) and the hit-test over an item must return that item,
    ...    not the widget beneath the popup.
    Launch Main Window Only
    Open The Context Menu
    Wait Until Keyword Succeeds    ${POPUP_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Present    ${APP}${CTX_MENU_COPY}
    BM.Pointer Move To    ${APP}${CTX_MENU_COPY}
    ${p}=    BM.Get Pointer Position
    ${el}=    BM.Get Element At Point    ${p.x}    ${p.y}
    Should Not Be Equal    ${el}    ${None}    msg=hit-test resolved nothing over the open context-menu item
    Should Be Equal    ${el.name}    ctx-copy
    ...    msg=element under the cursor was '${el.name}', expected 'ctx-copy'

Closed Context Menu Items Are Not Found
    [Documentation]    Dismissing the menu must remove its items from the tree again — no stale
    ...    popup may stay grafted (guards the registry's remove path).
    Launch Main Window Only
    Open The Context Menu
    Wait Until Keyword Succeeds    ${POPUP_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Present    ${APP}${CTX_MENU_COPY}
    Dismiss The Context Menu
    Wait Until Keyword Succeeds    ${POPUP_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Absent    ${APP}${CTX_MENU_COPY}

Open Submenu Items Are Findable And Hit-Testable
    [Documentation]    Clicking the "ctx-more" entry cascades a submenu — on X11 yet another
    ...    override-redirect QMenu window. Its items must resolve by XPath query, and the hit-test
    ...    over one must return that item even though the submenu is drawn beside (outside) the
    ...    root popup's own bounds.
    Launch Main Window Only
    Open The Context Menu
    Wait Until Keyword Succeeds    ${POPUP_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Present    ${APP}${CTX_MENU_MORE}
    BM.Pointer Click    ${APP}${CTX_MENU_MORE}
    Wait Until Keyword Succeeds    ${POPUP_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Present    ${APP}${CTX_MENU_SUB_ALPHA}
    BM.Pointer Move To    ${APP}${CTX_MENU_SUB_ALPHA}
    ${p}=    BM.Get Pointer Position
    ${el}=    BM.Get Element At Point    ${p.x}    ${p.y}
    Should Not Be Equal    ${el}    ${None}    msg=hit-test resolved nothing over the open submenu item
    Should Be Equal    ${el.name}    ctx-sub-alpha
    ...    msg=element under the cursor was '${el.name}', expected 'ctx-sub-alpha'

A Nested Submenu Item Is Resolved By Hit Test
    [Documentation]    Two cascade levels deep: ctx-more → ctx-deep → ctx-deep-item. Each open
    ...    level is its own transient window, so this guards that grafted popups stay walkable in
    ...    depth, not just at the first level.
    Launch Main Window Only
    Open The Context Menu
    Wait Until Keyword Succeeds    ${POPUP_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Present    ${APP}${CTX_MENU_MORE}
    BM.Pointer Click    ${APP}${CTX_MENU_MORE}
    Wait Until Keyword Succeeds    ${POPUP_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Present    ${APP}${CTX_MENU_DEEP}
    BM.Pointer Click    ${APP}${CTX_MENU_DEEP}
    Wait Until Keyword Succeeds    ${POPUP_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Present    ${APP}${CTX_MENU_DEEP_ITEM}
    BM.Pointer Move To    ${APP}${CTX_MENU_DEEP_ITEM}
    ${p}=    BM.Get Pointer Position
    ${el}=    BM.Get Element At Point    ${p.x}    ${p.y}
    Should Not Be Equal    ${el}    ${None}    msg=hit-test resolved nothing over the nested submenu item
    Should Be Equal    ${el.name}    ctx-deep-item
    ...    msg=element under the cursor was '${el.name}', expected 'ctx-deep-item'

Dismissed Submenus Leave No Stale Items
    [Documentation]    After opening the full cascade and dismissing everything, no item from any
    ...    level may remain findable — neither the root menu's nor a submenu's (guards the remove
    ...    path across cascade levels).
    Launch Main Window Only
    Open The Context Menu
    Wait Until Keyword Succeeds    ${POPUP_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Present    ${APP}${CTX_MENU_MORE}
    BM.Pointer Click    ${APP}${CTX_MENU_MORE}
    Wait Until Keyword Succeeds    ${POPUP_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Present    ${APP}${CTX_MENU_SUB_ALPHA}
    Dismiss The Context Menu
    Wait Until Keyword Succeeds    ${POPUP_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Absent    ${APP}${CTX_MENU_SUB_ALPHA}
    Wait Until Keyword Succeeds    ${POPUP_SETTLE_TIMEOUT}    0.25s
    ...    Node Is Absent    ${APP}${CTX_MENU_COPY}


*** Keywords ***
Launch Main Window Only
    [Documentation]    Launch a single top-level Qt window (``--dialogs 0``) and pin it as ``${APP}``.
    ${handle}=    Launch Qt Test App    PlatynUI Test App (Qt Ctx)    org.platynui.test.qt.ctx    --dialogs    0
    ${root}=    App Root    ${handle}
    VAR    ${APP}    ${root}    scope=TEST
    VAR    ${QT_APP_HANDLE}    ${handle}    scope=TEST

Open The Context Menu
    [Documentation]    Right-click the main window's center; ``contextMenuEvent`` pops the pre-built
    ...    menu at the cursor via non-blocking ``popup()``, so it stays open after the release.
    ...    Retried as a whole: right after launch the WM may still be placing the window, so a
    ...    single right-click can miss — click again until an item is actually on the tree.
    Wait Until Keyword Succeeds    3x    0.5s    Right Click Opens The Menu

Right Click Opens The Menu
    BM.Pointer Click    ${APP}${MAIN_WINDOW}    button=right
    Wait Until Keyword Succeeds    2s    0.25s    Node Is Present    ${APP}${CTX_MENU_COPY}

Dismiss The Context Menu
    [Documentation]    Close the menu — including any open submenu cascade — without activating an
    ...    item. Each Escape closes ONE level (deepest submenu first), so send one per possible
    ...    level; surplus Escapes land in the main window and are harmless.
    FOR    ${level}    IN RANGE    3
        BM.Keyboard Type    ${None}    <Escape>
    END

Teardown Context Menu Test
    [Documentation]    Best-effort menu dismissal (so no popup grab outlives a failed test), then
    ...    terminate the app instance.
    Run Keyword And Ignore Error    Dismiss The Context Menu
    Terminate Default Qt Instance
