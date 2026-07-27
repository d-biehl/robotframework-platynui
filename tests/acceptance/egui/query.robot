*** Settings ***
Documentation       BareMetal query + attribute-read coverage against the egui
...                 test app. Read-only — order-independent (no UI mutation).
...                 The launcher scopes the suite to the app window (``Set
...                 Root``), so locators are relative and address widgets by
...                 their stable ``@Id``.

Resource            resources/testapp.resource

Suite Setup         Launch Default Instance
Suite Teardown      Terminate Default Instance


*** Test Cases ***
Query Window By Name And Read Its Attributes
    ${win}=    BM.Query    .    only_first=${True}
    Should Not Be Equal    ${win}    ${None}    msg=egui window not found
    BM.Get Attribute    ${win}    Name    ==    PlatynUI Test App
    ${bounds}=    BM.Get Attribute    ${win}    Bounds
    # `Get Attribute` fetches directly, so an ABSENT attribute already failed the read above — unlike
    # `Query`, where a non-matching XPath yields ${None}. What is left for this check is the other
    # case: the attribute is there and its value is null.
    Should Not Be Equal    ${bounds}    ${None}    msg=window Bounds is present but null

Known Buttons Exist By Id
    [Documentation]    Each expected button — the three action buttons and the three menu buttons —
    ...    resolves by its stable @Id and reports Role Button.
    FOR    ${id}    IN    btn-click-me    btn-reset    btn-conditional    menu-file    menu-edit    menu-help
        BM.Get Attribute    .//*[@Id="${id}"]    Role    ==    Button
    END

Query The Link Widget
    ${link}=    BM.Query    .//(Link|Hyperlink)[@Id="link-platynui"]    only_first=${True}
    Should Not Be Equal    ${link}    ${None}    msg=link 'link-platynui' not found

Set Root Narrows Subsequent Relative Queries
    [Documentation]    Roots chain: a LOCAL Set Root drills from the suite root (the window) into a
    ...    single widget, and ``.`` then resolves that widget. The LOCAL scope (default) clears
    ...    itself when the test ends, so no teardown reset is needed.
    BM.Set Root    .//*[@Id="btn-click-me"]
    ${b}=    BM.Query    .    only_first=${True}
    Should Not Be Equal    ${b}    ${None}    msg=relative query under the narrowed root did not resolve
    Should Be Equal    ${b.id}    btn-click-me

Text Input Exposes Its Content Via control:Text
    [Documentation]    A text-bearing widget (the TextEdit exposes the AT-SPI Text interface) surfaces
    ...    its current content as the canonical read-only ``control:Text`` attribute (TextContent).
    BM.Get Attribute    .//*[@Id="input-name"]    control:Text    ==    PlatynUI

Non-Text Widget Has No control:Text
    [Documentation]    ``control:Text`` is sourced only from a genuine text interface, never the
    ...    accessible name — a button (no text interface) exposes no ``control:Text`` even though it
    ...    has a label in ``control:Name``.
    BM.Get Attribute    .//*[@Id="btn-click-me"]    Name    ==    Click Me
    Run Keyword And Expect Error    *attribute not found*Text*
    ...    BM.Get Attribute    .//*[@Id="btn-click-me"]    control:Text

Widget With AccessKit Description Exposes control:Description
    [Documentation]    The Click Me button sets an AccessKit description, forwarded through
    ...    AT-SPI ``Accessible.Description`` to the common ``control:Description`` attribute.
    BM.Get Attribute    .//*[@Id="btn-click-me"]    Description    ==    Increments the click counter

Widget Without A Description Has No control:Description
    [Documentation]    ``control:Description`` is emitted only when the platform value is non-empty —
    ...    the Reset button sets no AccessKit description, so the attribute is absent (not empty).
    Run Keyword And Expect Error    *attribute not found*Description*
    ...    BM.Get Attribute    .//*[@Id="btn-reset"]    Description
