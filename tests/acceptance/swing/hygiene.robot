*** Settings ***
Documentation       Handle-hygiene regression guard: ten full subtree walks in one session must return
...                 the same structure, and the target JVM must keep responding normally afterwards.
...                 Every walk fetches and releases JVM-side references for the whole tree
...                 (``releaseJavaObject`` RAII); a leak or stale-handle bug surfaces as drift, errors,
...                 or a wedged fixture.

Library             Collections
Resource            resources/testapp.resource

Suite Setup         Launch Default Swing Instance    PlatynUI Swing Hygiene
Suite Teardown      Terminate Default Swing Instance

Test Tags           real


*** Test Cases ***
Ten Full Walks Return The Same Structure
    ${baseline}=    Walk Signature
    Should Not Be Equal As Integers    ${baseline}[count]    0    msg=walk must reach the fixture subtree
    # The guard must cover the JTable (jab-interface-attributes): the table (and with it every cell
    # of its 100x6 model, which the bridge reports whether or not it is scrolled into view) is part
    # of every round, so leaked table/cell handles would surface as drift. This is also what makes
    # the suite the slowest of the Swing set — deliberately, because handle churn at that volume is
    # exactly the condition a leak shows up in.
    # Cell @Names are deliberately not asserted — the JDK bridge aliases all cells to the shared
    # renderer, so their names are volatile (see native_attributes.robot).
    Should Contain    ${baseline}[names]    main-table
    FOR    ${round}    IN RANGE    9
        ${walk}=    Walk Signature
        Dictionaries Should Be Equal    ${walk}    ${baseline}    msg=walk ${round + 2} diverged from the first
    END
    # The JVM is still healthy after the handle churn: a fresh identifying read still succeeds.
    BM.Get Attribute    .//*[@Name="stage1-button"]    Technology    ==    JAB


*** Keywords ***
Walk Signature
    [Documentation]    Materialize the whole fixture subtree (forcing one JAB round-trip per node) and
    ...    reduce it to a comparable signature: node count plus the sorted accessible names.
    ...
    ...    The count covers everything, cells included — that is the handle-churn signal. The names
    ...    deliberately exclude the table's cells: the JDK bridge resolves every JTable cell to the
    ...    one shared renderer component, so a cell's @Name reads whatever cell that renderer was
    ...    last configured for and changes with repaint timing rather than with structure. Comparing
    ...    them would make this guard fail on the very thing `native_attributes.robot` documents as
    ...    volatile — which is a false alarm, not a leak.
    ${nodes}=    BM.Query    .//Window[@Name="${SWING_TITLE}"]//*
    ${count}=    Get Length    ${nodes}
    ${names}=    BM.Query
    ...    .//Window[@Name="${SWING_TITLE}"]//*[@Name != ""][not(parent::*[@Name="main-table"])]/@Name
    VAR    ${joined}    ${{ "|".join(sorted(str(n) for n in $names)) }}
    VAR    &{signature}    count=${count}    names=${joined}
    RETURN    ${signature}
