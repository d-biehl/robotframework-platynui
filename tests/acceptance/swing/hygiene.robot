*** Settings ***
Documentation       Handle-hygiene regression guard: ten full subtree walks in one session must return
...                 the same structure, and the target JVM must keep responding normally afterwards.
...                 Every walk fetches and releases JVM-side references for the whole tree
...                 (``releaseJavaObject`` RAII); a leak or stale-handle bug surfaces as drift, errors,
...                 or a wedged fixture.

Library             Collections
Library             PlatynUI.BareMetal    AS    BM
Resource            resources/testapp.resource

Suite Setup         Launch Default Swing Instance    PlatynUI Swing Hygiene
Suite Teardown      Terminate Default Swing Instance

Test Tags           real


*** Test Cases ***
Ten Full Walks Return The Same Structure
    ${baseline}=    Walk Signature
    Should Not Be Equal As Integers    ${baseline}[count]    0    msg=walk must reach the fixture subtree
    # The guard must cover the JTable (jab-interface-attributes): the table (and with it its 12
    # walked cells) is part of every round, so leaked table/cell handles would surface as drift.
    # Cell @Names are deliberately not asserted — the JDK bridge aliases all cells to the shared
    # renderer, so their names are volatile (see testapp_locators.resource).
    Should Contain    ${baseline}[names]    main-table
    FOR    ${round}    IN RANGE    9
        ${walk}=    Walk Signature
        Dictionaries Should Be Equal    ${walk}    ${baseline}    msg=walk ${round + 2} diverged from the first
    END
    # The JVM is still healthy after the handle churn: a fresh identifying read still succeeds.
    Node Is Present    ${APP}${STAGE1_BUTTON}
    BM.Get Attribute    ${APP}${STAGE1_BUTTON}    Technology    ==    JAB


*** Keywords ***
Walk Signature
    [Documentation]    Materialize the whole fixture subtree (forcing one JAB round-trip per node) and
    ...    reduce it to a comparable signature: node count plus the sorted accessible names.
    ${nodes}=    BM.Query    ${APP}${WINDOW}//*
    ${count}=    Get Length    ${nodes}
    ${names}=    BM.Query    ${APP}${WINDOW}//*[@Name != ""]/@Name
    ${joined}=    Evaluate    "|".join(sorted(str(n) for n in $names))
    ${signature}=    Create Dictionary    count=${count}    names=${joined}
    RETURN    ${signature}
