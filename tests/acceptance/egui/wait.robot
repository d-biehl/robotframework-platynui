*** Settings ***
Documentation       Acceptance coverage for the explicit wait keywords — Wait Until Exists, Wait Until
...                 Gone and Wait Until Query — against the real egui app over the live AT-SPI tree.
...                 The File menu is the appear/disappear hook: its items (``menu-file-new`` …) exist on
...                 the tree only while the menu is open, so opening it makes one appear and closing it
...                 (Escape) makes it vanish. This is the only place the captured-element "gone success"
...                 direction can be exercised, since the mock never invalidates a captured node.
...
...                 Suite Setup scopes the whole suite to the app window with Set Root, so widget
...                 locators are relative (``.//<Role>[@Id=...]``), exactly like interaction.robot.

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Open Wait Window
Suite Teardown      Terminate Default Instance

Test Teardown       Close Any Open Menu


*** Variables ***
# Menu items are buttons inside the File menu popup; the role union keeps the locator robust across
# how the popup item surfaces on the AT-SPI tree, while still pinning the stable @Id (never a bare *).
${MENU_FILE_NEW}        .//(Button|MenuItem)[@Id="menu-file-new"]


*** Test Cases ***
Wait Until Exists Returns A Widget That Appears
    [Documentation]    Opening the File menu makes its items appear; Wait Until Exists waits for one
    ...    and returns it.
    BM.Pointer Click    .${MENU_FILE}
    ${item}=    BM.Wait Until Exists    ${MENU_FILE_NEW}
    Should Be Equal    ${item.id}    menu-file-new

Wait Until Gone Waits For A Selector To Disappear
    [Documentation]    With the menu open the item is on the tree; closing the menu removes it, and
    ...    Wait Until Gone returns once the selector matches nothing.
    BM.Pointer Click    .${MENU_FILE}
    BM.Wait Until Exists    ${MENU_FILE_NEW}
    BM.Keyboard Type    ${None}    <Escape>
    BM.Wait Until Gone    ${MENU_FILE_NEW}

Wait Until Gone Waits For A Captured Element To Become Invalid
    [Documentation]    Capture the menu item while open, close the menu, then wait until the captured
    ...    node reports itself invalid — the real-provider path the mock cannot cover.
    BM.Pointer Click    .${MENU_FILE}
    ${item}=    BM.Wait Until Exists    ${MENU_FILE_NEW}
    BM.Keyboard Type    ${None}    <Escape>
    BM.Wait Until Gone    ${item}

Wait Until Query Waits For A Computed Condition
    [Documentation]    Click the button, then wait until the status label's text reflects the new
    ...    count — a real attribute condition that becomes true after the action.
    ${before}=    Get Click Count
    BM.Pointer Click    .${BTN_CLICK_ME}
    BM.Wait Until Query    .${STATUS_CLICKS}/@Name    ==    Clicks: ${{ $before + 1 }}


*** Keywords ***
Open Wait Window
    [Documentation]    Launch this suite's egui instance and scope all relative locators to it with
    ...    Set Root, so the tests address widgets as ``.//<Role>[@Id=...]``.
    Launch Default Instance
    BM.Set Root    ${WINDOW}    scope=SUITE

Close Any Open Menu
    [Documentation]    Best-effort: dismiss an open menu so a failing test cannot leak menu state into
    ...    the next one.
    Run Keyword And Ignore Error    BM.Keyboard Type    ${None}    <Escape>
