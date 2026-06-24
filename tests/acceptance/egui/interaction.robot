*** Settings ***
Documentation       BareMetal pointer/keyboard/focus interaction coverage against
...                 the egui app, verified through observable UI changes on the
...                 accessibility tree. The app instance is shared across the run,
...                 so the click tests assert a delta, not an absolute count.
...
...                 Suite Setup scopes the whole suite to the app window with
...                 ``Set Root``, so the widget locators are relative
...                 (``.//*[@Id=...]``) and need no window prefix. Status-bar
...                 effects are checked with ``Get Attribute    ==``, which waits
...                 for the change instead of sleeping a fixed time.

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Open Interaction Window
Suite Teardown      Terminate Default Instance


*** Test Cases ***
Pointer Click Increments The Click Counter
    ${before}=    Get Click Count
    BM.Pointer Click    .${BTN_CLICK_ME}
    BM.Get Attribute    .${STATUS_CLICKS}    Name    ==    Clicks: ${{ $before + 1 }}
    ...    msg=click did not increment the counter

Selecting A Radio Button Updates The Status Bar
    BM.Pointer Click    .${RADIO_OPTION_C}
    BM.Get Attribute    .${STATUS_RADIO}    Name    ==    Radio: Option C
    ...    msg=radio selection not reflected in status bar

Focus Plus Keyboard Activates The Focused Button
    ${before}=    Get Click Count
    BM.Focus    .${BTN_CLICK_ME}
    BM.Keyboard Type    ${None}    <Return>
    BM.Get Attribute    .${STATUS_CLICKS}    Name    ==    Clicks: ${{ $before + 1 }}
    ...    msg=keyboard activation did not register a click

Highlight Does Not Error
    [Documentation]    Highlight draws its overlay around the window (the current root) without
    ...    raising; Take Screenshot is covered on both backends by auto_activate.robot.
    ${win}=    BM.Query    .    only_first=${True}
    BM.Highlight    ${win}    duration=0.5


*** Keywords ***
Open Interaction Window
    [Documentation]    Launch this suite's egui instance and scope all relative locators to it with
    ...    Set Root, so the tests address widgets as ``.//<Role>[@Id=...]``.
    Launch Default Instance
    BM.Set Root    ${WINDOW}    scope=SUITE
