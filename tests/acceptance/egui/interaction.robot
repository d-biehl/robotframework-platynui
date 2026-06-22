*** Settings ***
Documentation       BareMetal pointer/keyboard/focus interaction coverage against
...                 the egui app, verified through observable UI changes on the
...                 accessibility tree. The app instance is shared across the run,
...                 so the click tests assert a delta, not an absolute count.

Library             PlatynUI.BareMetal    AS    BM


*** Variables ***
${WINDOW}           //*[@Name="PlatynUI Test App"]


*** Test Cases ***
Pointer Click Increments The Click Counter
    ${before}=    Get Click Count
    BM.Pointer Click    ${WINDOW}//Button[@Name="Click Me"]
    Sleep    0.2s
    ${after}=    Get Click Count
    Should Be Equal As Integers    ${after}    ${{ $before + 1 }}    msg=click did not increment the counter

Selecting A Radio Button Updates The Status Bar
    BM.Pointer Click    ${WINDOW}//RadioButton[@Name="Option C"]
    Sleep    0.2s
    ${status}=    BM.Query    ${WINDOW}//Label[starts-with(@Name, "Radio:")]    only_first=${True}
    Should Not Be Equal    ${status}    ${None}    msg=Radio status label not found
    Should Be Equal    ${status.name}    Radio: Option C    msg=radio selection not reflected in status bar

Focus Plus Keyboard Activates The Focused Button
    ${before}=    Get Click Count
    BM.Focus    ${WINDOW}//Button[@Name="Click Me"]
    BM.Keyboard Type    ${None}    <Return>
    Sleep    0.2s
    ${after}=    Get Click Count
    Should Be True    ${after} > ${before}    msg=keyboard activation did not register a click

Highlight Does Not Error
    [Documentation]    Highlight draws its overlay around the window without raising; Take Screenshot
    ...    is covered on both backends by auto_activate.robot.
    ${win}=    BM.Query    ${WINDOW}    only_first=${True}
    BM.Highlight    ${win}    duration=0.5


*** Keywords ***
Get Click Count
    [Documentation]    Read the integer from the "Clicks: N" status label.
    ${label}=    BM.Query    ${WINDOW}//Label[starts-with(@Name, "Clicks:")]    only_first=${True}
    Should Not Be Equal    ${label}    ${None}    msg=Clicks counter label not found
    RETURN    ${{ int($label.name.split(":")[1]) }}
