*** Settings ***
Documentation       BareMetal query + attribute-read coverage against the egui
...                 test app. Read-only — order-independent (no UI mutation).
...                 Locators come from the page-object resource and address
...                 widgets by their stable ``@Id``.

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Launch Default Instance
Suite Teardown      Terminate Default Instance


*** Test Cases ***
Query Window By Name And Read Its Attributes
    ${win}=    BM.Query    ${WINDOW}    only_first=${True}
    Should Not Be Equal    ${win}    ${None}    msg=egui window not found by name
    BM.Get Attribute    ${win}    Name    ==    PlatynUI Test App
    ${bounds}=    BM.Get Attribute    ${win}    Bounds
    Should Not Be Equal    ${bounds}    ${None}    msg=window has no Bounds attribute

Known Buttons Exist By Id
    [Documentation]    Each expected button — the three action buttons and the three menu buttons —
    ...    resolves by its stable @Id and reports Role Button. Replaces a fuzzy "at least N buttons"
    ...    count with explicit, meaningful existence checks.
    FOR    ${button}    IN    ${BTN_CLICK_ME}    ${BTN_RESET}    ${BTN_CONDITIONAL}    ${MENU_FILE}    ${MENU_EDIT}    ${MENU_HELP}
        BM.Get Attribute    ${WINDOW}${button}    Role    ==    Button
    END

Query Menu Buttons By Id
    FOR    ${menu}    IN    ${MENU_FILE}    ${MENU_EDIT}    ${MENU_HELP}
        ${b}=    BM.Query    ${WINDOW}${menu}    only_first=${True}
        Should Not Be Equal    ${b}    ${None}    msg=menu button '${menu}' not found
    END

Query The Link Widget
    ${link}=    BM.Query    ${WINDOW}${LINK_PLATYNUI}    only_first=${True}
    Should Not Be Equal    ${link}    ${None}    msg=link 'link-platynui' not found

Set Root Scopes Subsequent Relative Queries
    [Documentation]    Set Root narrows relative queries to the window. The LOCAL scope (default)
    ...    clears itself when the test ends, so no teardown reset is needed.
    BM.Set Root    ${WINDOW}
    ${b}=    BM.Query    .${BTN_CLICK_ME}    only_first=${True}
    Should Not Be Equal    ${b}    ${None}    msg=relative query under Set Root did not resolve
