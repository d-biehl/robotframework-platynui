*** Settings ***
Documentation       Where the **in-JVM agent** takes an element's name, description and text from (specs
...                 ``name-attribute``, ``description-attribute``, ``textcontent-pattern``).
...
...                 ``@Name`` is the accessible name Swing reports, and nothing stands in for it. The
...                 values that are not the name stay readable on their own: the component name as
...                 ``@Id`` and ``native:ComponentName``, a window's title as ``native:WindowTitle``, a
...                 table cell's model value as ``native:TableCell.ModelValue``. ``control:Text`` is
...                 only the text Swing provides; Swing's labels, buttons and renderers provide none
...                 unless their text is HTML, and nothing is put in its place.
...
...                 The fixture's ``names-panel`` carries the cases: a button whose accessible name,
...                 component name, description and label all differ, and a one-row table whose
...                 displayed text differs from its model values. Locators key on ``@Id`` or on names
...                 that hold whichever value were the name, so a wrong name fails an assertion
...                 instead of timing out a lookup.

Resource            resources/testapp_agent.resource

Suite Setup         Launch Default Swing Agent Instance    PlatynUI Swing AgentNames
Suite Teardown      Terminate Default Swing Agent Instance


*** Test Cases ***
A Component's Developer Name Is Its Id, Not Its Name
    [Documentation]    The button's ``setName`` value is its identifier and its accessible name is its
    ...    name; neither stands in for the other.
    ${button}=    BM.Wait Until Exists    .//*[@Id="namesButton"]
    Should Be Equal    ${button.name}    names-button
    Should Be Equal    ${button.id}    namesButton
    BM.Get Attribute Value    ${button}    Name    ==    names-button
    BM.Get Attribute Value    ${button}    native:ComponentName    ==    namesButton
    ${stray}=    BM.Query    .//*[@Name="namesButton"]    only_first=${True}
    Should Be Equal    ${stray}    ${None}    msg=the component name must not match as a name

A Component's Description Is Swing's Accessible Description
    ${button}=    BM.Wait Until Exists    .//*[@Id="namesButton"]
    Should Be Equal    ${button.description}    A button with a developer name
    BM.Get Attribute Value    ${button}    Description    ==    A button with a developer name

A Component With A Developer Name But No Accessible Name Has An Empty Name
    [Documentation]    Swing names the layered pane of a root pane ``null.layeredPane`` and gives it no
    ...    accessible name: that name is its ``@Id``, its ``@Name`` is empty, and an empty ``@Name``
    ...    matches as empty.
    ${pane}=    BM.Wait Until Exists    .//Window[@Name="${SWING_AGENT_TITLE}"]//*[@Id="null.layeredPane"]
    Should Be Equal    ${pane.name}    ${EMPTY}
    BM.Get Attribute Value    ${pane}    Name    ==    ${EMPTY}
    ${matched}=    BM.Query
    ...    .//Window[@Name="${SWING_AGENT_TITLE}"]//*[@Id="null.layeredPane"][@Name=""]    only_first=${True}
    Should Not Be Equal    ${matched}    ${None}    msg=an empty Name must match [@Name=""]

A Table Cell Is Named By The Text It Displays
    [Documentation]    The ``amount`` cell holds ``1234.5`` and displays ``1,234.50``: the displayed text
    ...    is its name, the model value stays readable as a number, and the cell has no
    ...    ``control:Text``, because its renderer is a plain label.
    BM.Get Attribute Value    (.//*[@Name="names-table"])[1]/*[1]/*[1]    Name    ==    1,234.50
    BM.Get Attribute Value    (.//*[@Name="names-table"])[1]/*[1]/*[1]    native:TableCell.ModelValue    ==    ${1234.5}
    ${by_value}=    BM.Query    (.//*[@Name="names-table"])[1]//*[@Name="1234.5"]    only_first=${True}
    Should Be Equal    ${by_value}    ${None}    msg=the model value must not match as a name
    ${text}=    BM.Query    (.//*[@Name="names-table"])[1]/*[1]/*[1][@Text]    only_first=${True}
    Should Be Equal    ${text}    ${None}    msg=a plain-text cell has no text interface

A Table Cell Whose Renderer Displays No Text Has An Empty Name
    [Documentation]    The ``active`` cell holds ``true`` and displays a check box without text. Its name
    ...    is empty rather than the model value, which stays readable as a boolean.
    BM.Get Attribute Value    (.//*[@Name="names-table"])[1]/*[1]/*[2]    Name    ==    ${EMPTY}
    BM.Get Attribute Value    (.//*[@Name="names-table"])[1]/*[1]/*[2]    native:TableCell.ModelValue    ==    ${True}
    ${text}=    BM.Query    (.//*[@Name="names-table"])[1]/*[1]/*[2][@Text]    only_first=${True}
    Should Be Equal    ${text}    ${None}    msg=a check box renderer has no text interface

A String Cell Keeps Its Model Value And Lists No Component Name
    [Documentation]    A cell has no component name, so it lists none, and its model value is a
    ...    string where the model holds one.
    BM.Get Attribute Value    (.//*[@Name="main-table"])[1]/*[3]/*[1]    native:TableCell.ModelValue    ==    r2c0
    ${component}=    BM.Query    (.//*[@Name="main-table"])[1]/*[3]/*[1][@native:ComponentName]    only_first=${True}
    Should Be Equal    ${component}    ${None}    msg=a cell must not list a component name

Column Headers Are Named By Their Accessible Name And Have No Text
    [Documentation]    A header's name is what Swing reports for it; its renderer is a plain label, so it
    ...    has no ``control:Text``.
    BM.Get Attribute Value    .//*[@Name="names-panel"]//*[@Name="amount"]    Role    ==    ColumnHeader
    ${amount}=    BM.Query    .//*[@Name="names-panel"]//*[@Name="amount"][@Text]    only_first=${True}
    Should Be Equal    ${amount}    ${None}    msg=the amount header has no text interface
    BM.Get Attribute Value    .//*[@Name="table-panel"]//*[@Name="col-1"]    Role    ==    ColumnHeader
    ${col_1}=    BM.Query    .//*[@Name="table-panel"]//*[@Name="col-1"][@Text]    only_first=${True}
    Should Be Equal    ${col_1}    ${None}    msg=the col-1 header has no text interface

A Plain Label And Button Have No Text
    [Documentation]    Swing gives a label or button a text interface only for HTML text. The status
    ...    label displays ``clicks-0`` and keeps its accessible name as its name; neither value is
    ...    turned into ``control:Text``.
    BM.Get Attribute Value    .//*[@Name="stage1-status-clicks-0"]    Name    ==    stage1-status-clicks-0
    ${label}=    BM.Query    .//*[@Name="stage1-status-clicks-0"][@Text]    only_first=${True}
    Should Be Equal    ${label}    ${None}    msg=a plain label has no text interface
    BM.Wait Until Exists    .//*[@Name="stage1-button"]
    ${button}=    BM.Query    .//*[@Name="stage1-button"][@Text]    only_first=${True}
    Should Be Equal    ${button}    ${None}    msg=a plain button has no text interface

A Window Without An Explicit Accessible Name Is Named After Its Title By Swing
    [Documentation]    The main frame sets no accessible name, so Swing reports its title as one.
    BM.Get Attribute Value    .//Window[@Name="${SWING_AGENT_TITLE}"]    native:WindowTitle    ==    ${SWING_AGENT_TITLE}

A Window Is Named By Its Accessible Name, Not By Its Title
    [Documentation]    A second instance in its second-``AppContext`` mode shows the launcher's companion
    ...    window, whose accessible name ``companion-window`` differs from its title. The name is the
    ...    accessible name, and the title stays readable.
    VAR    ${title}    PlatynUI Swing AgentNames Companion
    ${handle}=    Launch Swing Agent Test App    ${title}    --app-context    --companion-window
    ${pid}=    Get Process Id    ${handle}
    ${window}=    BM.Wait Until Exists
    ...    /app:Application[@ProcessId=${pid}][@Technology="JavaAgent"]//Window[@Name="companion-window"]
    BM.Get Attribute Value    ${window}    native:WindowTitle    ==    ${title} companion
    [Teardown]    Run Keyword And Ignore Error    Terminate Process    ${handle}    kill=${True}
