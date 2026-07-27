*** Settings ***
Documentation       JAB interface-attribute projection (``native:<Interface>.<Property>``): the data
...                 behind each supported accessibility interface is visible as native attributes —
...                 container-level table/value/text properties read live through the bridge, per-cell
...                 ``TableCell.*`` resolved on demand from the parent table, everything gated by the
...                 element's supported-interface set (an unsupported interface contributes nothing).
...                 Reads are live per access, so state changes show up on the next read without any
...                 cache invalidation; Swing applies input on its event-dispatch thread, so the
...                 assertions after input rely on the self-waiting ``Get Attribute    ==``.
...
...                 Table cells are addressed by POSITION (row-major enumeration order, XPath
...                 1-based; 100×6 grid → cell (row, col) is child [row*6 + col + 1]), never by
...                 @Name: the JDK's AccessBridge answers JTable cell lookups with the one shared
...                 cell-renderer component, so every cell's @Name reads whatever cell the renderer
...                 was configured for last. Content/coordinates are asserted via the stable
...                 ``@native:TableCell.*`` attributes instead.
...
...                 The fixture table scrolls: only a few of its 100 rows and 6 columns are in the
...                 viewport at a time, while the accessible view reports every cell of the model
...                 regardless. That gap is the bridge's normal condition, not a defect of the
...                 fixture — none of the assertions here depend on a cell being on screen.

Resource            resources/testapp.resource

Suite Setup         Launch Default Swing Instance    PlatynUI Swing NativeAttributes
Suite Teardown      Terminate Default Swing Instance

Test Tags           real


*** Test Cases ***
The Table Reports Its Dimensions And Selection Counts
    [Documentation]    Container-level ``native:Table.*`` from ``getAccessibleTableInfo`` plus the
    ...    selection-count calls — the fixture table is fixed at 100×6 with row 2 preselected (row
    ...    selection, no column selection) and carries neither caption nor summary. The counts are
    ...    the model's, so scrolling does not change them.
    BM.Get Attribute    .//*[@Name="main-table"]    native:Table.RowCount    ==    ${100}
    BM.Get Attribute    .//*[@Name="main-table"]    native:Table.ColumnCount    ==    ${6}
    BM.Get Attribute    .//*[@Name="main-table"]    native:Table.SelectedRowCount    ==    ${1}
    BM.Get Attribute    .//*[@Name="main-table"]    native:Table.SelectedColumnCount    ==    ${0}
    BM.Get Attribute    .//*[@Name="main-table"]    native:Table.HasCaption    ==    ${False}
    BM.Get Attribute    .//*[@Name="main-table"]    native:Table.HasSummary    ==    ${False}

A Data Cell Reports Its Coordinates
    [Documentation]    Per-cell ``native:TableCell.*`` resolves on demand (targeted attribute lookup,
    ...    never during enumeration): the designated data cell — positionally the table's 9th child
    ...    (1*6 + 2 + 1), holding "r1c2" — sits at row 1 / column 2, spans one cell in each
    ...    direction, and is outside the preselected row.
    BM.Get Attribute    .//*[@Name="main-table"]/*[9]    native:TableCell.Row    ==    ${1}
    BM.Get Attribute    .//*[@Name="main-table"]/*[9]    native:TableCell.Column    ==    ${2}
    BM.Get Attribute    .//*[@Name="main-table"]/*[9]    native:TableCell.RowExtent    ==    ${1}
    BM.Get Attribute    .//*[@Name="main-table"]/*[9]    native:TableCell.ColumnExtent    ==    ${1}
    BM.Get Attribute    .//*[@Name="main-table"]/*[9]    native:TableCell.IsSelected    ==    ${False}

A Cell In The Preselected Row Reports Its Selection
    [Documentation]    The table's 14th child (2*6 + 1 + 1) is cell (2, 1) — inside the preselected
    ...    row 2.
    BM.Get Attribute    .//*[@Name="main-table"]/*[14]    native:TableCell.Row    ==    ${2}
    BM.Get Attribute    .//*[@Name="main-table"]/*[14]    native:TableCell.Column    ==    ${1}
    BM.Get Attribute    .//*[@Name="main-table"]/*[14]    native:TableCell.IsSelected    ==    ${True}

A Cell Far Down The Model Is Still Addressable
    [Documentation]    The point of a table that does not fit its viewport: the accessible view
    ...    reports every cell of the model, not only the realized ones, so a cell 90 rows below the
    ...    fold answers its coordinates without anyone scrolling to it. Child (90*6 + 3 + 1).
    BM.Get Attribute    .//*[@Name="main-table"]/*[544]    native:TableCell.Row    ==    ${90}
    BM.Get Attribute    .//*[@Name="main-table"]/*[544]    native:TableCell.Column    ==    ${3}
    BM.Get Attribute    .//*[@Name="main-table"]/*[544]    native:TableCell.IsSelected    ==    ${False}

The Slider Reports Its Value Range And Tracks Changes Live
    [Documentation]    ``native:Value.*`` on an ``AccessibleValue`` element (fixture default: 50 in
    ...    0..100); one keyboard unit-increment must be visible on the next read of the same runtime —
    ...    live reads, no sticky cache.
    BM.Get Attribute    .//*[@Name="stage2-slider"]    native:Value.Current    ==    ${50}
    BM.Get Attribute    .//*[@Name="stage2-slider"]    native:Value.Minimum    ==    ${0}
    BM.Get Attribute    .//*[@Name="stage2-slider"]    native:Value.Maximum    ==    ${100}
    BM.Keyboard Type    .//*[@Name="stage2-slider"]    <Right>
    BM.Get Attribute    .//*[@Name="stage2-slider"]    native:Value.Current    ==    ${51}

The Text Field Reports Char Count And Caret Live
    [Documentation]    ``native:Text.CharCount``/``native:Text.CaretIndex`` from
    ...    ``getAccessibleTextInfo``: empty field first, then real keyboard input; the caret lands
    ...    behind the typed text, read back on the same runtime.
    BM.Get Attribute    .//*[@Name="stage1-textfield"]    native:Text.CharCount    ==    ${0}
    BM.Keyboard Type    .//*[@Name="stage1-textfield"]    live
    BM.Get Attribute    .//*[@Name="stage1-textfield"]    native:Text.CharCount    ==    ${4}
    BM.Get Attribute    .//*[@Name="stage1-textfield"]    native:Text.CaretIndex    ==    ${4}

Only Supported Interfaces Contribute Attributes
    [Documentation]    The bitfield gate: a plain label supports neither Table nor Value nor Text, so
    ...    none of those attributes exist on it (existence-predicate resolves nothing) — while the same
    ...    predicate finds the table. The ``Interfaces`` name-list still names the label's real set.
    BM.Wait Until Exists    .//*[@Name="main-table"][@native:Table.RowCount]
    FOR    ${attribute}    IN    native:Table.RowCount    native:Value.Current    native:Text.CharCount
        ${node}=    BM.Query    .//*[@Name="stage1-status-clicks-0"][@${attribute}]    only_first=${True}
        Should Be Equal    ${node}    ${None}    msg=label unexpectedly exposes ${attribute}
    END
    ${interfaces}=    BM.Get Attribute    .//*[@Name="stage1-status-clicks-0"]    native:Interfaces
    Should Contain    ${interfaces}    component
    Should Not Contain    ${interfaces}    table
