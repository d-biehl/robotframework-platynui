*** Settings ***
Documentation       JAB interface-attribute projection (``native:<Interface>.<Property>``): the data
...                 behind each supported accessibility interface is visible as native attributes —
...                 container-level table/value/text properties read live through the bridge, per-cell
...                 ``TableCell.*`` resolved on demand from the parent table, everything gated by the
...                 element's supported-interface set (an unsupported interface contributes nothing).
...                 Reads are live per access, so state changes show up on the next read without any
...                 cache invalidation; Swing applies input on its event-dispatch thread, so assertions
...                 after input poll (``Wait Until Keyword Succeeds``) rather than assuming immediacy.

Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Launch Default Swing Instance    PlatynUI Swing NativeAttributes
Suite Teardown      Terminate Default Swing Instance

Test Tags           real


*** Test Cases ***
The Table Reports Its Dimensions And Selection Counts
    [Documentation]    Container-level ``native:Table.*`` from ``getAccessibleTableInfo`` plus the
    ...    selection-count calls — the fixture table is fixed at 4×3 with row 2 preselected (row
    ...    selection, no column selection) and carries neither caption nor summary.
    BM.Get Attribute    ${APP}${MAIN_TABLE}    native:Table.RowCount    ==    ${4}
    BM.Get Attribute    ${APP}${MAIN_TABLE}    native:Table.ColumnCount    ==    ${3}
    BM.Get Attribute    ${APP}${MAIN_TABLE}    native:Table.SelectedRowCount    ==    ${1}
    BM.Get Attribute    ${APP}${MAIN_TABLE}    native:Table.SelectedColumnCount    ==    ${0}
    BM.Get Attribute    ${APP}${MAIN_TABLE}    native:Table.HasCaption    ==    ${False}
    BM.Get Attribute    ${APP}${MAIN_TABLE}    native:Table.HasSummary    ==    ${False}

A Data Cell Reports Its Coordinates
    [Documentation]    Per-cell ``native:TableCell.*`` resolves on demand (targeted attribute lookup,
    ...    never during enumeration): the designated data cell — positionally the table's 6th child,
    ...    holding "r1c2" — sits at row 1 / column 2, spans one cell in each direction, and is outside
    ...    the preselected row. Cells are addressed by position, not @Name (see the locator notes).
    BM.Get Attribute    ${APP}${MAIN_TABLE_CELL_R1C2}    native:TableCell.Row    ==    ${1}
    BM.Get Attribute    ${APP}${MAIN_TABLE_CELL_R1C2}    native:TableCell.Column    ==    ${2}
    BM.Get Attribute    ${APP}${MAIN_TABLE_CELL_R1C2}    native:TableCell.RowExtent    ==    ${1}
    BM.Get Attribute    ${APP}${MAIN_TABLE_CELL_R1C2}    native:TableCell.ColumnExtent    ==    ${1}
    BM.Get Attribute    ${APP}${MAIN_TABLE_CELL_R1C2}    native:TableCell.IsSelected    ==    ${False}

A Cell In The Preselected Row Reports Its Selection
    BM.Get Attribute    ${APP}${MAIN_TABLE_CELL_R2C1}    native:TableCell.Row    ==    ${2}
    BM.Get Attribute    ${APP}${MAIN_TABLE_CELL_R2C1}    native:TableCell.Column    ==    ${1}
    BM.Get Attribute    ${APP}${MAIN_TABLE_CELL_R2C1}    native:TableCell.IsSelected    ==    ${True}

The Slider Reports Its Value Range And Tracks Changes Live
    [Documentation]    ``native:Value.*`` on an ``AccessibleValue`` element (fixture default: 50 in
    ...    0..100); one keyboard unit-increment must be visible on the next read of the same runtime —
    ...    live reads, no sticky cache.
    BM.Get Attribute    ${APP}${STAGE2_SLIDER}    native:Value.Current    ==    ${50}
    BM.Get Attribute    ${APP}${STAGE2_SLIDER}    native:Value.Minimum    ==    ${0}
    BM.Get Attribute    ${APP}${STAGE2_SLIDER}    native:Value.Maximum    ==    ${100}
    BM.Keyboard Type    ${APP}${STAGE2_SLIDER}    <Right>
    Wait Until Keyword Succeeds    5s    0.25s
    ...    BM.Get Attribute    ${APP}${STAGE2_SLIDER}    native:Value.Current    ==    ${51}

The Text Field Reports Char Count And Caret Live
    [Documentation]    ``native:Text.CharCount``/``native:Text.CaretIndex`` from
    ...    ``getAccessibleTextInfo``: empty field first, then real keyboard input; the caret lands
    ...    behind the typed text, read back on the same runtime.
    BM.Get Attribute    ${APP}${STAGE1_TEXTFIELD}    native:Text.CharCount    ==    ${0}
    BM.Keyboard Type    ${APP}${STAGE1_TEXTFIELD}    live
    Wait Until Keyword Succeeds    5s    0.25s
    ...    BM.Get Attribute    ${APP}${STAGE1_TEXTFIELD}    native:Text.CharCount    ==    ${4}
    BM.Get Attribute    ${APP}${STAGE1_TEXTFIELD}    native:Text.CaretIndex    ==    ${4}

Only Supported Interfaces Contribute Attributes
    [Documentation]    The bitfield gate: a plain label supports neither Table nor Value nor Text, so
    ...    none of those attributes exist on it (existence-predicate resolves nothing) — while the same
    ...    predicate finds the table. The ``Interfaces`` name-list still names the label's real set.
    Node Is Present    ${APP}${MAIN_TABLE}\[@native:Table.RowCount]
    Node Is Absent    ${APP}${STAGE1_STATUS_0}\[@native:Table.RowCount]
    Node Is Absent    ${APP}${STAGE1_STATUS_0}\[@native:Value.Current]
    Node Is Absent    ${APP}${STAGE1_STATUS_0}\[@native:Text.CharCount]
    ${interfaces}=    BM.Get Attribute    ${APP}${STAGE1_STATUS_0}    native:Interfaces
    Should Contain    ${interfaces}    component
    Should Not Contain    ${interfaces}    table
