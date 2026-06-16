*** Settings ***
Documentation       BareMetal query + attribute-read coverage against the egui
...                 test app. Read-only — order-independent (no UI mutation).

Library             PlatynUI.BareMetal    AS    BM


*** Variables ***
${WINDOW}           //*[@Name="PlatynUI Test App"]


*** Test Cases ***
Query Window By Name And Read Its Attributes
    ${win}=    BM.Query    ${WINDOW}    only_first=${True}
    Should Not Be Equal    ${win}    ${None}    msg=egui window not found by name
    BM.Get Attribute    ${win}    Name    ==    PlatynUI Test App
    ${bounds}=    BM.Get Attribute    ${win}    Bounds
    Should Not Be Equal    ${bounds}    ${None}    msg=window has no Bounds attribute

Query Returns Several Buttons
    @{buttons}=    BM.Query    ${WINDOW}//Button
    Should Be True    ${{ len($buttons) >= 5 }}    msg=expected several buttons, got ${{ len($buttons) }}

Query Menu Buttons By Name
    FOR    ${name}    IN    File    Edit    Help
        ${b}=    BM.Query    ${WINDOW}//Button[@Name="${name}"]    only_first=${True}
        Should Not Be Equal    ${b}    ${None}    msg=menu button '${name}' not found
    END

Query The Link Widget
    ${link}=    BM.Query    ${WINDOW}//Link[@Name="PlatynUI"]    only_first=${True}
    Should Not Be Equal    ${link}    ${None}    msg=Link 'PlatynUI' not found

Set Root Scopes Subsequent Relative Queries
    [Teardown]    BM.Set Root    ${None}
    BM.Set Root    ${WINDOW}
    ${b}=    BM.Query    .//Button[@Name="Click Me"]    only_first=${True}
    Should Not Be Equal    ${b}    ${None}    msg=relative query under Set Root did not resolve
