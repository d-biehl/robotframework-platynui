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

Application Has No Id
    [Documentation]    An application is identified by its ``@ProcessId`` and carries no ``Id`` (spec
    ...    *id-attribute*): the accessor answers ``None``, the named lookup finds nothing, no ``Id`` is
    ...    enumerated, and its one-line description ends without a ``#`` suffix.
    ${pid}=    Get Process Id    ${TEST_APP_HANDLE}
    ${app}=    BM.Query    /app:Application[@ProcessId=${pid}]    only_first=${True}
    Should Not Be Equal    ${app}    ${None}    msg=no application with the launched process ID ${pid}
    Should Be Equal    ${app.id}    ${None}
    Run Keyword And Expect Error    *attribute not found*Id*
    ...    BM.Get Attribute    ${app}    Id
    ${listed}=    BM.Query    count(/app:Application[@ProcessId=${pid}]/@*[local-name()="Id"])    only_first=${True}
    Should Be Equal As Integers    ${listed}    0    msg=an Id is enumerated on the application
    Should Not Contain    ${app.describe()}    \#

No Application Is Selected By Id
    [Documentation]    No application on the desktop carries an ``Id``, so ``/app:*[@Id]`` selects
    ...    nothing (spec *id-attribute*).
    ${application}=    BM.Query    /app:*[@Id]    only_first=${True}
    Should Be Equal    ${application}    ${None}    msg=an application is selected by Id

Element Without An Author Id Has No Id
    [Documentation]    An element whose toolkit reports no identifier carries no ``Id`` at all, not an
    ...    empty one (spec *id-attribute*). The ``Buttons`` heading is such an element: egui sets no
    ...    author id of its own, and the fixture gives the heading none. ``[@Id]`` does not match it,
    ...    the named lookup finds nothing, the accessor answers ``None`` and no ``Id`` is enumerated.
    ${heading}=    BM.Query    .//*[@Name="Buttons"]    only_first=${True}
    Should Not Be Equal    ${heading}    ${None}    msg=the Buttons heading was not found
    ${with_id}=    BM.Query    .//*[@Name="Buttons"][@Id]    only_first=${True}
    Should Be Equal    ${with_id}    ${None}    msg=[@Id] matches the heading
    Run Keyword And Expect Error    *attribute not found*Id*
    ...    BM.Get Attribute    ${heading}    Id
    Should Be Equal    ${heading.id}    ${None}
    ${listed}=    BM.Query    count(.//*[@Name="Buttons"]/@*[local-name()="Id"])    only_first=${True}
    Should Be Equal As Integers    ${listed}    0    msg=an Id is enumerated on the heading

Author Id Is The Same Through Every Read
    [Documentation]    The Click Me button carries the AccessKit author id ``btn-click-me``. It is
    ...    located by its name, so that each read stands on its own: the enumerated attributes
    ...    (``@*``), the named lookup and the accessor all answer the author id (spec *id-attribute*).
    ${button}=    BM.Query    .//*[@Name="Click Me"]    only_first=${True}
    Should Not Be Equal    ${button}    ${None}    msg=the Click Me button was not found
    ${listed}=    BM.Query    .//*[@Name="Click Me"]/@*[local-name()="Id"]    only_first=${True}
    Should Not Be Equal    ${listed}    ${None}    msg=no Id is enumerated on the button
    Should Be Equal    ${listed.value}    btn-click-me
    BM.Get Attribute    ${button}    Id    ==    btn-click-me
    Should Be Equal    ${button.id}    btn-click-me
