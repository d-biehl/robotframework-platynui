*** Settings ***
Documentation       Acceptance coverage for the explicit wait keywords — Wait Until Exists, Wait Until
...                 Gone and Wait Until Query — against the real egui app over the live AT-SPI tree.
...                 The File menu is the appear/disappear hook: its items (``menu-file-new`` …) exist on
...                 the tree only while the menu is open, so opening it makes one appear and closing it
...                 (Escape) makes it vanish. This is the only place the captured-element "gone success"
...                 direction can be exercised, since the mock never invalidates a captured node.
...
...                 The launcher scopes the whole suite to the app window with Set Root, so widget
...                 locators are relative (``.//*[@Id=...]``), exactly like interaction.robot.

Resource            resources/testapp.resource

Suite Setup         Launch Default Instance
Suite Teardown      Terminate Default Instance

Test Teardown       Close Any Open Menu


*** Test Cases ***
Wait Until Exists Returns A Widget That Appears
    [Documentation]    Opening the File menu makes its items appear; Wait Until Exists waits for one
    ...    and returns it.
    BM.Pointer Click    .//*[@Id="menu-file"]
    ${item}=    BM.Wait Until Exists    .//*[@Id="menu-file-new"]
    Should Be Equal    ${item.id}    menu-file-new

Wait Until Gone Waits For A Selector To Disappear
    [Documentation]    With the menu open the item is on the tree; closing the menu removes it, and
    ...    Wait Until Gone returns once the selector matches nothing.
    BM.Pointer Click    .//*[@Id="menu-file"]
    BM.Wait Until Exists    .//*[@Id="menu-file-new"]
    BM.Keyboard Type    ${None}    <Escape>
    BM.Wait Until Gone    .//*[@Id="menu-file-new"]

Wait Until Gone Waits For A Captured Element To Become Invalid
    [Documentation]    Capture the menu item while open, close the menu, then wait until the captured
    ...    node reports itself invalid — the real-provider path the mock cannot cover.
    BM.Pointer Click    .//*[@Id="menu-file"]
    ${item}=    BM.Wait Until Exists    .//*[@Id="menu-file-new"]
    BM.Keyboard Type    ${None}    <Escape>
    BM.Wait Until Gone    ${item}

Wait Until Query Waits For A Computed Condition
    [Documentation]    Click the button, then wait until the status label's text reflects the new
    ...    count — a real attribute condition that becomes true after the action.
    ${before}=    Get Click Count
    BM.Pointer Click    .//*[@Id="btn-click-me"]
    BM.Wait Until Query    .//*[@Id="status-clicks"]/@Name    ==    Clicks: ${{ $before + 1 }}


*** Keywords ***
Close Any Open Menu
    [Documentation]    Best-effort: dismiss an open menu so a failing test cannot leak menu state into
    ...    the next one.
    Run Keyword And Ignore Error    BM.Keyboard Type    ${None}    <Escape>
