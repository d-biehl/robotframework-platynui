*** Settings ***
Documentation       Real hit-test acceptance against the Qt (PySide6) test app — the counterpart to the
...                 egui hit-test suite. Moves the cursor onto a known Qt widget, reads the cursor
...                 position, then resolves the element under the cursor via ``BM.Get Element At Point``
...                 and asserts it is that widget. Covers the single-window case and, crucially, a
...                 multi-top-level case (a child dialog) so window selection is exercised — a hit-test
...                 over the dialog must resolve the dialog's widget, not something in the main window.

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Test Teardown       Terminate Default Qt Instance
Test Tags           real


*** Test Cases ***
Element In The Main Window Is Resolved
    [Documentation]    Single top-level window: hit-test the main window's "Pick Me" button.
    Launch Main Window Only
    BM.Pointer Move To    ${APP}${MAIN_WINDOW}${MAIN_PICK_BUTTON}
    ${p}=    BM.Get Pointer Position
    ${el}=    BM.Get Element At Point    ${p.x}    ${p.y}
    Should Not Be Equal    ${el}    ${None}    msg=hit-test resolved nothing over the main-window button
    Should Be Equal    ${el.name}    main-pick-button
    ...    msg=element under the cursor was '${el.name}', expected 'main-pick-button'

Element In A Child Dialog Is Resolved
    [Documentation]    Multi-window: hit-test a button in a *different* top-level window than the main
    ...    one. The hit-test must select the dialog's frame, not resolve something in the main window.
    Launch Single Dialog Qt Instance
    BM.Pointer Move To    ${APP}${DIALOG_1}${DIALOG_1_BUTTON}
    ${p}=    BM.Get Pointer Position
    ${el}=    BM.Get Element At Point    ${p.x}    ${p.y}
    Should Not Be Equal    ${el}    ${None}    msg=hit-test resolved nothing over the dialog button
    Should Be Equal    ${el.name}    child-dialog-1-button
    ...    msg=element under the cursor was '${el.name}', expected 'child-dialog-1-button'

Menu Item In An Open Menu Is Resolved
    [Documentation]    Open the File menu (a QMenu popup — on X11 an override-redirect window drawn
    ...    outside its owning frame) and hit-test one of its items. This exercises the resolver's popup
    ...    path: it searches the whole application subtree, not a single window, and picks the
    ...    smallest-area node under the cursor — so the menu item wins over the content beneath it.
    Launch Main Window Only
    BM.Pointer Click    ${APP}${MENU_FILE}
    BM.Pointer Move To    ${APP}${MENU_FILE_NEW}
    ${p}=    BM.Get Pointer Position
    ${el}=    BM.Get Element At Point    ${p.x}    ${p.y}
    Should Not Be Equal    ${el}    ${None}    msg=hit-test resolved nothing over the open menu item
    Should Be Equal    ${el.name}    menu-file-new
    ...    msg=element under the cursor was '${el.name}', expected 'menu-file-new'


*** Keywords ***
Launch Main Window Only
    [Documentation]    Launch a single top-level Qt window (``--dialogs 0``) and pin it as ``${APP}``.
    ${handle}=    Launch Qt Test App    PlatynUI Test App (Qt Hit)    org.platynui.test.qt.hit    --dialogs    0
    ${root}=    App Root    ${handle}
    VAR    ${APP}    ${root}    scope=TEST
    VAR    ${QT_APP_HANDLE}    ${handle}    scope=TEST
