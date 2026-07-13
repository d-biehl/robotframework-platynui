*** Settings ***
Documentation       Real hit-test acceptance against the egui test app. Moves the cursor onto a
...                 known widget, reads the cursor position, then resolves the element under the
...                 cursor via ``BM.Get Element At Point`` and asserts it is that widget. Exercises
...                 the full path end-to-end: window-manager window selection (EWMH stacking on
...                 X11 / compositor surface-under-point) plus the AT-SPI in-window descent.

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Launch Default Instance
Suite Teardown      Terminate Default Instance
Test Tags           real


*** Test Cases ***
Element Under The Cursor Is Resolved
    [Documentation]    Move the real cursor onto the "Click Me" button, read the cursor position,
    ...    and hit-test there — the resolved element must be that button (by stable @Id).
    BM.Pointer Move To    ${WINDOW}${BTN_CLICK_ME}
    ${p}=    BM.Get Pointer Position
    ${el}=    BM.Get Element At Point    ${p.x}    ${p.y}
    Should Not Be Equal    ${el}    ${None}    msg=hit-test resolved nothing under the cursor
    Should Be Equal    ${el.id}    btn-click-me
    ...    msg=element under the cursor was '${el.id}', expected 'btn-click-me'

Hit Test Follows The Cursor To A Second Widget
    [Documentation]    Moving to a different widget resolves that widget — proves the result tracks
    ...    the cursor rather than returning a stale/first element.
    BM.Pointer Move To    ${WINDOW}${BTN_RESET}
    ${p}=    BM.Get Pointer Position
    ${el}=    BM.Get Element At Point    ${p.x}    ${p.y}
    Should Not Be Equal    ${el}    ${None}    msg=hit-test resolved nothing under the cursor
    Should Be Equal    ${el.id}    btn-reset
    ...    msg=element under the cursor was '${el.id}', expected 'btn-reset'

Menu Item In An Open Menu Is Resolved
    [Documentation]    Open the File menu and hit-test one of its items. Menu items live in the menu
    ...    overlay, which is drawn on top of the widgets beneath it; the resolver picks the
    ...    smallest-area node under the cursor, so the menu item wins over the content behind it. (This
    ...    is a geometric bounds search, not AccessKit's own point hit-test, which returns the widget
    ...    *beneath* the overlay.)
    BM.Pointer Click    ${WINDOW}${MENU_FILE}
    Sleep    0.5s    reason=let the menu overlay open and register its items on the tree
    BM.Pointer Move To    ${WINDOW}//*[@Id="menu-file-new"]
    ${p}=    BM.Get Pointer Position
    ${el}=    BM.Get Element At Point    ${p.x}    ${p.y}
    Should Not Be Equal    ${el}    ${None}    msg=hit-test resolved nothing over the open menu item
    Should Be Equal    ${el.id}    menu-file-new
    ...    msg=element under the cursor was '${el.id}', expected 'menu-file-new'
    [Teardown]    BM.Keyboard Type    ${None}    <Escape>
