*** Settings ***
Documentation     Mock-backed checks that two coexisting library imports keep their scoped state to
...               themselves. ``A`` and ``B`` are separate imports of the same library, so Robot
...               Framework builds two instances with two runtimes: a root or query setting set
...               through one must never be visible through the other, at any scope, and a node
...               produced by one runtime must not be usable with the other. Counts come from the
...               mock tree — the desktop holds 6 windows and 8 item:ListItem, the Operations Console
...               window holds 4 of those list items and no window of its own.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}    AS    A
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}    AS    B


*** Variables ***
${OPS}            //control:Window[@Name="Operations Console"]
${DETAILS}        //control:Window[@Name="Detail View"]


*** Test Cases ***
Local Root Stays With Its Own Import
    A.Set Root    ${OPS}
    ${own}=    A.Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${own}    4
    ${other}=    B.Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${other}    8    msg=B must still resolve against the desktop

Test Scoped Root Stays With Its Own Import
    A.Set Root    ${OPS}    scope=TEST
    ${own}=    A.Query    count(.//control:Window)    only_first=${True}
    Should Be Equal As Integers    ${own}    0    msg=the Operations Console holds no window of its own
    ${other}=    B.Query    count(.//control:Window)    only_first=${True}
    Should Be Equal As Integers    ${other}    6

Suite Scoped Root Stays With Its Own Import
    [Documentation]    Restores the suite scope at the end so the suite stays order-independent.
    TRY
        A.Set Root    ${OPS}    scope=SUITE
        ${own}=    A.Query    count(.//item:ListItem)    only_first=${True}
        Should Be Equal As Integers    ${own}    4
        ${other}=    B.Query    count(.//item:ListItem)    only_first=${True}
        Should Be Equal As Integers    ${other}    8
    FINALLY
        A.Set Root    ${None}    scope=SUITE
    END

Each Import Owns Its Own Root Variable
    A.Set Root    ${OPS}
    B.Set Root    ${DETAILS}
    Variable Should Exist    \${PLATYNUI_ROOT_DESCRIPTOR_A}
    Variable Should Exist    \${PLATYNUI_ROOT_DESCRIPTOR_B}
    Variable Should Not Exist    \${PLATYNUI_ROOT_DESCRIPTOR}
    ${a}=    A.Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${a}    4
    ${b}=    B.Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${b}    0    msg=the Detail View holds no list items

Query Settings Stay With Their Own Import
    A.Set Query Settings    {'timeout': 0.6}    scope=TEST
    Variable Should Exist    \${PLATYNUI_QUERY_SETTINGS_A}
    Variable Should Not Exist    \${PLATYNUI_QUERY_SETTINGS_B}
    Run Keyword And Expect Error    *within timeout of 0.6 seconds*
    ...    A.Get Attribute    //control:Button[@Name="NoSuchButton"]    Name
    Run Keyword And Expect Error    *within timeout of 0.2 seconds*
    ...    B.Get Attribute    //control:Button[@Name="NoSuchButton"]    Name

A Selector Handed To Another Import Resolves There
    [Documentation]    The counterpart to the rejected element: a selector is pure data, so handing
    ...    A's root to B is legitimate — and B must resolve it on B's own runtime, not A's.
    A.Set Root    ${OPS}
    ${binding}=    A.Set Root    ${None}
    B.Set Root    ${binding}
    ${n}=    B.Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4
    ${via_root}=    B.Query    .//control:Text    only_first=${True}
    ${direct}=    B.Query    ${DETAILS}    only_first=${True}
    Should Be Equal As Integers    ${{ $via_root.owner_id }}    ${{ $direct.owner_id }}
    ...    msg=B must resolve the handed-over root on its own runtime

A Node From Another Import Is Rejected As A Target
    ${node}=    A.Query    ${OPS}    only_first=${True}
    Run Keyword And Expect Error    *different library instance*    B.Get Attribute    ${node}    Name

A Node From Another Import Is Rejected As A Query Root
    ${node}=    A.Query    ${OPS}    only_first=${True}
    Run Keyword And Expect Error    *different library instance*    B.Query    .//item:ListItem    root=${node}

A Node From Another Import Is Never Reported As Gone
    ${node}=    A.Query    ${OPS}    only_first=${True}
    Run Keyword And Expect Error    *different library instance*    B.Wait Until Gone    ${node}

A Root From Another Import Is Rejected
    ${node}=    A.Query    ${OPS}    only_first=${True}
    Run Keyword And Expect Error    *different library instance*    B.Set Root    ${node}
