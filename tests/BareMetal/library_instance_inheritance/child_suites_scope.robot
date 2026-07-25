*** Settings ***
Documentation     The parent pinned ``TREE``'s context with ``scope=SUITES``, so it reaches this
...               child suite — the opt-in counterpart to child_suite_scope.robot. What travels is
...               the selector: this suite has its own library instance and its own runtime, and
...               re-resolves the root there.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}    AS    TREE


*** Test Cases ***
The Parent Suite Root Reaches This Suite
    ${n}=    TREE.Query    count(.//item:ListItem)    only_first=${True}
    Should Be Equal As Integers    ${n}    4
    ...    msg=expected the Operations Console root pinned by the parent at scope=SUITES

The Inherited Root Resolves On This Suite's Own Runtime
    [Documentation]    Every node carries the id of the runtime that produced it. A node reached
    ...    through the inherited root must come from the same runtime as one this suite resolves
    ...    directly — otherwise the root is being resolved on the parent instance's runtime.
    ${via_root}=    TREE.Query    .//control:Text    only_first=${True}
    ${direct}=    TREE.Query    //control:Window[@Name="Detail View"]    only_first=${True}
    Should Be Equal As Integers    ${{ $via_root.owner_id }}    ${{ $direct.owner_id }}

The Parent Suite Query Settings Reach This Suite Too
    [Documentation]    This import's own default is 0.2 s; the parent's 0.5 s at ``SUITES`` wins.
    Run Keyword And Expect Error    *within timeout of 0.5 seconds*
    ...    TREE.Get Attribute    //control:Button[@Name="NoSuchButton"]    Name
