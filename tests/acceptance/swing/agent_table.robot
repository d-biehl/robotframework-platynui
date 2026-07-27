*** Settings ***
Documentation       Tabular content served by the **in-JVM agent**: a table's children are its rows,
...                 and the cells sit beneath them.
...
...                 This is the first Swing acceptance suite that drives the agent rather than the
...                 Access Bridge, and the contrast with ``native_attributes.robot`` is the point.
...                 Through the bridge a table is one flat, row-major list of all 600 cells, whose
...                 ``@Name`` is whatever the shared cell renderer was configured for last — so those
...                 suites address cells by position and assert coordinates through
...                 ``native:TableCell.*``. The agent reads the toolkit's own model, where a row is a
...                 first-class thing and a cell's name is its model value, so here a cell is
...                 addressable by both.
...
...                 A table's shape therefore follows the backend that serves it. That is deliberate
...                 and observable: ``@Technology`` says which one answered, and this suite waits for
...                 ``"JavaAgent"`` before asserting anything.
...
...                 The fixture's table is fixed at 100x6 with content ``r<row>c<column>`` and row 2
...                 preselected; tests must not change the selection or the scroll position. It does
...                 not fit its viewport, which is deliberate: rows below the fold are in the tree
...                 with their names and coordinates but have no on-screen rectangle, and that
...                 distinction is asserted here rather than assumed.

Resource            resources/testapp_agent.resource

Suite Setup         Launch Default Swing Agent Instance    PlatynUI Swing AgentTable
Suite Teardown      Terminate Default Swing Agent Instance


*** Test Cases ***
The Agent Serves The Fixture Window
    [Documentation]    The premise of every other test here: the provider injected an agent into the
    ...    fixture JVM and serves the window through it. If this fails, the rest would be asserting
    ...    the bridge's shape under an agent suite's name.
    BM.Get Attribute    .//Window[@Name="${SWING_AGENT_TITLE}"]    Technology    ==    JavaAgent

A Table's Children Are Its Rows
    [Documentation]    One hundred rows, not six hundred cells. The flat cell list is what
    ...    ``AccessibleContext.getAccessibleChild(i)`` offers and all the bridge ever had; the model
    ...    underneath has rows, and the other providers (AT-SPI, UIA) already surface them.
    BM.Wait Until Query    count(.//*[@Name="main-table"]/*)    ==    ${100}
    FOR    ${position}    IN    ${1}    ${3}    ${91}    ${100}
        BM.Get Attribute    .//*[@Name="main-table"]/*[${position}]    Role    ==    TableRow
    END

A Row Holds One Cell Per Column
    [Documentation]    Six cells under the third row (row index 2), carrying their model values in
    ...    model order — which through the bridge is impossible, because every cell resolves to the
    ...    one shared renderer.
    BM.Wait Until Query    count(.//*[@Name="main-table"]/*[3]/*)    ==    ${6}
    FOR    ${column}    ${position}    IN
    ...    ${0}    ${1}
    ...    ${1}    ${2}
    ...    ${2}    ${3}
    ...    ${3}    ${4}
    ...    ${4}    ${5}
    ...    ${5}    ${6}
        BM.Get Attribute    .//*[@Name="main-table"]/*[3]/*[${position}]    Role    ==    TableCell
        BM.Get Attribute    .//*[@Name="main-table"]/*[3]/*[${position}]    Name    ==    r2c${column}
    END

A Row Below The Fold Is In The Tree But Not On Screen
    [Documentation]    The table does not fit its viewport, and the two halves of that must both
    ...    hold: row 90 is present with its position, its name and its cells' model values, and it
    ...    reports **no** bounds and is not in view. A rectangle two thousand pixels below the window
    ...    would aim the pointer at whatever happens to be there.
    BM.Get Attribute    .//*[@Name="main-table"]/*[91]    native:TableRow.Index    ==    ${90}
    BM.Get Attribute    .//*[@Name="main-table"]/*[91]    IsInView    ==    ${False}
    BM.Get Attribute    .//*[@Name="main-table"]/*[91]/*[1]    Name    ==    r90c0
    BM.Get Attribute    .//*[@Name="main-table"]/*[91]/*[1]    IsInView    ==    ${False}
    # Absence is asserted with an existence predicate and `Query`, not by reading the attribute:
    # `Get Attribute` fetches directly and RAISES on a missing one, while a non-matching XPath
    # yields ${None}. Written out rather than looped — a FOR over these three would be read as
    # `name=value` iteration, and escaping the `=` inside an XPath only hides that from the parser.
    ${row_bounds}=    BM.Query    .//*[@Name="main-table"]/*[91][@Bounds]    only_first=${True}
    Should Be Equal    ${row_bounds}    ${None}    msg=an off-view row must report no bounds
    ${row_point}=    BM.Query    .//*[@Name="main-table"]/*[91][@ActivationPoint]    only_first=${True}
    Should Be Equal    ${row_point}    ${None}    msg=and therefore nothing to aim pointer input at
    ${cell_bounds}=    BM.Query    .//*[@Name="main-table"]/*[91]/*[1][@Bounds]    only_first=${True}
    Should Be Equal    ${cell_bounds}    ${None}    msg=nor may the cells of an off-view row

A Cell Keeps Its Coordinates One Level Deeper
    [Documentation]    The row level is additive: a cell answers exactly what it answered as a direct
    ...    child of the table, and ``native:TableCell.*`` stays the coordinate-independent identity.
    BM.Get Attribute    .//*[@Name="main-table"]/*[3]/*[1]    native:TableCell.Row    ==    ${2}
    BM.Get Attribute    .//*[@Name="main-table"]/*[3]/*[1]    native:TableCell.Column    ==    ${0}
    BM.Get Attribute    .//*[@Name="main-table"]/*[3]/*[1]    native:TableCell.IsSelected    ==    ${True}
    BM.Get Attribute    .//*[@Name="main-table"]/*[2]/*[3]    native:TableCell.Row    ==    ${1}
    BM.Get Attribute    .//*[@Name="main-table"]/*[2]/*[3]    native:TableCell.Column    ==    ${2}
    BM.Get Attribute    .//*[@Name="main-table"]/*[2]/*[3]    native:TableCell.IsSelected    ==    ${False}

A Cell Is Reachable Through Its Row By Name
    [Documentation]    The row-scoped locator a user would actually write. Both halves matter: the
    ...    cell is found *under its row*, and it is found by its content — the agent reads the model,
    ...    so a cell's name is stable rather than whatever the renderer last held.
    BM.Get Attribute    .//*[@Name="main-table"]/*[3]//*[@Name="r2c1"]    native:TableCell.Column    ==    ${1}
    ${stray}=    BM.Query    .//*[@Name="main-table"]/*[1]//*[@Name="r2c1"]    only_first=${True}
    Should Be Equal    ${stray}    ${None}    msg=a cell must only be found under the row it belongs to

A Row Reports Its Position And Selection
    [Documentation]    A row is addressable in its own right: it knows where it sits, and whether it
    ...    is the selected one. Row 2 is preselected in the fixture and never changes.
    BM.Get Attribute    .//*[@Name="main-table"]/*[3]    native:TableRow.Index    ==    ${2}
    BM.Get Attribute    .//*[@Name="main-table"]/*[3]    native:TableRow.IsSelected    ==    ${True}
    BM.Get Attribute    .//*[@Name="main-table"]/*[3]    item:IsSelected    ==    ${True}
    BM.Get Attribute    .//*[@Name="main-table"]/*[1]    native:TableRow.Index    ==    ${0}
    BM.Get Attribute    .//*[@Name="main-table"]/*[1]    native:TableRow.IsSelected    ==    ${False}

A Row Has A Rectangle Spanning Its Cells
    [Documentation]    A row is something a user can point at, not a bookkeeping node: its rectangle
    ...    covers the cells it contains, so it is at least as wide as all of them together and as
    ...    tall as one of them. Everything here is clipped to the viewport, row and cells alike —
    ...    the fourth column is the last one in view, and the fifth has no rectangle at all.
    ${row}=    BM.Get Attribute    .//*[@Name="main-table"]/*[3]    Bounds
    ${first}=    BM.Get Attribute    .//*[@Name="main-table"]/*[3]/*[1]    Bounds
    ${last}=    BM.Get Attribute    .//*[@Name="main-table"]/*[3]/*[4]    Bounds
    ${beyond}=    BM.Query    .//*[@Name="main-table"]/*[3]/*[6][@Bounds]    only_first=${True}
    Should Be Equal    ${beyond}    ${None}    msg=a column scrolled out to the right has no rectangle either
    Should Be True    $row.x <= $first.x and $row.x + $row.width >= $last.x + $last.width
    ...    msg=row ${row.x}+${row.width} does not span its cells ${first.x}..${last.x}+${last.width}
    Should Be True    $row.height >= $first.height
    ...    msg=row height ${row.height} is smaller than a cell's ${first.height}

The Table Names Its Selected Row
    [Documentation]    ``control:SelectedItems`` publishes RuntimeIds, and they have to resolve. With
    ...    a row level the accessible child index no longer addresses a direct child, so the
    ...    selection is re-derived from the table's own model — and what it names is the selected
    ...    **row**, which is what row selection means.
    ${row_id}=    BM.Get Attribute    .//*[@Name="main-table"]/*[3]    RuntimeId
    ${selected}=    BM.Get Attribute    .//*[@Name="main-table"]    SelectedItems
    Should Be Equal    ${selected}    ${{ [$row_id] }}
    ...    msg=SelectedItems ${selected} must name exactly the preselected row ${row_id}
