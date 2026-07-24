*** Settings ***
Documentation       Real hit-test acceptance against the Qt (PySide6) test app — the counterpart to the
...                 egui hit-test suite. Moves the cursor onto a known Qt widget, reads the cursor
...                 position, then resolves the element under the cursor via ``BM.Get Element At Point``
...                 and asserts it is that widget. Covers the single-window case and, crucially, a
...                 multi-top-level case (a child dialog) so window selection is exercised — a hit-test
...                 over the dialog must resolve the dialog's widget, not something in the main window.

Resource            resources/testapp.resource

Test Teardown       Terminate Qt Instance

Test Tags           real


*** Test Cases ***
Element In The Main Window Is Resolved
    [Documentation]    Single top-level window: hit-test the main window's "Pick Me" button.
    Launch Qt Test App    PlatynUI Test App (Qt Hit)    org.platynui.test.qt.hit    --dialogs    0
    ...    scope=TEST
    BM.Pointer Move To    .//*[@Name="main-pick-button"]
    ${p}=    BM.Get Pointer Position
    ${el}=    BM.Get Element At Point    ${p.x}    ${p.y}
    Should Not Be Equal    ${el}    ${None}    msg=hit-test resolved nothing over the main-window button
    Should Be Equal    ${el.name}    main-pick-button
    ...    msg=element under the cursor was '${el.name}', expected 'main-pick-button'

Element In A Child Dialog Is Resolved
    [Documentation]    Multi-window: hit-test a button in a *different* top-level window than the main
    ...    one. The hit-test must select the dialog's frame, not resolve something in the main window.
    ...    A single-dialog instance avoids sibling occlusion (windows stack at the origin on Wayland).
    Launch Qt Test App    PlatynUI Test App (Qt Hit Dialog)    org.platynui.test.qt.hitdialog    --dialogs    1
    ...    scope=TEST
    BM.Pointer Move To    .//*[@Name="child-dialog-1-button"]
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
    Launch Qt Test App    PlatynUI Test App (Qt Hit Menu)    org.platynui.test.qt.hitmenu    --dialogs    0
    ...    scope=TEST
    BM.Pointer Click    .//MenuItem[@Name="File"]
    BM.Pointer Move To    .//MenuItem[@Name="menu-file-new"]
    ${p}=    BM.Get Pointer Position
    ${el}=    BM.Get Element At Point    ${p.x}    ${p.y}
    Should Not Be Equal    ${el}    ${None}    msg=hit-test resolved nothing over the open menu item
    Should Be Equal    ${el.name}    menu-file-new
    ...    msg=element under the cursor was '${el.name}', expected 'menu-file-new'
